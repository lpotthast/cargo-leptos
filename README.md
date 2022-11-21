# cargo-leptos

Build tool for [Leptos](https://crates.io/crates/leptos).

<br/>

## Features

- SCSS compilation using [dart-sass](https://sass-lang.com/dart-sass).
- CSS transformation and minification using [Lightning CSS](https://lightningcss.dev). See docs for full details.
- Builds server and client (wasm) binaries using Cargo.
- Generates JS - Wasm bindings with [wasm-bindgen](https://crates.io/crates/wasm-bindgen)
- Optimises the wasm with _wasm-opt_ from [Binaryen](https://github.com/WebAssembly/binaryen)
- Generation of rust code for integrating with a server of choice.
- `--csr` mode, building only the client side rendered wasm, for a fast development experience.
- Standard mode for building full server and client.
- `watch` command for automatic rebuilds with browser autoreload. Works both for `--csr` and standard mode.
- `test` command for running tests. Note that this runs `cargo test` for the three different modes (`csr`, `hydrate` and `ssr`).
- `build` command for building (normal mode or `--csr`).
- `end2end` command (WIP!) for building, running the server and calling a bash shell hook. The hook would typically launch Playwright or similar.
- `init` command (WIP!) for creating a new project based on templates, using [cargo-generate](https://cargo-generate.github.io/cargo-generate/index.html).

  <br/>

## Getting started

Install:

> `cargo install --locked cargo-leptos`

Help:

> `cargo leptos --help`

<br/>

## Folder structure

```
├── src/
│ ├── app/             (the app logic)
│ ├── client/          (client packaging)
│ ├── server/          (the http server)
│ │ └── generated.rs   (generated by build)
│ ├── lib.rs
│ ├── app.scss         (root css/sass/scss file)
│ └── main.rs
│
├── static/
│ └── favicon.png
│
├── index.html         (template for generating the root page)
├── Cargo.toml         (needs the [package.metadata.leptos] config)
│
├── end2end/           (end-to-end test using Playwright)
│ ├── tests/
│ ├── playwright.config.ts
│ └── package.json
│
└── target/
  └── site/
    ├── index.html     (generated by build)
    ├── favicon.png
    └── pkg/
      ├── app.wasm
      ├── app.js
      └── app.css
```
