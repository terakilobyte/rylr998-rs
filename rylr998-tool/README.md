# rylr998

Command-line tool for the REYAX **RYLR998** LoRa radio module.

## Install

```sh
cargo install rylr998
```

## Usage

```text
rylr998 [--port /dev/cu.usbserial-X] <command>

Commands:
  info        Print all readable settings
  provision   Set address + network (and optionally cpin, band / RF params)
  send        Transmit bytes to a destination address
  listen      Print incoming +RCV events as they arrive
  reset       Issue a factory reset (AT+RESET)
```

Auto-discovers `/dev/cu.usbserial*` on macOS if `--port` isn't supplied.

## Example

```sh
# provision two radios on net 18
rylr998 --port /dev/cu.usbserial-A1 provision --address 1 --net 18
rylr998 --port /dev/cu.usbserial-B1 provision --address 2 --net 18

# listen on one terminal
rylr998 --port /dev/cu.usbserial-B1 listen

# send to it from another
rylr998 --port /dev/cu.usbserial-A1 send --to 2 "hello world"
# listener prints:
# from=1 rssi=-12 snr=10 "hello world"
```

## Built on

- [`rylr998-std`](https://crates.io/crates/rylr998-std) — blocking driver.
- [`rylr998-core`](https://crates.io/crates/rylr998-core) — protocol layer.

## License

Dual-licensed: MIT or Apache-2.0, at your option.
