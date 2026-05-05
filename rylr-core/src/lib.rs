#![cfg_attr(not(feature = "alloc"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod types;
pub use types::{Command, Error, Event, Poll, Response, RfParams};

mod encode;
pub use encode::encode;

#[cfg(feature = "alloc")]
pub use types::OwnedEvent;
