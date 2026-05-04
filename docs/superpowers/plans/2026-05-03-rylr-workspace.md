# rylr workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a 5-crate Rust workspace (`rylr-core`, `rylr-std`, `rylr-tokio`, `rylr-embassy`, `rylr-tool`) for working with REYAX RYLR998 LoRa modules. End state: `rylr-tool` works against real hardware once the user finishes the marked exercises; `rylr-tokio` and `rylr-embassy` exist as guided exercises.

**Architecture:** Sans-I/O state machine (`Driver`) in `rylr-core`. Each transport crate plumbs bytes between its native I/O model and the Driver. Borrow-based events by default; an `alloc` feature exposes `OwnedEvent` for std/tokio callers.

**Tech Stack:** Rust 2024 edition. `heapless` for fixed-size buffers. `serialport` 4.x for blocking USB serial. `tokio` + `tokio-serial` for async. `embassy-rp` 0.10 + `embedded-io-async` for RP2040. `clap` 4.x derive for the CLI. `thiserror` 2 for error types in transport crates.

**Spec:** `docs/superpowers/specs/2026-05-03-rylr-workspace-design.md`

---

## Glossary of recurring commands

```bash
cargo check --workspace            # compile every crate
cargo test -p <crate>              # run tests for one crate
cargo test --workspace --exclude rylr-embassy  # run all desktop tests
cargo build -p rylr-tool           # build the CLI
```

`rylr-embassy` is `no_std` for `thumbv6m-none-eabi`; it does not run on the host. Use `cargo check -p rylr-embassy --target thumbv6m-none-eabi` to verify it compiles. (Install once: `rustup target add thumbv6m-none-eabi`.)

---

## Task 1: Workspace skeleton

**Files:**
- Create: `embedded/rylr/Cargo.toml`
- Create: `embedded/rylr/.gitignore`
- Create: `embedded/rylr/rylr-core/Cargo.toml`
- Create: `embedded/rylr/rylr-core/src/lib.rs`
- Create: `embedded/rylr/rylr-std/Cargo.toml`
- Create: `embedded/rylr/rylr-std/src/lib.rs`
- Create: `embedded/rylr/rylr-tokio/Cargo.toml`
- Create: `embedded/rylr/rylr-tokio/src/lib.rs`
- Create: `embedded/rylr/rylr-embassy/Cargo.toml`
- Create: `embedded/rylr/rylr-embassy/src/lib.rs`
- Create: `embedded/rylr/rylr-tool/Cargo.toml`
- Create: `embedded/rylr/rylr-tool/src/main.rs`

- [ ] **Step 1: Initialize git in `embedded/rylr/` and write `.gitignore`**

The parent directory `embedded/` is not a git repository today. We initialize *inside* `rylr/` so the new workspace has its own history.

```bash
cd /Users/nathanleniz/developer/embedded/rylr && git init
```

`.gitignore`:

```gitignore
/target
Cargo.lock.bak
**/*.rs.bk
.DS_Store
```

- [ ] **Step 2: Write the workspace root `Cargo.toml`**

```toml
[workspace]
resolver = "3"
members = [
    "rylr-core",
    "rylr-std",
    "rylr-tokio",
    "rylr-embassy",
    "rylr-tool",
]

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
rylr-core = { path = "rylr-core", version = "0.1.0" }
rylr-std  = { path = "rylr-std",  version = "0.1.0" }

heapless    = "0.8"
serialport  = "4.9"
clap        = { version = "4", features = ["derive"] }
ctrlc       = "3"
thiserror   = "2"

# Async / embedded transitive pins (tokio/embassy crates use these)
tokio        = { version = "1", default-features = false }
tokio-serial = "5"

embedded-io-async = "0.7"
embassy-executor  = { version = "0.9", features = ["arch-cortex-m", "executor-thread"] }
embassy-rp        = { version = "0.10", features = ["rp2040", "time-driver", "critical-section-impl", "defmt", "unstable-pac"] }
embassy-time      = { version = "0.5", features = ["defmt", "defmt-timestamp-uptime"] }
defmt             = "1"
defmt-rtt         = "1"
panic-probe       = { version = "1", features = ["print-defmt"] }
cortex-m          = { version = "0.7", features = ["inline-asm"] }
cortex-m-rt       = "0.7"

[profile.release]
debug = 2
lto = "fat"
codegen-units = 1
opt-level = "s"

[profile.dev]
debug = 2
opt-level = "s"
```

- [ ] **Step 3: Write each crate's empty `Cargo.toml` and `lib.rs`/`main.rs`**

`rylr-core/Cargo.toml`:

```toml
[package]
name = "rylr-core"
version.workspace = true
edition.workspace = true

[features]
default = []
alloc = []
defmt = ["dep:defmt"]

[dependencies]
heapless = { workspace = true }
defmt    = { workspace = true, optional = true }
```

`rylr-core/src/lib.rs`:

```rust
#![cfg_attr(not(feature = "alloc"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;
```

`rylr-std/Cargo.toml`:

```toml
[package]
name = "rylr-std"
version.workspace = true
edition.workspace = true

[dependencies]
rylr-core  = { workspace = true, features = ["alloc"] }
serialport = { workspace = true }
thiserror  = { workspace = true }
```

`rylr-std/src/lib.rs`:

```rust
//! Blocking, single-radio transport for `rylr-core` over `serialport`.
```

`rylr-tokio/Cargo.toml`:

```toml
[package]
name = "rylr-tokio"
version.workspace = true
edition.workspace = true

[dependencies]
rylr-core    = { workspace = true, features = ["alloc"] }
tokio        = { workspace = true, features = ["rt", "io-util", "sync", "macros", "time"] }
tokio-serial = { workspace = true }
thiserror    = { workspace = true }
```

`rylr-tokio/src/lib.rs`:

```rust
//! Async (Tokio) transport. Scaffold; user implements per Task 17.
```

`rylr-embassy/Cargo.toml`:

```toml
[package]
name = "rylr-embassy"
version.workspace = true
edition.workspace = true

[dependencies]
rylr-core         = { workspace = true }
embedded-io-async = { workspace = true }
embassy-rp        = { workspace = true }
embassy-time      = { workspace = true }
embassy-executor  = { workspace = true }
defmt             = { workspace = true }
defmt-rtt         = { workspace = true }
panic-probe       = { workspace = true }
cortex-m          = { workspace = true }
cortex-m-rt       = { workspace = true }
```

`rylr-embassy/src/lib.rs`:

```rust
#![no_std]
//! Embassy + RP2040 UART transport. Scaffold; user implements per Task 18.
```

`rylr-tool/Cargo.toml`:

```toml
[package]
name = "rylr-tool"
version.workspace = true
edition.workspace = true

[dependencies]
rylr-std = { workspace = true }
clap     = { workspace = true }
ctrlc    = { workspace = true }
```

`rylr-tool/src/main.rs`:

```rust
fn main() {
    println!("rylr-tool placeholder");
}
```

- [ ] **Step 4: Verify the workspace compiles end-to-end (excluding embassy)**

```bash
cd /Users/nathanleniz/developer/embedded/rylr && cargo check --workspace --exclude rylr-embassy
```

Expected: `Finished` for `rylr-core`, `rylr-std`, `rylr-tokio`, `rylr-tool`. Warnings about unused crate imports are fine.

- [ ] **Step 5: Verify the embassy crate cross-compiles**

```bash
rustup target add thumbv6m-none-eabi   # one-time
cd /Users/nathanleniz/developer/embedded/rylr && cargo check -p rylr-embassy --target thumbv6m-none-eabi
```

Expected: `Finished`. (If this fails with linker errors, that's expected for an empty `lib.rs` — we'll add a `memory.x` in Task 18. For Task 1 the only thing that matters is that source-level compilation succeeds.)

- [ ] **Step 6: Commit**

```bash
cd /Users/nathanleniz/developer/embedded/rylr && git add -A && git commit -m "scaffold: 5-crate workspace skeleton"
```

---

## Task 2: `rylr-core` types

**Files:**
- Create: `rylr-core/src/types.rs`
- Modify: `rylr-core/src/lib.rs`

- [ ] **Step 1: Write `types.rs` with the public enums**

```rust
//! Public types for the RYLR protocol. No I/O, no clocks.

use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RfParams {
    pub sf: u8,
    pub bw: u8,
    pub cr: u8,
    pub preamble: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> {
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
    Send { to: u16, data: &'a [u8] },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response<'a> {
    Ok,
    Err(u8),
    Address(u16),
    NetworkId(u8),
    Band(u32),
    Parameters(RfParams),
    Crfop(u8),
    Uid(&'a str),
    Version(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event<'a> {
    Recv { from: u16, data: &'a [u8], rssi: i16, snr: i16 },
    Ready,
}

#[derive(Debug)]
pub enum Poll<'a> {
    Idle,
    NeedTx(&'a [u8]),
    Response(Response<'a>),
    Event(Event<'a>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Busy,
    TxOverflow,
    RxOverflow,
    Parse,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("a command is already in flight"),
            Self::TxOverflow => f.write_str("encoded command exceeds TX buffer"),
            Self::RxOverflow => f.write_str("RX buffer is full"),
            Self::Parse => f.write_str("could not parse line from radio"),
        }
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Error {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            Self::Busy => defmt::write!(fmt, "Busy"),
            Self::TxOverflow => defmt::write!(fmt, "TxOverflow"),
            Self::RxOverflow => defmt::write!(fmt, "RxOverflow"),
            Self::Parse => defmt::write!(fmt, "Parse"),
        }
    }
}
```

- [ ] **Step 2: Re-export the types from `lib.rs`**

Replace `rylr-core/src/lib.rs` body with:

```rust
#![cfg_attr(not(feature = "alloc"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod types;
pub use types::{Command, Error, Event, Poll, Response, RfParams};
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p rylr-core --no-default-features
cargo check -p rylr-core --features alloc
cargo check -p rylr-core --features defmt
```

Expected: all three succeed.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "core: public types (Command/Response/Event/Poll/Error)"
```

---

## Task 3: `OwnedEvent` (alloc feature)

**Files:**
- Modify: `rylr-core/src/types.rs`
- Test: `rylr-core/src/types.rs` (inline `#[cfg(test)]` mod)

- [ ] **Step 1: Write the failing test**

Append to `rylr-core/src/types.rs`:

```rust
#[cfg(all(test, feature = "alloc"))]
mod owned_tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn recv_to_owned_copies_data() {
        let bytes = [0xDE, 0xAD, 0xBE, 0xEF];
        let ev = Event::Recv { from: 5, data: &bytes, rssi: -42, snr: 8 };
        let owned = ev.to_owned();
        match owned {
            OwnedEvent::Recv { from, data, rssi, snr } => {
                assert_eq!(from, 5);
                assert_eq!(data, vec![0xDE, 0xAD, 0xBE, 0xEF]);
                assert_eq!(rssi, -42);
                assert_eq!(snr, 8);
            }
            _ => panic!("expected Recv"),
        }
    }

    #[test]
    fn ready_to_owned() {
        assert!(matches!(Event::Ready.to_owned(), OwnedEvent::Ready));
    }
}
```

- [ ] **Step 2: Run the test (it should fail to compile)**

```bash
cargo test -p rylr-core --features alloc -- owned_tests
```

Expected: compile error — `OwnedEvent` undefined, `to_owned` undefined.

- [ ] **Step 3: Add `OwnedEvent` and `Event::to_owned`**

Append to `rylr-core/src/types.rs` (above the test module):

```rust
#[cfg(feature = "alloc")]
pub use owned::OwnedEvent;

#[cfg(feature = "alloc")]
mod owned {
    use super::Event;
    use alloc::vec::Vec;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum OwnedEvent {
        Recv { from: u16, data: Vec<u8>, rssi: i16, snr: i16 },
        Ready,
    }

    impl<'a> Event<'a> {
        pub fn to_owned(&self) -> OwnedEvent {
            match self {
                Event::Recv { from, data, rssi, snr } => OwnedEvent::Recv {
                    from: *from,
                    data: data.to_vec(),
                    rssi: *rssi,
                    snr: *snr,
                },
                Event::Ready => OwnedEvent::Ready,
            }
        }
    }
}
```

Re-export from `lib.rs`:

```rust
#[cfg(feature = "alloc")]
pub use types::OwnedEvent;
```

- [ ] **Step 4: Run the test**

```bash
cargo test -p rylr-core --features alloc -- owned_tests
```

Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "core: OwnedEvent under alloc feature"
```

---

## Task 4: `encode.rs` (Command → bytes)

**Files:**
- Create: `rylr-core/src/encode.rs`
- Modify: `rylr-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `rylr-core/src/encode.rs`:

```rust
//! Encode `Command` values into ASCII command lines (`AT...\r\n`).

use crate::{Command, Error, RfParams};

/// Write the encoded form of `cmd` into `buf`. Returns the number of bytes
/// written, or `Error::TxOverflow` if `buf` is too small.
pub fn encode(cmd: Command<'_>, buf: &mut [u8]) -> Result<usize, Error> {
    let mut w = Writer::new(buf);
    match cmd {
        Command::Ping            => w.lit(b"AT")?,
        Command::GetAddress      => w.lit(b"AT+ADDRESS?")?,
        Command::SetAddress(n)   => { w.lit(b"AT+ADDRESS=")?; w.u16(n)?; }
        Command::GetNetworkId    => w.lit(b"AT+NETWORKID?")?,
        Command::SetNetworkId(n) => { w.lit(b"AT+NETWORKID=")?; w.u16(n as u16)?; }
        Command::GetBand         => w.lit(b"AT+BAND?")?,
        Command::SetBand(hz)     => { w.lit(b"AT+BAND=")?; w.u32(hz)?; }
        Command::GetParameters   => w.lit(b"AT+PARAMETER?")?,
        Command::SetParameters(RfParams { sf, bw, cr, preamble }) => {
            w.lit(b"AT+PARAMETER=")?;
            w.u16(sf as u16)?; w.lit(b",")?;
            w.u16(bw as u16)?; w.lit(b",")?;
            w.u16(cr as u16)?; w.lit(b",")?;
            w.u16(preamble as u16)?;
        }
        Command::GetCrfop        => w.lit(b"AT+CRFOP?")?,
        Command::GetUid          => w.lit(b"AT+UID?")?,
        Command::GetVersion      => w.lit(b"AT+VER?")?,
        Command::FactoryReset    => w.lit(b"AT+FACTORY")?,
        Command::Send { to, data } => {
            w.lit(b"AT+SEND=")?;
            w.u16(to)?; w.lit(b",")?;
            w.u16(data.len() as u16)?; w.lit(b",")?;
            w.bytes(data)?;
        }
    }
    w.lit(b"\r\n")?;
    Ok(w.pos)
}

struct Writer<'b> { buf: &'b mut [u8], pos: usize }
impl<'b> Writer<'b> {
    fn new(buf: &'b mut [u8]) -> Self { Self { buf, pos: 0 } }
    fn lit(&mut self, s: &[u8]) -> Result<(), Error> { self.bytes(s) }
    fn bytes(&mut self, s: &[u8]) -> Result<(), Error> {
        if self.pos + s.len() > self.buf.len() { return Err(Error::TxOverflow); }
        self.buf[self.pos..self.pos + s.len()].copy_from_slice(s);
        self.pos += s.len();
        Ok(())
    }
    fn u16(&mut self, mut n: u16) -> Result<(), Error> {
        let mut digits = [0u8; 5];
        let mut i = 0;
        if n == 0 { return self.bytes(b"0"); }
        while n > 0 { digits[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        for j in (0..i).rev() { self.bytes(&[digits[j]])?; }
        Ok(())
    }
    fn u32(&mut self, mut n: u32) -> Result<(), Error> {
        let mut digits = [0u8; 10];
        let mut i = 0;
        if n == 0 { return self.bytes(b"0"); }
        while n > 0 { digits[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        for j in (0..i).rev() { self.bytes(&[digits[j]])?; }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RfParams;

    fn encode_to_string(cmd: Command<'_>) -> String {
        let mut buf = [0u8; 512];
        let n = encode(cmd, &mut buf).unwrap();
        String::from_utf8(buf[..n].to_vec()).unwrap()
    }

    #[test] fn ping()           { assert_eq!(encode_to_string(Command::Ping), "AT\r\n"); }
    #[test] fn get_address()    { assert_eq!(encode_to_string(Command::GetAddress), "AT+ADDRESS?\r\n"); }
    #[test] fn set_address()    { assert_eq!(encode_to_string(Command::SetAddress(5)), "AT+ADDRESS=5\r\n"); }
    #[test] fn set_address_zero() { assert_eq!(encode_to_string(Command::SetAddress(0)), "AT+ADDRESS=0\r\n"); }
    #[test] fn set_address_max()  { assert_eq!(encode_to_string(Command::SetAddress(65535)), "AT+ADDRESS=65535\r\n"); }
    #[test] fn set_network_id() { assert_eq!(encode_to_string(Command::SetNetworkId(18)), "AT+NETWORKID=18\r\n"); }
    #[test] fn set_band()       { assert_eq!(encode_to_string(Command::SetBand(915_000_000)), "AT+BAND=915000000\r\n"); }
    #[test] fn set_parameters() {
        let p = RfParams { sf: 9, bw: 7, cr: 1, preamble: 12 };
        assert_eq!(encode_to_string(Command::SetParameters(p)), "AT+PARAMETER=9,7,1,12\r\n");
    }
    #[test] fn factory_reset()  { assert_eq!(encode_to_string(Command::FactoryReset), "AT+FACTORY\r\n"); }
    #[test] fn send_text() {
        assert_eq!(encode_to_string(Command::Send { to: 2, data: b"hello" }), "AT+SEND=2,5,hello\r\n");
    }
    #[test] fn send_with_comma_in_payload() {
        assert_eq!(encode_to_string(Command::Send { to: 7, data: b"a,b" }), "AT+SEND=7,3,a,b\r\n");
    }

    #[test]
    fn tx_overflow() {
        let mut buf = [0u8; 4]; // too small for "AT\r\n"
        assert_eq!(encode(Command::Ping, &mut buf), Ok(4));  // exactly fits
        let mut buf = [0u8; 3];
        assert_eq!(encode(Command::Ping, &mut buf), Err(Error::TxOverflow));
    }
}
```

- [ ] **Step 2: Run tests (they should fail to compile)**

```bash
cargo test -p rylr-core
```

Expected: `unresolved import` or similar — `encode` module not in `lib.rs` yet.

- [ ] **Step 3: Wire up `lib.rs`**

Add to `rylr-core/src/lib.rs`:

```rust
mod encode;
pub use encode::encode;
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p rylr-core
```

Expected: 13 passed (encode module).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "core: Command encode (pure fn) with full test table"
```

---

## Task 5: `Driver` struct, `submit`, `push_rx`, `ack_tx`

This task lays down the Driver's data structures and the three methods that don't require state-machine logic. The `poll()` body stays as `unimplemented!()`; Task 7 turns it into the user's exercise.

**Files:**
- Create: `rylr-core/src/driver.rs`
- Modify: `rylr-core/src/lib.rs`

- [ ] **Step 1: Write `driver.rs`**

Create `rylr-core/src/driver.rs`:

```rust
//! State-machine `Driver` over a fixed-size RX buffer and an outbound TX queue.

use crate::{Command, Error, Poll};
use heapless::Vec as HVec;

pub(crate) const RX_BUF: usize = 512;
pub(crate) const TX_BUF: usize = 512;

#[derive(Default, Clone, Copy)]
pub(crate) enum State {
    #[default]
    Idle,
    /// A command has been encoded; bytes are pending in `tx`.
    /// Once they're all `ack_tx`'d, transition to `Awaiting`.
    SendingTx,
    /// All TX bytes acked, awaiting the matching response line.
    Awaiting { kind: AwaitKind },
}

#[derive(Clone, Copy)]
pub(crate) enum AwaitKind {
    /// Setter / Ping / FactoryReset — expects `+OK` or `+ERR`.
    Ack,
    /// Query — expects `+<KEY>=...` or `+ERR`.
    Query(QueryKey),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryKey {
    Address, NetworkId, Band, Parameters, Crfop, Uid, Version,
}

pub struct Driver {
    pub(crate) rx: HVec<u8, RX_BUF>,
    pub(crate) tx: HVec<u8, TX_BUF>,
    /// Bytes already handed out via `Poll::NeedTx` but not yet acked.
    pub(crate) tx_in_flight: usize,
    pub(crate) state: State,
    /// True iff the next `+READY` should resolve as `Response::Ok`
    /// (i.e. `AT+FACTORY` is in flight).
    pub(crate) awaiting_ready_as_ok: bool,
    pub(crate) pending_kind: Option<AwaitKind>,
}

impl Driver {
    pub const fn new() -> Self {
        Self {
            rx: HVec::new(),
            tx: HVec::new(),
            tx_in_flight: 0,
            state: State::Idle,
            awaiting_ready_as_ok: false,
            pending_kind: None,
        }
    }

    pub fn submit(&mut self, cmd: Command<'_>) -> Result<(), Error> {
        if !matches!(self.state, State::Idle) {
            return Err(Error::Busy);
        }
        let kind = match cmd {
            Command::GetAddress    => AwaitKind::Query(QueryKey::Address),
            Command::GetNetworkId  => AwaitKind::Query(QueryKey::NetworkId),
            Command::GetBand       => AwaitKind::Query(QueryKey::Band),
            Command::GetParameters => AwaitKind::Query(QueryKey::Parameters),
            Command::GetCrfop      => AwaitKind::Query(QueryKey::Crfop),
            Command::GetUid        => AwaitKind::Query(QueryKey::Uid),
            Command::GetVersion    => AwaitKind::Query(QueryKey::Version),
            _                      => AwaitKind::Ack,
        };
        self.awaiting_ready_as_ok = matches!(cmd, Command::FactoryReset);

        let mut tmp = [0u8; TX_BUF];
        let n = crate::encode::encode(cmd, &mut tmp)?;
        if self.tx.extend_from_slice(&tmp[..n]).is_err() {
            return Err(Error::TxOverflow);
        }
        self.state = State::SendingTx;
        self.pending_kind = Some(kind);
        Ok(())
    }

    pub fn push_rx(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let room = self.rx.capacity() - self.rx.len();
        if bytes.len() > room {
            // Accept what fits, then signal overflow.
            let _ = self.rx.extend_from_slice(&bytes[..room]);
            return Err(Error::RxOverflow);
        }
        let _ = self.rx.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    pub fn ack_tx(&mut self, n: usize) {
        let n = n.min(self.tx_in_flight);
        // Shift the first `n` bytes off `self.tx` (heapless::Vec has no drain).
        let len = self.tx.len();
        if n > 0 && n <= len {
            self.tx.copy_within(n..len, 0);
            self.tx.truncate(len - n);
        }
        self.tx_in_flight -= n;
        if self.tx.is_empty() && matches!(self.state, State::SendingTx) {
            if let Some(kind) = self.pending_kind.take() {
                self.state = State::Awaiting { kind };
            }
        }
    }

    pub fn poll(&mut self) -> Poll<'_> {
        // EXERCISE: see Task 7.
        unimplemented!("Driver::poll — Task 7 exercise")
    }
}
```

- [ ] **Step 2: Add unit tests for `submit`/`push_rx`/`ack_tx` (do *not* call `poll`)**

Append to `rylr-core/src/driver.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_idle_then_busy() {
        let mut d = Driver::new();
        assert!(d.submit(Command::Ping).is_ok());
        assert_eq!(d.submit(Command::Ping), Err(Error::Busy));
    }

    #[test]
    fn submit_encodes_into_tx_buffer() {
        let mut d = Driver::new();
        d.submit(Command::SetAddress(5)).unwrap();
        // tx buffer holds "AT+ADDRESS=5\r\n" = 14 bytes
        assert_eq!(d.tx.len(), 14);
        assert_eq!(&d.tx[..14], b"AT+ADDRESS=5\r\n");
    }

    #[test]
    fn push_rx_appends() {
        let mut d = Driver::new();
        d.push_rx(b"+OK\r\n").unwrap();
        assert_eq!(&d.rx[..], b"+OK\r\n");
    }

    #[test]
    fn push_rx_overflow_signals_error() {
        let mut d = Driver::new();
        let big = [0u8; RX_BUF + 1];
        assert_eq!(d.push_rx(&big), Err(Error::RxOverflow));
    }

    #[test]
    fn ack_tx_shifts_buffer() {
        let mut d = Driver::new();
        d.submit(Command::Ping).unwrap();
        // Pretend poll() handed out the bytes:
        d.tx_in_flight = d.tx.len();
        let total = d.tx.len();
        d.ack_tx(2);
        assert_eq!(d.tx.len(), total - 2);
    }
}
```

- [ ] **Step 3: Wire up `lib.rs` and verify**

Add to `rylr-core/src/lib.rs`:

```rust
mod driver;
pub use driver::Driver;
```

```bash
cargo test -p rylr-core driver::tests
```

Expected: 5 passed.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "core: Driver struct + submit/push_rx/ack_tx (poll deferred)"
```

---

## Task 6: `decode.rs` — EXERCISE scaffold

This task creates `decode.rs` as a guided exercise: full fixture tests, complete public-function signatures, doc comments explaining each transform, and `unimplemented!()` bodies. The user fills in the bodies.

**Files:**
- Create: `rylr-core/src/decode.rs`
- Modify: `rylr-core/src/lib.rs`

- [ ] **Step 1: Write the scaffold + fixture tests**

Create `rylr-core/src/decode.rs`:

```rust
//! Parse RYLR response lines.
//!
//! ## EXERCISE
//!
//! All four parsing functions in this module are `unimplemented!()`.
//! Your job: replace each `unimplemented!()` with real logic that makes
//! the test table pass. Work in this order — each builds on the last:
//!
//! 1. `parse_response` for `+OK` and `+ERR=N`.
//! 2. `parse_response` for `+ADDRESS=`, `+NETWORKID=`, `+BAND=`,
//!    `+PARAMETER=`, `+CRFOP=`, `+UID=`, `+VER=` (one branch at a time).
//! 3. `parse_event` for `+READY`.
//! 4. `parse_event` for `+RCV=...` — the interesting one. Read the
//!    "embedded comma" note carefully.
//!
//! ### `+RCV` framing
//!
//! Wire form: `+RCV=<addr>,<len>,<data>,<rssi>,<snr>` where `<data>`
//! is exactly `<len>` raw bytes. `<data>` *may itself contain commas*.
//! You CANNOT split the whole line on `,` and expect to recover.
//! Instead:
//!
//! 1. Take the prefix up to the first `,` → `addr`.
//! 2. Take the next prefix up to the next `,` → `len`.
//! 3. Skip exactly `len` bytes → `data`.
//! 4. Expect the next byte to be `,`.
//! 5. Take the next prefix up to the next `,` → `rssi`.
//! 6. Take the rest → `snr`.

use crate::{Error, Event, Response, RfParams};

/// Parse a complete response line (no `\r\n`).
///
/// Returns `Err(Error::Parse)` on any unrecognized form.
pub fn parse_response<'a>(line: &'a [u8]) -> Result<Response<'a>, Error> {
    // TODO: dispatch on prefix:
    //   "+OK"       -> Response::Ok
    //   "+ERR="     -> parse u8, return Response::Err(n)
    //   "+ADDRESS=" -> parse u16,        return Response::Address(n)
    //   "+NETWORKID=" -> parse u8,       return Response::NetworkId(n)
    //   "+BAND="    -> parse u32,        return Response::Band(n)
    //   "+PARAMETER=" -> parse 4 u8s,    return Response::Parameters(...)
    //   "+CRFOP="   -> parse u8,         return Response::Crfop(n)
    //   "+UID="     -> rest as &str,     return Response::Uid(s)
    //   "+VER="     -> rest as &str,     return Response::Version(s)
    // Any other prefix → Err(Error::Parse).
    let _ = (line, RfParams { sf: 0, bw: 0, cr: 0, preamble: 0 }); // silence warnings
    unimplemented!("decode::parse_response")
}

/// Parse a complete unsolicited line (no `\r\n`).
///
/// Returns `Err(Error::Parse)` if this isn't a known event.
pub fn parse_event<'a>(line: &'a [u8]) -> Result<Event<'a>, Error> {
    // TODO:
    //   "+READY"  -> Event::Ready
    //   "+RCV="   -> see length-prefixed framing in module docs
    //   anything else -> Err(Error::Parse)
    let _ = line;
    unimplemented!("decode::parse_event")
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Responses -------------------------------------------------------

    #[test] fn ok()  { assert_eq!(parse_response(b"+OK"), Ok(Response::Ok)); }
    #[test] fn err() { assert_eq!(parse_response(b"+ERR=4"), Ok(Response::Err(4))); }

    #[test] fn address()    { assert_eq!(parse_response(b"+ADDRESS=5"),     Ok(Response::Address(5))); }
    #[test] fn address_max(){ assert_eq!(parse_response(b"+ADDRESS=65535"), Ok(Response::Address(65535))); }

    #[test] fn network_id() { assert_eq!(parse_response(b"+NETWORKID=18"), Ok(Response::NetworkId(18))); }

    #[test] fn band()       { assert_eq!(parse_response(b"+BAND=915000000"), Ok(Response::Band(915_000_000))); }

    #[test]
    fn parameters() {
        assert_eq!(
            parse_response(b"+PARAMETER=9,7,1,12"),
            Ok(Response::Parameters(RfParams { sf: 9, bw: 7, cr: 1, preamble: 12 }))
        );
    }

    #[test] fn crfop()   { assert_eq!(parse_response(b"+CRFOP=22"), Ok(Response::Crfop(22))); }
    #[test] fn uid()     { assert_eq!(parse_response(b"+UID=DEADBEEF"), Ok(Response::Uid("DEADBEEF"))); }
    #[test] fn version() { assert_eq!(parse_response(b"+VER=AT_V1.2.5"), Ok(Response::Version("AT_V1.2.5"))); }

    #[test] fn unknown_response() { assert_eq!(parse_response(b"+WAT="), Err(Error::Parse)); }
    #[test] fn empty_response()   { assert_eq!(parse_response(b""), Err(Error::Parse)); }

    // --- Events ----------------------------------------------------------

    #[test] fn ready() { assert_eq!(parse_event(b"+READY"), Ok(Event::Ready)); }

    #[test]
    fn rcv_simple() {
        let ev = parse_event(b"+RCV=2,5,hello,-42,8").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 2,
            data: b"hello",
            rssi: -42,
            snr: 8,
        });
    }

    #[test]
    fn rcv_payload_with_embedded_comma() {
        let ev = parse_event(b"+RCV=7,3,a,b,-50,4").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 7,
            data: b"a,b",
            rssi: -50,
            snr: 4,
        });
    }

    #[test]
    fn rcv_payload_only_commas() {
        // 3-byte payload that is literally `,,,` -- length-prefix proves itself
        let ev = parse_event(b"+RCV=1,3,,,,,-30,3").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 1,
            data: b",,,",
            rssi: -30,
            snr: 3,
        });
    }

    #[test]
    fn rcv_negative_snr() {
        let ev = parse_event(b"+RCV=2,1,x,-100,-5").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 2,
            data: b"x",
            rssi: -100,
            snr: -5,
        });
    }

    #[test]
    fn rcv_zero_length_payload() {
        let ev = parse_event(b"+RCV=2,0,,-40,7").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 2,
            data: b"",
            rssi: -40,
            snr: 7,
        });
    }

    #[test]
    fn rcv_truncated_payload_is_parse_error() {
        // Claims len=10 but only 3 bytes follow before the next field.
        assert_eq!(parse_event(b"+RCV=1,10,abc,-30,3"), Err(Error::Parse));
    }

    #[test] fn unknown_event() { assert_eq!(parse_event(b"+WAT"), Err(Error::Parse)); }
}
```

- [ ] **Step 2: Wire up `lib.rs`**

Add to `rylr-core/src/lib.rs`:

```rust
mod decode;
pub use decode::{parse_event, parse_response};
```

- [ ] **Step 3: Verify the scaffold compiles**

```bash
cargo check -p rylr-core
```

Expected: `Finished` with warnings about `unimplemented!()` panics in tests (that's expected — tests will panic when run).

- [ ] **Step 4: Run tests to confirm they fail clearly**

```bash
cargo test -p rylr-core decode
```

Expected: every test panics with `not implemented: decode::parse_response` or `decode::parse_event`. This is the user's TODO list — each failing test is a sub-step they implement.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "core: decode.rs scaffold (EXERCISE) with fixture tests"
```

---

## Task 7: `Driver::poll` — EXERCISE scaffold + integration tests

The user's main exercise. By the end of Task 6 they have a working parser; this task uses it inside `poll()`.

**Files:**
- Modify: `rylr-core/src/driver.rs`

- [ ] **Step 1: Replace the `poll()` body with a documented stub**

In `rylr-core/src/driver.rs`, replace the `poll` method with:

```rust
/// Drive the state machine.
///
/// ## EXERCISE
///
/// Priority order on each call:
/// 1. If we have undelivered TX bytes (`self.tx` has bytes that haven't
///    been handed out as `NeedTx` yet), return `Poll::NeedTx(slice)`.
///    Track them as in-flight via `self.tx_in_flight` so `ack_tx` can
///    drain them.
/// 2. Try to find a complete unsolicited event in `self.rx`:
///    a. Look for `\r\n`. If absent, no events available.
///    b. If the line starts with `+RCV` or `+READY`, parse it via
///       `crate::decode::parse_event`. Drain the line + `\r\n` from
///       `self.rx` and return `Poll::Event(...)`.
///    c. If `awaiting_ready_as_ok` is true and the line is `+READY`,
///       return `Poll::Response(Response::Ok)` instead, transition to
///       `Idle`, clear `awaiting_ready_as_ok`, drain the line.
/// 3. If we're in state `Awaiting`, look for a complete response line
///    (`+OK`, `+ERR=N`, or the matching `+<KEY>=...`) and parse via
///    `decode::parse_response`. On match, drain, transition to `Idle`,
///    return `Poll::Response(...)`.
/// 4. Otherwise, return `Poll::Idle`.
///
/// ### Hints
///
/// - You'll want a private helper `fn next_line_end(&self) -> Option<usize>`
///   that returns the index of `\r` if a complete `\r\n` is buffered.
/// - Use `self.rx.as_slice()[..end]` for the line bytes; then
///   `self.rx.copy_within(end + 2.., 0)` + `self.rx.truncate(...)` to
///   drain. (`heapless::Vec` has no `drain`, so manual copy is the move.)
/// - The borrow returned in `Poll::Event` / `Poll::Response` must outlive
///   the drain. Translation: parse first, then drain. Or parse into an
///   intermediate owned representation before draining — but that adds
///   an alloc. Cleanest: take a snapshot of the line into a stack buffer,
///   parse from that, then drain.
///
/// ### Note on borrowing
///
/// Returning `Poll::Event { data: &[u8] }` from `&mut self` while the
/// data lives in `self.rx` requires that we *not* mutate `self.rx`
/// between parse and return. The simplest solution: copy the line into
/// a small `[u8; 280]` field on `Driver` (`line_buf`), drain `self.rx`
/// of that line *before* parsing, then parse from `line_buf`. The borrow
/// returned then ties to `&self.line_buf`, which `&mut self` already
/// reserves.
pub fn poll(&mut self) -> Poll<'_> {
    // TODO: implement per the rules above.
    unimplemented!("Driver::poll — exercise")
}
```

- [ ] **Step 2: Add a `line_buf` field for the borrow workaround the hint references**

Update the struct (preserve the `pub(crate)` from Task 5):

```rust
pub struct Driver {
    pub(crate) rx: HVec<u8, RX_BUF>,
    pub(crate) tx: HVec<u8, TX_BUF>,
    pub(crate) tx_in_flight: usize,
    pub(crate) state: State,
    pub(crate) awaiting_ready_as_ok: bool,
    pub(crate) pending_kind: Option<AwaitKind>,
    /// Scratch space for poll() to copy a line into before draining rx.
    pub(crate) line_buf: [u8; 288],
    pub(crate) line_buf_len: usize,
}
```

And `new()`:

```rust
pub const fn new() -> Self {
    Self {
        rx: HVec::new(),
        tx: HVec::new(),
        tx_in_flight: 0,
        state: State::Idle,
        awaiting_ready_as_ok: false,
        pending_kind: None,
        line_buf: [0u8; 288],
        line_buf_len: 0,
    }
}
```

- [ ] **Step 3: Add integration tests that drive a full sequence**

Append to `rylr-core/src/driver.rs`:

```rust
#[cfg(all(test, feature = "alloc"))]
mod poll_tests {
    use super::*;
    use crate::{Event, Response};
    use alloc::vec::Vec;

    /// Helper: drain TX bytes the driver wants sent, ack them, and return
    /// what was sent for assertions.
    fn drain_tx(d: &mut Driver) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            match d.poll() {
                Poll::NeedTx(bytes) => {
                    out.extend_from_slice(bytes);
                    let n = bytes.len();
                    d.ack_tx(n);
                }
                _ => return out,
            }
        }
    }

    #[test]
    fn set_address_round_trip() {
        let mut d = Driver::new();
        d.submit(Command::SetAddress(5)).unwrap();

        assert_eq!(drain_tx(&mut d), b"AT+ADDRESS=5\r\n");

        d.push_rx(b"+OK\r\n").unwrap();
        assert!(matches!(d.poll(), Poll::Response(Response::Ok)));
        assert!(matches!(d.poll(), Poll::Idle));
    }

    #[test]
    fn get_address_returns_value() {
        let mut d = Driver::new();
        d.submit(Command::GetAddress).unwrap();
        let _ = drain_tx(&mut d);
        d.push_rx(b"+ADDRESS=5\r\n").unwrap();
        match d.poll() {
            Poll::Response(Response::Address(5)) => {}
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn err_response() {
        let mut d = Driver::new();
        d.submit(Command::Ping).unwrap();
        let _ = drain_tx(&mut d);
        d.push_rx(b"+ERR=4\r\n").unwrap();
        assert!(matches!(d.poll(), Poll::Response(Response::Err(4))));
    }

    #[test]
    fn factory_reset_resolves_on_ready() {
        let mut d = Driver::new();
        d.submit(Command::FactoryReset).unwrap();
        let _ = drain_tx(&mut d);
        d.push_rx(b"+OK\r\n+READY\r\n").unwrap();
        // Implementation choice: either +OK or +READY may resolve the
        // command. Both are acceptable; subsequent poll() must be Idle.
        assert!(matches!(d.poll(), Poll::Response(Response::Ok)));
    }

    #[test]
    fn unsolicited_recv() {
        let mut d = Driver::new();
        d.push_rx(b"+RCV=2,5,hello,-42,8\r\n").unwrap();
        match d.poll() {
            Poll::Event(Event::Recv { from: 2, data, rssi: -42, snr: 8 }) => {
                assert_eq!(data, b"hello");
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn recv_during_command_is_delivered_first() {
        let mut d = Driver::new();
        d.submit(Command::GetAddress).unwrap();
        let _ = drain_tx(&mut d);
        d.push_rx(b"+RCV=2,2,hi,-30,3\r\n+ADDRESS=5\r\n").unwrap();
        // Per priority order: events drain before responses.
        assert!(matches!(d.poll(), Poll::Event(_)));
        assert!(matches!(d.poll(), Poll::Response(Response::Address(5))));
    }
}
```

- [ ] **Step 4: Verify the scaffold compiles**

```bash
cargo check -p rylr-core --features alloc
```

Expected: `Finished`.

- [ ] **Step 5: Confirm the tests fail clearly**

```bash
cargo test -p rylr-core --features alloc poll_tests
```

Expected: every test panics with `not implemented: Driver::poll — exercise`. This is the user's exercise.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "core: Driver::poll scaffold (EXERCISE) with sequence tests"
```

---

## Task 8: `rylr-std` — `Error` type

**Files:**
- Create: `rylr-std/src/error.rs`
- Modify: `rylr-std/src/lib.rs`

- [ ] **Step 1: Write the error type**

Create `rylr-std/src/error.rs`:

```rust
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("core: {0}")]
    Core(#[from] rylr_core::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serial: {0}")]
    Serial(#[from] serialport::Error),
    #[error("timeout")]
    Timeout,
    #[error("radio error code {0}")]
    Radio(u8),
    #[error("no cu.usbserial* device found")]
    NoDevice,
    #[error("multiple candidate devices: {0:?} (use --port)")]
    Ambiguous(Vec<PathBuf>),
}

pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 2: Wire up `lib.rs`**

Replace `rylr-std/src/lib.rs` with:

```rust
//! Blocking, single-radio transport for `rylr-core` over `serialport`.

mod error;
pub use error::{Error, Result};
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p rylr-std
```

Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "std: Error type"
```

---

## Task 9: `rylr-std` — port discovery

**Files:**
- Create: `rylr-std/src/port.rs`
- Modify: `rylr-std/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `rylr-std/src/port.rs`:

```rust
//! Auto-discover a single `cu.usbserial*` device.

use crate::{Error, Result};
use std::path::PathBuf;

/// Test-friendly: filter a list of port names down to candidates.
pub(crate) fn filter(names: impl IntoIterator<Item = String>) -> Vec<PathBuf> {
    names
        .into_iter()
        .filter(|n| {
            // accept full paths (/dev/cu.usbserial-XYZ) or bare names
            let leaf = std::path::Path::new(n)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(n.as_str());
            leaf.starts_with("cu.usbserial")
        })
        .map(PathBuf::from)
        .collect()
}

pub fn discover() -> Result<PathBuf> {
    let names = serialport::available_ports()?
        .into_iter()
        .map(|p| p.port_name)
        .collect::<Vec<_>>();
    decide(filter(names))
}

fn decide(mut candidates: Vec<PathBuf>) -> Result<PathBuf> {
    match candidates.len() {
        0 => Err(Error::NoDevice),
        1 => Ok(candidates.remove(0)),
        _ => Err(Error::Ambiguous(candidates)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_match_is_no_device() {
        let r = decide(filter(vec!["/dev/ttyS0".into(), "/dev/cu.Bluetooth-Incoming-Port".into()]));
        assert!(matches!(r, Err(Error::NoDevice)));
    }

    #[test]
    fn one_match_returns_path() {
        let r = decide(filter(vec!["/dev/cu.usbserial-A1".into(), "/dev/cu.Bluetooth-Incoming-Port".into()]));
        assert_eq!(r.unwrap(), PathBuf::from("/dev/cu.usbserial-A1"));
    }

    #[test]
    fn two_matches_is_ambiguous() {
        let r = decide(filter(vec![
            "/dev/cu.usbserial-A1".into(),
            "/dev/cu.usbserial-B2".into(),
        ]));
        match r {
            Err(Error::Ambiguous(v)) => assert_eq!(v.len(), 2),
            _ => panic!("expected Ambiguous"),
        }
    }

    #[test]
    fn filter_accepts_bare_names() {
        let r = filter(vec!["cu.usbserial-X".into(), "ttyACM0".into()]);
        assert_eq!(r, vec![PathBuf::from("cu.usbserial-X")]);
    }
}
```

- [ ] **Step 2: Wire up and run tests**

Add to `rylr-std/src/lib.rs`:

```rust
mod port;
pub use port::discover;
```

```bash
cargo test -p rylr-std port::tests
```

Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "std: port discovery filter + tests"
```

---

## Task 10: `rylr-std` — `Radio` struct, generic, and `open*` constructors

**Files:**
- Create: `rylr-std/src/radio.rs`
- Modify: `rylr-std/src/lib.rs`

- [ ] **Step 1: Write the struct + constructors**

Create `rylr-std/src/radio.rs`:

```rust
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
```

- [ ] **Step 2: Wire up and verify**

Add to `rylr-std/src/lib.rs`:

```rust
mod radio;
pub use radio::Radio;
pub use rylr_core::{OwnedEvent, RfParams};
```

```bash
cargo check -p rylr-std
```

Expected: `Finished`.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "std: Radio<P> generic + open/open_auto/from_port"
```

---

## Task 11: `rylr-std` — `pump_until` EXERCISE scaffold

**Files:**
- Modify: `rylr-std/src/radio.rs`

- [ ] **Step 1: Add the helper signature with TODOs**

Append to `rylr-std/src/radio.rs`:

```rust
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
    ///      - `Poll::Event(e)`  -> push `e.to_owned()` to `self.events`,
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
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p rylr-std
```

Expected: `Finished`.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "std: pump_until scaffold (EXERCISE)"
```

---

## Task 12: `rylr-std` — per-AT-command `Radio` methods

Each method calls `submit` then `pump_until` with a closure that pattern-matches the response. Mechanical; not an exercise.

**Files:**
- Modify: `rylr-std/src/radio.rs`

- [ ] **Step 1: Add the method block**

Append to `rylr-std/src/radio.rs`:

```rust
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
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p rylr-std
```

Expected: `Finished`.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "std: per-AT-command Radio methods"
```

---

## Task 13: `rylr-std` — `Loopback` test helper + integration tests

These tests will fail until the user implements `pump_until` (Task 11) and `Driver::poll` (Task 7). They are the user's signal that exercises are done.

The helper lives at `tests/common/mod.rs` — Cargo treats `tests/<name>.rs` files as standalone test crates but ignores `tests/<dir>/`. Putting the loopback in `tests/common/mod.rs` and pulling it in via `mod common;` is the standard way to share test fixtures across integration tests.

**Files:**
- Create: `rylr-std/tests/common/mod.rs`
- Create: `rylr-std/tests/radio_methods.rs`

- [ ] **Step 1: Write the `Loopback` helper**

Create `rylr-std/tests/common/mod.rs`:

```rust
//! In-memory bidirectional pipe for testing `Radio<P>` without a real port.
//!
//! Two halves share a pair of `Arc<Mutex<VecDeque<u8>>>`. Writes to one
//! half land in the queue the other half reads from.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct PipeInner {
    buf: VecDeque<u8>,
}

#[derive(Clone)]
pub struct Pipe(Arc<Mutex<PipeInner>>);

impl Pipe {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(PipeInner::default())))
    }
    fn write_bytes(&self, bytes: &[u8]) {
        self.0.lock().unwrap().buf.extend(bytes);
    }
    fn read_bytes(&self, dst: &mut [u8]) -> usize {
        let mut g = self.0.lock().unwrap();
        let n = g.buf.len().min(dst.len());
        for slot in dst.iter_mut().take(n) {
            *slot = g.buf.pop_front().unwrap();
        }
        n
    }
}

pub struct Endpoint {
    rx: Pipe,
    tx: Pipe,
}

impl Read for Endpoint {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.rx.read_bytes(buf);
        if n == 0 {
            // Mimic a serial port read timeout.
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "no data"));
        }
        Ok(n)
    }
}

impl Write for Endpoint {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.tx.write_bytes(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct RadioSide(pub Endpoint);
pub struct WireSide(pub Endpoint);

pub fn pair() -> (RadioSide, WireSide) {
    let a = Pipe::new();
    let b = Pipe::new();
    (
        RadioSide(Endpoint { rx: a.clone(), tx: b.clone() }),
        WireSide(Endpoint { rx: b, tx: a }),
    )
}

impl WireSide {
    /// "From the radio's mouth": queue bytes that the Radio<P> will read.
    pub fn say(&mut self, bytes: &[u8]) {
        self.0.tx.write_bytes(bytes);
    }
    /// What did the Radio<P> just send?
    pub fn drain_outgoing(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = [0u8; 256];
        loop {
            let n = self.0.rx.read_bytes(&mut buf);
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        out
    }
}
```

- [ ] **Step 2: Write the integration test file**

Create `rylr-std/tests/radio_methods.rs`:

```rust
mod common;

use common::{pair, Endpoint, RadioSide, WireSide};
use rylr_std::Radio;

fn make() -> (Radio<Endpoint>, WireSide) {
    let (RadioSide(ep), wire) = pair();
    (Radio::from_port(ep), wire)
}

#[test]
fn ping() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.ping());

    // Wait briefly for the radio to push its command, then reply.
    std::thread::sleep(std::time::Duration::from_millis(50));
    let out = wire.drain_outgoing();
    assert_eq!(out, b"AT\r\n");
    wire.say(b"+OK\r\n");

    handle.join().unwrap().unwrap();
}

#[test]
fn set_address() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.set_address(5));
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(wire.drain_outgoing(), b"AT+ADDRESS=5\r\n");
    wire.say(b"+OK\r\n");
    handle.join().unwrap().unwrap();
}

#[test]
fn get_address() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.address());
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(wire.drain_outgoing(), b"AT+ADDRESS?\r\n");
    wire.say(b"+ADDRESS=5\r\n");
    assert_eq!(handle.join().unwrap().unwrap(), 5);
}

#[test]
fn err_response_propagates() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.set_address(0xFFFF));
    std::thread::sleep(std::time::Duration::from_millis(50));
    let _ = wire.drain_outgoing();
    wire.say(b"+ERR=4\r\n");
    let r = handle.join().unwrap();
    assert!(matches!(r, Err(rylr_std::Error::Radio(4))));
}

#[test]
fn next_event_returns_recv() {
    let (mut radio, mut wire) = make();
    wire.say(b"+RCV=2,5,hello,-42,8\r\n");
    let ev = radio.next_event(std::time::Duration::from_secs(1)).unwrap();
    match ev {
        rylr_std::OwnedEvent::Recv { from, data, rssi, snr } => {
            assert_eq!(from, 2);
            assert_eq!(data, b"hello");
            assert_eq!(rssi, -42);
            assert_eq!(snr, 8);
        }
        other => panic!("unexpected: {:?}", other),
    }
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo test -p rylr-std --no-run
```

Expected: `Finished`.

- [ ] **Step 4: Confirm tests fail with the right "exercise" panics**

```bash
cargo test -p rylr-std --tests
```

Expected: each test panics with `not implemented: Driver::poll — exercise` or `Radio::pump_until — exercise`. This is the user's "done" signal: when they finish both exercises, every test here passes.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "std: Loopback test helper + integration tests (fail until exercises done)"
```

---

## Task 14: `rylr-tool` — `main.rs` clap dispatch

**Files:**
- Modify: `rylr-tool/src/main.rs`

- [ ] **Step 1: Replace the placeholder `main.rs`**

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;

#[derive(Parser)]
#[command(name = "rylr-tool", about = "Configure and exercise REYAX RYLR998 modules.")]
struct Cli {
    /// Override auto-discovery of /dev/cu.usbserial*.
    #[arg(long, hide = true, global = true)]
    port: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print all readable settings.
    Info,
    /// Write address, network ID, and optionally band/parameters.
    Provision {
        #[arg(long)]
        address: u16,
        #[arg(long)]
        net: u8,
        #[arg(long)]
        band: Option<u32>,
        /// "S,B,C,P" -- four small ints
        #[arg(long, value_parser = parse_params)]
        params: Option<rylr_std::RfParams>,
    },
    /// AT+FACTORY then wait for +READY.
    Reset,
    /// AT+SEND. Use `-` for the message to read stdin.
    Send {
        #[arg(long)]
        to: u16,
        message: String,
    },
    /// Read events forever, one per line.
    Listen,
}

fn parse_params(s: &str) -> Result<rylr_std::RfParams, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err(format!("expected S,B,C,P (4 ints), got {} parts", parts.len()));
    }
    let n = |s: &str| s.parse::<u8>().map_err(|e| e.to_string());
    Ok(rylr_std::RfParams {
        sf: n(parts[0])?,
        bw: n(parts[1])?,
        cr: n(parts[2])?,
        preamble: n(parts[3])?,
    })
}

fn main() {
    let cli = Cli::parse();
    let result = (|| -> rylr_std::Result<()> {
        let radio = match cli.port {
            Some(p) => rylr_std::Radio::open(&p),
            None => {
                let path = rylr_std::Radio::discover()?;
                eprintln!("using port: {}", path.display());
                rylr_std::Radio::open(&path)
            }
        }?;
        match cli.cmd {
            Cmd::Info => commands::info(radio),
            Cmd::Provision { address, net, band, params } => {
                commands::provision(radio, address, net, band, params)
            }
            Cmd::Reset => commands::reset(radio),
            Cmd::Send { to, message } => commands::send(radio, to, &message),
            Cmd::Listen => commands::listen(radio),
        }
    })();

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

- [ ] **Step 2: Verify it compiles (commands module not yet present, so this fails)**

```bash
cargo check -p rylr-tool
```

Expected: `error[E0583]: file not found for module commands`. Proceed to Task 15.

---

## Task 15: `rylr-tool` — subcommand implementations

**Files:**
- Create: `rylr-tool/src/commands.rs`

- [ ] **Step 1: Write the subcommand bodies**

```rust
use rylr_std::{OwnedEvent, Radio, Result, RfParams};
use std::io::Read;
use std::time::Duration;

type R = Radio<Box<dyn serialport::SerialPort>>;

pub fn info(mut r: R) -> Result<()> {
    println!("ping        {}", marker(r.ping()));
    println!("address     {:?}", r.address()?);
    println!("network_id  {:?}", r.network_id()?);
    println!("band        {:?}", r.band()?);
    let p = r.parameters()?;
    println!("parameters  sf={} bw={} cr={} preamble={}", p.sf, p.bw, p.cr, p.preamble);
    println!("crfop       {:?}", r.crfop()?);
    println!("uid         {}", r.uid()?);
    println!("version     {}", r.version()?);
    Ok(())
}

pub fn provision(
    mut r: R,
    address: u16,
    net: u8,
    band: Option<u32>,
    params: Option<RfParams>,
) -> Result<()> {
    r.set_address(address)?;
    if r.address()? != address {
        return Err(verify_failed("address"));
    }
    r.set_network_id(net)?;
    if r.network_id()? != net {
        return Err(verify_failed("network_id"));
    }
    if let Some(b) = band {
        r.set_band(b)?;
        if r.band()? != b {
            return Err(verify_failed("band"));
        }
    }
    if let Some(p) = params {
        r.set_parameters(p)?;
        if r.parameters()? != p {
            return Err(verify_failed("parameters"));
        }
    }
    println!(
        "provisioned address={} net={} band={} params={}",
        address,
        net,
        band.map(|b| b.to_string()).unwrap_or_else(|| "(unchanged)".into()),
        match params {
            Some(p) => format!("{},{},{},{}", p.sf, p.bw, p.cr, p.preamble),
            None => "(unchanged)".into(),
        }
    );
    Ok(())
}

pub fn reset(mut r: R) -> Result<()> {
    r.factory_reset()
}

pub fn send(mut r: R, to: u16, message: &str) -> Result<()> {
    let bytes: Vec<u8> = if message == "-" {
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf)?;
        buf
    } else {
        message.as_bytes().to_vec()
    };
    r.send(to, &bytes)?;
    println!("+OK");
    Ok(())
}

pub fn listen(mut r: R) -> Result<()> {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let s2 = stop.clone();
    ctrlc::set_handler(move || s2.store(true, std::sync::atomic::Ordering::SeqCst))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    while !stop.load(std::sync::atomic::Ordering::SeqCst) {
        match r.next_event(Duration::from_secs(1)) {
            Ok(OwnedEvent::Recv { from, data, rssi, snr }) => {
                let s = String::from_utf8_lossy(&data);
                println!("from={from} rssi={rssi} snr={snr} \"{s}\"");
            }
            Ok(OwnedEvent::Ready) => eprintln!("(radio rebooted)"),
            Err(rylr_std::Error::Timeout) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn marker(r: Result<()>) -> &'static str {
    match r {
        Ok(()) => "ok",
        Err(_) => "fail",
    }
}

fn verify_failed(field: &str) -> rylr_std::Error {
    rylr_std::Error::Io(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("provisioning verify failed: {field}"),
    ))
}
```

- [ ] **Step 2: Verify the binary builds**

```bash
cargo build -p rylr-tool
```

Expected: `Finished`.

- [ ] **Step 3: Commit Tasks 14 + 15 together**

```bash
git add -A && git commit -m "tool: clap CLI + subcommand implementations"
```

---

## Task 16: `rylr-tool` — CLI smoke tests

**Files:**
- Modify: `rylr-tool/Cargo.toml`
- Create: `rylr-tool/tests/cli.rs`

- [ ] **Step 1: Add `assert_cmd` to dev-deps**

In `rylr-tool/Cargo.toml`, append:

```toml
[dev-dependencies]
assert_cmd = "2"
predicates = "3"
```

- [ ] **Step 2: Write the smoke tests**

Create `rylr-tool/tests/cli.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_runs() {
    Command::cargo_bin("rylr-tool")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("rylr-tool"));
}

#[test]
fn unknown_subcommand_fails() {
    Command::cargo_bin("rylr-tool")
        .unwrap()
        .arg("nope")
        .assert()
        .failure();
}

#[test]
fn missing_port_with_no_devices() {
    // Pass a clearly-invalid path; the tool should fail with non-zero exit
    // and a recognizable error message rather than panicking.
    Command::cargo_bin("rylr-tool")
        .unwrap()
        .args(["--port", "/dev/definitely-not-a-real-tty", "info"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"));
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p rylr-tool --tests
```

Expected: 3 passed.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "tool: CLI smoke tests"
```

---

## Task 17: `rylr-tokio` scaffold (EXERCISE)

**Files:**
- Modify: `rylr-tokio/src/lib.rs`

- [ ] **Step 1: Write the scaffold**

Replace `rylr-tokio/src/lib.rs` with:

```rust
//! Async (Tokio) transport for `rylr-core`.
//!
//! ## EXERCISE
//!
//! Implement `AsyncRadio`. Recommended structure: a background `tokio::task`
//! owns the `Driver` and the `tokio_serial::SerialStream`. The handle holds
//! two channels:
//!
//! - `cmd_tx: mpsc::Sender<(Command, oneshot::Sender<...>)>` to send commands
//! - `event_rx: mpsc::Receiver<OwnedEvent>` to receive unsolicited events
//!
//! ### Sketch
//!
//! ```ignore
//! pub async fn open(path: &Path) -> Result<Self> {
//!     let port = tokio_serial::new(path.to_string_lossy(), 115_200).open_native_async()?;
//!     let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(8);
//!     let (event_tx, event_rx) = tokio::sync::mpsc::channel(64);
//!     tokio::spawn(async move {
//!         let mut driver = Driver::new();
//!         loop { /* select! on cmd_rx, port reads; drive driver.poll() */ }
//!     });
//!     Ok(Self { cmd_tx, event_rx })
//! }
//! ```
//!
//! ### Hints
//!
//! - For each `async fn set_X` / `async fn X`, send `(Command, oneshot::Sender)`
//!   over `cmd_tx`, then `oneshot.recv().await`.
//! - Tokio's `AsyncReadExt::read` on the serial stream returns 0 bytes only
//!   on EOF; otherwise it yields whatever's available. Wrap reads in
//!   `tokio::select!` against `cmd_rx.recv()` so commands and incoming bytes
//!   don't starve each other.
//! - `Driver::poll`'s borrow lifetime ties to `&mut self`. To send an event
//!   over a channel, call `.to_owned()` first.

use rylr_core::{Command, Driver, OwnedEvent};
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("core: {0}")]
    Core(#[from] rylr_core::Error),
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

pub type Result<T> = std::result::Result<T, Error>;

pub struct AsyncRadio {
    // TODO: command sender + event receiver + JoinHandle.
}

impl AsyncRadio {
    pub async fn open(_path: &Path) -> Result<Self> {
        // TODO: open tokio_serial port, spawn task, return handle.
        let _ = (Command::Ping, Driver::new()); // silence warnings
        unimplemented!("AsyncRadio::open — exercise")
    }

    pub async fn ping(&mut self) -> Result<()> {
        unimplemented!("AsyncRadio::ping — exercise")
    }

    // TODO: mirror the rest of rylr_std::Radio's surface here as `async fn`s:
    //   address / set_address
    //   network_id / set_network_id
    //   band / set_band
    //   parameters / set_parameters
    //   crfop / uid / version
    //   factory_reset
    //   send
    //   next_event

    pub async fn next_event(&mut self) -> Result<OwnedEvent> {
        unimplemented!("AsyncRadio::next_event — exercise")
    }
}

#[cfg(test)]
mod tests {
    // TODO: write tests with a mock port (tokio's duplex pipe is good).
    // For now, this placeholder makes `cargo test` succeed.
    #[test]
    fn placeholder() {}
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p rylr-tokio && cargo test -p rylr-tokio
```

Expected: 1 passed (placeholder).

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "tokio: AsyncRadio scaffold (EXERCISE)"
```

---

## Task 18: `rylr-embassy` scaffold + `pico_smoke` example (EXERCISE)

**Files:**
- Modify: `rylr-embassy/src/lib.rs`
- Create: `rylr-embassy/memory.x`
- Modify: `rylr-embassy/Cargo.toml` (add `[[example]]`, `build` script)
- Create: `rylr-embassy/build.rs`
- Create: `rylr-embassy/.cargo/config.toml`
- Create: `rylr-embassy/examples/pico_smoke.rs`

- [ ] **Step 1: Add `memory.x` (RP2040 layout, copy from `pico-blink`)**

`rylr-embassy/memory.x`:

```text
MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}

SECTIONS {
    .boot2 ORIGIN(BOOT2) :
    {
        KEEP(*(.boot2));
    } > BOOT2
} INSERT BEFORE .text;
```

- [ ] **Step 2: Add `build.rs` to wire `memory.x` into the linker**

`rylr-embassy/build.rs`:

```rust
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
```

- [ ] **Step 3: Add cargo config for the cortex-m linker**

`rylr-embassy/.cargo/config.toml`:

```toml
[build]
target = "thumbv6m-none-eabi"

[target.thumbv6m-none-eabi]
rustflags = [
    "-C", "link-arg=--nmagic",
    "-C", "link-arg=-Tlink.x",
    "-C", "link-arg=-Tdefmt.x",
]
```

- [ ] **Step 4: Update `rylr-embassy/Cargo.toml` to register the build script and example**

Replace `rylr-embassy/Cargo.toml` with:

```toml
[package]
name = "rylr-embassy"
version.workspace = true
edition.workspace = true
build = "build.rs"

[lib]
test = false

[[example]]
name = "pico_smoke"
test = false

[dependencies]
rylr-core         = { workspace = true, features = ["defmt"] }
embedded-io-async = { workspace = true }
embassy-rp        = { workspace = true }
embassy-time      = { workspace = true }
embassy-executor  = { workspace = true }
defmt             = { workspace = true }
defmt-rtt         = { workspace = true }
panic-probe       = { workspace = true }
cortex-m          = { workspace = true }
cortex-m-rt       = { workspace = true }
```

- [ ] **Step 5: Write the `lib.rs` scaffold**

Replace `rylr-embassy/src/lib.rs` with:

```rust
#![no_std]
//! Embassy + RP2040 UART transport for `rylr-core`.
//!
//! ## EXERCISE
//!
//! Implement `Radio<UART>`. Recommended skeleton:
//!
//! ```ignore
//! pub struct Radio<'d, UART> {
//!     driver: rylr_core::Driver,
//!     uart: UART,
//! }
//!
//! impl<'d, UART> Radio<'d, UART>
//! where
//!     UART: embedded_io_async::Read + embedded_io_async::Write,
//! {
//!     pub fn new(uart: UART) -> Self { /* ... */ }
//!     pub async fn set_address(&mut self, n: u16) -> Result<(), Error<UART::Error>> { /* ... */ }
//!     // ...
//! }
//! ```
//!
//! ### Pump loop hints
//!
//! - In an `async` body, do this per command:
//!   1. `driver.submit(cmd)?` — encodes into TX buffer.
//!   2. Loop:
//!      - `driver.poll()`:
//!        - `Poll::NeedTx(b)` -> `uart.write_all(b).await?; driver.ack_tx(n);`
//!        - `Poll::Response(r)` -> match and return.
//!        - `Poll::Event(e)` -> stash into a small ring (no alloc) for
//!          a future `next_event` call.
//!        - `Poll::Idle` -> read bytes from UART into a stack buf with
//!          `embassy_time::with_timeout`, push via `driver.push_rx`.
//! - For the example binary's "listen" mode, you don't need a queue —
//!   just emit each `Event` via `defmt::info!` as it arrives.

use rylr_core::Driver;

#[derive(Debug)]
pub enum Error<E> {
    Core(rylr_core::Error),
    Io(E),
    Timeout,
    Radio(u8),
}

impl<E> From<rylr_core::Error> for Error<E> {
    fn from(e: rylr_core::Error) -> Self { Self::Core(e) }
}

pub struct Radio<UART> {
    _driver: Driver,
    _uart: UART,
}

impl<UART> Radio<UART> {
    pub fn new(uart: UART) -> Self {
        // TODO: hold the Driver and the UART; nothing else to do here.
        let _ = uart;
        unimplemented!("Radio::new — exercise")
    }
}

// TODO: impl block with embedded_io_async::{Read, Write} bounds and
//       async methods mirroring rylr_std::Radio. See module docs.
```

- [ ] **Step 6: Write the `pico_smoke` example skeleton**

`rylr-embassy/examples/pico_smoke.rs`:

```rust
//! Pico ↔ RYLR998 smoke test.
//!
//! ## EXERCISE
//!
//! Wire a RYLR998's TX → Pico GP1 (UART0 RX), RX → Pico GP0 (UART0 TX), GND-GND, 3V3-VDD.
//!
//! Then implement the `main()` body using `rylr_embassy::Radio<...>`:
//! 1. Init UART0 at 115200 baud on GP0/GP1.
//! 2. `radio.ping().await` → log result.
//! 3. `radio.set_address(5).await`.
//! 4. Loop: `radio.next_event(Duration::from_secs(60)).await` → `info!`.
//!
//! ### Boilerplate
//!
//! The cortex-m / embassy entrypoint scaffolding is below. You only need
//! to fill in the `// TODO:` block.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts, peripherals, uart};
use embassy_time::Duration;

use defmt_rtt as _;
use panic_probe as _;

bind_interrupts!(struct Irqs {
    UART0_IRQ => uart::InterruptHandler<peripherals::UART0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // TODO: configure UART0 on GP0 (TX) / GP1 (RX) at 115200 8N1, then
    //       construct rylr_embassy::Radio::new(...) and exercise it as
    //       described in the module docs above.
    let _ = (p, Irqs, Duration::from_secs(60));
    info!("rylr_embassy::pico_smoke — fill me in");
    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}
```

- [ ] **Step 7: Verify the embassy crate compiles**

```bash
cd /Users/nathanleniz/developer/embedded/rylr && cargo check -p rylr-embassy --target thumbv6m-none-eabi
cargo check -p rylr-embassy --example pico_smoke --target thumbv6m-none-eabi
```

Expected: both `Finished`.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "embassy: Radio<UART> scaffold + pico_smoke example skeleton (EXERCISE)"
```

---

## Task 19: Workspace `README.md`

**Files:**
- Create: `rylr/README.md`

- [ ] **Step 1: Write the README**

```markdown
# rylr

Rust workspace for working with REYAX RYLR998 LoRa modules.

## Crates

- `rylr-core` — sans-I/O state machine. `no_std`, no I/O traits, no clocks.
- `rylr-std` — blocking, USB serial, for Mac and Pi.
- `rylr-tool` — CLI binary built on `rylr-std`.
- `rylr-tokio` — async; scaffolded as an exercise.
- `rylr-embassy` — RP2040 + embassy UART; scaffolded as an exercise.

## Status

`rylr-tool` is functional once the user finishes the marked exercises:

1. `rylr-core/src/decode.rs` — line parser.
2. `rylr-core/src/driver.rs` `poll()` — state machine.
3. `rylr-std/src/radio.rs` `pump_until` — blocking read/write loop.

The `rylr-tokio` and `rylr-embassy` crates are scaffolds with detailed
TODOs. Implement them when needed.

## Build

```sh
cargo check --workspace --exclude rylr-embassy
cargo check -p rylr-embassy --target thumbv6m-none-eabi
cargo test --workspace --exclude rylr-embassy
cargo build -p rylr-tool --release
```

## Manual smoke test (two RYLR998s)

You will need two RYLR998 modules attached to two USB-UART adapters,
plugged into the same machine.

```sh
# Identify them:
ls /dev/cu.usbserial*

# Provision radio A as address 1, network 18:
./target/release/rylr-tool --port /dev/cu.usbserial-A1 provision --address 1 --net 18

# Provision radio B as address 2, network 18:
./target/release/rylr-tool --port /dev/cu.usbserial-B1 provision --address 2 --net 18

# Listen on B:
./target/release/rylr-tool --port /dev/cu.usbserial-B1 listen &

# Send from A:
./target/release/rylr-tool --port /dev/cu.usbserial-A1 send --to 2 hello
# Expected on the listen window:
# from=1 rssi=<n> snr=<n> "hello"
```

## Spec & plan

- Spec: `docs/superpowers/specs/2026-05-03-rylr-workspace-design.md`
- Plan: `docs/superpowers/plans/2026-05-03-rylr-workspace.md`
```

- [ ] **Step 2: Commit**

```bash
git add -A && git commit -m "docs: workspace README with smoke-test sequence"
```

---

## Done

End-state when this plan completes:

- `cargo check --workspace --exclude rylr-embassy` → green.
- `cargo check -p rylr-embassy --target thumbv6m-none-eabi` → green.
- `cargo test --workspace --exclude rylr-embassy` → green for everything *except* the marked exercise tests, which fail with `unimplemented!` panics. Those failures are the user's TODO list.
- `rylr-tool --help` runs.

User-implemented exercises remaining (in suggested order):

1. `rylr-core/src/decode.rs` — `parse_response` / `parse_event` (passes Task 6 fixture tests).
2. `rylr-core/src/driver.rs` `poll()` (passes Task 7 sequence tests).
3. `rylr-std/src/radio.rs` `pump_until` (passes Task 13 integration tests).
4. `rylr-tokio/src/lib.rs` — `AsyncRadio` impl.
5. `rylr-embassy/src/lib.rs` + `examples/pico_smoke.rs` — embassy UART glue.
