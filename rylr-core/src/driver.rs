//! State-machine `Driver` over a fixed-size RX buffer and an outbound TX queue.

use crate::{Command, Error, Poll};
use heapless::Vec as HVec;

pub(crate) const RX_BUF: usize = 512;
pub(crate) const TX_BUF: usize = 512;

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

#[derive(Clone, Copy)]
pub(crate) enum AwaitKind {
    /// Setter / Ping / FactoryReset — expects `+OK` or `+ERR`.
    Ack,
    /// Query — expects `+<KEY>=...` or `+ERR`.
    Query(QueryKey),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryKey {
    Address,
    NetworkId,
    Band,
    Parameters,
    Crfop,
    Uid,
    Version,
}

/// State machine that turns RYLR998 protocol traffic into a sequence of
/// [`Poll`] steps.
///
/// `Driver` does no I/O: callers feed it raw UART bytes via
/// [`push_rx`](Driver::push_rx), drain its TX queue when
/// [`poll`](Driver::poll) returns [`Poll::NeedTx`], and acknowledge sent
/// bytes via [`ack_tx`](Driver::ack_tx). The wrapper crates
/// (`rylr998-std`, `rylr998-tokio`, `rylr998-embassy`) compose this loop
/// with a real transport.
///
/// Buffers are fixed-size and stack-allocated; the driver is `no_std`
/// and never allocates.
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
    pub(crate) line_buf: [u8; 288],
    pub(crate) line_buf_len: usize,
}

impl Default for Driver {
    fn default() -> Self {
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
}

impl Driver {
    /// Construct a fresh driver with empty buffers and idle state.
    pub fn new() -> Self {
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

    /// Queue a command for transmission.
    ///
    /// The encoded `AT+…` line is buffered immediately; subsequent calls
    /// to [`poll`](Self::poll) will yield it via [`Poll::NeedTx`] and
    /// then await the matching [`Response`](crate::Response).
    ///
    /// # Errors
    ///
    /// - [`Error::Busy`] if another command is already in flight.
    /// - [`Error::TxOverflow`] if the encoded line exceeds the TX buffer
    ///   (only realistic for very large `Command::Send` payloads).
    pub fn submit(&mut self, cmd: Command<'_>) -> Result<(), Error> {
        if !matches!(self.state, State::Idle) {
            return Err(Error::Busy);
        }
        let kind = match cmd {
            Command::GetAddress => AwaitKind::Query(QueryKey::Address),
            Command::GetNetworkId => AwaitKind::Query(QueryKey::NetworkId),
            Command::GetBand => AwaitKind::Query(QueryKey::Band),
            Command::GetParameters => AwaitKind::Query(QueryKey::Parameters),
            Command::GetCrfop => AwaitKind::Query(QueryKey::Crfop),
            Command::GetUid => AwaitKind::Query(QueryKey::Uid),
            Command::GetVersion => AwaitKind::Query(QueryKey::Version),
            _ => AwaitKind::Ack,
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

    /// Feed bytes read from the UART into the RX buffer.
    ///
    /// Returns the number of bytes accepted (always `bytes.len()` on
    /// success).
    ///
    /// # Errors
    ///
    /// [`Error::RxOverflow`] if `bytes` does not fit in the remaining
    /// RX-buffer space. As much as fits is appended before returning.
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

    /// Acknowledge that `n` bytes from a prior [`Poll::NeedTx`] have
    /// been written to the UART.
    ///
    /// Call this after every successful write so the driver can release
    /// the bytes from its TX queue and advance the state machine.
    pub fn ack_tx(&mut self, n: usize) {
        let n = n.min(self.tx_in_flight);
        // Shift the first `n` bytes off `self.tx` (heapless::Vec has no drain).
        let len = self.tx.len();
        if n > 0 && n <= len {
            self.tx.copy_within(n..len, 0);
            self.tx.truncate(len - n);
        }
        self.tx_in_flight -= n;
        if self.tx.is_empty()
            && matches!(self.state, State::SendingTx)
            && let Some(kind) = self.pending_kind.take()
        {
            self.state = State::Awaiting { kind };
        }
    }

    fn next_line_end(&self) -> Option<usize> {
        if let Some(r) = self.rx.iter().position(|b| *b == b'\r')
            && self.rx.get(r + 1).is_some_and(|b| *b == b'\n')
        {
            return Some(r);
        }

        None
    }

    fn drain_rx_line(&mut self, end: usize) {
        let len = self.rx.len();
        self.rx.copy_within(end + 2..len, 0);
        self.rx.truncate(len - (end + 2));
    }

    fn response_matches(kind: AwaitKind, line: &[u8]) -> bool {
        if line.starts_with(b"+ERR=") {
            return true;
        }

        match kind {
            AwaitKind::Ack => line == b"+OK" || line == b"+RESET" || line == b"+READY",
            AwaitKind::Query(QueryKey::Address) => line.starts_with(b"+ADDRESS="),
            AwaitKind::Query(QueryKey::NetworkId) => line.starts_with(b"+NETWORKID="),
            AwaitKind::Query(QueryKey::Band) => line.starts_with(b"+BAND="),
            AwaitKind::Query(QueryKey::Parameters) => line.starts_with(b"+PARAMETER="),
            AwaitKind::Query(QueryKey::Crfop) => line.starts_with(b"+CRFOP="),
            AwaitKind::Query(QueryKey::Uid) => line.starts_with(b"+UID="),
            AwaitKind::Query(QueryKey::Version) => line.starts_with(b"+VER="),
        }
    }

    /// Take one step of the state machine.
    ///
    /// Returns whatever the driver wants to do next: emit bytes
    /// ([`Poll::NeedTx`]), surface a parsed line as a
    /// [`Response`](Poll::Response) or an [`Event`](Poll::Event), or
    /// signal that it's [`Idle`](Poll::Idle) and needs more RX bytes.
    ///
    /// Drain by calling in a loop until you see `Poll::Idle`, then read
    /// more bytes from the UART and feed them via
    /// [`push_rx`](Self::push_rx) before polling again.
    pub fn poll(&mut self) -> Poll<'_> {
        if self.tx_in_flight < self.tx.len() {
            let bytes = &self.tx[self.tx_in_flight..];
            self.tx_in_flight = self.tx.len();
            return Poll::NeedTx(bytes);
        }

        if !self.rx.is_empty()
            && let Some(end) = self.next_line_end()
        {
            // put the line to read in line_buf
            self.line_buf[..end].copy_from_slice(&self.rx[..end]);
            self.line_buf_len = end;

            // `AT+RESET` produces "+RESET\r\n" then "+READY\r\n". The +RESET
            // line is intermediate — silently drain it and re-poll so the
            // caller's drain loop sees the +READY without us returning Idle
            // mid-batch (which would force them to wait for more bytes that
            // are already buffered).
            if self.awaiting_ready_as_ok && self.line_buf[..end].starts_with(b"+RESET") {
                self.drain_rx_line(end);
                return self.poll();
            }

            let is_event = {
                let line = &self.line_buf[..end];
                line.starts_with(b"+RCV") || line.starts_with(b"+READY")
            };

            if is_event {
                let ready_as_ok = self.awaiting_ready_as_ok && self.line_buf[..end] == *b"+READY";
                if ready_as_ok {
                    self.drain_rx_line(end);
                    self.awaiting_ready_as_ok = false;
                    self.state = State::Idle;
                    return Poll::Response(crate::Response::Ok);
                }
                self.drain_rx_line(end);
                let line = &self.line_buf[..self.line_buf_len];
                match crate::decode::parse_event(line) {
                    Ok(e) => {
                        return Poll::Event(e);
                    }
                    Err(_) => return Poll::Idle,
                }
            }

            let response_matches = if let State::Awaiting { kind } = self.state {
                Self::response_matches(kind, &self.line_buf[..end])
            } else {
                false
            };

            if response_matches {
                self.drain_rx_line(end);
                self.awaiting_ready_as_ok = false;
                self.state = State::Idle;
                let line = &self.line_buf[..self.line_buf_len];
                match crate::decode::parse_response(line) {
                    Ok(response) => return Poll::Response(response),
                    Err(_) => return Poll::Idle,
                }
            }
        }

        Poll::Idle
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
            Poll::Event(Event::Recv {
                from: 2,
                data,
                rssi: -42,
                snr: 8,
            }) => {
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
