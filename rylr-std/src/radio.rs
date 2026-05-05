//! Blocking `Radio<P>`. `P` must be `Read + Write`. The default `P` is
//! `Box<dyn SerialPort>`, used by `open*`. Tests construct `Radio<P>`
//! directly with their own port type.

use crate::{Error, Result};
use rylr_core::{Driver, OwnedEvent};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Radio<P: Read + Write = Box<dyn serialport::SerialPort>> {
    pub(crate) driver: Driver,
    pub(crate) port: P,
    pub(crate) events: VecDeque<OwnedEvent>,
}

impl Radio<Box<dyn serialport::SerialPort>> {
    pub fn discover() -> Result<PathBuf> {
        crate::port::discover()
    }

    pub fn open(path: &Path) -> Result<Self> {
        let port = serialport::new(path.to_string_lossy(), 115_200)
            .timeout(Duration::from_millis(50))
            .dtr_on_open(true)
            .open()?;
        Ok(Self::from_port(port))
    }

    pub fn open_auto() -> Result<Self> {
        let path = Self::discover()?;
        Self::open(&path)
    }
}

impl<P: Read + Write> Radio<P> {
    /// Test- and integration-friendly constructor: bring your own port.
    pub fn from_port(port: P) -> Self {
        Self { driver: Driver::new(), port, events: VecDeque::new() }
    }
}
