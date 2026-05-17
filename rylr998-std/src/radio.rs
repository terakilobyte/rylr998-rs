//! Blocking `Radio<P>`. `P` must be `Read + Write`. The default `P` is
//! `Box<dyn SerialPort>`, used by `open*`. Tests construct `Radio<P>`
//! directly with their own port type.

use crate::{Error, Result};
use rylr998_core::{Driver, OwnedEvent};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Blocking driver for a RYLR998 attached to a serial port.
///
/// `Radio` owns a [`rylr998_core::Driver`] (the protocol state machine)
/// and a port that implements `Read + Write`. The default port type is
/// `Box<dyn serialport::SerialPort>`, opened by [`open`](Radio::open) /
/// [`open_auto`](Radio::open_auto); tests and integration code can
/// substitute their own port via [`from_port`](Radio::from_port).
///
/// Each command method submits one `AT+…` line and blocks until the
/// matching `+OK`/`+ERR`/value response arrives or the per-call deadline
/// expires (1 s, except `factory_reset` which uses 4 s). Unsolicited
/// `+RCV` events received in the meantime are queued and surfaced by
/// [`next_event`](Radio::next_event).
pub struct Radio<P: Read + Write = Box<dyn serialport::SerialPort>> {
    pub(crate) driver: Driver,
    pub(crate) port: P,
    pub(crate) events: VecDeque<OwnedEvent>,
}

impl Radio<Box<dyn serialport::SerialPort>> {
    /// Look for a single `cu.usbserial*` device on the system.
    ///
    /// # Errors
    ///
    /// - [`Error::NoDevice`] if no matching port is found.
    /// - [`Error::Ambiguous`] if more than one matches; open the correct
    ///   one explicitly with [`open`](Self::open).
    pub fn discover() -> Result<PathBuf> {
        crate::port::discover()
    }

    /// Open the radio on a specific serial-port path.
    ///
    /// The port is configured at 115 200 baud (the RYLR998 factory
    /// default; see [`rylr998_core::BAUD`]) with DTR asserted.
    pub fn open(path: &Path) -> Result<Self> {
        let port = serialport::new(path.to_string_lossy(), 115_200)
            .timeout(Duration::from_millis(50))
            .dtr_on_open(true)
            .open()?;
        Ok(Self::from_port(port))
    }

    /// Auto-discover the serial port and open it.
    ///
    /// Equivalent to [`Self::discover`] followed by [`Self::open`].
    pub fn open_auto() -> Result<Self> {
        let path = Self::discover()?;
        Self::open(&path)
    }
}

impl<P: Read + Write> Radio<P> {
    /// Construct a `Radio` from an already-open `Read + Write` port.
    ///
    /// Useful for tests (pair with `std::io::duplex`-like adapters) and
    /// for embedding `Radio` in code that already owns its port.
    pub fn from_port(port: P) -> Self {
        Self {
            driver: Driver::new(),
            port,
            events: VecDeque::new(),
        }
    }
}

use rylr998_core::{Poll, Response};
use std::time::Instant;

impl<P: Read + Write> Radio<P> {
    /// Drive the state machine and the underlying port until the supplied
    /// predicate returns `Some`, or `deadline` is reached.
    pub(crate) fn pump_until<R, F>(&mut self, deadline: Instant, mut want: F) -> Result<R>
    where
        F: FnMut(Response<'_>) -> Option<Result<R>>,
    {
        loop {
            loop {
                match self.driver.poll() {
                    Poll::NeedTx(bytes) => {
                        let n = bytes.len();
                        self.port.write_all(bytes).map_err(Error::Io)?;
                        self.driver.ack_tx(n);
                    }
                    Poll::Response(r) => {
                        if let Some(out) = want(r) {
                            return out;
                        }
                    }
                    Poll::Event(e) => self.events.push_back(e.into_owned()),
                    Poll::Idle => break,
                }
            }
            let mut buf = [0u8; 256];
            match self.port.read(&mut buf[..]) {
                Ok(n) => {
                    self.driver.push_rx(&buf[..n])?;
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {}
                    _ => return Err(Error::Io(e)),
                },
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
        }
    }
}

use rylr998_core::Command;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const FACTORY_RESET_TIMEOUT: Duration = Duration::from_secs(4);

impl<P: Read + Write> Radio<P> {
    fn deadline(d: Duration) -> Instant {
        Instant::now() + d
    }

    /// Send `AT` and wait for `+OK`. Useful as a liveness check.
    pub fn ping(&mut self) -> Result<()> {
        self.driver.submit(Command::Ping)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    /// Send `AT+RESET` and wait for the module to come back up.
    ///
    /// Uses an extended (4 s) timeout to allow for the post-reset
    /// `+READY` line.
    pub fn factory_reset(&mut self) -> Result<()> {
        self.driver.submit(Command::FactoryReset)?;
        self.pump_until(Self::deadline(FACTORY_RESET_TIMEOUT), wait_ok)
    }

    /// Set this node's address (`AT+ADDRESS=<n>`).
    pub fn set_address(&mut self, n: u16) -> Result<()> {
        self.driver.submit(Command::SetAddress(n))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    /// Query this node's address (`AT+ADDRESS?`).
    pub fn address(&mut self) -> Result<u16> {
        self.driver.submit(Command::GetAddress)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Address(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    /// Set the network ID (`AT+NETWORKID=<n>`). Peers must share an ID
    /// to communicate.
    pub fn set_network_id(&mut self, n: u8) -> Result<()> {
        self.driver.submit(Command::SetNetworkId(n))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    /// Query the network ID (`AT+NETWORKID?`).
    pub fn network_id(&mut self) -> Result<u8> {
        self.driver.submit(Command::GetNetworkId)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::NetworkId(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    /// Set the carrier frequency in Hz (`AT+BAND=<hz>`).
    pub fn set_band(&mut self, hz: u32) -> Result<()> {
        self.driver.submit(Command::SetBand(hz))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    /// Query the carrier frequency in Hz (`AT+BAND?`).
    pub fn band(&mut self) -> Result<u32> {
        self.driver.submit(Command::GetBand)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Band(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    /// Query the 8-character domain password. Empty means `No Password!`.
    pub fn cpin(&mut self) -> Result<String> {
        self.driver.submit(Command::GetCpin)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Cpin(s) => Some(Ok(s.to_owned())),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    /// Set the 8-character domain password.
    ///
    /// The radio replies with `+ERR=5` if the password length is invalid.
    /// Valid passwords are 8 ASCII hex bytes in the documented `00000001`
    /// through `FFFFFFFF` range.
    pub fn set_cpin(&mut self, password: &[u8]) -> Result<()> {
        self.driver.submit(Command::SetCpin(password))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    /// Set the LoRa PHY parameters (`AT+PARAMETER=<sf>,<bw>,<cr>,<preamble>`).
    pub fn set_parameters(&mut self, p: rylr998_core::RfParams) -> Result<()> {
        self.driver.submit(Command::SetParameters(p))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    /// Query the LoRa PHY parameters (`AT+PARAMETER?`).
    pub fn parameters(&mut self) -> Result<rylr998_core::RfParams> {
        self.driver.submit(Command::GetParameters)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Parameters(p) => Some(Ok(p)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    /// Query the configured RF output power (`AT+CRFOP?`).
    pub fn crfop(&mut self) -> Result<u8> {
        self.driver.submit(Command::GetCrfop)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Crfop(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    /// Query the module's unique ID (`AT+UID?`).
    pub fn uid(&mut self) -> Result<String> {
        self.driver.submit(Command::GetUid)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Uid(s) => Some(Ok(s.to_owned())),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    /// Query the firmware version string (`AT+VER?`).
    pub fn version(&mut self) -> Result<String> {
        self.driver.submit(Command::GetVersion)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Version(s) => Some(Ok(s.to_owned())),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
    }

    /// Transmit `data` to the node at address `to`
    /// (`AT+SEND=<to>,<len>,<data>`). Use `to = 0` to broadcast.
    pub fn send(&mut self, to: u16, data: &[u8]) -> Result<()> {
        self.driver.submit(Command::Send { to, data })?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok)
    }

    /// Wait up to `timeout` for the next unsolicited event from the
    /// radio (currently always a `+RCV` payload).
    ///
    /// Events received while another command is in flight are buffered
    /// and returned here in arrival order, so this never misses an event.
    ///
    /// # Errors
    ///
    /// [`Error::Timeout`] if no event arrives before the deadline.
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
