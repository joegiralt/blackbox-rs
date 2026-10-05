//! 11 active-high indicator LEDs (README §LEDs).

use embassy_stm32::gpio::{AnyPin, Level, Output, Speed};
use embassy_stm32::Peri;

/// Number of LEDs on the board.
pub const COUNT: usize = 11;

/// The board's LED bank. Construct via [`crate::init`].
pub struct Leds {
    out: [Output<'static>; COUNT],
}

impl Leds {
    /// Pins in board order; pass already-degraded [`AnyPin`]s.
    pub fn new(pins: [Peri<'static, AnyPin>; COUNT]) -> Self {
        Self {
            out: pins.map(|p| Output::new(p, Level::Low, Speed::Low)),
        }
    }

    /// Drive LED `i` on or off.
    pub fn set(&mut self, i: usize, on: bool) {
        if on {
            self.out[i].set_high();
        } else {
            self.out[i].set_low();
        }
    }

    /// Light exactly LED `i`, clearing the rest (handy for a heartbeat/chase).
    pub fn only(&mut self, i: usize) {
        for (n, led) in self.out.iter_mut().enumerate() {
            if n == i {
                led.set_high();
            } else {
                led.set_low();
            }
        }
    }

    pub const fn count(&self) -> usize {
        COUNT
    }
}

/// LED pins in board order as (GPIO port, pin).
const PINS: [(embassy_stm32::pac::gpio::Gpio, usize); COUNT] = {
    use embassy_stm32::pac::{GPIOA, GPIOB, GPIOG, GPIOJ, GPIOK};
    [
        (GPIOG, 9),
        (GPIOJ, 8),
        (GPIOB, 10),
        (GPIOB, 8),
        (GPIOB, 9),
        (GPIOK, 2),
        (GPIOA, 5),
        (GPIOJ, 5),
        (GPIOJ, 4),
        (GPIOB, 11),
        (GPIOA, 4),
    ]
};

/// LED lit by [`started`]: the last in board order, so stage LED 0 is never mistaken for it.
pub const STARTED: usize = 10;

/// Light LED [`STARTED`] (PA4) before anything that can hang. Call it first thing in `main`,
/// before `embassy_stm32::init`, whose clock bring-up waits on oscillators without a timeout:
/// a unit dark after install never started, one with only this LED stopped in clock bring-up.
/// Raw register access like [`panic_blink`]; [`Leds::new`] drives it low again.
pub fn started() {
    use embassy_stm32::pac::gpio::vals::{Moder, Odr, Ot};
    let (port, n) = PINS[STARTED];
    embassy_stm32::pac::RCC.ahb4enr().modify(|w| w.set_gpioaen(true));
    cortex_m::asm::delay(1_000); // clock-enable settle
    port.odr().modify(|w| w.set_odr(n, Odr::HIGH));
    port.otyper().modify(|w| w.set_ot(n, Ot::PUSH_PULL));
    port.moder().modify(|w| w.set_moder(n, Moder::OUTPUT));
}

/// Fast-blink all LEDs forever (the rate follows the core clock: roughly 2 Hz on the 64 MHz
/// HSI, faster once the PLL is up). For panics and faults: masks interrupts, then
/// needs nothing initialised: enables the port clocks and forces the pins to push-pull
/// outputs with raw register access (no HAL, no `defmt`, no allocation, cannot panic).
pub fn panic_blink() -> ! {
    use embassy_stm32::pac::gpio::vals::{Moder, Odr, Ot};
    cortex_m::interrupt::disable();
    embassy_stm32::pac::RCC.ahb4enr().modify(|w| {
        w.set_gpioaen(true);
        w.set_gpioben(true);
        w.set_gpiogen(true);
        w.set_gpiojen(true);
        w.set_gpioken(true);
    });
    cortex_m::asm::delay(1_000); // clock-enable settle
    for (port, n) in PINS {
        port.otyper().modify(|w| w.set_ot(n, Ot::PUSH_PULL));
        port.moder().modify(|w| w.set_moder(n, Moder::OUTPUT));
    }
    let mut on = true;
    loop {
        for (port, n) in PINS {
            port.odr().modify(|w| w.set_odr(n, Odr::from(on as u8)));
        }
        on = !on;
        cortex_m::asm::delay(16_000_000);
    }
}
