//! In-memory bidirectional pipe for testing `Radio<P>` without a real port.
//!
//! Two halves share a pair of `Arc<Mutex<VecDeque<u8>>>`. Writes to one
//! half land in the queue the other half reads from.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct PipeInner {
    buf: VecDeque<u8>,
}

#[derive(Clone)]
pub struct Pipe(Arc<Mutex<PipeInner>>);

impl Pipe {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(PipeInner::default())))
    }
    fn write_bytes(&self, bytes: &[u8]) {
        self.0.lock().unwrap().buf.extend(bytes);
    }
    fn read_bytes(&self, dst: &mut [u8]) -> usize {
        let mut g = self.0.lock().unwrap();
        let n = g.buf.len().min(dst.len());
        for slot in dst.iter_mut().take(n) {
            *slot = g.buf.pop_front().unwrap();
        }
        n
    }
}

pub struct Endpoint {
    rx: Pipe,
    tx: Pipe,
}

impl Read for Endpoint {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.rx.read_bytes(buf);
        if n == 0 {
            // Mimic a serial port read timeout.
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "no data"));
        }
        Ok(n)
    }
}

impl Write for Endpoint {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.tx.write_bytes(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct RadioSide(pub Endpoint);
pub struct WireSide(pub Endpoint);

pub fn pair() -> (RadioSide, WireSide) {
    let a = Pipe::new();
    let b = Pipe::new();
    (
        RadioSide(Endpoint {
            rx: a.clone(),
            tx: b.clone(),
        }),
        WireSide(Endpoint { rx: b, tx: a }),
    )
}

impl WireSide {
    /// "From the radio's mouth": queue bytes that the Radio<P> will read.
    pub fn say(&mut self, bytes: &[u8]) {
        self.0.tx.write_bytes(bytes);
    }
    /// What did the Radio<P> just send?
    pub fn drain_outgoing(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = [0u8; 256];
        loop {
            let n = self.0.rx.read_bytes(&mut buf);
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
        }
        out
    }
}
