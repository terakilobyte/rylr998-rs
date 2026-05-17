mod common;

use common::{Endpoint, RadioSide, WireSide, pair};
use rylr998_std::Radio;

fn make() -> (Radio<Endpoint>, WireSide) {
    let (RadioSide(ep), wire) = pair();
    (Radio::from_port(ep), wire)
}

#[test]
fn ping() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.ping());

    // Wait briefly for the radio to push its command, then reply.
    std::thread::sleep(std::time::Duration::from_millis(50));
    let out = wire.drain_outgoing();
    assert_eq!(out, b"AT\r\n");
    wire.say(b"+OK\r\n");

    handle.join().unwrap().unwrap();
}

#[test]
fn set_address() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.set_address(5));
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(wire.drain_outgoing(), b"AT+ADDRESS=5\r\n");
    wire.say(b"+OK\r\n");
    handle.join().unwrap().unwrap();
}

#[test]
fn get_address() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.address());
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(wire.drain_outgoing(), b"AT+ADDRESS?\r\n");
    wire.say(b"+ADDRESS=5\r\n");
    assert_eq!(handle.join().unwrap().unwrap(), 5);
}

#[test]
fn set_cpin() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.set_cpin(b"EEDCAA90"));
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(wire.drain_outgoing(), b"AT+CPIN=EEDCAA90\r\n");
    wire.say(b"+OK\r\n");
    handle.join().unwrap().unwrap();
}

#[test]
fn set_cpin_propagates_radio_error_5_for_wrong_length() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.set_cpin(b"hunter2"));
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(wire.drain_outgoing(), b"AT+CPIN=hunter2\r\n");
    wire.say(b"+ERR=5\r\n");
    let err = handle.join().unwrap().unwrap_err();
    assert!(matches!(err, rylr998_std::Error::Radio(5)));
}

#[test]
fn cpin() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.cpin());
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(wire.drain_outgoing(), b"AT+CPIN?\r\n");
    wire.say(b"+CPIN=eedcaa90\r\n");
    assert_eq!(handle.join().unwrap().unwrap(), "eedcaa90");
}

#[test]
fn cpin_no_password() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.cpin());
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(wire.drain_outgoing(), b"AT+CPIN?\r\n");
    wire.say(b"+CPIN=No Password!\r\n");
    assert_eq!(handle.join().unwrap().unwrap(), "");
}

#[test]
fn err_response_propagates() {
    let (mut radio, mut wire) = make();
    let handle = std::thread::spawn(move || radio.set_address(0xFFFF));
    std::thread::sleep(std::time::Duration::from_millis(50));
    let _ = wire.drain_outgoing();
    wire.say(b"+ERR=4\r\n");
    let r = handle.join().unwrap();
    assert!(matches!(r, Err(rylr998_std::Error::Radio(4))));
}

#[test]
fn next_event_returns_recv() {
    let (mut radio, mut wire) = make();
    wire.say(b"+RCV=2,5,hello,-42,8\r\n");
    let ev = radio.next_event(std::time::Duration::from_secs(1)).unwrap();
    match ev {
        rylr998_std::OwnedEvent::Recv {
            from,
            data,
            rssi,
            snr,
        } => {
            assert_eq!(from, 2);
            assert_eq!(data, b"hello");
            assert_eq!(rssi, -42);
            assert_eq!(snr, 8);
        }
        other => panic!("unexpected: {:?}", other),
    }
}
