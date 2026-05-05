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

use rylr_core::{Poll, Response};
use std::time::Instant;

impl<P: Read + Write> Radio<P> {
    /// Drive the state machine and the underlying port until the supplied
    /// predicate returns `Some`, or `deadline` is reached.
    ///
    /// ## EXERCISE (rylr-std)
    ///
    /// Implement the body. Per iteration:
    ///
    /// 1. While `self.driver.poll()` returns `Poll::NeedTx(bytes)`:
    ///       write all `bytes` to `self.port`, then `self.driver.ack_tx(n)`.
    ///       Propagate I/O errors as `Error::Io`.
    /// 2. Drain the driver:
    ///    Loop calling `self.driver.poll()`:
    ///      - `Poll::Event(e)`  -> push `e.into_owned()` to `self.events`,
    ///                             continue.
    ///      - `Poll::Response(r)` -> hand to `want`. If `Some(out)`, return
    ///                             `out`. If `None`, continue.
    ///      - `Poll::NeedTx(_)` -> handle as in step 1.
    ///      - `Poll::Idle`      -> break out of the inner loop.
    /// 3. Read up to 256 bytes from `self.port` into a stack buffer with
    ///    a 50 ms read timeout (already configured on the port). Wrap
    ///    `WouldBlock` / `TimedOut` as "no bytes this round" — not an
    ///    error. Feed accepted bytes via `self.driver.push_rx`.
    /// 4. Check `Instant::now() >= deadline`. If so, return `Err(Timeout)`.
    pub(crate) fn pump_until<R, F>(
        &mut self,
        deadline: Instant,
        mut want: F,
    ) -> Result<R>
    where
        F: FnMut(Response<'_>) -> Option<Result<R>>,
    {
        // TODO: implement per the rules above.
        let _ = (deadline, &mut want);
        unimplemented!("Radio::pump_until — exercise")
    }
}
