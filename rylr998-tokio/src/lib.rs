//! Async host driver for the REYAX RYLR998 LoRa radio module, built on
//! [Tokio](https://tokio.rs) and `tokio-serial`.
//!
//! A background task owns the [`rylr998_core::Driver`] and the serial port;
//! the handle ([`AsyncRadio`]) ships commands over an `mpsc` channel and
//! awaits replies via per-request `oneshot`s. Unsolicited events arrive on
//! a second channel — call [`AsyncRadio::next_event`] to receive them.
//!
//! ```no_run
//! # use std::path::Path;
//! # async fn run() -> Result<(), rylr998_tokio::Error> {
//! let mut radio = rylr998_tokio::AsyncRadio::open(Path::new("/dev/cu.usbserial-X")).await?;
//! radio.set_address(5).await?;
//! radio.set_network_id(18).await?;
//! radio.send(2, b"hello").await?;
//! # Ok(())
//! # }
//! ```
//!
//! See [`rylr998-std`] for a blocking equivalent and [`rylr998-embassy`]
//! for the `no_std` embassy variant.
//!
//! [`rylr998-std`]: https://crates.io/crates/rylr998-std
//! [`rylr998-embassy`]: https://crates.io/crates/rylr998-embassy

pub use rylr998_core::RadioError;
use rylr998_core::{BAUD, Command, Driver, OwnedEvent, Poll, Response, RfParams};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio_serial::SerialPortBuilderExt;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const FACTORY_RESET_TIMEOUT: Duration = Duration::from_secs(2);

/// Failure modes for the async driver.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// A protocol-layer error from [`rylr998_core`].
    #[error("core: {0}")]
    Core(#[from] rylr998_core::Error),
    /// An I/O error from the underlying serial port.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// An error opening the serial port.
    #[error("serial: {0}")]
    Serial(#[from] tokio_serial::Error),
    /// A command did not receive its response within the per-call
    /// deadline.
    #[error("timeout")]
    Timeout,
    /// The radio replied with `+ERR=<code>`. The numeric code is from
    /// the REYAX AT command manual.
    #[error("radio error code {0}")]
    Radio(u8),
    /// The background reader/writer task has exited, usually because
    /// the serial port closed.
    #[error("background task ended")]
    Closed,
}

/// Heap-backed [`Response`] variant.
///
/// The driver's background task only sees the `'static`-bounded form,
/// so the channel between the task and [`AsyncRadio`] carries this
/// owned analogue of [`rylr998_core::Response`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedResponse {
    /// `+OK` — command accepted.
    Ok,
    /// `+ERR=<code>`.
    Err(u8),
    /// Reply to a `GetAddress` request.
    Address(u16),
    /// Reply to a `GetNetworkId` request.
    NetworkId(u8),
    /// Reply to a `GetBand` request.
    Band(u32),
    /// Reply to a `GetParameters` request.
    Parameters(RfParams),
    /// Reply to a `GetCrfop` request.
    Crfop(u8),
    /// Reply to a `GetUid` request.
    Uid(String),
    /// Reply to a `GetVersion` request.
    Version(String),
    /// Reply to a `GetCpin` request. Empty means `No Password!`.
    Cpin(String),
}

impl From<Response<'_>> for OwnedResponse {
    fn from(value: Response<'_>) -> OwnedResponse {
        match value {
            Response::Ok => Self::Ok,
            Response::Err(n) => Self::Err(n),
            Response::Address(n) => Self::Address(n),
            Response::NetworkId(n) => Self::NetworkId(n),
            Response::Band(n) => Self::Band(n),
            Response::Parameters(rf_params) => Self::Parameters(rf_params),
            Response::Crfop(n) => Self::Crfop(n),
            Response::Uid(s) => Self::Uid(s.to_string()),
            Response::Version(s) => Self::Version(s.to_string()),
            Response::Cpin(s) => Self::Cpin(s.to_owned()),
        }
    }
}

/// Heap-backed [`Command`] variant.
///
/// `Command<'a>` borrows its `Send` payload, which makes it unsendable
/// across the channel into the driver task. `OwnedCommand` owns the
/// payload so jobs can be queued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedCommand {
    /// See [`Command::Ping`].
    Ping,
    /// See [`Command::GetAddress`].
    GetAddress,
    /// See [`Command::SetAddress`].
    SetAddress(u16),
    /// See [`Command::GetNetworkId`].
    GetNetworkId,
    /// See [`Command::SetNetworkId`].
    SetNetworkId(u8),
    /// See [`Command::GetBand`].
    GetBand,
    /// See [`Command::SetBand`].
    SetBand(u32),
    /// See [`Command::GetCpin`].
    GetCpin,
    /// See [`Command::SetCpin`].
    SetCpin(Vec<u8>),
    /// See [`Command::GetParameters`].
    GetParameters,
    /// See [`Command::SetParameters`].
    SetParameters(RfParams),
    /// See [`Command::GetCrfop`].
    GetCrfop,
    /// See [`Command::GetUid`].
    GetUid,
    /// See [`Command::GetVersion`].
    GetVersion,
    /// See [`Command::FactoryReset`].
    FactoryReset,
    /// See [`Command::Send`]; payload owned as a `Vec<u8>`.
    Send {
        /// Destination address.
        to: u16,
        /// Payload bytes, owned.
        data: Vec<u8>,
    },
}

impl OwnedCommand {
    fn as_command(&self) -> Command<'_> {
        match self {
            Self::Ping => Command::Ping,
            Self::GetAddress => Command::GetAddress,
            Self::SetAddress(n) => Command::SetAddress(*n),
            Self::GetNetworkId => Command::GetNetworkId,
            Self::SetNetworkId(n) => Command::SetNetworkId(*n),
            Self::GetBand => Command::GetBand,
            Self::SetBand(n) => Command::SetBand(*n),
            Self::GetCpin => Command::GetCpin,
            Self::SetCpin(password) => Command::SetCpin(password),
            Self::GetParameters => Command::GetParameters,
            Self::SetParameters(p) => Command::SetParameters(*p),
            Self::GetCrfop => Command::GetCrfop,
            Self::GetUid => Command::GetUid,
            Self::GetVersion => Command::GetVersion,
            Self::FactoryReset => Command::FactoryReset,
            Self::Send { to, data } => Command::Send { to: *to, data },
        }
    }
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

/// One queued unit of work for the background driver task: a command,
/// the reply channel to send its [`OwnedResponse`] back on, and the
/// per-call timeout.
pub struct Job {
    cmd: OwnedCommand,
    reply: oneshot::Sender<Result<OwnedResponse>>,
    timeout: Duration,
}

impl Job {
    /// Build a new job. Mainly useful if you're driving the
    /// `Job`-channel side yourself; ordinary use goes through
    /// [`AsyncRadio`]'s methods.
    pub fn new(
        cmd: OwnedCommand,
        reply: oneshot::Sender<Result<OwnedResponse>>,
        timeout: Duration,
    ) -> Self {
        Self {
            cmd,
            reply,
            timeout,
        }
    }
}

/// Async handle to a RYLR998 driven by a background Tokio task.
///
/// The actual `rylr998_core::Driver` and the serial port live in a
/// `tokio::spawn`ed task; `AsyncRadio` is a thin handle that sends
/// commands over an `mpsc` and awaits their replies on `oneshot`s.
/// Unsolicited events flow on a separate channel — pull them with
/// [`next_event`](Self::next_event).
///
/// Methods take `&mut self` because the command channel is single-
/// consumer; serialize sends through one `AsyncRadio` per radio.
pub struct AsyncRadio {
    cmd_tx: mpsc::Sender<Job>,
    event_rx: mpsc::Receiver<OwnedEvent>,
}

impl AsyncRadio {
    /// Open the radio on a specific serial-port path.
    ///
    /// Configures the port at [`rylr998_core::BAUD`] and spawns the
    /// background driver task.
    pub async fn open(path: &Path) -> Result<Self> {
        let port = tokio_serial::new(path.to_string_lossy(), BAUD).open_native_async()?;
        Ok(Self::from_port(port))
    }

    /// Construct an `AsyncRadio` from an already-open async port.
    ///
    /// Useful for tests with `tokio::io::duplex` and for embedding the
    /// driver in code that owns its own transport. Spawns the
    /// background driver task; the returned handle owns one end of the
    /// command and event channels.
    pub fn from_port<S>(port: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Job>(8);
        let (event_tx, event_rx) = mpsc::channel::<OwnedEvent>(64);
        tokio::spawn(run_task(port, cmd_rx, event_tx));
        Self { cmd_tx, event_rx }
    }

    async fn request(&mut self, cmd: OwnedCommand, timeout: Duration) -> Result<OwnedResponse> {
        let (reply, reply_rx) = oneshot::channel();
        self.cmd_tx
            .send(Job::new(cmd, reply, timeout))
            .await
            .map_err(|_| Error::Closed)?;
        match tokio::time::timeout(timeout, reply_rx).await {
            Ok(Ok(inner)) => inner,
            Ok(Err(_)) => Err(Error::Closed),
            Err(_) => Err(Error::Timeout),
        }
    }

    /// Send `AT` and await `+OK`. Useful as a liveness check.
    pub async fn ping(&mut self) -> Result<()> {
        match self.request(OwnedCommand::Ping, DEFAULT_TIMEOUT).await? {
            OwnedResponse::Ok => Ok(()),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Set this node's address (`AT+ADDRESS=<n>`).
    pub async fn set_address(&mut self, n: u16) -> Result<()> {
        match self
            .request(OwnedCommand::SetAddress(n), DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::Ok => Ok(()),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Query this node's address (`AT+ADDRESS?`).
    pub async fn address(&mut self) -> Result<u16> {
        match self
            .request(OwnedCommand::GetAddress, DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::Address(n) => Ok(n),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Set the network ID (`AT+NETWORKID=<n>`). Peers must share an ID
    /// to communicate.
    pub async fn set_network_id(&mut self, n: u8) -> Result<()> {
        match self
            .request(OwnedCommand::SetNetworkId(n), DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::Ok => Ok(()),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Query the network ID (`AT+NETWORKID?`).
    pub async fn network_id(&mut self) -> Result<u8> {
        match self
            .request(OwnedCommand::GetNetworkId, DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::NetworkId(n) => Ok(n),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Set the carrier frequency in Hz (`AT+BAND=<hz>`).
    pub async fn set_band(&mut self, hz: u32) -> Result<()> {
        match self
            .request(OwnedCommand::SetBand(hz), DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::Ok => Ok(()),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Query the carrier frequency in Hz (`AT+BAND?`).
    pub async fn band(&mut self) -> Result<u32> {
        match self.request(OwnedCommand::GetBand, DEFAULT_TIMEOUT).await? {
            OwnedResponse::Band(n) => Ok(n),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Query the 8-character domain password (`AT+CPIN?`).
    ///
    /// Returns an empty string when the radio reports `No Password!`.
    pub async fn cpin(&mut self) -> Result<String> {
        match self.request(OwnedCommand::GetCpin, DEFAULT_TIMEOUT).await? {
            OwnedResponse::Cpin(s) => Ok(s),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Set the 8-character domain password (`AT+CPIN=<password>`).
    ///
    /// The radio replies with `+ERR=5` if the password length is invalid.
    /// Valid passwords are 8 ASCII hex bytes in the documented `00000001`
    /// through `FFFFFFFF` range.
    pub async fn set_cpin(&mut self, password: &[u8]) -> Result<()> {
        match self
            .request(OwnedCommand::SetCpin(password.into()), DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::Ok => Ok(()),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Set LoRa PHY parameters (`AT+PARAMETER=<sf>,<bw>,<cr>,<preamble>`).
    pub async fn set_parameters(&mut self, p: RfParams) -> Result<()> {
        match self
            .request(OwnedCommand::SetParameters(p), DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::Ok => Ok(()),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Query the LoRa PHY parameters (`AT+PARAMETER?`).
    pub async fn parameters(&mut self) -> Result<RfParams> {
        match self
            .request(OwnedCommand::GetParameters, DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::Parameters(p) => Ok(p),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Query the configured RF output power (`AT+CRFOP?`).
    pub async fn crfop(&mut self) -> Result<u8> {
        match self
            .request(OwnedCommand::GetCrfop, DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::Crfop(n) => Ok(n),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Query the module's unique ID (`AT+UID?`).
    pub async fn uid(&mut self) -> Result<String> {
        match self.request(OwnedCommand::GetUid, DEFAULT_TIMEOUT).await? {
            OwnedResponse::Uid(s) => Ok(s),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Query the firmware version string (`AT+VER?`).
    pub async fn version(&mut self) -> Result<String> {
        match self
            .request(OwnedCommand::GetVersion, DEFAULT_TIMEOUT)
            .await?
        {
            OwnedResponse::Version(s) => Ok(s),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Send `AT+RESET` and await the module's `+READY` reboot signal.
    ///
    /// Uses an extended (2 s) timeout for the post-reset settling time.
    pub async fn factory_reset(&mut self) -> Result<()> {
        match self
            .request(OwnedCommand::FactoryReset, FACTORY_RESET_TIMEOUT)
            .await?
        {
            OwnedResponse::Ok => Ok(()),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Transmit `data` to the node at address `to`
    /// (`AT+SEND=<to>,<len>,<data>`). Use `to = 0` to broadcast.
    pub async fn send(&mut self, to: u16, data: &[u8]) -> Result<()> {
        match self
            .request(
                OwnedCommand::Send {
                    to,
                    data: data.into(),
                },
                DEFAULT_TIMEOUT,
            )
            .await?
        {
            OwnedResponse::Ok => Ok(()),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

    /// Wait up to `timeout` for the next unsolicited event from the
    /// radio.
    ///
    /// Events received while another command is in flight are queued
    /// by the background task and surfaced here in arrival order.
    ///
    /// # Errors
    ///
    /// - [`Error::Timeout`] if no event arrives before the deadline.
    /// - [`Error::Closed`] if the background task has exited.
    pub async fn next_event(&mut self, timeout: Duration) -> Result<OwnedEvent> {
        tokio::time::timeout(timeout, self.event_rx.recv())
            .await
            .map_err(|_| Error::Timeout)?
            .ok_or(Error::Closed)
    }
}

async fn run_task<S>(
    mut port: S,
    mut cmd_rx: mpsc::Receiver<Job>,
    event_tx: mpsc::Sender<OwnedEvent>,
) where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let mut driver = Driver::new();
    let mut buf = [0u8; 256];

    loop {
        tokio::select! {
            maybe_job = cmd_rx.recv() => {
                let Some(Job { cmd, reply, timeout }) = maybe_job else { break };
                let result = process_job(&mut driver, &mut port, &event_tx, &mut buf, cmd, timeout).await;
                let _ = reply.send(result);
            }
            read = port.read(&mut buf) => {
                match read {
                    Ok(0) => break,
                    Ok(n) => {
                        let _ = driver.push_rx(&buf[..n]);
                        loop {
                            match driver.poll() {
                                Poll::Event(e) => {
                                    if event_tx.send(e.into_owned()).await.is_err() {
                                        return;
                                    }
                                }
                                Poll::Idle => break,
                                _ => break,
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
    }
}

async fn process_job<S>(
    driver: &mut Driver,
    port: &mut S,
    event_tx: &mpsc::Sender<OwnedEvent>,
    buf: &mut [u8; 256],
    cmd: OwnedCommand,
    timeout: Duration,
) -> Result<OwnedResponse>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    driver.submit(cmd.as_command()).map_err(Error::Core)?;
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        loop {
            match driver.poll() {
                Poll::NeedTx(bytes) => {
                    let n = bytes.len();
                    port.write_all(bytes).await.map_err(Error::Io)?;
                    driver.ack_tx(n);
                }
                Poll::Response(r) => return Ok(r.into()),
                Poll::Event(e) => {
                    let _ = event_tx.send(e.into_owned()).await;
                }
                Poll::Idle => break,
            }
        }

        match tokio::time::timeout_at(deadline, port.read(buf)).await {
            Ok(Ok(0)) => return Err(Error::Closed),
            Ok(Ok(n)) => {
                let _ = driver.push_rx(&buf[..n]);
            }
            Ok(Err(e)) => return Err(Error::Io(e)),
            Err(_) => return Err(Error::Timeout),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{AsyncRadio, Error};
    use rylr998_core::{OwnedEvent, RfParams};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn read_and_expect(wire: &mut tokio::io::DuplexStream, expected: &[u8]) {
        let mut buf = [0u8; 64];
        let n = wire.read(&mut buf).await.unwrap();
        let actual = &buf[..n];
        assert_eq!(
            actual,
            expected,
            "actual: {}, expected: {}",
            String::from_utf8_lossy(actual),
            String::from_utf8_lossy(expected),
        );
    }

    #[tokio::test]
    async fn ping() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.ping().await });
        read_and_expect(&mut wire, b"AT\r\n").await;
        wire.write_all(b"+OK\r\n").await.unwrap();
        radio_fut.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn set_address() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.set_address(5).await });
        read_and_expect(&mut wire, b"AT+ADDRESS=5\r\n").await;
        wire.write_all(b"+OK\r\n").await.unwrap();
        radio_fut.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn address() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.address().await });
        read_and_expect(&mut wire, b"AT+ADDRESS?\r\n").await;
        wire.write_all(b"+ADDRESS=5\r\n").await.unwrap();
        assert_eq!(radio_fut.await.unwrap().unwrap(), 5);
    }

    #[tokio::test]
    async fn send() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.send(2, b"ping").await });
        read_and_expect(&mut wire, b"AT+SEND=2,4,ping\r\n").await;
        wire.write_all(b"+OK\r\n").await.unwrap();
        radio_fut.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn next_event() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);

        wire.write_all(b"+RCV=1,5,hello,-23,5\r\n").await.unwrap();

        let radio_fut =
            tokio::spawn(async move { radio.next_event(Duration::from_millis(200)).await });

        assert_eq!(
            radio_fut.await.unwrap().unwrap(),
            OwnedEvent::Recv {
                from: 1,
                data: b"hello".into(),
                rssi: -23,
                snr: 5
            }
        )
    }

    #[tokio::test]
    async fn set_network_id() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.set_network_id(18).await });
        read_and_expect(&mut wire, b"AT+NETWORKID=18\r\n").await;
        wire.write_all(b"+OK\r\n").await.unwrap();
        radio_fut.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn network_id() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.network_id().await });
        read_and_expect(&mut wire, b"AT+NETWORKID?\r\n").await;
        wire.write_all(b"+NETWORKID=18\r\n").await.unwrap();
        assert_eq!(radio_fut.await.unwrap().unwrap(), 18);
    }

    #[tokio::test]
    async fn set_band() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.set_band(915_000_000).await });
        read_and_expect(&mut wire, b"AT+BAND=915000000\r\n").await;
        wire.write_all(b"+OK\r\n").await.unwrap();
        radio_fut.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn band() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.band().await });
        read_and_expect(&mut wire, b"AT+BAND?\r\n").await;
        wire.write_all(b"+BAND=915000000\r\n").await.unwrap();
        assert_eq!(radio_fut.await.unwrap().unwrap(), 915_000_000);
    }

    #[tokio::test]
    async fn set_cpin() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.set_cpin(b"EEDCAA90").await });
        read_and_expect(&mut wire, b"AT+CPIN=EEDCAA90\r\n").await;
        wire.write_all(b"+OK\r\n").await.unwrap();
        radio_fut.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn set_cpin_propagates_radio_error_5_for_wrong_length() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.set_cpin(b"hunter2").await });
        read_and_expect(&mut wire, b"AT+CPIN=hunter2\r\n").await;
        wire.write_all(b"+ERR=5\r\n").await.unwrap();
        let err = radio_fut.await.unwrap().unwrap_err();
        assert!(matches!(err, Error::Radio(5)));
    }

    #[tokio::test]
    async fn cpin() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.cpin().await });
        read_and_expect(&mut wire, b"AT+CPIN?\r\n").await;
        wire.write_all(b"+CPIN=eedcaa90\r\n").await.unwrap();
        assert_eq!(radio_fut.await.unwrap().unwrap(), "eedcaa90");
    }

    #[tokio::test]
    async fn cpin_no_password() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.cpin().await });
        read_and_expect(&mut wire, b"AT+CPIN?\r\n").await;
        wire.write_all(b"+CPIN=No Password!\r\n").await.unwrap();
        assert_eq!(radio_fut.await.unwrap().unwrap(), "");
    }

    #[tokio::test]
    async fn set_parameters() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let p = RfParams {
            sf: 9,
            bw: 7,
            cr: 1,
            preamble: 12,
        };
        let radio_fut = tokio::spawn(async move { radio.set_parameters(p).await });
        read_and_expect(&mut wire, b"AT+PARAMETER=9,7,1,12\r\n").await;
        wire.write_all(b"+OK\r\n").await.unwrap();
        radio_fut.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn parameters() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.parameters().await });
        read_and_expect(&mut wire, b"AT+PARAMETER?\r\n").await;
        wire.write_all(b"+PARAMETER=9,7,1,12\r\n").await.unwrap();
        assert_eq!(
            radio_fut.await.unwrap().unwrap(),
            RfParams {
                sf: 9,
                bw: 7,
                cr: 1,
                preamble: 12
            },
        );
    }

    #[tokio::test]
    async fn crfop() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.crfop().await });
        read_and_expect(&mut wire, b"AT+CRFOP?\r\n").await;
        wire.write_all(b"+CRFOP=15\r\n").await.unwrap();
        assert_eq!(radio_fut.await.unwrap().unwrap(), 15);
    }

    #[tokio::test]
    async fn uid() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.uid().await });
        read_and_expect(&mut wire, b"AT+UID?\r\n").await;
        wire.write_all(b"+UID=ABCD1234\r\n").await.unwrap();
        assert_eq!(radio_fut.await.unwrap().unwrap(), "ABCD1234");
    }

    #[tokio::test]
    async fn version() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.version().await });
        read_and_expect(&mut wire, b"AT+VER?\r\n").await;
        wire.write_all(b"+VER=1.2.3\r\n").await.unwrap();
        assert_eq!(radio_fut.await.unwrap().unwrap(), "1.2.3");
    }

    #[tokio::test]
    async fn factory_reset_waits_for_ready() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.factory_reset().await });
        read_and_expect(&mut wire, b"AT+RESET\r\n").await;
        // AT+RESET emits both +RESET (intermediate) and +READY (done). The
        // driver drains +RESET silently and resolves only on +READY.
        wire.write_all(b"+RESET\r\n+READY\r\n").await.unwrap();
        radio_fut.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn err_response_propagates() {
        let (radio_side, mut wire) = tokio::io::duplex(4096);
        let mut radio = AsyncRadio::from_port(radio_side);
        let radio_fut = tokio::spawn(async move { radio.set_address(0xFFFF).await });
        read_and_expect(&mut wire, b"AT+ADDRESS=65535\r\n").await;
        wire.write_all(b"+ERR=4\r\n").await.unwrap();
        let r = radio_fut.await.unwrap();
        assert!(matches!(r, Err(Error::Radio(4))));
    }
}
