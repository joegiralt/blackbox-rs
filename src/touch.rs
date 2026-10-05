//! Goodix GT9147 capacitive touch over I2C1 (README §touch).
//!
//! The I2C address latches at chip POR from the INT level (low→0x5D, high→0x14). PG12 is
//! high-Z until firmware drives it, so we bias it low briefly then probe both addresses.
//! The chip's X axis runs down the panel and its Y axis across it, and its output range is
//! whatever its stored config says (320 × 240 on the unit measured), not the panel's pixel
//! grid — so points are swapped and scaled by the range read back at start-up.

use embassy_stm32::gpio::{AnyPin, Flex, Pull, Speed};
use embassy_stm32::i2c::{I2c, Master};
use embassy_stm32::mode::Blocking;
use embassy_stm32::Peri;
use embassy_time::Timer;

/// Drive PG12 (INT) low push-pull. Call as early as possible in board bring-up — the GT9147
/// samples this line across its own power-on reset, so holding it low through bring-up biases
/// the address toward 0x5D and lets the chip settle against a defined level. Hand the result
/// to [`Touch::new`], which floats it once bring-up is done.
pub fn bias_int_low(int_pin: Peri<'static, AnyPin>) -> Flex<'static> {
    let mut int = Flex::new(int_pin);
    int.set_as_output(Speed::Low);
    int.set_low();
    int
}

/// A touch report: number of active points and the first point in panel coordinates.
#[derive(Clone, Copy)]
pub struct TouchPoint {
    pub count: u8,
    pub x: u16,
    pub y: u16,
}

/// GT9147 driver. Holds the detected address; the shared I2C1 bus (codec shares it) is passed
/// in per call.
pub struct Touch {
    addr: Option<u8>,
    range: (u16, u16),
}

/// Panel size in pixels; touch points are scaled into it.
const PANEL: (u16, u16) = (320, 240);

/// The chip's (X, Y) output range when its config cannot be read: measured on a unit.
const DEFAULT_RANGE: (u16, u16) = (320, 240);

/// Chip coordinates → panel pixels. Chip X runs down the panel, chip Y across it.
fn to_panel(raw_x: u16, raw_y: u16, range: (u16, u16)) -> (u16, u16) {
    let scale = |v: u16, from: u16, to: u16| ((v as u32 * to as u32) / from as u32).min(to as u32 - 1) as u16;
    (scale(raw_y, range.1, PANEL.0), scale(raw_x, range.0, PANEL.1))
}

impl Touch {
    /// Take the early-biased PG12 (still driven low — see [`bias_int_low`]), hold it low a
    /// touch longer to cover the chip's POR window, then float it as an input so the chip can
    /// drive INT itself. Then probe both addresses and start normal scan.
    pub async fn new(i2c: &mut I2c<'_, Blocking, Master>, mut int: Flex<'static>) -> Self {
        Timer::after_millis(50).await;
        int.set_as_input(Pull::None);
        Timer::after_millis(50).await;
        core::mem::forget(int); // leave PG12 floating for the run

        let mut addr = None;
        for a in [0x5Du8, 0x14] {
            let mut id = [0u8; 4]; // product id ASCII at 0x8140, e.g. "9147"
            if i2c.blocking_write_read(a, &[0x81, 0x40], &mut id).is_ok() {
                defmt::info!("touch: GT9147 @ {=u8:#04x} id {=[u8]:a}", a, id);
                addr = Some(a);
                break;
            }
        }
        let mut range = DEFAULT_RANGE;
        if let Some(a) = addr {
            // X/Y output max, u16 LE each, from the config the chip already holds (0x8048).
            let mut max = [0u8; 4];
            if i2c.blocking_write_read(a, &[0x80, 0x48], &mut max).is_ok() {
                let (x, y) = (u16::from_le_bytes([max[0], max[1]]), u16::from_le_bytes([max[2], max[3]]));
                if x != 0 && y != 0 {
                    range = (x, y);
                }
            }
            defmt::info!("touch: output range {=u16} x {=u16}", range.0, range.1);
            let _ = i2c.blocking_write(a, &[0x80, 0x40, 0x00]); // normal scan mode
            Timer::after_millis(25).await;
            let _ = i2c.blocking_write(a, &[0x81, 0x4E, 0x00]); // clear status latch
        } else {
            defmt::info!("touch: GT9147 not responding on 0x5d/0x14");
        }
        Self { addr, range }
    }

    /// The chip's (X, Y) output range used for scaling: its stored config, or the default.
    pub fn range(&self) -> (u16, u16) {
        self.range
    }

    pub fn detected(&self) -> bool {
        self.addr.is_some()
    }

    /// Poll the first touch point, or `None` if no fresh data. Always clears the status
    /// latch (the chip stops scanning otherwise). Points are in panel pixels.
    pub fn poll(&self, i2c: &mut I2c<'_, Blocking, Master>) -> Option<TouchPoint> {
        let addr = self.addr?;
        let mut st = [0u8; 1];
        i2c.blocking_write_read(addr, &[0x81, 0x4E], &mut st).ok()?;
        if st[0] & 0x80 == 0 {
            return None; // no new data; leave the latch alone
        }
        let count = st[0] & 0x0F;
        let mut pt = [0u8; 8]; // [id, x_lo, x_hi, y_lo, y_hi, area_lo, area_hi, _]
        let read = if count > 0 {
            i2c.blocking_write_read(addr, &[0x81, 0x4F], &mut pt)
        } else {
            Ok(())
        };
        let _ = i2c.blocking_write(addr, &[0x81, 0x4E, 0x00]); // ALWAYS clear or scanning stops
        read.ok()?;
        if count == 0 {
            return None;
        }
        let raw_x = u16::from_le_bytes([pt[1], pt[2]]);
        let raw_y = u16::from_le_bytes([pt[3], pt[4]]);
        let (x, y) = to_panel(raw_x, raw_y, self.range);
        Some(TouchPoint { count, x, y })
    }
}
