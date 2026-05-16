//! Parse RYLR response lines.
//!
//! ### `+RCV` framing
//!
//! Wire form: `+RCV=<addr>,<len>,<data>,<rssi>,<snr>` where `<data>`
//! is exactly `<len>` raw bytes. `<data>` *may itself contain commas*.

use crate::{Error, Event, Response, RfParams};

struct Prefix;
impl Prefix {
    const OK: &'static [u8; 3] = b"+OK";
    const ERR: &'static [u8; 5] = b"+ERR=";
    const ADDRESS: &'static [u8; 9] = b"+ADDRESS=";
    const NETWORKID: &'static [u8; 11] = b"+NETWORKID=";
    const BAND: &'static [u8; 6] = b"+BAND=";
    const PARAMETER: &'static [u8; 11] = b"+PARAMETER=";
    const CRFOP: &'static [u8; 7] = b"+CRFOP=";
    const UID: &'static [u8; 5] = b"+UID=";
    const VER: &'static [u8; 5] = b"+VER=";
    const READY: &'static [u8; 6] = b"+READY";
    const RCV: &'static [u8; 5] = b"+RCV=";
}

fn parse_bytes<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: core::str::FromStr,
{
    core::str::from_utf8(bytes)
        .map_err(|_| Error::Parse)?
        .parse()
        .map_err(|_| Error::Parse)
}

fn parse_str(bytes: &[u8]) -> Result<&str, Error> {
    core::str::from_utf8(bytes).map_err(|_| Error::Parse)
}

/// Parse a complete response line (no `\r\n`).
///
/// Returns `Err(Error::Parse)` on any unrecognized form.
pub fn parse_response<'a>(line: &'a [u8]) -> Result<Response<'a>, Error> {
    if line == Prefix::OK {
        return Ok(Response::Ok);
    }
    if let Some(bytes) = line.strip_prefix(Prefix::ERR) {
        return Ok(Response::Err(parse_bytes(bytes)?));
    }
    if let Some(bytes) = line.strip_prefix(Prefix::ADDRESS) {
        return Ok(Response::Address(parse_bytes(bytes)?));
    }
    if let Some(bytes) = line.strip_prefix(Prefix::PARAMETER) {
        let mut parts = bytes.split(|b| *b == b',');

        let sf = parse_bytes(parts.next().ok_or(Error::Parse)?)?;
        let bw = parse_bytes(parts.next().ok_or(Error::Parse)?)?;
        let cr = parse_bytes(parts.next().ok_or(Error::Parse)?)?;
        let preamble = parse_bytes(parts.next().ok_or(Error::Parse)?)?;

        if parts.next().is_some() {
            return Err(Error::Parse);
        }

        return Ok(Response::Parameters(RfParams {
            sf,
            bw,
            cr,
            preamble,
        }));
    }
    if let Some(bytes) = line.strip_prefix(Prefix::NETWORKID) {
        return Ok(Response::NetworkId(parse_bytes(bytes)?));
    }
    if let Some(bytes) = line.strip_prefix(Prefix::BAND) {
        return Ok(Response::Band(parse_bytes(bytes)?));
    }
    if let Some(bytes) = line.strip_prefix(Prefix::CRFOP) {
        return Ok(Response::Crfop(parse_bytes(bytes)?));
    }
    if let Some(bytes) = line.strip_prefix(Prefix::UID) {
        return Ok(Response::Uid(parse_str(bytes)?));
    }
    if let Some(bytes) = line.strip_prefix(Prefix::VER) {
        return Ok(Response::Version(parse_str(bytes)?));
    }
    Err(Error::Parse)
}

fn take_until_comma(bytes: &[u8]) -> Result<(&[u8], &[u8]), Error> {
    let comma = bytes.iter().position(|b| *b == b',').ok_or(Error::Parse)?;
    Ok((&bytes[..comma], &bytes[comma + 1..]))
}

/// Parse a complete unsolicited line (no `\r\n`).
///
/// Returns `Err(Error::Parse)` if this isn't a known event.
pub fn parse_event<'a>(line: &'a [u8]) -> Result<Event<'a>, Error> {
    if line == Prefix::READY {
        return Ok(Event::Ready);
    }
    // b"+RCV=2,5,hello,-42,8"
    if let Some(bytes) = line.strip_prefix(Prefix::RCV) {
        let (from_bytes, rest) = take_until_comma(bytes)?;
        //(b"2", b"5,hello,-42,8")
        let from = parse_bytes(from_bytes)?;

        let (data_bytes, rest) = take_until_comma(rest)?;
        // (b"5", b"hello,-42,8")
        let data_len = parse_bytes(data_bytes)?;

        // b"hello,-42,8"
        if rest.len() < data_len + 1 {
            return Err(Error::Parse);
        }

        // b"hello
        let data = &rest[..data_len];
        // b",-42,8"
        if rest.get(data_len) != Some(&b',') {
            return Err(Error::Parse);
        }

        let rest = &rest[data_len + 1..];

        let (rssi_bytes, snr_bytes) = take_until_comma(rest)?;
        // b"-42", b"8"
        let rssi = parse_bytes(rssi_bytes)?;
        let snr = parse_bytes(snr_bytes)?;
        return Ok(Event::Recv {
            from,
            data,
            rssi,
            snr,
        });
    }
    Err(Error::Parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Responses -------------------------------------------------------

    #[test]
    fn ok() {
        assert_eq!(parse_response(b"+OK"), Ok(Response::Ok));
    }
    #[test]
    fn err() {
        assert_eq!(parse_response(b"+ERR=4"), Ok(Response::Err(4)));
    }

    #[test]
    fn address() {
        assert_eq!(parse_response(b"+ADDRESS=5"), Ok(Response::Address(5)));
    }
    #[test]
    fn address_max() {
        assert_eq!(
            parse_response(b"+ADDRESS=65535"),
            Ok(Response::Address(65535))
        );
    }

    #[test]
    fn network_id() {
        assert_eq!(
            parse_response(b"+NETWORKID=18"),
            Ok(Response::NetworkId(18))
        );
    }

    #[test]
    fn band() {
        assert_eq!(
            parse_response(b"+BAND=915000000"),
            Ok(Response::Band(915_000_000))
        );
    }

    #[test]
    fn parameters() {
        assert_eq!(
            parse_response(b"+PARAMETER=9,7,1,12"),
            Ok(Response::Parameters(RfParams {
                sf: 9,
                bw: 7,
                cr: 1,
                preamble: 12
            }))
        );
    }

    #[test]
    fn crfop() {
        assert_eq!(parse_response(b"+CRFOP=22"), Ok(Response::Crfop(22)));
    }
    #[test]
    fn uid() {
        assert_eq!(
            parse_response(b"+UID=DEADBEEF"),
            Ok(Response::Uid("DEADBEEF"))
        );
    }
    #[test]
    fn version() {
        assert_eq!(
            parse_response(b"+VER=AT_V1.2.5"),
            Ok(Response::Version("AT_V1.2.5"))
        );
    }

    #[test]
    fn unknown_response() {
        assert_eq!(parse_response(b"+WAT="), Err(Error::Parse));
    }
    #[test]
    fn empty_response() {
        assert_eq!(parse_response(b""), Err(Error::Parse));
    }

    // --- Events ----------------------------------------------------------

    #[test]
    fn ready() {
        assert_eq!(parse_event(b"+READY"), Ok(Event::Ready));
    }

    #[test]
    fn rcv_simple() {
        let ev = parse_event(b"+RCV=2,5,hello,-42,8").unwrap();
        assert_eq!(
            ev,
            Event::Recv {
                from: 2,
                data: b"hello",
                rssi: -42,
                snr: 8,
            }
        );
    }

    #[test]
    fn rcv_payload_with_embedded_comma() {
        let ev = parse_event(b"+RCV=7,3,a,b,-50,4").unwrap();
        assert_eq!(
            ev,
            Event::Recv {
                from: 7,
                data: b"a,b",
                rssi: -50,
                snr: 4,
            }
        );
    }

    #[test]
    fn rcv_payload_only_commas() {
        // 3-byte payload that is literally `,,,` -- length-prefix proves itself
        let ev = parse_event(b"+RCV=1,3,,,,,-30,3").unwrap();
        assert_eq!(
            ev,
            Event::Recv {
                from: 1,
                data: b",,,",
                rssi: -30,
                snr: 3,
            }
        );
    }

    #[test]
    fn rcv_negative_snr() {
        let ev = parse_event(b"+RCV=2,1,x,-100,-5").unwrap();
        assert_eq!(
            ev,
            Event::Recv {
                from: 2,
                data: b"x",
                rssi: -100,
                snr: -5,
            }
        );
    }

    #[test]
    fn rcv_zero_length_payload() {
        let ev = parse_event(b"+RCV=2,0,,-40,7").unwrap();
        assert_eq!(
            ev,
            Event::Recv {
                from: 2,
                data: b"",
                rssi: -40,
                snr: 7,
            }
        );
    }

    #[test]
    fn rcv_truncated_payload_is_parse_error() {
        // Claims len=10 but only 3 bytes follow before the next field.
        assert_eq!(parse_event(b"+RCV=1,10,abc,-30,3"), Err(Error::Parse));
    }

    #[test]
    fn unknown_event() {
        assert_eq!(parse_event(b"+WAT"), Err(Error::Parse));
    }
}
