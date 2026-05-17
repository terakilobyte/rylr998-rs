use std::path::PathBuf;

/// Failure modes for the blocking driver.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// A protocol-layer error from [`rylr998_core`].
    #[error("core: {0}")]
    Core(#[from] rylr998_core::Error),
    /// An I/O error from reading or writing the serial port.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// An error opening or enumerating serial ports.
    #[error("serial: {0}")]
    Serial(#[from] serialport::Error),
    /// A command did not receive its expected response within the
    /// per-call deadline.
    #[error("timeout")]
    Timeout,
    /// The radio replied with `+ERR=<code>`. The numeric code is from
    /// the REYAX AT command manual.
    #[error("radio error code {0}")]
    Radio(u8),
    /// [`Radio::discover`](crate::Radio::discover) found no
    /// `cu.usbserial*` device on the system.
    #[error("no cu.usbserial* device found")]
    NoDevice,
    /// [`Radio::discover`](crate::Radio::discover) found multiple
    /// candidates and refused to guess. Open the right one explicitly
    /// via [`Radio::open`](crate::Radio::open).
    #[error("multiple candidate devices: {0:?} (use --port)")]
    Ambiguous(Vec<PathBuf>),
}

/// Convenience alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Map this error's radio `+ERR=<code>` value to a known manual entry.
    #[must_use]
    pub fn radio_error(&self) -> Option<rylr998_core::RadioError> {
        match self {
            Self::Radio(code) => rylr998_core::RadioError::from_code(*code),
            _ => None,
        }
    }
}
