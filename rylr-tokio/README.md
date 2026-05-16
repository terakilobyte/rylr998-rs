# rylr998-tokio

Async host driver for the REYAX **RYLR998** LoRa radio module, built on
[Tokio](https://tokio.rs) and `tokio-serial`.

A background task owns the serial port and the
[`rylr998-core`](https://crates.io/crates/rylr998-core) state machine.
The `AsyncRadio` handle ships commands over an `mpsc` channel and awaits
each reply via a `oneshot`. Unsolicited events stream over a separate
channel — pull them with [`AsyncRadio::next_event`].

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

## API surface

`AsyncRadio::open` / `AsyncRadio::from_port`, plus the AT-command methods:

`ping`, `set_address` / `address`, `set_network_id` / `network_id`,
`set_band` / `band`, `set_parameters` / `parameters`, `crfop`, `uid`,
`version`, `factory_reset`, `send`, and `next_event`.

`from_port` accepts any `AsyncRead + AsyncWrite + Send + Unpin + 'static`,
which makes the crate easy to unit-test against `tokio::io::duplex`.

## Related crates

- [`rylr998-core`](https://crates.io/crates/rylr998-core) — pure protocol logic.
- [`rylr998-std`](https://crates.io/crates/rylr998-std) — blocking equivalent of this crate.
- [`rylr998-embassy`](https://crates.io/crates/rylr998-embassy) — `no_std` embedded variant.

## License

Dual-licensed: MIT or Apache-2.0, at your option.
