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
