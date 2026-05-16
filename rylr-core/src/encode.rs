//! Encode `Command` values into ASCII command lines (`AT...\r\n`).

use crate::{Command, Error, RfParams};

/// Write the encoded form of `cmd` into `buf`. Returns the number of bytes
/// written, or `Error::TxOverflow` if `buf` is too small.
pub fn encode(cmd: Command<'_>, buf: &mut [u8]) -> Result<usize, Error> {
    let mut w = Writer::new(buf);
    match cmd {
        Command::Ping => w.lit(b"AT")?,
        Command::GetAddress => w.lit(b"AT+ADDRESS?")?,
        Command::SetAddress(n) => {
            w.lit(b"AT+ADDRESS=")?;
            w.u16(n)?;
        }
        Command::GetNetworkId => w.lit(b"AT+NETWORKID?")?,
        Command::SetNetworkId(n) => {
            w.lit(b"AT+NETWORKID=")?;
            w.u16(n as u16)?;
        }
        Command::GetBand => w.lit(b"AT+BAND?")?,
        Command::SetBand(hz) => {
            w.lit(b"AT+BAND=")?;
            w.u32(hz)?;
        }
        Command::GetParameters => w.lit(b"AT+PARAMETER?")?,
        Command::SetParameters(RfParams {
            sf,
            bw,
            cr,
            preamble,
        }) => {
            w.lit(b"AT+PARAMETER=")?;
            w.u16(sf as u16)?;
            w.lit(b",")?;
            w.u16(bw as u16)?;
            w.lit(b",")?;
            w.u16(cr as u16)?;
            w.lit(b",")?;
            w.u16(preamble as u16)?;
        }
        Command::GetCrfop => w.lit(b"AT+CRFOP?")?,
        Command::GetUid => w.lit(b"AT+UID?")?,
        Command::GetVersion => w.lit(b"AT+VER?")?,
        Command::FactoryReset => w.lit(b"AT+RESET")?,
        Command::Send { to, data } => {
            w.lit(b"AT+SEND=")?;
            w.u16(to)?;
            w.lit(b",")?;
            w.u16(data.len() as u16)?;
            w.lit(b",")?;
            w.bytes(data)?;
        }
    }
    w.lit(b"\r\n")?;
    Ok(w.pos)
}

struct Writer<'b> {
    buf: &'b mut [u8],
    pos: usize,
}
impl<'b> Writer<'b> {
    fn new(buf: &'b mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn lit(&mut self, s: &[u8]) -> Result<(), Error> {
        self.bytes(s)
    }
    fn bytes(&mut self, s: &[u8]) -> Result<(), Error> {
        if self.pos + s.len() > self.buf.len() {
            return Err(Error::TxOverflow);
        }
        self.buf[self.pos..self.pos + s.len()].copy_from_slice(s);
        self.pos += s.len();
        Ok(())
    }
    fn u16(&mut self, mut n: u16) -> Result<(), Error> {
        let mut digits = [0u8; 5];
        let mut i = 0;
        if n == 0 {
            return self.bytes(b"0");
        }
        while n > 0 {
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            i += 1;
        }
        for j in (0..i).rev() {
            self.bytes(&[digits[j]])?;
        }
        Ok(())
    }
    fn u32(&mut self, mut n: u32) -> Result<(), Error> {
        let mut digits = [0u8; 10];
        let mut i = 0;
        if n == 0 {
            return self.bytes(b"0");
        }
        while n > 0 {
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            i += 1;
        }
        for j in (0..i).rev() {
            self.bytes(&[digits[j]])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::RfParams;
    use std::string::String;

    fn encode_to_string(cmd: Command<'_>) -> String {
        let mut buf = [0u8; 512];
        let n = encode(cmd, &mut buf).unwrap();
        String::from_utf8(buf[..n].to_vec()).unwrap()
    }

    #[test]
    fn ping() {
        assert_eq!(encode_to_string(Command::Ping), "AT\r\n");
    }
    #[test]
    fn get_address() {
        assert_eq!(encode_to_string(Command::GetAddress), "AT+ADDRESS?\r\n");
    }
    #[test]
    fn get_network_id() {
        assert_eq!(encode_to_string(Command::GetNetworkId), "AT+NETWORKID?\r\n");
    }
    #[test]
    fn get_band() {
        assert_eq!(encode_to_string(Command::GetBand), "AT+BAND?\r\n");
    }
    #[test]
    fn get_parameters() {
        assert_eq!(
            encode_to_string(Command::GetParameters),
            "AT+PARAMETER?\r\n"
        );
    }
    #[test]
    fn get_crfop() {
        assert_eq!(encode_to_string(Command::GetCrfop), "AT+CRFOP?\r\n");
    }
    #[test]
    fn get_uid() {
        assert_eq!(encode_to_string(Command::GetUid), "AT+UID?\r\n");
    }
    #[test]
    fn get_version() {
        assert_eq!(encode_to_string(Command::GetVersion), "AT+VER?\r\n");
    }
    #[test]
    fn set_address() {
        assert_eq!(encode_to_string(Command::SetAddress(5)), "AT+ADDRESS=5\r\n");
    }
    #[test]
    fn set_address_zero() {
        assert_eq!(encode_to_string(Command::SetAddress(0)), "AT+ADDRESS=0\r\n");
    }
    #[test]
    fn set_address_max() {
        assert_eq!(
            encode_to_string(Command::SetAddress(65535)),
            "AT+ADDRESS=65535\r\n"
        );
    }
    #[test]
    fn set_network_id() {
        assert_eq!(
            encode_to_string(Command::SetNetworkId(18)),
            "AT+NETWORKID=18\r\n"
        );
    }
    #[test]
    fn set_band() {
        assert_eq!(
            encode_to_string(Command::SetBand(915_000_000)),
            "AT+BAND=915000000\r\n"
        );
    }
    #[test]
    fn set_parameters() {
        let p = RfParams {
            sf: 9,
            bw: 7,
            cr: 1,
            preamble: 12,
        };
        assert_eq!(
            encode_to_string(Command::SetParameters(p)),
            "AT+PARAMETER=9,7,1,12\r\n"
        );
    }
    #[test]
    fn factory_reset() {
        assert_eq!(encode_to_string(Command::FactoryReset), "AT+RESET\r\n");
    }
    #[test]
    fn send_text() {
        assert_eq!(
            encode_to_string(Command::Send {
                to: 2,
                data: b"hello"
            }),
            "AT+SEND=2,5,hello\r\n"
        );
    }
    #[test]
    fn send_with_comma_in_payload() {
        assert_eq!(
            encode_to_string(Command::Send {
                to: 7,
                data: b"a,b"
            }),
            "AT+SEND=7,3,a,b\r\n"
        );
    }

    #[test]
    fn tx_overflow() {
        let mut buf = [0u8; 4]; // exactly fits "AT\r\n"
        assert_eq!(encode(Command::Ping, &mut buf), Ok(4));
        let mut buf = [0u8; 3];
        assert_eq!(encode(Command::Ping, &mut buf), Err(Error::TxOverflow));
    }
}
