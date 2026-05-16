# rylr998-embassy

`no_std` driver for the REYAX **RYLR998** LoRa radio module, built on
[Embassy](https://embassy.dev/) and the
[`embedded-io-async`](https://crates.io/crates/embedded-io-async) traits.

Hand in any UART that implements `embedded_io_async::Read + Write` and you
get a `Radio<UART>` with the same surface as the host crates — but it runs
on a Cortex-M target with no allocator.

Validated on Raspberry Pi Pico 2 (RP2350) + a real RYLR998 module. The
provided `examples/pico_smoke.rs` flashes via probe-rs, brings up the
radio over UART1, and reports activity over either RTT or a UART console
forwarded through picoprobe.

## Example

```rust,ignore
use embassy_rp::{bind_interrupts, peripherals, uart};
use rylr998_core::Event;

bind_interrupts!(struct Irqs {
    UART1_IRQ => uart::BufferedInterruptHandler<peripherals::UART1>;
});

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());

    let tx_buf: &'static mut [u8; 256] = cortex_m::singleton!(: [u8; 256] = [0; 256]).unwrap();
    let rx_buf: &'static mut [u8; 256] = cortex_m::singleton!(: [u8; 256] = [0; 256]).unwrap();

    let uart = uart::BufferedUart::new(
        p.UART1, p.PIN_8, p.PIN_9, Irqs, tx_buf, rx_buf, uart::Config::default(),
    );
    let mut radio = rylr998_embassy::Radio::new(uart);

    radio.set_address(5).await.unwrap();

    loop {
        let _ = radio.next_event(embassy_time::Duration::from_secs(60), |e: Event<'_>| {
            // log e via defmt or your channel of choice
            None::<()>
        }).await;
    }
}
```

## API surface

- `Radio::new(uart)` — wrap any `embedded_io_async::Read + Write` UART.
- AT commands: `ping`, `set_address` / `address`, `set_network_id` /
  `network_id`, `set_band` / `band`, `set_parameters` / `parameters`,
  `crfop`, `factory_reset`, `send`.
- `next_event<F, R>(timeout, handler)` — runs a callback for each
  unsolicited event arriving on the wire; returns when the callback yields
  `Some(_)` or the timeout fires.

`uid` / `version` are omitted in this crate; their `&str` payload would
require an `alloc`-backed `String` to return owned. If you need them, drop
to `rylr998-core` directly.

## Memory layout (RP2350)

The example uses a hand-written `memory.x` modeled on rp-hal's RP2350
layout: vector table first, then a `.start_block` with the boot ROM's
`IMAGE_DEF` (configured as a **secure** EXE), then `.text`, then a
`.end_block` at the end.

## Related crates

- [`rylr998-core`](https://crates.io/crates/rylr998-core) — pure protocol logic.
- [`rylr998-std`](https://crates.io/crates/rylr998-std) — blocking host driver.
- [`rylr998-tokio`](https://crates.io/crates/rylr998-tokio) — async host driver.

## License

Dual-licensed: MIT or Apache-2.0, at your option.
