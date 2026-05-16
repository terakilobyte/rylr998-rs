use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("core: {0}")]
    Core(#[from] rylr998_core::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serial: {0}")]
    Serial(#[from] serialport::Error),
    #[error("timeout")]
    Timeout,
    #[error("radio error code {0}")]
    Radio(u8),
    #[error("no cu.usbserial* device found")]
    NoDevice,
    #[error("multiple candidate devices: {0:?} (use --port)")]
    Ambiguous(Vec<PathBuf>),
}

pub type Result<T> = std::result::Result<T, Error>;
