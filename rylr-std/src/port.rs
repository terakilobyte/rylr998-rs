//! Auto-discover a single `cu.usbserial*` device.

use crate::{Error, Result};
use std::path::PathBuf;

/// Test-friendly: filter a list of port names down to candidates.
pub(crate) fn filter(names: impl IntoIterator<Item = String>) -> Vec<PathBuf> {
    names
        .into_iter()
        .filter(|n| {
            // accept full paths (/dev/cu.usbserial-XYZ) or bare names
            let leaf = std::path::Path::new(n)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(n.as_str());
            leaf.starts_with("cu.usbserial")
        })
        .map(PathBuf::from)
        .collect()
}

pub fn discover() -> Result<PathBuf> {
    let names = serialport::available_ports()?
        .into_iter()
        .map(|p| p.port_name)
        .collect::<Vec<_>>();
    decide(filter(names))
}

fn decide(mut candidates: Vec<PathBuf>) -> Result<PathBuf> {
    match candidates.len() {
        0 => Err(Error::NoDevice),
        1 => Ok(candidates.remove(0)),
        _ => Err(Error::Ambiguous(candidates)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_match_is_no_device() {
        let r = decide(filter(vec!["/dev/ttyS0".into(), "/dev/cu.Bluetooth-Incoming-Port".into()]));
        assert!(matches!(r, Err(Error::NoDevice)));
    }

    #[test]
    fn one_match_returns_path() {
        let r = decide(filter(vec!["/dev/cu.usbserial-A1".into(), "/dev/cu.Bluetooth-Incoming-Port".into()]));
        assert_eq!(r.unwrap(), PathBuf::from("/dev/cu.usbserial-A1"));
    }

    #[test]
    fn two_matches_is_ambiguous() {
        let r = decide(filter(vec![
            "/dev/cu.usbserial-A1".into(),
            "/dev/cu.usbserial-B2".into(),
        ]));
        match r {
            Err(Error::Ambiguous(v)) => assert_eq!(v.len(), 2),
            _ => panic!("expected Ambiguous"),
        }
    }

    #[test]
    fn filter_accepts_bare_names() {
        let r = filter(vec!["cu.usbserial-X".into(), "ttyACM0".into()]);
        assert_eq!(r, vec![PathBuf::from("cu.usbserial-X")]);
    }
}
