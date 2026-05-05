//! Parse RYLR response lines.
//!
//! ## EXERCISE
//!
//! All four parsing functions in this module are `unimplemented!()`.
//! Your job: replace each `unimplemented!()` with real logic that makes
//! the test table pass. Work in this order — each builds on the last:
//!
//! 1. `parse_response` for `+OK` and `+ERR=N`.
//! 2. `parse_response` for `+ADDRESS=`, `+NETWORKID=`, `+BAND=`,
//!    `+PARAMETER=`, `+CRFOP=`, `+UID=`, `+VER=` (one branch at a time).
//! 3. `parse_event` for `+READY`.
//! 4. `parse_event` for `+RCV=...` — the interesting one. Read the
//!    "embedded comma" note carefully.
//!
//! ### `+RCV` framing
//!
//! Wire form: `+RCV=<addr>,<len>,<data>,<rssi>,<snr>` where `<data>`
//! is exactly `<len>` raw bytes. `<data>` *may itself contain commas*.
//! You CANNOT split the whole line on `,` and expect to recover.
//! Instead:
//!
//! 1. Take the prefix up to the first `,` → `addr`.
//! 2. Take the next prefix up to the next `,` → `len`.
//! 3. Skip exactly `len` bytes → `data`.
//! 4. Expect the next byte to be `,`.
//! 5. Take the next prefix up to the next `,` → `rssi`.
//! 6. Take the rest → `snr`.

use crate::{Error, Event, Response, RfParams};

/// Parse a complete response line (no `\r\n`).
///
/// Returns `Err(Error::Parse)` on any unrecognized form.
pub fn parse_response<'a>(line: &'a [u8]) -> Result<Response<'a>, Error> {
    // TODO: dispatch on prefix:
    //   "+OK"       -> Response::Ok
    //   "+ERR="     -> parse u8, return Response::Err(n)
    //   "+ADDRESS=" -> parse u16,        return Response::Address(n)
    //   "+NETWORKID=" -> parse u8,       return Response::NetworkId(n)
    //   "+BAND="    -> parse u32,        return Response::Band(n)
    //   "+PARAMETER=" -> parse 4 u8s,    return Response::Parameters(...)
    //   "+CRFOP="   -> parse u8,         return Response::Crfop(n)
    //   "+UID="     -> rest as &str,     return Response::Uid(s)
    //   "+VER="     -> rest as &str,     return Response::Version(s)
    // Any other prefix → Err(Error::Parse).
    let _ = (line, RfParams { sf: 0, bw: 0, cr: 0, preamble: 0 }); // silence warnings
    unimplemented!("decode::parse_response")
}

/// Parse a complete unsolicited line (no `\r\n`).
///
/// Returns `Err(Error::Parse)` if this isn't a known event.
pub fn parse_event<'a>(line: &'a [u8]) -> Result<Event<'a>, Error> {
    // TODO:
    //   "+READY"  -> Event::Ready
    //   "+RCV="   -> see length-prefixed framing in module docs
    //   anything else -> Err(Error::Parse)
    let _ = line;
    unimplemented!("decode::parse_event")
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Responses -------------------------------------------------------

    #[test] fn ok()  { assert_eq!(parse_response(b"+OK"), Ok(Response::Ok)); }
    #[test] fn err() { assert_eq!(parse_response(b"+ERR=4"), Ok(Response::Err(4))); }

    #[test] fn address()    { assert_eq!(parse_response(b"+ADDRESS=5"),     Ok(Response::Address(5))); }
    #[test] fn address_max(){ assert_eq!(parse_response(b"+ADDRESS=65535"), Ok(Response::Address(65535))); }

    #[test] fn network_id() { assert_eq!(parse_response(b"+NETWORKID=18"), Ok(Response::NetworkId(18))); }

    #[test] fn band()       { assert_eq!(parse_response(b"+BAND=915000000"), Ok(Response::Band(915_000_000))); }

    #[test]
    fn parameters() {
        assert_eq!(
            parse_response(b"+PARAMETER=9,7,1,12"),
            Ok(Response::Parameters(RfParams { sf: 9, bw: 7, cr: 1, preamble: 12 }))
        );
    }

    #[test] fn crfop()   { assert_eq!(parse_response(b"+CRFOP=22"), Ok(Response::Crfop(22))); }
    #[test] fn uid()     { assert_eq!(parse_response(b"+UID=DEADBEEF"), Ok(Response::Uid("DEADBEEF"))); }
    #[test] fn version() { assert_eq!(parse_response(b"+VER=AT_V1.2.5"), Ok(Response::Version("AT_V1.2.5"))); }

    #[test] fn unknown_response() { assert_eq!(parse_response(b"+WAT="), Err(Error::Parse)); }
    #[test] fn empty_response()   { assert_eq!(parse_response(b""), Err(Error::Parse)); }

    // --- Events ----------------------------------------------------------

    #[test] fn ready() { assert_eq!(parse_event(b"+READY"), Ok(Event::Ready)); }

    #[test]
    fn rcv_simple() {
        let ev = parse_event(b"+RCV=2,5,hello,-42,8").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 2,
            data: b"hello",
            rssi: -42,
            snr: 8,
        });
    }

    #[test]
    fn rcv_payload_with_embedded_comma() {
        let ev = parse_event(b"+RCV=7,3,a,b,-50,4").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 7,
            data: b"a,b",
            rssi: -50,
            snr: 4,
        });
    }

    #[test]
    fn rcv_payload_only_commas() {
        // 3-byte payload that is literally `,,,` -- length-prefix proves itself
        let ev = parse_event(b"+RCV=1,3,,,,,-30,3").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 1,
            data: b",,,",
            rssi: -30,
            snr: 3,
        });
    }

    #[test]
    fn rcv_negative_snr() {
        let ev = parse_event(b"+RCV=2,1,x,-100,-5").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 2,
            data: b"x",
            rssi: -100,
            snr: -5,
        });
    }

    #[test]
    fn rcv_zero_length_payload() {
        let ev = parse_event(b"+RCV=2,0,,-40,7").unwrap();
        assert_eq!(ev, Event::Recv {
            from: 2,
            data: b"",
            rssi: -40,
            snr: 7,
        });
    }

    #[test]
    fn rcv_truncated_payload_is_parse_error() {
        // Claims len=10 but only 3 bytes follow before the next field.
        assert_eq!(parse_event(b"+RCV=1,10,abc,-30,3"), Err(Error::Parse));
    }

    #[test] fn unknown_event() { assert_eq!(parse_event(b"+WAT"), Err(Error::Parse)); }
}
