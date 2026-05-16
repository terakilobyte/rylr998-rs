# rylr998-rs

A complete Rust stack for the REYAX **RYLR998** LoRa radio module, from a
hardware-agnostic protocol layer up to a CLI you can drive from your shell.

## Crates

| Crate | What it is |
|---|---|
| [`rylr998-core`](rylr-core)       | `no_std`, sans-I/O state machine. Encodes `AT+…` commands, parses response and event lines, exposes a `Driver` you feed bytes to and pull events from. |
| [`rylr998-std`](rylr-std)         | Blocking host driver over `serialport`. The "talk to a USB-connected radio from your laptop" crate. |
| [`rylr998-tokio`](rylr-tokio)     | Async host driver over `tokio-serial`. A background task owns the protocol; the handle is `Send` + cheap to share. |
| [`rylr998-embassy`](rylr-embassy) | `no_std` driver over `embedded-io-async`. Works on RP2040 / RP2350 / anything with an async UART (Embassy, etc.). |
| [`rylr998`](rylr-tool)            | The CLI front-end. `rylr998 info`, `rylr998 send --to N "msg"`, `rylr998 listen`, etc. |

All five share `rylr998-core`'s state machine, so the wire-format logic is
written and tested in exactly one place.

## Quick start

```sh
# install the CLI
cargo install rylr998

# auto-find your USB serial radio and dump its config
rylr998 info

# provision two radios
rylr998 --port /dev/cu.usbserial-A1 provision --address 1 --net 18
rylr998 --port /dev/cu.usbserial-B1 provision --address 2 --net 18

# listen on one terminal:
rylr998 --port /dev/cu.usbserial-B1 listen

# in another terminal, send to it:
rylr998 --port /dev/cu.usbserial-A1 send --to 2 "hello world"
```

## Using as a library

```rust
// Blocking (rylr998-std):
let mut radio = rylr998_std::Radio::open_auto()?;
radio.set_address(5)?;
radio.send(2, b"hello")?;

// Async (rylr998-tokio):
let mut radio = rylr998_tokio::AsyncRadio::open(&path).await?;
radio.set_address(5).await?;
radio.send(2, b"hello").await?;

// Embedded (rylr998-embassy, no_std):
let mut radio = rylr998_embassy::Radio::new(uart);
radio.set_address(5).await?;
radio.send(2, b"hello").await?;
```

## Build matrix

```sh
# host crates
cargo test --workspace --exclude rylr998-embassy

# embedded crate (RP2350 by default; switch target for RP2040)
cd rylr-embassy && cargo check --example pico_smoke
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
