//! Blocking, single-radio transport for `rylr-core` over `serialport`.

mod error;
pub use error::{Error, Result};

mod port;
pub use port::discover;
