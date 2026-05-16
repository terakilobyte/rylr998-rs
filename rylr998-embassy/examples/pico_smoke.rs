//! Pico ↔ RYLR998 smoke test.
//!
//! ## Wiring
//!
//! Radio (UART1):
//!   Pico GP8  (UART1 TX) → RYLR998 RX
//!   Pico GP9  (UART1 RX) ← RYLR998 TX
//!   GND ↔ GND, 3V3 ↔ VDD (do **not** use 5V — RYLR998 isn't 5V tolerant).
//!
//! Console (UART0, picoprobe pass-through to host USB-CDC):
//!   Pico GP0  (UART0 TX) → probe Pico GP5 (probe's UART RX in)
//!   Pico GP1  (UART0 RX) ← probe Pico GP4 (probe's UART TX out, unused for logging)
//!
//! ## Flash + view logs
//!
//!   cargo run --release --example pico_smoke
//!   # In another terminal:
//!   screen /dev/cu.usbmodem<probe-serial>3 115200
//!
//! Expected console output:
//!   boot
//!   ping + set_address ok
//!   listening
//!   recv from=2 len=… rssi=… snr=…   (when another radio sends to addr 5)

#![no_std]
#![no_main]
#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: embassy_rp::block::ImageDef = embassy_rp::block::ImageDef::secure_exe();

use core::fmt::Write;
use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts, peripherals, uart};
use embassy_time::Duration;
use heapless::String;
use rylr998_core::Event;

// rylr998-core has a `defmt` feature enabled, so its types reference defmt's
// global logger symbols. We don't actually use RTT output anymore, but
// linking defmt-rtt provides those symbols and keeps the linker happy.
use defmt_rtt as _;

bind_interrupts!(struct Irqs {
    UART1_IRQ => uart::BufferedInterruptHandler<peripherals::UART1>;
});

/// Custom panic handler: rapid SOS-style continuous blinking so any fault
/// is visually obvious without needing a working log channel.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Safety: panic means we own the chip; reach in for GP25 directly.
    let p = unsafe { embassy_rp::Peripherals::steal() };
    let mut led = embassy_rp::gpio::Output::new(p.PIN_25, embassy_rp::gpio::Level::Low);
    loop {
        led.set_high();
        cortex_m::asm::delay(15_000_000);
        led.set_low();
        cortex_m::asm::delay(15_000_000);
    }
}

/// Blocking write of a string + CRLF to the supplied UART. Drops any error
/// silently — this is a smoke test, not a critical telemetry path.
fn logln(uart: &mut uart::Uart<'_, uart::Blocking>, msg: &str) {
    let _ = uart.blocking_write(msg.as_bytes());
    let _ = uart.blocking_write(b"\r\n");
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // UART0 = host console via picoprobe pass-through.
    let mut console_config = uart::Config::default();
    console_config.baudrate = 115_200;
    let mut console = uart::Uart::new_blocking(p.UART0, p.PIN_0, p.PIN_1, console_config);
    logln(&mut console, "boot");

    // LED-based diagnostic. Independent of any log channel.
    //
    // Patterns:
    //   GOOD       — 3 fast blinks, solid 2s, 3 fast blinks
    //   PING FAIL  — 3 slow blinks, 2s dark, 3 slow blinks
    //   PANIC      — continuous fast blinking forever (custom panic handler)
    let mut led = embassy_rp::gpio::Output::new(p.PIN_25, embassy_rp::gpio::Level::Low);

    // UART1 = the radio.
    let tx_buf: &'static mut [u8; 256] = cortex_m::singleton!(: [u8; 256] = [0; 256]).unwrap();
    let rx_buf: &'static mut [u8; 256] = cortex_m::singleton!(: [u8; 256] = [0; 256]).unwrap();

    let mut radio_config = uart::Config::default();
    radio_config.baudrate = 115_200;

    let radio_uart = uart::BufferedUart::new(
        p.UART1,
        p.PIN_8, // TX → RYLR998 RX
        p.PIN_9, // RX ← RYLR998 TX
        Irqs,
        tx_buf,
        rx_buf,
        radio_config,
    );

    let mut radio = rylr998_embassy::Radio::new(radio_uart);
    logln(&mut console, "radio initialized");

    let ping_ok = radio.ping().await.is_ok();
    let set_ok = if ping_ok {
        radio.set_address(5).await.is_ok()
    } else {
        false
    };

    if ping_ok && set_ok {
        logln(&mut console, "ping + set_address ok");
        // GOOD: 3 fast, solid 2s, 3 fast.
        for _ in 0..3 {
            led.set_high();
            embassy_time::Timer::after_millis(500).await;
            led.set_low();
            embassy_time::Timer::after_millis(500).await;
        }
        led.set_high();
        embassy_time::Timer::after_millis(2000).await;
        led.set_low();
        embassy_time::Timer::after_millis(2000).await;
        for _ in 0..3 {
            led.set_high();
            embassy_time::Timer::after_millis(100).await;
            led.set_low();
            embassy_time::Timer::after_millis(100).await;
        }
    } else {
        logln(&mut console, "ping or set_address FAILED");
        // PING FAIL: 3 slow, dark 2s, 3 slow.
        for _ in 0..3 {
            led.set_high();
            embassy_time::Timer::after_millis(400).await;
            led.set_low();
            embassy_time::Timer::after_millis(400).await;
        }
        embassy_time::Timer::after_millis(2000).await;
        for _ in 0..3 {
            led.set_high();
            embassy_time::Timer::after_millis(400).await;
            led.set_low();
            embassy_time::Timer::after_millis(400).await;
        }
    }

    logln(&mut console, "listening");
    loop {
        let _ = radio
            .next_event(Duration::from_secs(60), |e: Event<'_>| -> Option<()> {
                match e {
                    Event::Recv {
                        from,
                        data,
                        rssi,
                        snr,
                    } => {
                        let mut line: String<160> = String::new();
                        let _ = write!(
                            line,
                            "recv from={} rssi={} snr={} len={} data=",
                            from,
                            rssi,
                            snr,
                            data.len(),
                        );
                        match core::str::from_utf8(data) {
                            Ok(s) => {
                                let _ = write!(line, "{:?}", s);
                            }
                            Err(_) => {
                                let _ = write!(line, "<{} bytes non-utf8>", data.len());
                            }
                        }
                        logln(&mut console, &line);
                    }
                    Event::Ready => logln(&mut console, "event: ready"),
                }
                None
            })
            .await;
        // 60s passed with no events (or transport error); keep listening.
    }
}
