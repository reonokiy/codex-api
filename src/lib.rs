pub mod backend;
pub mod error;
pub mod images;
mod lite_instructions;
pub mod output;
pub mod request;
mod search;
pub mod server;
pub mod settings;
pub mod standalone;
mod tools;
pub mod transport;
pub mod websocket;

pub const CODEX_RELEASE: &str = "0.155.0";
pub const CODEX_REV: &str = "f0a1b8f0849d90960bc406b848f32e5a129b0457";
