#![no_std]
//! `no_std` driver for the REYAX RYLR998 LoRa radio module, built on
//! [embassy](https://embassy.dev/) and the [`embedded_io_async`] traits.
//!
//! Wrap any UART that implements `embedded_io_async::Read + Write` and you
//! get an async `Radio<UART>` exposing the same surface as
//! [`rylr998-std`](https://crates.io/crates/rylr998-std) /
//! [`rylr998-tokio`](https://crates.io/crates/rylr998-tokio):
//! `ping`, `set_address` / `address`, `set_network_id` / `network_id`,
//! `set_band` / `band`, `set_parameters` / `parameters`, `crfop`,
//! `factory_reset`, `send`, and a callback-style `next_event`.
//!
//! ## Example
//!
//! ```ignore
//! let uart = uart::BufferedUart::new(/* … */);
//! let mut radio = rylr998_embassy::Radio::new(uart);
//! radio.ping().await?;
//! radio.set_address(5).await?;
//! radio.next_event(Duration::from_secs(60), |e| {
//!     defmt::info!("event: {:?}", defmt::Debug2Format(&e));
//!     None::<()>
//! }).await.ok();
//! ```
//!
//! See `examples/pico_smoke.rs` for an end-to-end Pico 2 + RYLR998 sketch.

use embassy_time::{Duration, Instant};
use rylr998_core::{Command, Driver, Event, Poll, Response, RfParams};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const FACTORY_RESET_TIMEOUT: Duration = Duration::from_secs(2);

/// Failure modes for the embassy driver.
///
/// `E` is the associated error type of the wrapped UART's
/// `embedded_io_async::ErrorType` impl.
#[derive(Debug)]
pub enum Error<E> {
    /// A protocol-layer error from [`rylr998_core`].
    Core(rylr998_core::Error),
    /// An I/O error from the underlying UART.
    Io(E),
    /// A command did not receive its response within the per-call
    /// deadline.
    Timeout,
    /// The radio replied with `+ERR=<code>`. The numeric code is from
    /// the REYAX AT command manual.
    Radio(u8),
}

impl<E> From<rylr998_core::Error> for Error<E> {
    fn from(e: rylr998_core::Error) -> Self {
        Self::Core(e)
    }
}

/// `no_std` async driver for a RYLR998 attached to an
/// `embedded_io_async::Read + Write` UART.
///
/// Wraps a [`rylr998_core::Driver`] and an `embedded_io_async` UART;
/// every method submits one `AT+…` line and awaits its response, with
/// a 1 s deadline (`factory_reset` uses 2 s). Unsolicited `+RCV`
/// events arriving while a command is in flight are forwarded to
/// `defmt::info!` and otherwise dropped — to capture them, drive the
/// radio with [`next_event`](Self::next_event) when not otherwise
/// commanding it.
///
/// No allocator is required: the entire pipeline lives on the stack
/// or inside the `Driver`'s fixed buffers.
pub struct Radio<UART> {
    driver: Driver,
    uart: UART,
}

fn wait_ok<E>(r: Response<'_>) -> Option<Result<(), Error<E>>> {
    match r {
        Response::Ok => Some(Ok(())),
        Response::Err(n) => Some(Err(Error::Radio(n))),
        _ => None,
    }
}

fn log_event(e: Event<'_>) {
    match e {
        Event::Recv { from, data, rssi, snr } => {
            defmt::info!(
                "recv from={} len={} rssi={} snr={}",
                from,
                data.len(),
                rssi,
                snr,
            );
        }
        Event::Ready => defmt::info!("event: ready"),
    }
}

impl<UART> Radio<UART>
where
    UART: embedded_io_async::Read + embedded_io_async::Write,
{
    /// Wrap an `embedded_io_async` UART. The caller is responsible for
    /// configuring the UART at the RYLR998's baud (115 200 by default;
    /// see [`rylr998_core::BAUD`]).
    pub fn new(uart: UART) -> Self {
        let driver = Driver::new();
        Self { uart, driver }
    }

    fn deadline(d: Duration) -> Instant {
        Instant::now() + d
    }

    /// Send `AT` and await `+OK`. Useful as a liveness check.
    pub async fn ping(&mut self) -> Result<(), Error<UART::Error>> {
        self.driver.submit(Command::Ping)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok).await
    }

    /// Set this node's address (`AT+ADDRESS=<n>`).
    pub async fn set_address(&mut self, n: u16) -> Result<(), Error<UART::Error>> {
        self.driver.submit(Command::SetAddress(n))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok).await
    }

    /// Query this node's address (`AT+ADDRESS?`).
    pub async fn address(&mut self) -> Result<u16, Error<UART::Error>> {
        self.driver.submit(Command::GetAddress)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Address(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
        .await
    }

    /// Set the network ID (`AT+NETWORKID=<n>`). Peers must share an ID
    /// to communicate.
    pub async fn set_network_id(&mut self, n: u8) -> Result<(), Error<UART::Error>> {
        self.driver.submit(Command::SetNetworkId(n))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok).await
    }

    /// Query the network ID (`AT+NETWORKID?`).
    pub async fn network_id(&mut self) -> Result<u8, Error<UART::Error>> {
        self.driver.submit(Command::GetNetworkId)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::NetworkId(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
        .await
    }

    /// Set the carrier frequency in Hz (`AT+BAND=<hz>`).
    pub async fn set_band(&mut self, hz: u32) -> Result<(), Error<UART::Error>> {
        self.driver.submit(Command::SetBand(hz))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok).await
    }

    /// Query the carrier frequency in Hz (`AT+BAND?`).
    pub async fn band(&mut self) -> Result<u32, Error<UART::Error>> {
        self.driver.submit(Command::GetBand)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Band(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
        .await
    }

    /// Set LoRa PHY parameters (`AT+PARAMETER=<sf>,<bw>,<cr>,<preamble>`).
    pub async fn set_parameters(&mut self, p: RfParams) -> Result<(), Error<UART::Error>> {
        self.driver.submit(Command::SetParameters(p))?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok).await
    }

    /// Query the LoRa PHY parameters (`AT+PARAMETER?`).
    pub async fn parameters(&mut self) -> Result<RfParams, Error<UART::Error>> {
        self.driver.submit(Command::GetParameters)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Parameters(p) => Some(Ok(p)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
        .await
    }

    /// Query the configured RF output power (`AT+CRFOP?`).
    pub async fn crfop(&mut self) -> Result<u8, Error<UART::Error>> {
        self.driver.submit(Command::GetCrfop)?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), |r| match r {
            Response::Crfop(n) => Some(Ok(n)),
            Response::Err(n) => Some(Err(Error::Radio(n))),
            _ => None,
        })
        .await
    }

    // `uid` and `version` return owned Strings in the std/tokio ports; here we'd
    // need a `heapless::String<N>` to avoid alloc. Skipped for the smoke test.

    /// Send `AT+RESET` and await the module's `+READY` reboot signal.
    ///
    /// Uses an extended (2 s) timeout for the post-reset settling time.
    pub async fn factory_reset(&mut self) -> Result<(), Error<UART::Error>> {
        self.driver.submit(Command::FactoryReset)?;
        self.pump_until(Self::deadline(FACTORY_RESET_TIMEOUT), wait_ok).await
    }

    /// Transmit `data` to the node at address `to`
    /// (`AT+SEND=<to>,<len>,<data>`). Use `to = 0` to broadcast.
    pub async fn send(&mut self, to: u16, data: &[u8]) -> Result<(), Error<UART::Error>> {
        self.driver.submit(Command::Send { to, data })?;
        self.pump_until(Self::deadline(DEFAULT_TIMEOUT), wait_ok).await
    }

    /// Wait for the next unsolicited event. The handler is invoked with each
    /// `Event` as it arrives; return `Some(r)` from the handler to terminate
    /// and yield `r`, or `None` to continue waiting.
    ///
    /// `Event<'_>` borrows from the driver's line buffer, so the handler
    /// can't store it. Copy what you need out (e.g., `from`, `rssi`, `snr`,
    /// or a slice of `data`) before returning.
    pub async fn next_event<F, R>(
        &mut self,
        timeout: Duration,
        mut handler: F,
    ) -> Result<R, Error<UART::Error>>
    where
        F: FnMut(Event<'_>) -> Option<R>,
    {
        let deadline = Self::deadline(timeout);
        loop {
            loop {
                match self.driver.poll() {
                    Poll::NeedTx(bytes) => {
                        let n = bytes.len();
                        self.uart.write_all(bytes).await.map_err(Error::Io)?;
                        self.driver.ack_tx(n);
                    }
                    Poll::Event(e) => {
                        if let Some(r) = handler(e) {
                            return Ok(r);
                        }
                    }
                    Poll::Response(_) => {} // unexpected without an in-flight command
                    Poll::Idle => break,
                }
            }
            let mut buf = [0u8; 256];
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.as_ticks() == 0 {
                return Err(Error::Timeout);
            }
            match embassy_time::with_timeout(remaining, self.uart.read(&mut buf)).await {
                Ok(Ok(n)) => {
                    self.driver.push_rx(&buf[..n])?;
                }
                Ok(Err(e)) => return Err(Error::Io(e)),
                Err(_) => {} // outer loop will check deadline next iteration
            }
        }
    }

    pub(crate) async fn pump_until<R, F>(
        &mut self,
        deadline: Instant,
        mut want: F,
    ) -> Result<R, Error<UART::Error>>
    where
        F: FnMut(Response<'_>) -> Option<Result<R, Error<UART::Error>>>,
    {
        loop {
            loop {
                match self.driver.poll() {
                    Poll::NeedTx(bytes) => {
                        let n = bytes.len();
                        self.uart.write_all(bytes).await.map_err(Error::Io)?;
                        self.driver.ack_tx(n);
                    }
                    Poll::Response(r) => {
                        if let Some(out) = want(r) {
                            return out;
                        }
                    }
                    Poll::Event(e) => log_event(e),
                    Poll::Idle => break,
                }
            }
            let mut buf = [0u8; 256];
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.as_ticks() == 0 {
                return Err(Error::Timeout);
            }

            match embassy_time::with_timeout(remaining, self.uart.read(&mut buf)).await {
                Ok(Ok(n)) => {
                    self.driver.push_rx(&buf[..n])?;
                }
                Ok(Err(e)) => return Err(Error::Io(e)),
                Err(_) => {} // unhandled for now
            }
        }
    }
}
