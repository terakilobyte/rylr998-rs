# rylr998-core

Hardware-agnostic protocol driver for the REYAX **RYLR998** LoRa radio
module.

This crate is pure logic: it encodes `AT+…` command lines, parses response
and event lines, and drives the state machine that pairs commands with
their replies. It does **no I/O** and assumes nothing about the runtime —
works under `no_std`, with `alloc`, or under full `std`.

If you want a driver you can actually plug into a serial port, pick one
of the I/O-bearing crates that consume `rylr998-core`:

- [`rylr998-std`](https://crates.io/crates/rylr998-std) — blocking host driver.
- [`rylr998-tokio`](https://crates.io/crates/rylr998-tokio) — async host driver.
- [`rylr998-embassy`](https://crates.io/crates/rylr998-embassy) — `no_std` embedded driver.

## Example

```rust
use rylr998_core::{Command, Driver, Poll, Response};

let mut d = Driver::new();
d.submit(Command::SetAddress(5)).unwrap();

// Drain the encoder; in a real driver you'd write these bytes to a UART.
while let Poll::NeedTx(bytes) = d.poll() {
    // tx.write_all(bytes)?;
    let n = bytes.len();
    d.ack_tx(n);
}

// Imagine the radio replied with "+OK\r\n":
d.push_rx(b"+OK\r\n").unwrap();
assert!(matches!(d.poll(), Poll::Response(Response::Ok)));
```

## Driver contract

`Driver` is single-in-flight: `submit` returns `Err(Error::Busy)` if a
prior command hasn't yet resolved to a `Response`. Typical loop:

1. `submit(Command::…)` once.
2. Pump `poll()` in a loop: write any `Poll::NeedTx(bytes)` to the wire
   then `ack_tx(n)`, surface `Poll::Response` to your caller, and forward
   `Poll::Event` somewhere durable (the inner `Event<'_>` borrows the
   driver's line buffer and is invalidated on the next `poll`). Stop on
   `Poll::Idle`.
3. Feed RX bytes back in with `push_rx`, then resume polling.

Events can arrive unsolicited between commands — drive `poll()` even
when no command is in flight if you care about `+RCV`. The I/O-bearing
host crates do this for you.

## CPIN Behavior

`Command::SetCpin` encodes `AT+CPIN=<password>` and requires the password
to be exactly 8 ASCII hex characters in the documented `00000001` through
`FFFFFFFF` range. The radio reports invalid CPIN length as `+ERR=5`, which
maps to `RadioError::DataLengthMismatch`.

The manual also documents `AT+CPIN=<password>,M`, but tested RYLR998
hardware rejects that form with `+ERR=5`; this crate intentionally exposes
only the working `AT+CPIN=<password>` setter.

`Response::Cpin("")` represents the radio's `+CPIN=No Password!` reply.
Any non-empty `Response::Cpin` value is an 8-character password borrowed
from the driver's line buffer.

## Radio Error Codes

`Response::Err(code)` carries the raw `+ERR=<code>` value. Use
`RadioError::from_code(code)` to map known manual codes to stable enum
variants and descriptions. The full code table lives in the
[workspace README](https://github.com/terakilobyte/rylr998-rs#radio-error-codes).

## Features

- `alloc` — exposes `OwnedEvent`, a heap-backed variant of `Event` you can
  store across poll calls.
- `std` — implies `alloc`; reserved for std-only conveniences.
- `defmt` — `defmt::Format` impls on public error types for embedded logging.

## License

Dual-licensed: MIT or Apache-2.0, at your option.
