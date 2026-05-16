# rylr998-std

Blocking host driver for the REYAX **RYLR998** LoRa radio module, built on
[`serialport`](https://crates.io/crates/serialport).

Auto-detects `/dev/cu.usbserial*` on macOS, opens at 115200 8N1, drives the
[`rylr998-core`](https://crates.io/crates/rylr998-core) state machine, and
gives you a simple synchronous API.

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

## API surface

`Radio::open` / `Radio::open_auto` / `Radio::from_port`, plus the AT
commands:

`ping`, `set_address` / `address`, `set_network_id` / `network_id`,
`set_band` / `band`, `set_parameters` / `parameters`, `crfop`, `uid`,
`version`, `factory_reset`, `send`, and `next_event`.

## Related crates

- [`rylr998-core`](https://crates.io/crates/rylr998-core) — pure protocol logic.
- [`rylr998-tokio`](https://crates.io/crates/rylr998-tokio) — async equivalent of this crate.
- [`rylr998-embassy`](https://crates.io/crates/rylr998-embassy) — `no_std` embedded variant.
- [`rylr998`](https://crates.io/crates/rylr998) — CLI that uses this crate.

## License

Dual-licensed: MIT or Apache-2.0, at your option.
