//! Blocking, single-radio transport for `rylr-core` over `serialport`.

mod error;
pub use error::{Error, Result};

mod port;
pub use port::discover;

mod radio;
pub use radio::Radio;
pub use rylr_core::{OwnedEvent, RfParams};
