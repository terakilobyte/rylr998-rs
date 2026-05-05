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
