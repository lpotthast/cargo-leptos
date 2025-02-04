use serde::Deserialize;
use std::fmt::Display;

#[derive(Default, Debug, Clone, Copy, Deserialize)]
pub enum WsProtocol {
    #[serde(rename(serialize = "ws", deserialize = "ws"))]
    #[default]
    Ws,
    #[serde(rename(serialize = "wss", deserialize = "wss"))]
    Wss,
}

impl Display for WsProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsProtocol::Ws => write!(f, "ws"),
            WsProtocol::Wss => write!(f, "wss"),
        }
    }
}
