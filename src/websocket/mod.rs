mod server;
mod types;

pub use server::WebSocketServer;
pub use types::{CanUpdate, SignalData};
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub use types::CanTransmitRequest;
