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

use rylr998_core::{BAUD, Command, Driver, OwnedEvent, Poll, Response, RfParams};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio_serial::SerialPortBuilderExt;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const FACTORY_RESET_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("core: {0}")]
    Core(#[from] rylr998_core::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serial: {0}")]
    Serial(#[from] tokio_serial::Error),
    #[error("timeout")]
    Timeout,
    #[error("radio error code {0}")]
    Radio(u8),
    #[error("background task ended")]
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedResponse {
    Ok,
    Err(u8),
    Address(u16),
    NetworkId(u8),
    Band(u32),
    Parameters(RfParams),
    Crfop(u8),
    Uid(String),
    Version(String),
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
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedCommand {
    Ping,
    GetAddress,
    SetAddress(u16),
    GetNetworkId,
    SetNetworkId(u8),
    GetBand,
    SetBand(u32),
    GetParameters,
    SetParameters(RfParams),
    GetCrfop,
    GetUid,
    GetVersion,
    FactoryReset,
    Send { to: u16, data: Vec<u8> },
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

pub type Result<T> = std::result::Result<T, Error>;

pub struct Job {
    cmd: OwnedCommand,
    reply: oneshot::Sender<Result<OwnedResponse>>,
    timeout: Duration,
}

impl Job {
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

pub struct AsyncRadio {
    cmd_tx: mpsc::Sender<Job>,
    event_rx: mpsc::Receiver<OwnedEvent>,
}

impl AsyncRadio {
    pub async fn open(path: &Path) -> Result<Self> {
        let port = tokio_serial::new(path.to_string_lossy(), BAUD).open_native_async()?;
        Ok(Self::from_port(port))
    }

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

    pub async fn ping(&mut self) -> Result<()> {
        match self.request(OwnedCommand::Ping, DEFAULT_TIMEOUT).await? {
            OwnedResponse::Ok => Ok(()),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

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

    pub async fn band(&mut self) -> Result<u32> {
        match self.request(OwnedCommand::GetBand, DEFAULT_TIMEOUT).await? {
            OwnedResponse::Band(n) => Ok(n),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

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

    pub async fn uid(&mut self) -> Result<String> {
        match self.request(OwnedCommand::GetUid, DEFAULT_TIMEOUT).await? {
            OwnedResponse::Uid(s) => Ok(s),
            OwnedResponse::Err(n) => Err(Error::Radio(n)),
            _ => Err(Error::Core(rylr998_core::Error::Parse)),
        }
    }

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
