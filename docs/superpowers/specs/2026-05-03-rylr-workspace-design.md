# rylr workspace — design

**Date:** 2026-05-03
**Status:** approved
**Relationship to prior spec:** Supersedes the implementation portion of
`serialuart/docs/superpowers/specs/2026-05-01-rylr-tool-design.md`. The
prior spec's user-visible CLI behavior (subcommands, defaults, exit
codes, `+RCV` framing rules) carries over verbatim; only the code's home
changes.

## Purpose

A small Rust workspace for working with REYAX RYLR998 LoRa modules over
USB serial (today) and over a Raspberry Pi Pico's UART (later). The
workspace splits the protocol's logic from its I/O so the same parser
and state machine drive a desktop CLI, an async hub service, and an
embedded firmware binary.

This is also a learning project. Several of the most interesting parts
are intentionally left as guided exercises (see "Exercises" below);
the rest is implemented end-to-end so the CLI is usable on real
hardware as soon as the exercises are done.

## Workspace layout

```
embedded/rylr/
├── Cargo.toml              # [workspace] members, shared deps
├── README.md               # quickstart + manual smoke-test steps
├── rylr-core/              # no_std, no I/O. Driver state machine.
├── rylr-std/               # blocking. Radio<...> over `serialport`.
├── rylr-tokio/             # SCAFFOLD (exercise). Async, for the hub service.
├── rylr-embassy/           # SCAFFOLD (exercise). no_std + RP2040 UART.
└── rylr-tool/              # CLI binary. Depends on rylr-std.
```

`embedded/serialuart/` is not migrated; it stays as a proof-of-concept.

## Dependency graph

| Crate          | Depends on                                   | std? | alloc?                  |
|----------------|----------------------------------------------|------|-------------------------|
| `rylr-core`    | `heapless`                                   | no   | optional (`alloc` flag) |
| `rylr-std`     | `rylr-core` (with `alloc`), `serialport`, `thiserror` | yes  | yes            |
| `rylr-tokio`   | `rylr-core` (with `alloc`), `tokio`, `tokio-serial`, `thiserror` | yes  | yes |
| `rylr-embassy` | `rylr-core` (default features), `embassy-rp`, `embassy-time`, `embedded-io-async`, `defmt` | no | no |
| `rylr-tool`    | `rylr-std`, `clap`, `ctrlc`                  | yes  | yes                     |

Single shared workspace version `0.1.0`. Workspace-level
`[workspace.dependencies]` pins `rylr-core` once. Not published to
crates.io. No `LICENSE` file yet.

`rylr-embassy`'s versions track `embedded/pico-blink/Cargo.toml`:
`embassy-rp = "0.10"` (rp2040 feature), `embassy-executor = "0.9"`,
`embassy-time = "0.5"`, `defmt = "1.0"`.

## `rylr-core` — sans-I/O state machine

`rylr-core` is a sans-I/O state machine: bytes go in, bytes come out,
no traits over I/O, no clocks. The whole protocol — command encoding,
response parsing, event parsing, request/response correlation — lives
here. Every transport crate translates between its native I/O model
and `Driver::push_rx` / `Driver::poll`.

### Public types

```rust
pub struct RfParams { pub sf: u8, pub bw: u8, pub cr: u8, pub preamble: u8 }

pub enum Command<'a> {
    Ping,                                   // AT
    GetAddress, SetAddress(u16),
    GetNetworkId, SetNetworkId(u8),
    GetBand, SetBand(u32),
    GetParameters, SetParameters(RfParams),
    GetCrfop,
    GetUid,
    GetVersion,
    FactoryReset,                           // AT+FACTORY
    Send { to: u16, data: &'a [u8] },       // AT+SEND=to,len,data
}

pub enum Response<'a> {
    Ok,                                     // +OK
    Err(u8),                                // +ERR=N
    Address(u16),
    NetworkId(u8),
    Band(u32),
    Parameters(RfParams),
    Crfop(u8),
    Uid(&'a str),
    Version(&'a str),
}

pub enum Event<'a> {
    Recv { from: u16, data: &'a [u8], rssi: i16, snr: i16 },  // +RCV=...
    Ready,                                                     // +READY
}

pub enum Poll<'a> {
    Idle,                              // nothing to do; await more RX bytes
    NeedTx(&'a [u8]),                  // please write these bytes to UART
    Response(Response<'a>),            // command completed
    Event(Event<'a>),                  // unsolicited message
}

pub enum Error {
    Busy,                              // submit() while another command in flight
    TxOverflow,                        // command encoding > TX buffer
    RxOverflow,                        // RX buffer full
    Parse,                             // bad/unrecognized line from radio
}
```

Borrow-based by design: `Response::Uid`, `Response::Version`,
`Event::Recv::data`, and `Poll::NeedTx` all borrow from the `Driver`'s
internal buffers. Callers process the borrow inline or copy out via
`OwnedEvent`.

### Owned events (for `alloc` callers)

Behind the `alloc` Cargo feature, `rylr-core` exposes:

```rust
#[cfg(feature = "alloc")]
pub enum OwnedEvent {
    Recv { from: u16, data: Vec<u8>, rssi: i16, snr: i16 },
    Ready,
}

#[cfg(feature = "alloc")]
impl<'a> Event<'a> {
    pub fn into_owned(self) -> OwnedEvent;
}
```

`rylr-std` and `rylr-tokio` enable `alloc`; `rylr-embassy` does not.

### `Driver`

```rust
pub struct Driver { /* fixed-size buffers + small state enum */ }

impl Driver {
    pub const fn new() -> Self;

    /// Submit a command. Encodes it into the TX buffer; the `data`
    /// borrow in `Command::Send` is released at return. Returns
    /// `Err(Busy)` if a command is already in flight.
    pub fn submit(&mut self, cmd: Command<'_>) -> Result<(), Error>;

    /// Append RX bytes from UART. Returns the number accepted; if the
    /// internal buffer is full, returns `Err(RxOverflow)`.
    pub fn push_rx(&mut self, bytes: &[u8]) -> Result<usize, Error>;

    /// Confirm that `n` bytes from the most recent `Poll::NeedTx`
    /// were written to UART. Drains them from the TX buffer.
    pub fn ack_tx(&mut self, n: usize);

    /// Drive the state machine. Priority order:
    ///   pending TX bytes -> complete +RCV event -> matched Response -> Idle.
    pub fn poll(&mut self) -> Poll<'_>;
}
```

**Buffer sizes (hardcoded for v0.1):**
- `RX_BUF = 512` bytes (RYLR line ≤ ~280 bytes; covers a line + a stray `+READY`).
- `TX_BUF = 512` bytes (encoded `AT+SEND=...,240,<240 bytes>` is ~256 bytes).

If anyone needs other sizes later, lift to const generics with these
as defaults — non-breaking.

**State semantics:**
- One in-flight command at a time. `submit` while busy → `Error::Busy`.
- `submit` encodes immediately; the `Command::Send` borrow is released
  at return.
- `+READY` arriving while a `FactoryReset` is in flight resolves it as
  `Response::Ok`. `+READY` outside that case surfaces as
  `Event::Ready`; transport crates may ignore it.
- No clocks, no timeouts in core. Transport crates own all time-based
  logic.

### Wire format details (carried from prior spec)

- Command terminator: `\r\n` on both directions.
- Setters expect `+OK` or `+ERR=<n>`.
- Queries get `+<KEY>=<value>`; key matches the command (e.g.
  `AT+ADDRESS?` → `+ADDRESS=5`).
- `+RCV=<addr>,<len>,<data>,<rssi>,<snr>`: `<len>` is the byte length
  of `<data>`. The parser uses length-prefixed extraction (split on
  the first three commas, take next `<len>` bytes for data, then split
  the remainder for RSSI/SNR). It does NOT comma-split the whole line.
- `Display` impls on `Response`, `Event`, `Error` (no reliance on
  `core::fmt::Debug` for the CLI). `defmt::Format` impls behind a
  `defmt` feature.

### File layout

```
rylr-core/src/
  lib.rs        # re-exports + crate-level docs
  types.rs      # Command, Response, Event, RfParams, Error, OwnedEvent
  encode.rs     # Command -> bytes (pure fns)               [implemented]
  decode.rs     # line -> Response | Event (pure fns)       [EXERCISE]
  driver.rs     # Driver: state machine, buffers, poll()    [partially EXERCISE]
```

Files stay ≤ ~150 lines each. If `decode.rs` outgrows that, split into
`decode/event.rs` + `decode/response.rs`.

## `rylr-std` — blocking transport

```rust
/// Generic over a port type that's `Read + Write`. The default and the
/// only constructor in non-test builds is `Radio<Box<dyn SerialPort>>`,
/// reachable via `Radio::open*`. Tests construct `Radio<LoopbackPort>`
/// directly without ever implementing `serialport::SerialPort`.
pub struct Radio<P: Read + Write = Box<dyn serialport::SerialPort>> {
    /* Driver + port: P + event queue */
}

impl Radio<Box<dyn serialport::SerialPort>> {
    pub fn discover() -> Result<PathBuf, Error>;       // /dev/cu.usbserial* filter
    pub fn open(path: &Path) -> Result<Self, Error>;
    pub fn open_auto() -> Result<Self, Error>;         // discover() + open()
}

impl<P: Read + Write> Radio<P> {

    pub fn ping(&mut self) -> Result<()>;
    pub fn address(&mut self) -> Result<u16>;
    pub fn set_address(&mut self, n: u16) -> Result<()>;
    pub fn network_id(&mut self) -> Result<u8>;
    pub fn set_network_id(&mut self, n: u8) -> Result<()>;
    pub fn band(&mut self) -> Result<u32>;
    pub fn set_band(&mut self, hz: u32) -> Result<()>;
    pub fn parameters(&mut self) -> Result<RfParams>;
    pub fn set_parameters(&mut self, p: RfParams) -> Result<()>;
    pub fn crfop(&mut self) -> Result<u8>;
    pub fn uid(&mut self) -> Result<String>;
    pub fn version(&mut self) -> Result<String>;
    pub fn factory_reset(&mut self) -> Result<()>;
    pub fn send(&mut self, to: u16, data: &[u8]) -> Result<()>;
    pub fn next_event(&mut self, timeout: Duration) -> Result<OwnedEvent>;
}
```

**Run loop.** A single private helper drives both command and event
paths:

```rust
fn pump_until<R>(
    &mut self,
    deadline: Instant,
    want: impl FnMut(Response<'_>) -> Option<Result<R>>,
) -> Result<R>
```

Body (sequence per iteration):
1. While `driver.poll() == Poll::NeedTx(bytes)`: write to port, then
   `driver.ack_tx(n)`.
2. While `driver.poll()` returns `Response`/`Event`: events go onto
   `self.events`; a `Response` is handed to `want`; if `want` returns
   `Some`, return.
3. Read from the port with a 50 ms read timeout into a stack buffer;
   feed accepted bytes to `driver.push_rx`.
4. If `Instant::now() >= deadline`, return `Err(Timeout)`.

Default per-command deadline: 1 s. `factory_reset` extends to 2 s.
Both `factory_reset` and every other command just wait for
`Response::Ok` — `rylr-core` is responsible for translating the
`+READY` that follows `AT+FACTORY` into `Response::Ok`, so transport
code has no `+READY` special case.

`+READY` arriving while no command is in flight (e.g. user power-cycled
the radio) becomes `OwnedEvent::Ready` queued in `self.events`.
`next_event(timeout)` drains the queue first; if empty, pumps with
`want = |_| None` until an event arrives or the deadline expires.

**Port discovery** is the same pure filter from the prior spec:

| Match count | Behavior                                                    |
|-------------|-------------------------------------------------------------|
| 0           | `Err(NoDevice)`                                             |
| 1           | `Ok(path)` (path also printed to stderr by the CLI)         |
| 2+          | `Err(Ambiguous(Vec<String>))`                               |

The filter is a pure function over `Vec<String>` — unit-tested without
touching the OS.

### File layout

```
rylr-std/src/
  lib.rs        # re-exports
  port.rs       # discover() filter + Error::NoDevice/Ambiguous       [implemented]
  radio.rs      # Radio struct + per-AT-command methods + pump_until  [pump_until is EXERCISE]
```

## `rylr-tool` — CLI binary

CLI surface and per-subcommand behavior carry forward from
`2026-05-01-rylr-tool-design.md` unchanged:

```
rylr-tool info
rylr-tool provision --address <N> --net <N> [--band <Hz>] [--params S,B,C,P]
rylr-tool reset
rylr-tool send --to <N> <message>
rylr-tool listen
rylr-tool --port <path> ...        # global, hidden override
```

What changes:

- `transport.rs` and `radio.rs` from the prior spec move out of the
  binary entirely. The binary uses `rylr_std::Radio`.
- `port::discover()` becomes `rylr_std::Radio::discover()`.
- `mockall` goes away; correctness lives in `rylr-core`'s pure-function
  tests, and the CLI tests just exercise argument parsing.

```
rylr-tool/src/
  main.rs       # clap CLI + dispatch
  commands.rs   # info / provision / reset / send / listen
```

## `rylr-tokio` and `rylr-embassy` — scaffolds (exercises)

Both are real workspace members that compile, but the public type is a
placeholder backed by detailed `// TODO:` comments. The scaffold's
purpose is to lock the dependency graph and reserve the API contour.

**`rylr-tokio/src/lib.rs`** intended surface (the user fills in):

```rust
pub struct AsyncRadio { /* mpsc command tx, mpsc event rx, JoinHandle */ }

impl AsyncRadio {
    pub async fn open(path: &Path) -> Result<Self>;
    pub async fn ping(&mut self) -> Result<()>;
    pub async fn address(&mut self) -> Result<u16>;
    pub async fn set_address(&mut self, n: u16) -> Result<()>;
    /* ... mirror of rylr_std::Radio with `async fn` ... */
    pub async fn next_event(&mut self) -> Result<OwnedEvent>;
}
```

Implementation pattern (TODO comments in the file): a background
`tokio::task` owns the `Driver` + `tokio_serial::SerialStream`, reads
commands from an mpsc channel, broadcasts events on another channel.
Public `AsyncRadio` is a handle holding the channel ends.

**`rylr-embassy/src/lib.rs`** intended surface (the user fills in):

```rust
pub struct Radio<'d, UART> { /* Driver + UART halves */ }

impl<'d, UART: embedded_io_async::Read + embedded_io_async::Write> Radio<'d, UART> {
    pub async fn ping(&mut self) -> Result<()>;
    pub async fn set_address(&mut self, n: u16) -> Result<()>;
    /* ... mirror of rylr_std::Radio ... */
    pub async fn next_event(&mut self) -> Result<Event<'_>>;  // borrow-based, no alloc
}
```

Implementation pattern (TODO comments): wrap `embassy_rp::uart::Uart`
behind `embedded_io_async`, drive `Driver::poll` in an async loop,
yield via `embassy_time::with_timeout` for deadlines. No alloc; events
are borrow-based.

A `rylr-embassy/examples/pico_smoke.rs` binary is also scaffolded:
configure UART0 on GP0/GP1, ping the radio, set address 5, listen for
incoming messages and `defmt::info!` them.

## Errors

Error types are layered, not unified. `rylr-core` has a small no_std
enum with manual `Display`. Each transport crate has its own top-level
`Error` that wraps core's plus its native I/O errors:

```rust
// rylr-std (thiserror)
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("core: {0}")]   Core(#[from] rylr_core::Error),
    #[error("io: {0}")]     Io(#[from] std::io::Error),
    #[error("serial: {0}")] Serial(#[from] serialport::Error),
    #[error("timeout")]     Timeout,
    #[error("radio error code {0}")]                         Radio(u8),
    #[error("no cu.usbserial* device found")]                NoDevice,
    #[error("multiple candidate devices: {0:?} (use --port)")] Ambiguous(Vec<String>),
}
```

`rylr-tokio` and `rylr-embassy` define analogous enums when they're
implemented.

## Testing

| Crate           | Approach                                                                                                                           |
|-----------------|------------------------------------------------------------------------------------------------------------------------------------|
| `rylr-core`     | All tests are pure-function. Encode tables for every `Command`. Decode tables for every `Response` and `Event`, including `+RCV` payloads with embedded commas and varying RSSI/SNR signs. Driver tests script byte sequences and assert `Poll` priority order, Busy/Overflow paths, factory-reset's `+READY` interpretation, and Recv arriving mid-command. No mocks, no I/O. |
| `rylr-std`      | Integration tests use a `loopback::Pair` helper: a fake "port" backed by two `VecDeque<u8>`s, writing one feeds the other. Each `Radio` method gets a test that scripts the radio side and asserts the call's result. Port-discovery filter tested with synthetic `Vec<String>`. |
| `rylr-tool`     | `assert_cmd` smoke tests: `--help`, exit codes, that `--port /dev/null` is rejected sensibly. No business-logic tests here.        |
| `rylr-tokio`    | Test scaffolding (channel plumbing test, basic round-trip with a mock task) included in the file but `#[ignore]`d until the user's impl is in place. |
| `rylr-embassy`  | Protocol correctness is covered transitively by `rylr-core`'s tests. The crate itself has no automated tests; UART glue is exercised manually via `examples/pico_smoke.rs` on real hardware. |

End-to-end tests against real hardware are not automated. A
`README.md` in the workspace root documents the manual smoke-test
sequence (provision, info, send/listen between two radios), same as
the prior spec said.

## Exercises

Parts intentionally left for the user to implement. Each ships with
stubbed signatures, doc comments, `// TODO:` markers identifying each
sub-step, and tests that fail until the impl is correct.

| File                                  | What's there                                              | What you write                                                  |
|---------------------------------------|-----------------------------------------------------------|------------------------------------------------------------------|
| `rylr-core/src/decode.rs`             | Function signatures, fixture tests covering every form    | The body: `parse_response`, `parse_event`, `+RCV` length-prefix logic |
| `rylr-core/src/driver.rs` `poll()`    | Driver struct, `submit`/`push_rx`/`ack_tx`, state enum, fixture tests scripting full sequences | The `poll()` body — the priority logic and state transitions   |
| `rylr-tokio/src/lib.rs`               | Cargo deps, `AsyncRadio` type signature, doc comments laying out the actor pattern, `#[ignore]`d round-trip test | Background task, channel wiring, every `async fn` body         |
| `rylr-embassy/src/lib.rs` + `examples/pico_smoke.rs` | Cargo deps, `Radio<UART>` signature, doc comments laying out the loop pattern, example skeleton | UART glue, async pump loop, example binary's main              |

Implemented end-to-end (no exercises):

- All of `rylr-core/types.rs`, `encode.rs`, plus the data structures
  in `driver.rs`.
- All of `rylr-std` except `pump_until` (which is the exercise).
- All of `rylr-tool`.

## Out of scope

- Firmware update of the RYLR module (not supported by the device).
- Multi-radio batch flashing or hub orchestration; one radio per
  `Radio` instance.
- Configuration profiles in TOML/YAML files (deferred from prior spec).
- AT commands beyond the listed set (`AT+CRFOP=` setter, `AT+CPIN=`,
  `AT+RESET`, etc.). Easy to extend on the same pattern.
- Persistent logging or telemetry.
- Generic support for RYLR896 / RYLR993 (998-only this cut).
- Publishing to crates.io.

## Open questions

None at design time. Anything that comes up during implementation
prompts a small follow-up doc, not an edit to this one.
