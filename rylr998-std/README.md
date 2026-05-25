# rylr998-std

Blocking host driver for the REYAX **RYLR998** LoRa radio module, built on
[`serialport`](https://crates.io/crates/serialport).

Auto-detects `/dev/cu.usbserial*` on macOS, opens at 115 200 8N1 with DTR
asserted, drives the [`rylr998-core`](https://crates.io/crates/rylr998-core)
state machine, and gives you a simple synchronous API.

> **Hardware reminder.** The RYLR998 is 3.3 V only (not 5 V tolerant) and
> defaults to 115 200 baud 8N1. See the [workspace README](https://github.com/terakilobyte/rylr998-rs)
> for full wiring and concurrency notes.

## Example

```rust,no_run
let mut radio = rylr998_std::Radio::open_auto()?;
radio.set_address(5)?;
radio.set_network_id(18)?;
radio.send(2, b"hello")?;
let event = radio.next_event(std::time::Duration::from_secs(60))?;
println!("{:?}", event);
# Ok::<(), rylr998_std::Error>(())
```

## Method signatures

All command methods take `&mut self` and have a per-call deadline (1 s,
or 4 s for `factory_reset`); a missing response yields `Error::Timeout`
and a `+ERR=<code>` reply yields `Error::Radio(code)`.

```rust,ignore
impl Radio<Box<dyn serialport::SerialPort>> {
    pub fn open(path: &Path) -> Result<Self>;          // 115_200 8N1, DTR on
    pub fn open_auto() -> Result<Self>;                // discover + open
    pub fn discover() -> Result<PathBuf>;              // cu.usbserial* on macOS
}

impl<P: Read + Write> Radio<P> {
    pub fn from_port(port: P) -> Self;

    // liveness
    pub fn ping(&mut self) -> Result<()>;
    pub fn factory_reset(&mut self) -> Result<()>;     // 4 s deadline (waits for +READY)

    // settings (1 s deadline each)
    pub fn set_address(&mut self, n: u16) -> Result<()>;
    pub fn address(&mut self) -> Result<u16>;
    pub fn set_network_id(&mut self, n: u8) -> Result<()>;
    pub fn network_id(&mut self) -> Result<u8>;
    pub fn set_band(&mut self, hz: u32) -> Result<()>;
    pub fn band(&mut self) -> Result<u32>;
    pub fn set_cpin(&mut self, password: &[u8]) -> Result<()>;
    pub fn cpin(&mut self) -> Result<String>;
    pub fn set_parameters(&mut self, p: RfParams) -> Result<()>;
    pub fn parameters(&mut self) -> Result<RfParams>;
    pub fn crfop(&mut self) -> Result<u8>;
    pub fn uid(&mut self) -> Result<String>;
    pub fn version(&mut self) -> Result<String>;

    // data path
    pub fn send(&mut self, to: u16, data: &[u8]) -> Result<()>;
    pub fn next_event(&mut self, timeout: Duration) -> Result<OwnedEvent>;
}
```

## Concurrency

`Radio` is single-owner — every method takes `&mut self`. Unsolicited
`+RCV` events received while a command is in flight are buffered in an
internal `VecDeque<OwnedEvent>` and surfaced FIFO from the next
`next_event` call, so nothing is dropped. Use one `Radio` per physical
radio and serialize work through it.

`next_event(timeout)` drains the buffer first, then reads more from the
port up to the deadline; `Error::Timeout` means no event arrived in the
window.

## Errors

| Variant | Cause |
|---|---|
| `Error::Core(_)`      | Protocol error from `rylr998-core` (`Busy`, `TxOverflow`, `RxOverflow`, `Parse`) |
| `Error::Io(_)`        | `std::io::Error` from the serial port |
| `Error::Serial(_)`    | `serialport::Error` opening or enumerating ports |
| `Error::Timeout`      | Per-call deadline elapsed without a response |
| `Error::Radio(code)`  | Radio replied `+ERR=<code>`; see the workspace README's +ERR table or call `Error::radio_error()` to get a `RadioError` |
| `Error::NoDevice`     | `discover()` found no `cu.usbserial*` ports |
| `Error::Ambiguous(_)` | `discover()` found more than one candidate; open the right port explicitly via `Radio::open` |

## CPIN

`set_cpin` accepts exactly 8 ASCII hex bytes in the documented `00000001`
through `FFFFFFFF` range. Invalid CPIN length is reported by the radio as
`Error::Radio(5)`, which maps to `RadioError::DataLengthMismatch`.
`cpin` returns that 8-character password, or an empty string when the
module reports `No Password!`. The manual's `,M` memory flag is not exposed
here because tested RYLR998 hardware rejects it with `+ERR=5`.

## Related crates

- [`rylr998-core`](https://crates.io/crates/rylr998-core) — pure protocol logic.
- [`rylr998-tokio`](https://crates.io/crates/rylr998-tokio) — async equivalent of this crate.
- [`rylr998-embassy`](https://crates.io/crates/rylr998-embassy) — `no_std` embedded variant.
- [`rylr998`](https://crates.io/crates/rylr998) — CLI that uses this crate.

## License

Dual-licensed: MIT or Apache-2.0, at your option.
