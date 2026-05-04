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
