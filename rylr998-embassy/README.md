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

> **Hardware reminder.** The RYLR998 is **not** 5 V tolerant. Wire VDD to
> 3V3 and use a 3.3 V MCU (the Pico is fine). The factory baud is
> 115 200 8N1 — configure your UART to match (see
> `rylr998_core::BAUD`). The [workspace README](https://github.com/terakilobyte/rylr998-rs)
> has the full wiring story.

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

    let mut radio_config = uart::Config::default();
    radio_config.baudrate = 115_200; // the RYLR998 factory default

    let uart = uart::BufferedUart::new(
        p.UART1, p.PIN_8, p.PIN_9, Irqs, tx_buf, rx_buf, radio_config,
    );
    let mut radio = rylr998_embassy::Radio::new(uart);

    radio.set_address(5).await.unwrap();

    loop {
        let _ = radio.next_event(embassy_time::Duration::from_secs(60), |e: Event<'_>| {
            // log e via defmt, or copy fields out into your own state
            None::<()>
        }).await;
    }
}
```

## Method signatures

`E` below is the wrapped UART's `embedded_io_async::ErrorType::Error`.
All command methods take `&mut self` and have a per-call deadline (1 s,
or 2 s for `factory_reset`); a missing response yields `Error::Timeout`
and a `+ERR=<code>` reply yields `Error::Radio(code)`.

```rust,ignore
impl<UART> Radio<UART>
where
    UART: embedded_io_async::Read + embedded_io_async::Write,
{
    pub fn new(uart: UART) -> Self;                    // caller configures baud

    pub async fn ping(&mut self) -> Result<(), Error<UART::Error>>;
    pub async fn factory_reset(&mut self) -> Result<(), Error<UART::Error>>; // 2 s

    pub async fn set_address(&mut self, n: u16) -> Result<(), Error<UART::Error>>;
    pub async fn address(&mut self) -> Result<u16, Error<UART::Error>>;
    pub async fn set_network_id(&mut self, n: u8) -> Result<(), Error<UART::Error>>;
    pub async fn network_id(&mut self) -> Result<u8, Error<UART::Error>>;
    pub async fn set_band(&mut self, hz: u32) -> Result<(), Error<UART::Error>>;
    pub async fn band(&mut self) -> Result<u32, Error<UART::Error>>;
    pub async fn set_cpin(&mut self, password: &[u8]) -> Result<(), Error<UART::Error>>;
    pub async fn cpin(&mut self) -> Result<heapless::String<8>, Error<UART::Error>>;
    pub async fn set_parameters(&mut self, p: RfParams) -> Result<(), Error<UART::Error>>;
    pub async fn parameters(&mut self) -> Result<RfParams, Error<UART::Error>>;
    pub async fn crfop(&mut self) -> Result<u8, Error<UART::Error>>;

    pub async fn send(&mut self, to: u16, data: &[u8]) -> Result<(), Error<UART::Error>>;

    pub async fn next_event<F, R>(
        &mut self,
        timeout: Duration,
        handler: F,
    ) -> Result<R, Error<UART::Error>>
    where
        F: FnMut(Event<'_>) -> Option<R>;
}
```

`uid` / `version` are omitted in this crate; their `&str` payload would
require an `alloc`-backed `String` to return owned. If you need them,
drop to `rylr998-core` directly and read `Response::Uid` / `Response::Version`
from a `Poll::Response`.

## Concurrency

`Radio<UART>` is single-owner — methods take `&mut self`, there is no
internal task, and the driver only consumes UART bytes while you're
awaiting one of its methods. That has one consequence worth flagging
loudly:

- **Events during a command are dropped.** While a command method is
  awaiting its response, any `+RCV` arriving on the wire is logged via
  `defmt::info!` and discarded. There is no internal queue.
- **To capture events, sit inside `next_event`.** Treat it as the steady
  state: hand it a closure, copy what you need from `Event<'_>` (the
  slice borrows the driver's line buffer and won't survive the next
  poll), and run commands only between `next_event` sessions.

If you need fanout, build it in your closure: forward each event onto an
`embassy_sync::channel::Channel` and consume from elsewhere.

## Errors

| Variant | Cause |
|---|---|
| `Error::Core(_)`     | Protocol error from `rylr998-core` |
| `Error::Io(E)`       | UART error from the wrapped `embedded_io_async` impl |
| `Error::Timeout`     | Per-call deadline elapsed without a response |
| `Error::Radio(code)` | Radio replied `+ERR=<code>`; see the workspace README's +ERR table |

## CPIN

`set_cpin` accepts exactly 8 ASCII hex bytes in the documented `00000001`
through `FFFFFFFF` range. Invalid CPIN length is reported by the radio as
`Error::Radio(5)`, which maps to `RadioError::DataLengthMismatch`.
`cpin` returns a `heapless::String<8>` containing that password, or an
empty buffer when the module reports `No Password!`. The manual's `,M`
memory flag is not exposed here because tested RYLR998 hardware rejects it
with `+ERR=5`.

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
