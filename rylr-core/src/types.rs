//! Public types for the RYLR protocol. No I/O, no clocks.

use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RfParams {
    pub sf: u8,
    pub bw: u8,
    pub cr: u8,
    pub preamble: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> {
    Ping,
    GetAddress,
    SetAddress(u16),
    GetNetworkId,
    SetNetworkId(u8),
    GetBand,
    SetBand(u32),
    GetParameters,
    SetParameters(RfParams),
    GetCrfop,
    GetUid,
    GetVersion,
    FactoryReset,
    Send { to: u16, data: &'a [u8] },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response<'a> {
    Ok,
    Err(u8),
    Address(u16),
    NetworkId(u8),
    Band(u32),
    Parameters(RfParams),
    Crfop(u8),
    Uid(&'a str),
    Version(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event<'a> {
    Recv { from: u16, data: &'a [u8], rssi: i16, snr: i16 },
    Ready,
}

#[derive(Debug)]
pub enum Poll<'a> {
    Idle,
    NeedTx(&'a [u8]),
    Response(Response<'a>),
    Event(Event<'a>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Busy,
    TxOverflow,
    RxOverflow,
    Parse,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("a command is already in flight"),
            Self::TxOverflow => f.write_str("encoded command exceeds TX buffer"),
            Self::RxOverflow => f.write_str("RX buffer is full"),
            Self::Parse => f.write_str("could not parse line from radio"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(feature = "defmt")]
impl defmt::Format for Error {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            Self::Busy => defmt::write!(fmt, "Busy"),
            Self::TxOverflow => defmt::write!(fmt, "TxOverflow"),
            Self::RxOverflow => defmt::write!(fmt, "RxOverflow"),
            Self::Parse => defmt::write!(fmt, "Parse"),
        }
    }
}

#[cfg(feature = "alloc")]
pub use owned::OwnedEvent;

#[cfg(feature = "alloc")]
mod owned {
    use super::Event;
    use alloc::vec::Vec;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum OwnedEvent {
        Recv { from: u16, data: Vec<u8>, rssi: i16, snr: i16 },
        Ready,
    }

    impl<'a> Event<'a> {
        #[must_use]
        pub fn into_owned(self) -> OwnedEvent {
            match self {
                Event::Recv { from, data, rssi, snr } => OwnedEvent::Recv {
                    from,
                    data: data.to_vec(),
                    rssi,
                    snr,
                },
                Event::Ready => OwnedEvent::Ready,
            }
        }
    }
}

#[cfg(all(test, feature = "alloc"))]
mod owned_tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn recv_into_owned_copies_data() {
        let bytes = [0xDE, 0xAD, 0xBE, 0xEF];
        let ev = Event::Recv { from: 5, data: &bytes, rssi: -42, snr: 8 };
        let owned = ev.into_owned();
        match owned {
            OwnedEvent::Recv { from, data, rssi, snr } => {
                assert_eq!(from, 5);
                assert_eq!(data, vec![0xDE, 0xAD, 0xBE, 0xEF]);
                assert_eq!(rssi, -42);
                assert_eq!(snr, 8);
            }
            _ => panic!("expected Recv"),
        }
    }

    #[test]
    fn ready_into_owned() {
        assert!(matches!(Event::Ready.into_owned(), OwnedEvent::Ready));
    }
}
