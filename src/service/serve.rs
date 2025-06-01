use std::sync::Arc;

use crate::{
    config::Project,
    ext::{append_str_to_filename, determine_pdb_filename, fs, Paint},
    internal_prelude::*,
    logger::GRAY,
    signal::{Interrupt, ReloadSignal, ServerRestart},
};
use camino::Utf8PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::{process::Command, select, task::JoinHandle};
use tokio_process_tools::{Inspector, ProcessHandle};

pub async fn spawn(proj: &Arc<Project>) -> JoinHandle<Result<()>> {
    let mut int = Interrupt::subscribe_shutdown();
    let proj = proj.clone();
    let mut change = ServerRestart::subscribe();
    tokio::spawn(async move {
        let mut server = ServerProcess::start_new(&proj).await?;
        loop {
            select! {
                res = change.recv() => {
                    if let Ok(()) = res {
                          server.restart().await?;
                          ReloadSignal::send_full();
                    }
                },
                _ = int.recv() => {
                    info!("Serve received interrupt signal");
                    server.terminate().await;
                    return Ok(())
                },
            }
        }
    })
}

pub async fn spawn_oneshot(proj: &Arc<Project>) -> JoinHandle<Result<()>> {
    let mut int = Interrupt::subscribe_shutdown();
    let proj = proj.clone();
    tokio::spawn(async move {
        let mut server = ServerProcess::start_new(&proj).await?;
        select! {
          _ = server.wait() => {},
          _ = int.recv() => {
                server.terminate().await;
          },
        };
        Ok(())
    })
}

struct ServerProcess {
    process: Option<(ProcessHandle, Inspector, Inspector)>,
    envs: Vec<(&'static str, String)>,
    binary: Utf8PathBuf,
    bin_args: Option<Vec<String>>,
    graceful_shutdown: bool,
}

impl ServerProcess {
    fn new(proj: &Project) -> Self {
        Self {
            process: None,
            envs: proj.to_envs(false),
            binary: proj.bin.exe_file.clone(),
            bin_args: proj.bin.bin_args.clone(),
            graceful_shutdown: proj.graceful_shutdown,
        }
    }

    async fn start_new(proj: &Project) -> Result<Self> {
        let mut me = Self::new(proj);
        me.start().await?;
        Ok(me)
    }

    async fn terminate(&mut self) {
        let Some((mut handle, stdout_inspector, stderr_inspector)) = self.process.take() else {
            return;
        };

        let (interrupt_timeout, terminate_timeout) = match self.graceful_shutdown {
            true => (Duration::from_secs(4), Duration::from_secs(8)),
            false => (Duration::from_secs(0), Duration::from_secs(0)),
        };

        if let Err(e) = handle.terminate(interrupt_timeout, terminate_timeout).await {
            error!("Serve error terminating server process: {e}");
        } else {
            trace!("Serve stopped");
        }

        if let Err(err) = stdout_inspector.abort().await {
            error!("Serve error aborting stdout inspector: {err}");
        };
        if let Err(err) = stderr_inspector.abort().await {
            error!("Serve error aborting stderr inspector: {err}");
        };
    }

    async fn restart(&mut self) -> Result<()> {
        self.terminate().await;
        self.start().await?;
        trace!("Serve restarted");
        Ok(())
    }

    async fn wait(&mut self) -> Result<()> {
        if let Some((proc, _, _)) = self.process.as_mut() {
            if let Err(e) = proc.wait().await {
                error!("Serve error while waiting for server process to exit: {e}");
            } else {
                trace!("Serve process exited");
            }
        }
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let bin = &self.binary;
        let process = if bin.exists() {
            // windows doesn't like to overwrite a running binary, so we copy it to a new name
            let bin_path = if cfg!(target_os = "windows") {
                // solution to allow cargo to overwrite a running binary on some platforms:
                //   copy cargo's output bin to [filename]_leptos and then run it
                let new_bin_path = append_str_to_filename(bin, "_leptos")?;
                debug!(
                    "Copying server binary {} to {}",
                    GRAY.paint(bin.as_str()),
                    GRAY.paint(new_bin_path.as_str())
                );
                fs::copy(bin, &new_bin_path).await?;
                // also copy the .pdb file if it exists to allow debugging to attach
                if let Some(pdb) = determine_pdb_filename(bin) {
                    let new_pdb_path = append_str_to_filename(&pdb, "_leptos")?;
                    debug!(
                        "Copying server binary debug info {} to {}",
                        GRAY.paint(pdb.as_str()),
                        GRAY.paint(new_pdb_path.as_str())
                    );
                    fs::copy(&pdb, &new_pdb_path).await?;
                }
                new_bin_path
            } else {
                bin.clone()
            };

            let bin_args = match &self.bin_args {
                Some(bin_args) => bin_args.as_slice(),
                None => &[],
            };

            debug!("Serve running {}", GRAY.paint(bin_path.as_str()));
            let mut cmd = Command::new(bin_path);
            cmd.envs(self.envs.clone());
            cmd.args(bin_args);

            let handle = ProcessHandle::spawn("server", cmd)?;

            async fn write_to_stdout(data: &str) -> tokio::io::Result<()> {
                use tokio::io::AsyncWriteExt;
                let mut stdout = tokio::io::stdout();
                stdout.write_all(data.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                Ok(())
            }

            async fn write_to_stderr(data: &str) -> tokio::io::Result<()> {
                use tokio::io::AsyncWriteExt;
                let mut stdout = tokio::io::stderr();
                stdout.write_all(data.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                Ok(())
            }

            // Let's forward captured stdout/stderr lines to the output of our process.
            // We do this asynchronously using the tokio::io::std{out|err}() handles,
            // as writing to stdout/stderr directly using print!() could result in unhandled
            // "failed printing to stdout: Resource temporarily unavailable (os error 35)" errors.
            let stdout_inspector = handle.stdout().inspect_async(|line| {
                Box::pin(async move {
                    write_to_stdout(line.as_str()).await?;
                    Ok(())
                })
            });
            let stderr_inspector = handle.stderr().inspect_async(|line| {
                Box::pin(async move {
                    write_to_stderr(line.as_str()).await?;
                    Ok(())
                })
            });

            let port = self
                .envs
                .iter()
                .find_map(|(k, v)| {
                    if k == &"LEPTOS_SITE_ADDR" {
                        Some(v.to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            info!(
                "Serving at http://{port}{}",
                match self.graceful_shutdown {
                    true => " (with graceful shutdown)",
                    false => " (graceful shutdown disabled)",
                }
            );
            Some((handle, stdout_inspector, stderr_inspector))
        } else {
            debug!("Serve no exe found {}", GRAY.paint(bin.as_str()));
            None
        };
        self.process = process;
        Ok(())
    }
}
