//! Pico ↔ RYLR998 smoke test.
//!
//! ## EXERCISE
//!
//! Wire a RYLR998's TX → Pico GP1 (UART0 RX), RX → Pico GP0 (UART0 TX), GND-GND, 3V3-VDD.
//!
//! Then implement the `main()` body using `rylr_embassy::Radio<...>`:
//! 1. Init UART0 at 115200 baud on GP0/GP1.
//! 2. `radio.ping().await` → log result.
//! 3. `radio.set_address(5).await`.
//! 4. Loop: `radio.next_event(Duration::from_secs(60)).await` → `info!`.
//!
//! ### Boilerplate
//!
//! The cortex-m / embassy entrypoint scaffolding is below. You only need
//! to fill in the `// TODO:` block.

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts, peripherals, uart};
use embassy_time::Duration;

use defmt_rtt as _;
use panic_probe as _;

bind_interrupts!(struct Irqs {
    UART0_IRQ => uart::InterruptHandler<peripherals::UART0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // TODO: configure UART0 on GP0 (TX) / GP1 (RX) at 115200 8N1, then
    //       construct rylr_embassy::Radio::new(...) and exercise it as
    //       described in the module docs above.
    let _ = (p, Irqs, Duration::from_secs(60));
    info!("rylr_embassy::pico_smoke — fill me in");
    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}
