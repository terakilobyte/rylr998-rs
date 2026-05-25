# rylr998-rs

A complete Rust stack for the REYAX **RYLR998** LoRa radio module —
hardware-agnostic protocol layer, blocking/async host drivers, a `no_std`
embassy driver, and a CLI. All five crates share `rylr998-core`'s state
machine, so the wire-format logic is written and tested in one place.

## Before you wire anything

Two hardware gotchas cost a module if you get them wrong:

- **3.3 V, not 5 V.** The RYLR998 is **not** 5 V tolerant on VDD or on
  its UART. Power it from 3V3, and use a level shifter (or a 3.3 V MCU)
  on TX/RX. Plugging a 5 V Arduino directly into one will fry it.
- **115 200 baud, 8N1, no flow control.** That's the factory default and
  every crate in this workspace opens the port at that rate (see
  `rylr998_core::BAUD`). If you've changed it with `AT+IPR=…`,
  reconfigure your UART to match — there's no auto-detect.

UART wires cross: MCU TX → radio RX, MCU RX → radio TX, common GND.

## Crates

| Crate | What it is |
|---|---|
| [`rylr998-core`](rylr998-core)       | `no_std`, sans-I/O state machine. Encodes `AT+…` commands, parses response and event lines, exposes a `Driver` you feed bytes to and pull events from. |
| [`rylr998-std`](rylr998-std)         | Blocking host driver over `serialport`. The "talk to a USB-connected radio from your laptop" crate. |
| [`rylr998-tokio`](rylr998-tokio)     | Async host driver over `tokio-serial`. A background task owns the protocol; the handle is `Send` + cheap to share. |
| [`rylr998-embassy`](rylr998-embassy) | `no_std` driver over `embedded-io-async`. Works on RP2040 / RP2350 / anything with an async UART (Embassy, etc.). |
| [`rylr998`](rylr998-tool)            | CLI built on `rylr998-std`. `rylr998 info`, `rylr998 send --to N "msg"`, `rylr998 listen`, etc. |

## The concurrency contract

Every host-driver `Radio` (and `AsyncRadio`) is a **single-owner** handle.
Methods take `&mut self`, the UART is owned, and exactly one logical
actor drives the radio at a time. There are two modes to be in:

- **Command mode** — call methods (`set_address`, `send`, `address`, …).
  Each submits one `AT+…` line and blocks/awaits the matching
  `+OK` / `+ERR` / value response, with a per-call deadline (1 s for
  everything except `factory_reset`, which uses 2 s in async and 4 s
  blocking).
- **Listen mode** — call `next_event` to receive unsolicited `+RCV` /
  `+READY` messages.

`+RCV` can arrive whenever a peer transmits, including while a command
is in flight. Each crate handles that overlap differently — this is the
one non-obvious thing to know up front:

| Crate | Events received while a command is in flight |
|---|---|
| `rylr998-std`     | Buffered in an internal `VecDeque<OwnedEvent>` and surfaced FIFO on the next `next_event` call. Nothing is dropped. |
| `rylr998-tokio`   | Pushed through an `mpsc::Sender` (capacity 64) from the background task. Pull via `next_event`. Drops only if the channel fills and no one drains. |
| `rylr998-embassy` | **Dropped** — logged via `defmt::info!` then discarded. To capture events, spend your time inside `next_event` and command between event-receive sessions. |

In all three: there is exactly one consumer. Don't clone-and-share an
`AsyncRadio` across tasks — that's not the API. If you need multi-consumer
fanout, build it on top of `next_event`.

The protocol layer itself is single-in-flight: `Driver::submit` returns
`Err(Error::Busy)` if you start a second command before the first
resolves. In normal use the host crates' methods don't return until their
in-flight slot is clear, so this only bites if you're driving
`rylr998-core::Driver` directly.

## Library usage

```rust,no_run
// Blocking (rylr998-std):
let mut radio = rylr998_std::Radio::open_auto()?;
radio.set_address(5)?;
radio.set_network_id(18)?;
radio.send(2, b"hello")?;
let event = radio.next_event(std::time::Duration::from_secs(60))?;
# Ok::<(), rylr998_std::Error>(())
```

```rust,no_run
// Async (rylr998-tokio):
# async fn run(path: &std::path::Path) -> Result<(), rylr998_tokio::Error> {
let mut radio = rylr998_tokio::AsyncRadio::open(path).await?;
radio.set_address(5).await?;
radio.send(2, b"hello").await?;
let event = radio.next_event(std::time::Duration::from_secs(60)).await?;
# Ok(())
# }
```

```rust,ignore
// Embedded (rylr998-embassy, no_std):
let mut radio = rylr998_embassy::Radio::new(uart);
radio.set_address(5).await?;
radio.send(2, b"hello").await?;
radio.next_event(Duration::from_secs(60), |e| {
    // copy out what you need; `e` borrows the driver's line buffer
    None::<()>
}).await.ok();
```

Full method signatures, return types, and per-crate error semantics are
in the per-crate READMEs:

- [`rylr998-core`](rylr998-core) — `Driver`, `Command`, `Response`, `Event`, `Poll`.
- [`rylr998-std`](rylr998-std) — blocking `Radio`.
- [`rylr998-tokio`](rylr998-tokio) — async `AsyncRadio`.
- [`rylr998-embassy`](rylr998-embassy) — `no_std` `Radio<UART>`.

## Radio error codes

Failed commands resolve to `Error::Radio(code)` in the host crates (or
`Response::Err(code)` at the core layer). `rylr998_core::RadioError::from_code`
maps the documented codes to enum variants:

| Code | Variant | Description |
|---:|---|---|
|  1 | `MissingLineTerminator` | missing line terminator |
|  2 | `InvalidAtPrefix`       | command does not start with `AT` |
|  4 | `UnknownCommand`        | unknown command |
|  5 | `DataLengthMismatch`    | data length mismatch (also: wrong CPIN length, the `AT+CPIN=…,M` form) |
| 10 | `TxTimeout`             | TX timed out |
| 12 | `Crc`                   | CRC error |
| 13 | `TxDataTooLong`         | TX data exceeds 240 bytes |
| 14 | `FlashWriteFailed`      | failed to write flash memory |
| 15 | `UnknownFailure`        | unknown failure |
| 17 | `LastTxNotCompleted`    | last TX was not completed |
| 18 | `InvalidPreamble`       | preamble value is not allowed |
| 19 | `RxHeader`              | RX failed, header error |
| 20 | `InvalidSmartReceivingPowerSavingTime` | smart-receive power-save time invalid |

Codes outside this table return `None` from `RadioError::from_code` but
are still surfaced as `Error::Radio(n)` — match on the raw `n` if you
need to handle one.

## CLI

There's also a `rylr998` binary on crates.io:

```sh
cargo install rylr998

rylr998 info                                   # auto-discover; dump config
rylr998 --port /dev/cu.usbserial-A1 provision --address 1 --net 18
rylr998 --port /dev/cu.usbserial-B1 listen
rylr998 --port /dev/cu.usbserial-A1 send --to 2 "hello world"
```

See [`rylr998-tool`](rylr998-tool) for the full command list.

## Build matrix

```sh
# host crates
cargo test --workspace --exclude rylr998-embassy

# embedded crate (RP2350 by default; switch target for RP2040)
cd rylr998-embassy && cargo check --example pico_smoke
```

## License

Licensed under either of:

- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this project by you shall be dual-licensed as
above, without any additional terms or conditions.
