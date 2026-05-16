#![cfg_attr(not(feature = "std"), no_std)]
//! Hardware-agnostic protocol layer for the REYAX **RYLR998** LoRa radio
//! module.
//!
//! `rylr998-core` is pure logic — encoding `AT+…` command lines, parsing
//! response and event lines, and driving the state machine that bridges the
//! two. It does **no** I/O and assumes nothing about the runtime: works on
//! `no_std` MCUs, with `alloc`, or under full `std`.
//!
//! The crate is consumed by the I/O-bearing wrappers:
//!
//! - [`rylr998-std`](https://crates.io/crates/rylr998-std) — blocking host
//!   driver over `serialport`.
//! - [`rylr998-tokio`](https://crates.io/crates/rylr998-tokio) — async host
//!   driver over `tokio-serial`.
//! - [`rylr998-embassy`](https://crates.io/crates/rylr998-embassy) — `no_std`
//!   embassy driver over any `embedded-io-async` UART.
//!
//! ## Core types
//!
//! - [`Driver`] — the state machine. Push raw RX bytes in, pull
//!   [`Poll`] events out. Submit `AT+…` commands via [`Command`].
//! - [`Command`] / [`Response`] / [`Event`] — protocol types. Borrow from
//!   internal buffers; use [`OwnedEvent`] when you need to outlive a poll.
//! - [`encode`] / [`parse_response`] / [`parse_event`] — the pure parsers
//!   and encoder, exposed in case you want to skip the state machine.
//!
//! ## Features
//!
//! - `alloc` — exposes [`OwnedEvent`] (a heap-backed copy of [`Event`]).
//! - `std` — implies `alloc`; reserved for std-only conveniences.
//! - `defmt` — implements `defmt::Format` on public error types for
//!   nicer embedded logging.

#[cfg(feature = "alloc")]
extern crate alloc;

mod types;
pub use types::{Command, Error, Event, Poll, Response, RfParams};

mod encode;
pub use encode::encode;

mod decode;
pub use decode::{parse_event, parse_response};

mod driver;
pub use driver::Driver;

#[cfg(feature = "alloc")]
pub use types::OwnedEvent;

/// Default UART baud rate of a RYLR998 in its factory configuration.
pub const BAUD: u32 = 115_200;
