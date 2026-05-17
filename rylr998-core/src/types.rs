//! Public types for the RYLR protocol. No I/O, no clocks.

use core::fmt;

/// LoRa physical-layer parameters as accepted by `AT+PARAMETER`.
///
/// The four values correspond to the radio's spreading factor, signal
/// bandwidth, coding rate, and preamble length. Refer to the REYAX
/// RYLR998 AT command manual for the encoding of each field and the
/// allowed value ranges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RfParams {
    /// Spreading factor (SF). Higher values trade throughput for range.
    pub sf: u8,
    /// Signal bandwidth (BW), encoded as the radio expects (not in Hz).
    pub bw: u8,
    /// Forward-error-correction coding rate (CR).
    pub cr: u8,
    /// Preamble length in symbols.
    pub preamble: u8,
}

/// A command to submit to a [`crate::Driver`].
///
/// `Command` is the input vocabulary of the state machine: every variant
/// maps to a single `AT+…` line on the wire. Submit one with
/// [`Driver::submit`](crate::Driver::submit); the driver encodes it,
/// emits the bytes via [`Poll::NeedTx`], and resolves the matching
/// [`Response`] when the radio replies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> {
    /// `AT` — liveness probe. Expects `+OK`.
    Ping,
    /// `AT+ADDRESS?` — query this node's address.
    GetAddress,
    /// `AT+ADDRESS=<n>` — set this node's address (`0..=65535`).
    SetAddress(u16),
    /// `AT+NETWORKID?` — query the network ID.
    GetNetworkId,
    /// `AT+NETWORKID=<n>` — set the network ID.
    SetNetworkId(u8),
    /// `AT+BAND?` — query the carrier frequency in Hz.
    GetBand,
    /// `AT+BAND=<hz>` — set the carrier frequency, in Hz.
    SetBand(u32),
    /// `AT+CPIN?` — query the 8-character domain password.
    GetCpin,
    /// `AT+CPIN=<password>` — set the 8-character domain password.
    ///
    /// The radio replies with `+ERR=5` if the password length is invalid.
    /// Valid passwords are in the documented `00000001` through `FFFFFFFF`
    /// range.
    SetCpin(&'a [u8]),
    /// `AT+PARAMETER?` — query the LoRa PHY parameters.
    GetParameters,
    /// `AT+PARAMETER=<sf>,<bw>,<cr>,<preamble>` — set LoRa PHY parameters.
    SetParameters(RfParams),
    /// `AT+CRFOP?` — query the configured RF output power.
    GetCrfop,
    /// `AT+UID?` — query the module's unique ID.
    GetUid,
    /// `AT+VER?` — query the firmware version string.
    GetVersion,
    /// `AT+RESET` — factory reset. The radio emits an intermediate
    /// `+RESET` followed by `+READY`; the driver swallows the `+RESET`
    /// and resolves the command on `+READY`.
    FactoryReset,
    /// `AT+SEND=<to>,<len>,<data>` — transmit `data` to the given address.
    /// The payload is borrowed: it must outlive [`Driver::submit`](crate::Driver::submit).
    Send {
        /// Destination address (`0..=65535`; `0` broadcasts).
        to: u16,
        /// Raw payload bytes. May contain commas — `len` disambiguates.
        data: &'a [u8],
    },
}

/// A solicited reply to a previously submitted [`Command`].
///
/// String variants (`Uid`, `Version`) borrow from the driver's internal
/// line buffer and are valid only until the next call to
/// [`Driver::poll`](crate::Driver::poll). Copy them out if you need to
/// hold onto them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response<'a> {
    /// `+OK` — the command was accepted (and, for `AT+RESET`, has
    /// finished).
    Ok,
    /// `+ERR=<code>` — the radio rejected the command. Use
    /// [`RadioError::from_code`] to map known manual codes to descriptions.
    Err(u8),
    /// Reply to [`Command::GetAddress`].
    Address(u16),
    /// Reply to [`Command::GetNetworkId`].
    NetworkId(u8),
    /// Reply to [`Command::GetBand`].
    Band(u32),
    /// Reply to [`Command::GetParameters`].
    Parameters(RfParams),
    /// Reply to [`Command::GetCrfop`].
    Crfop(u8),
    /// Reply to [`Command::GetUid`]. Borrowed from the driver's line buffer.
    Uid(&'a str),
    /// Reply to [`Command::GetVersion`]. Borrowed from the driver's line buffer.
    Version(&'a str),
    /// Reply to [`Command::GetCpin`]. Empty means `No Password!`; otherwise
    /// this is an 8-character password borrowed from the driver's line buffer.
    Cpin(&'a str),
}

/// An unsolicited message from the radio.
///
/// Unlike a [`Response`], events can arrive at any time — even while a
/// command is in flight. The `data` slice in [`Event::Recv`] borrows from
/// the driver's line buffer and is invalidated on the next
/// [`Driver::poll`](crate::Driver::poll); use
/// [`Event::into_owned`] (requires the `alloc` feature) to copy it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event<'a> {
    /// `+RCV=<from>,<len>,<data>,<rssi>,<snr>` — payload received over the air.
    Recv {
        /// Address of the sending node.
        from: u16,
        /// Raw payload bytes (length is `len` from the wire framing).
        data: &'a [u8],
        /// Received signal strength indicator, in dBm.
        rssi: i16,
        /// Signal-to-noise ratio.
        snr: i16,
    },
    /// `+READY` — the radio is ready. Emitted at boot and after
    /// `AT+RESET` completes.
    Ready,
}

/// The result of one [`Driver::poll`](crate::Driver::poll) step.
///
/// Drive the radio by polling in a loop and dispatching on the variant:
/// shove [`NeedTx`](Poll::NeedTx) bytes onto the wire (then call
/// [`Driver::ack_tx`](crate::Driver::ack_tx)), surface
/// [`Response`](Poll::Response) and [`Event`](Poll::Event) values to
/// the caller, and treat [`Idle`](Poll::Idle) as "nothing left to do
/// until more RX bytes arrive."
#[derive(Debug)]
pub enum Poll<'a> {
    /// Nothing to do. Read more bytes from the UART and call
    /// [`Driver::push_rx`](crate::Driver::push_rx) before polling again.
    Idle,
    /// `bytes` must be written to the UART. Once written, call
    /// [`Driver::ack_tx`](crate::Driver::ack_tx) with the number of
    /// bytes actually transmitted.
    NeedTx(&'a [u8]),
    /// A reply to the in-flight command.
    Response(Response<'a>),
    /// An unsolicited event from the radio.
    Event(Event<'a>),
}

/// Errors produced by the protocol layer.
///
/// I/O errors are not represented here — they live in the wrappers that
/// own the UART (`rylr998-std`, `rylr998-tokio`, `rylr998-embassy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// [`Driver::submit`](crate::Driver::submit) was called while
    /// another command is in flight.
    Busy,
    /// An encoded command did not fit in the driver's TX buffer.
    TxOverflow,
    /// [`Driver::push_rx`](crate::Driver::push_rx) was given more bytes
    /// than the RX buffer can hold.
    RxOverflow,
    /// A line from the radio could not be parsed as a known response
    /// or event.
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

/// Known `+ERR=<code>` values reported by the RYLR998 AT firmware.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioError {
    /// `+ERR=1` — missing line terminator.
    MissingLineTerminator,
    /// `+ERR=2` — command does not start with `AT`.
    InvalidAtPrefix,
    /// `+ERR=4` — unknown command.
    UnknownCommand,
    /// `+ERR=5` — supplied data does not match the declared or required length.
    DataLengthMismatch,
    /// `+ERR=10` — TX timed out.
    TxTimeout,
    /// `+ERR=12` — CRC error.
    Crc,
    /// `+ERR=13` — TX data exceeds 240 bytes.
    TxDataTooLong,
    /// `+ERR=14` — failed to write flash memory.
    FlashWriteFailed,
    /// `+ERR=15` — unknown failure.
    UnknownFailure,
    /// `+ERR=17` — last TX was not completed.
    LastTxNotCompleted,
    /// `+ERR=18` — preamble value is not allowed.
    InvalidPreamble,
    /// `+ERR=19` — RX failed with header error.
    RxHeader,
    /// `+ERR=20` — smart receiving power-saving time setting is invalid.
    InvalidSmartReceivingPowerSavingTime,
}

impl RadioError {
    /// Map a numeric `+ERR=<code>` response to a known manual entry.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::MissingLineTerminator),
            2 => Some(Self::InvalidAtPrefix),
            4 => Some(Self::UnknownCommand),
            5 => Some(Self::DataLengthMismatch),
            10 => Some(Self::TxTimeout),
            12 => Some(Self::Crc),
            13 => Some(Self::TxDataTooLong),
            14 => Some(Self::FlashWriteFailed),
            15 => Some(Self::UnknownFailure),
            17 => Some(Self::LastTxNotCompleted),
            18 => Some(Self::InvalidPreamble),
            19 => Some(Self::RxHeader),
            20 => Some(Self::InvalidSmartReceivingPowerSavingTime),
            _ => None,
        }
    }

    /// Numeric `+ERR=<code>` value.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::MissingLineTerminator => 1,
            Self::InvalidAtPrefix => 2,
            Self::UnknownCommand => 4,
            Self::DataLengthMismatch => 5,
            Self::TxTimeout => 10,
            Self::Crc => 12,
            Self::TxDataTooLong => 13,
            Self::FlashWriteFailed => 14,
            Self::UnknownFailure => 15,
            Self::LastTxNotCompleted => 17,
            Self::InvalidPreamble => 18,
            Self::RxHeader => 19,
            Self::InvalidSmartReceivingPowerSavingTime => 20,
        }
    }

    /// Short human-readable description from the AT manual.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::MissingLineTerminator => "missing line terminator",
            Self::InvalidAtPrefix => "command does not start with AT",
            Self::UnknownCommand => "unknown command",
            Self::DataLengthMismatch => "data length mismatch",
            Self::TxTimeout => "TX timed out",
            Self::Crc => "CRC error",
            Self::TxDataTooLong => "TX data exceeds 240 bytes",
            Self::FlashWriteFailed => "failed to write flash memory",
            Self::UnknownFailure => "unknown failure",
            Self::LastTxNotCompleted => "last TX was not completed",
            Self::InvalidPreamble => "preamble value is not allowed",
            Self::RxHeader => "RX failed, header error",
            Self::InvalidSmartReceivingPowerSavingTime => {
                "smart receiving power-saving time setting is invalid"
            }
        }
    }
}

impl fmt::Display for RadioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "+ERR={}: {}", self.code(), self.description())
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

    /// Heap-backed copy of [`Event`], free of the borrow on the driver's
    /// line buffer.
    ///
    /// Available with the `alloc` (or `std`) feature. Produced by
    /// [`Event::into_owned`].
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum OwnedEvent {
        /// Owned form of [`Event::Recv`]; `data` is copied into a `Vec`.
        Recv {
            /// Address of the sending node.
            from: u16,
            /// Payload bytes, copied out of the driver's buffer.
            data: Vec<u8>,
            /// Received signal strength indicator, in dBm.
            rssi: i16,
            /// Signal-to-noise ratio.
            snr: i16,
        },
        /// Owned form of [`Event::Ready`].
        Ready,
    }

    impl<'a> Event<'a> {
        /// Copy this event onto the heap, severing the borrow on the
        /// driver's line buffer.
        ///
        /// Requires the `alloc` feature.
        #[must_use]
        pub fn into_owned(self) -> OwnedEvent {
            match self {
                Event::Recv {
                    from,
                    data,
                    rssi,
                    snr,
                } => OwnedEvent::Recv {
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
        let ev = Event::Recv {
            from: 5,
            data: &bytes,
            rssi: -42,
            snr: 8,
        };
        let owned = ev.into_owned();
        match owned {
            OwnedEvent::Recv {
                from,
                data,
                rssi,
                snr,
            } => {
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

#[cfg(test)]
mod radio_error_tests {
    use super::*;

    #[test]
    fn maps_known_error_code() {
        let err = RadioError::from_code(5).unwrap();
        assert_eq!(err, RadioError::DataLengthMismatch);
        assert_eq!(err.code(), 5);
        assert_eq!(err.description(), "data length mismatch");
        assert_eq!(err.to_string(), "+ERR=5: data length mismatch");
    }

    #[test]
    fn unknown_error_code_is_none() {
        assert_eq!(RadioError::from_code(3), None);
    }
}
