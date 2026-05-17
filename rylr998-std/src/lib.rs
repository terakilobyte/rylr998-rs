//! Blocking host driver for the REYAX RYLR998 LoRa radio module, built on
//! the [`serialport`] crate.
//!
//! Open a port (or hand in your own `Read + Write`) and call the AT-command
//! methods on [`Radio`]:
//!
//! ```no_run
//! let mut radio = rylr998_std::Radio::open_auto()?;
//! radio.set_address(5)?;
//! radio.set_network_id(18)?;
//! radio.send(2, b"hello")?;
//! # Ok::<(), rylr998_std::Error>(())
//! ```
//!
//! See the sibling crates [`rylr998-tokio`] for an async equivalent and
//! [`rylr998-embassy`] for the `no_std` embedded variant.
//!
//! [`rylr998-tokio`]: https://crates.io/crates/rylr998-tokio
//! [`rylr998-embassy`]: https://crates.io/crates/rylr998-embassy

mod error;
pub use error::{Error, Result};

mod port;
pub use port::discover;

mod radio;
pub use radio::Radio;
pub use rylr998_core::{OwnedEvent, RadioError, RfParams};
