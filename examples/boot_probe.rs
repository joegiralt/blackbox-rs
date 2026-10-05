//! Boot probe: the smallest image that proves the installer starts a Rust image.
//!
//! Touches clocks and LEDs only: no SDRAM, display, I2C, codec, SAI or flash.
//!
//! What to see on the panel:
//! - For 3 s, the silicon revision on LEDs 0-3 (the crate was written for revision Y):
//!   LED 0 = Y, LED 1 = V, LED 2 = Z, LED 3 = X, LEDs 0-3 together = unknown revision.
//! - Then one LED chases across all 11 LEDs, 100 ms per step, forever.
//!
//! A fault blinks the LEDs instead (see `common/fault.rs`).

#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::pac::DBGMCU;
use embassy_time::Timer;

use blackbox_rs::{clock, leds::Leds};

#[path = "common/fault.rs"]
mod fault;

/// LED mask (bit `n` = LED `n`) for the silicon revision, from DBGMCU IDC bits 31:16:
///
/// | REV_ID   | revision | LED       |
/// |----------|----------|-----------|
/// | `0x1003` | Y        | 0         |
/// | `0x2003` | V        | 1         |
/// | `0x1001` | Z        | 2         |
/// | `0x2001` | X        | 3         |
/// | other    | unknown  | 0-3 (all) |
fn rev_leds(rev_id: u16) -> u16 {
    match rev_id {
        0x1003 => 1 << 0,
        0x2003 => 1 << 1,
        0x1001 => 1 << 2,
        0x2001 => 1 << 3,
        _ => 0b1111,
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(clock::config());

    let mut leds = Leds::new([
        p.PG9.into(),
        p.PJ8.into(),
        p.PB10.into(),
        p.PB8.into(),
        p.PB9.into(),
        p.PK2.into(),
        p.PA5.into(),
        p.PJ5.into(),
        p.PJ4.into(),
        p.PB11.into(),
        p.PA4.into(),
    ]);

    let rev_id = DBGMCU.idc().read().rev_id();
    defmt::info!("boot_probe: REV_ID = {=u16:#x}", rev_id);

    let mask = rev_leds(rev_id);
    for n in 0..4 {
        leds.set(n, mask & (1 << n) != 0);
    }
    Timer::after_secs(3).await;

    loop {
        for i in 0..leds.count() {
            leds.only(i);
            Timer::after_millis(100).await;
        }
    }
}
