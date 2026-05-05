//! Async (Tokio) transport for `rylr-core`.
//!
//! ## EXERCISE
//!
//! Implement `AsyncRadio`. Recommended structure: a background `tokio::task`
//! owns the `Driver` and the `tokio_serial::SerialStream`. The handle holds
//! two channels:
//!
//! - `cmd_tx: mpsc::Sender<(Command, oneshot::Sender<...>)>` to send commands
//! - `event_rx: mpsc::Receiver<OwnedEvent>` to receive unsolicited events
//!
//! ### Sketch
//!
//! ```ignore
//! pub async fn open(path: &Path) -> Result<Self> {
//!     let port = tokio_serial::new(path.to_string_lossy(), 115_200).open_native_async()?;
//!     let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(8);
//!     let (event_tx, event_rx) = tokio::sync::mpsc::channel(64);
//!     tokio::spawn(async move {
//!         let mut driver = Driver::new();
//!         loop { /* select! on cmd_rx, port reads; drive driver.poll() */ }
//!     });
//!     Ok(Self { cmd_tx, event_rx })
//! }
//! ```
//!
//! ### Hints
//!
//! - For each `async fn set_X` / `async fn X`, send `(Command, oneshot::Sender)`
//!   over `cmd_tx`, then `oneshot.recv().await`.
//! - Tokio's `AsyncReadExt::read` on the serial stream returns 0 bytes only
//!   on EOF; otherwise it yields whatever's available. Wrap reads in
//!   `tokio::select!` against `cmd_rx.recv()` so commands and incoming bytes
//!   don't starve each other.
//! - `Driver::poll`'s borrow lifetime ties to `&mut self`. To send an event
//!   over a channel, call `.into_owned()` first.

use rylr_core::{Command, Driver, OwnedEvent};
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("core: {0}")]
    Core(#[from] rylr_core::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serial: {0}")]
    Serial(#[from] tokio_serial::Error),
    #[error("timeout")]
    Timeout,
    #[error("radio error code {0}")]
    Radio(u8),
    #[error("background task ended")]
    Closed,
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct AsyncRadio {
    // TODO: command sender + event receiver + JoinHandle.
}

impl AsyncRadio {
    pub async fn open(_path: &Path) -> Result<Self> {
        // TODO: open tokio_serial port, spawn task, return handle.
        let _ = (Command::Ping, Driver::new()); // silence warnings
        unimplemented!("AsyncRadio::open — exercise")
    }

    pub async fn ping(&mut self) -> Result<()> {
        unimplemented!("AsyncRadio::ping — exercise")
    }

    // TODO: mirror the rest of rylr_std::Radio's surface here as `async fn`s:
    //   address / set_address
    //   network_id / set_network_id
    //   band / set_band
    //   parameters / set_parameters
    //   crfop / uid / version
    //   factory_reset
    //   send
    //   next_event

    pub async fn next_event(&mut self) -> Result<OwnedEvent> {
        unimplemented!("AsyncRadio::next_event — exercise")
    }
}

#[cfg(test)]
mod tests {
    // TODO: write tests with a mock port (tokio's duplex pipe is good).
    // For now, this placeholder makes `cargo test` succeed.
    #[test]
    fn placeholder() {}
}
