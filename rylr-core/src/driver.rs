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

    pub fn poll(&mut self) -> Poll<'_> {
        // EXERCISE: see Task 7.
        unimplemented!("Driver::poll — Task 7 exercise")
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
