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

use rylr_core::Command;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const FACTORY_RESET_TIMEOUT: Duration = Duration::from_secs(2);

impl<P: Read + Write> Radio<P> {
    fn deadline(d: Duration) -> Instant {
        Instant::now() + d
    }

    pub fn ping(&mut self) -> Result<()> {
        self.driver.submit(Command::Ping)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    pub fn set_address(&mut self, n: u16) -> Result<()> {
        self.driver.submit(Command::SetAddress(n))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    pub fn address(&mut self) -> Result<u16> {
        self.driver.submit(Command::GetAddress)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Address(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    pub fn set_network_id(&mut self, n: u8) -> Result<()> {
        self.driver.submit(Command::SetNetworkId(n))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    pub fn network_id(&mut self) -> Result<u8> {
        self.driver.submit(Command::GetNetworkId)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::NetworkId(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    pub fn set_band(&mut self, hz: u32) -> Result<()> {
        self.driver.submit(Command::SetBand(hz))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    pub fn band(&mut self) -> Result<u32> {
        self.driver.submit(Command::GetBand)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Band(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    pub fn set_parameters(&mut self, p: rylr_core::RfParams) -> Result<()> {
        self.driver.submit(Command::SetParameters(p))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    pub fn parameters(&mut self) -> Result<rylr_core::RfParams> {
        self.driver.submit(Command::GetParameters)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Parameters(p) => Some(Ok(p)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    pub fn crfop(&mut self) -> Result<u8> {
        self.driver.submit(Command::GetCrfop)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Crfop(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    pub fn uid(&mut self) -> Result<String> {
        self.driver.submit(Command::GetUid)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Uid(s) => Some(Ok(s.to_owned())),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    pub fn version(&mut self) -> Result<String> {
        self.driver.submit(Command::GetVersion)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Version(s) => Some(Ok(s.to_owned())),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    pub fn factory_reset(&mut self) -> Result<()> {
        self.driver.submit(Command::FactoryReset)?;
        self.pump_until(Self::deadline(FACTORY_RESET_TIMEOUT), wait_ok)
    }

    pub fn send(&mut self, to: u16, data: &[u8]) -> Result<()> {
        self.driver.submit(Command::Send { to, data })?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    pub fn next_event(&mut self, timeout: Duration) -> Result<OwnedEvent> {
        if let Some(e) = self.events.pop_front() {
            return Ok(e);
        }
        let deadline = Instant::now() + timeout;
        // Pump with a `want` that never matches a Response: events accumulate.
        // When the queue becomes non-empty, return.
        self.pump_until(deadline, |_| None::<Result<()>>).ok();
        self.events.pop_front().ok_or(Error::Timeout)
    }
}

fn wait_ok(r: Response<'_>) -> Option<Result<()>> {
    match r {
        Response::Ok => Some(Ok(())),
        Response::Err(n) => Some(Err(Error::Radio(n))),
        _ => None,
    }
}
