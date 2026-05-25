# rylr998-tokio

Async host driver for the REYAX **RYLR998** LoRa radio module, built on
[Tokio](https://tokio.rs) and `tokio-serial`.

A background task owns the serial port and the
[`rylr998-core`](https://crates.io/crates/rylr998-core) state machine.
The `AsyncRadio` handle ships commands over an `mpsc` channel and awaits
each reply via a `oneshot`. Unsolicited events stream over a separate
channel — pull them with `AsyncRadio::next_event`.

> **Hardware reminder.** The RYLR998 is 3.3 V only (not 5 V tolerant) and
> defaults to 115 200 baud 8N1. See the [workspace README](https://github.com/terakilobyte/rylr998-rs)
> for full wiring and concurrency notes.

## Example

```rust,no_run
# use std::path::Path;
# async fn run() -> Result<(), rylr998_tokio::Error> {
let mut radio = rylr998_tokio::AsyncRadio::open(Path::new("/dev/cu.usbserial-X")).await?;
radio.set_address(5).await?;
radio.set_network_id(18).await?;
radio.send(2, b"hello").await?;

let event = radio.next_event(std::time::Duration::from_secs(60)).await?;
println!("{:?}", event);
# Ok(())
# }
```

## Method signatures

All command methods take `&mut self` and have a per-call deadline (1 s,
or 2 s for `factory_reset`); a missing response yields `Error::Timeout`
and a `+ERR=<code>` reply yields `Error::Radio(code)`.

```rust,ignore
impl AsyncRadio {
    pub async fn open(path: &Path) -> Result<Self>;        // 115_200 8N1
    pub fn from_port<S>(port: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static;

    pub async fn ping(&mut self) -> Result<()>;
    pub async fn factory_reset(&mut self) -> Result<()>;   // 2 s deadline

    pub async fn set_address(&mut self, n: u16) -> Result<()>;
    pub async fn address(&mut self) -> Result<u16>;
    pub async fn set_network_id(&mut self, n: u8) -> Result<()>;
    pub async fn network_id(&mut self) -> Result<u8>;
    pub async fn set_band(&mut self, hz: u32) -> Result<()>;
    pub async fn band(&mut self) -> Result<u32>;
    pub async fn set_cpin(&mut self, password: &[u8]) -> Result<()>;
    pub async fn cpin(&mut self) -> Result<String>;
    pub async fn set_parameters(&mut self, p: RfParams) -> Result<()>;
    pub async fn parameters(&mut self) -> Result<RfParams>;
    pub async fn crfop(&mut self) -> Result<u8>;
    pub async fn uid(&mut self) -> Result<String>;
    pub async fn version(&mut self) -> Result<String>;

    pub async fn send(&mut self, to: u16, data: &[u8]) -> Result<()>;
    pub async fn next_event(&mut self, timeout: Duration) -> Result<OwnedEvent>;
}
```

## Concurrency

A `tokio::spawn`ed background task owns the protocol driver and the
serial port. `AsyncRadio` holds the consumer side of an event channel
and the producer side of a command channel; both are single-consumer.
Methods take `&mut self` for that reason — clone-and-share is **not**
the API. Don't construct two `AsyncRadio`s for the same physical port;
construct one and route all work through it.

Events arriving while a command is in flight (or during idle polling in
the background task) are forwarded onto the event channel (capacity 64)
and surfaced FIFO from `next_event`. Drain regularly so the channel
doesn't back up; if it fills, the background task will start dropping
events on send failure.

`from_port` accepts any `AsyncRead + AsyncWrite + Send + Unpin + 'static`,
which makes the crate easy to unit-test against `tokio::io::duplex`.

## Errors

| Variant | Cause |
|---|---|
| `Error::Core(_)`     | Protocol error from `rylr998-core` |
| `Error::Io(_)`       | `std::io::Error` from the serial port |
| `Error::Serial(_)`   | `tokio_serial::Error` opening the port |
| `Error::Timeout`     | Per-call deadline elapsed without a response |
| `Error::Radio(code)` | Radio replied `+ERR=<code>`; see the workspace README's +ERR table or call `Error::radio_error()` to get a `RadioError` |
| `Error::Closed`      | The background reader/writer task has exited (port closed or channel dropped) |

## CPIN

`set_cpin` accepts exactly 8 ASCII hex bytes in the documented `00000001`
through `FFFFFFFF` range. Invalid CPIN length is reported by the radio as
`Error::Radio(5)`, which maps to `RadioError::DataLengthMismatch`.
`cpin` returns that 8-character password, or an empty string when the
module reports `No Password!`. The manual's `,M` memory flag is not exposed
here because tested RYLR998 hardware rejects it with `+ERR=5`.

## Related crates

- [`rylr998-core`](https://crates.io/crates/rylr998-core) — pure protocol logic.
- [`rylr998-std`](https://crates.io/crates/rylr998-std) — blocking equivalent of this crate.
- [`rylr998-embassy`](https://crates.io/crates/rylr998-embassy) — `no_std` embedded variant.

## License

Dual-licensed: MIT or Apache-2.0, at your option.
