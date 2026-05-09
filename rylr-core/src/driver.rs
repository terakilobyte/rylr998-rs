//! State-machine `Driver` over a fixed-size RX buffer and an outbound TX queue.

use crate::{Command, Error, Poll};
use heapless::Vec as HVec;

pub(crate) const RX_BUF: usize = 512;
pub(crate) const TX_BUF: usize = 512;

// `State::Awaiting.kind` and `AwaitKind::Query.0` aren't read yet — they're
// consumed by the `poll()` body that lives as the user's exercise (Task 7).
// Once `poll` reads them, drop these allows.
#[allow(dead_code)]
#[derive(Default, Clone, Copy)]
pub(crate) enum State {
    #[default]
    Idle,
    /// A command has been encoded; bytes are pending in `tx`.
    /// Once they're all `ack_tx`'d, transition to `Awaiting`.
    SendingTx,
    /// All TX bytes acked, awaiting the matching response line.
    Awaiting { kind: AwaitKind },
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum AwaitKind {
    /// Setter / Ping / FactoryReset — expects `+OK` or `+ERR`.
    Ack,
    /// Query — expects `+<KEY>=...` or `+ERR`.
    Query(QueryKey),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryKey {
    Address, NetworkId, Band, Parameters, Crfop, Uid, Version,
}

pub struct Driver {
    pub(crate) rx: HVec<u8, RX_BUF>,
    pub(crate) tx: HVec<u8, TX_BUF>,
    /// Bytes already handed out via `Poll::NeedTx` but not yet acked.
    pub(crate) tx_in_flight: usize,
    pub(crate) state: State,
    /// True iff the next `+READY` should resolve as `Response::Ok`
    /// (i.e. `AT+FACTORY` is in flight).
    pub(crate) awaiting_ready_as_ok: bool,
    pub(crate) pending_kind: Option<AwaitKind>,
    /// Scratch space for poll() to copy a line into before draining rx.
    #[allow(dead_code)]
    pub(crate) line_buf: [u8; 288],
    #[allow(dead_code)]
    pub(crate) line_buf_len: usize,
}

impl Driver {
    pub const fn new() -> Self {
        Self {
            rx: HVec::new(),
            tx: HVec::new(),
            tx_in_flight: 0,
            state: State::Idle,
            awaiting_ready_as_ok: false,
            pending_kind: None,
            line_buf: [0u8; 288],
            line_buf_len: 0,
        }
    }

    pub fn submit(&mut self, cmd: Command<'_>) -> Result<(), Error> {
        if !matches!(self.state, State::Idle) {
            return Err(Error::Busy);
        }
        let kind = match cmd {
            Command::GetAddress    => AwaitKind::Query(QueryKey::Address),
            Command::GetNetworkId  => AwaitKind::Query(QueryKey::NetworkId),
            Command::GetBand       => AwaitKind::Query(QueryKey::Band),
            Command::GetParameters => AwaitKind::Query(QueryKey::Parameters),
            Command::GetCrfop      => AwaitKind::Query(QueryKey::Crfop),
            Command::GetUid        => AwaitKind::Query(QueryKey::Uid),
            Command::GetVersion    => AwaitKind::Query(QueryKey::Version),
            _                      => AwaitKind::Ack,
        };
        self.awaiting_ready_as_ok = matches!(cmd, Command::FactoryReset);

        let mut tmp = [0u8; TX_BUF];
        let n = crate::encode::encode(cmd, &mut tmp)?;
        if self.tx.extend_from_slice(&tmp[..n]).is_err() {
            return Err(Error::TxOverflow);
        }
        self.state = State::SendingTx;
        self.pending_kind = Some(kind);
        Ok(())
    }

    pub fn push_rx(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let room = self.rx.capacity() - self.rx.len();
        if bytes.len() > room {
            // Accept what fits, then signal overflow.
            let _ = self.rx.extend_from_slice(&bytes[..room]);
            return Err(Error::RxOverflow);
        }
        let _ = self.rx.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    pub fn ack_tx(&mut self, n: usize) {
        let n = n.min(self.tx_in_flight);
        // Shift the first `n` bytes off `self.tx` (heapless::Vec has no drain).
        let len = self.tx.len();
        if n > 0 && n <= len {
            self.tx.copy_within(n..len, 0);
            self.tx.truncate(len - n);
        }
        self.tx_in_flight -= n;
        if self.tx.is_empty() && matches!(self.state, State::SendingTx) {
            if let Some(kind) = self.pending_kind.take() {
                self.state = State::Awaiting { kind };
            }
        }
    }

    /// Drive the state machine.
    ///
    /// ## EXERCISE
    ///
    /// Priority order on each call:
    /// 1. If we have undelivered TX bytes (`self.tx` has bytes that haven't
    ///    been handed out as `NeedTx` yet), return `Poll::NeedTx(slice)`.
    ///    Track them as in-flight via `self.tx_in_flight` so `ack_tx` can
    ///    drain them.
    /// 2. Try to find a complete unsolicited event in `self.rx`:
    ///    a. Look for `\r\n`. If absent, no events available.
    ///    b. If the line starts with `+RCV` or `+READY`, parse it via
    ///       `crate::decode::parse_event`. Drain the line + `\r\n` from
    ///       `self.rx` and return `Poll::Event(...)`.
    ///    c. If `awaiting_ready_as_ok` is true and the line is `+READY`,
    ///       return `Poll::Response(Response::Ok)` instead, transition to
    ///       `Idle`, clear `awaiting_ready_as_ok`, drain the line.
    /// 3. If we're in state `Awaiting`, look for a complete response line
    ///    (`+OK`, `+ERR=N`, or the matching `+<KEY>=...`) and parse via
    ///    `decode::parse_response`. On match, drain, transition to `Idle`,
    ///    return `Poll::Response(...)`.
    /// 4. Otherwise, return `Poll::Idle`.
    ///
    /// ### Hints
    ///
    /// - You'll want a private helper `fn next_line_end(&self) -> Option<usize>`
    ///   that returns the index of `\r` if a complete `\r\n` is buffered.
    /// - Use `self.rx.as_slice()[..end]` for the line bytes; then
    ///   `self.rx.copy_within(end + 2.., 0)` + `self.rx.truncate(...)` to
    ///   drain. (`heapless::Vec` has no `drain`, so manual copy is the move.)
    /// - The borrow returned in `Poll::Event` / `Poll::Response` must outlive
    ///   the drain. Translation: parse first, then drain. Or parse into an
    ///   intermediate owned representation before draining — but that adds
    ///   an alloc. Cleanest: take a snapshot of the line into a stack buffer,
    ///   parse from that, then drain.
    ///
    /// ### Note on borrowing
    ///
    /// Returning `Poll::Event { data: &[u8] }` from `&mut self` while the
    /// data lives in `self.rx` requires that we *not* mutate `self.rx`
    /// between parse and return. The simplest solution: copy the line into
    /// a small `[u8; 288]` field on `Driver` (`line_buf`), drain `self.rx`
    /// of that line *before* parsing, then parse from `line_buf`. The borrow
    /// returned then ties to `&self.line_buf`, which `&mut self` already
    /// reserves.
    pub fn poll(&mut self) -> Poll<'_> {
        // TODO: implement per the rules above.
        unimplemented!("Driver::poll — exercise")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_idle_then_busy() {
        let mut d = Driver::new();
        assert!(d.submit(Command::Ping).is_ok());
        assert_eq!(d.submit(Command::Ping), Err(Error::Busy));
    }

    #[test]
    fn submit_encodes_into_tx_buffer() {
        let mut d = Driver::new();
        d.submit(Command::SetAddress(5)).unwrap();
        // tx buffer holds "AT+ADDRESS=5\r\n" = 14 bytes
        assert_eq!(d.tx.len(), 14);
        assert_eq!(&d.tx[..14], b"AT+ADDRESS=5\r\n");
    }

    #[test]
    fn push_rx_appends() {
        let mut d = Driver::new();
        d.push_rx(b"+OK\r\n").unwrap();
        assert_eq!(&d.rx[..], b"+OK\r\n");
    }

    #[test]
    fn push_rx_overflow_signals_error() {
        let mut d = Driver::new();
        let big = [0u8; RX_BUF + 1];
        assert_eq!(d.push_rx(&big), Err(Error::RxOverflow));
    }

    #[test]
    fn ack_tx_shifts_buffer() {
        let mut d = Driver::new();
        d.submit(Command::Ping).unwrap();
        // tx now holds b"AT\r\n" (4 bytes).
        // Pretend poll() handed out the bytes:
        d.tx_in_flight = d.tx.len();
        d.ack_tx(2);
        // First two bytes drained; the surviving bytes should be b"\r\n".
        assert_eq!(d.tx.len(), 2);
        assert_eq!(&d.tx[..], b"\r\n");
    }
}

#[cfg(all(test, feature = "alloc"))]
mod poll_tests {
    use super::*;
    use crate::{Event, Response};
    use alloc::vec::Vec;

    /// Helper: drain TX bytes the driver wants sent, ack them, and return
    /// what was sent for assertions.
    fn drain_tx(d: &mut Driver) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            match d.poll() {
                Poll::NeedTx(bytes) => {
                    out.extend_from_slice(bytes);
                    let n = bytes.len();
                    d.ack_tx(n);
                }
                _ => return out,
            }
        }
    }

    #[test]
    fn set_address_round_trip() {
        let mut d = Driver::new();
        d.submit(Command::SetAddress(5)).unwrap();

        assert_eq!(drain_tx(&mut d), b"AT+ADDRESS=5\r\n");

        d.push_rx(b"+OK\r\n").unwrap();
        assert!(matches!(d.poll(), Poll::Response(Response::Ok)));
        assert!(matches!(d.poll(), Poll::Idle));
    }

    #[test]
    fn get_address_returns_value() {
        let mut d = Driver::new();
        d.submit(Command::GetAddress).unwrap();
        let _ = drain_tx(&mut d);
        d.push_rx(b"+ADDRESS=5\r\n").unwrap();
        match d.poll() {
            Poll::Response(Response::Address(5)) => {}
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn err_response() {
        let mut d = Driver::new();
        d.submit(Command::Ping).unwrap();
        let _ = drain_tx(&mut d);
        d.push_rx(b"+ERR=4\r\n").unwrap();
        assert!(matches!(d.poll(), Poll::Response(Response::Err(4))));
    }

    #[test]
    fn factory_reset_resolves_on_ready() {
        let mut d = Driver::new();
        d.submit(Command::FactoryReset).unwrap();
        let _ = drain_tx(&mut d);
        d.push_rx(b"+OK\r\n+READY\r\n").unwrap();
        // Implementation choice: either +OK or +READY may resolve the
        // command. Both are acceptable.
        assert!(matches!(d.poll(), Poll::Response(Response::Ok)));
        // The leftover line (whichever wasn't consumed by the response)
        // must be cleanly handled — either as a follow-up Ready event
        // or drained internally as Idle. Both are correct.
        match d.poll() {
            Poll::Idle | Poll::Event(Event::Ready) => {}
            other => panic!("unexpected after factory_reset: {:?}", other),
        }
    }

    #[test]
    fn unsolicited_recv() {
        let mut d = Driver::new();
        d.push_rx(b"+RCV=2,5,hello,-42,8\r\n").unwrap();
        match d.poll() {
            Poll::Event(Event::Recv { from: 2, data, rssi: -42, snr: 8 }) => {
                assert_eq!(data, b"hello");
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn recv_during_command_is_delivered_first() {
        let mut d = Driver::new();
        d.submit(Command::GetAddress).unwrap();
        let _ = drain_tx(&mut d);
        d.push_rx(b"+RCV=2,2,hi,-30,3\r\n+ADDRESS=5\r\n").unwrap();
        // Per priority order: events drain before responses.
        assert!(matches!(d.poll(), Poll::Event(_)));
        assert!(matches!(d.poll(), Poll::Response(Response::Address(5))));
    }
}
