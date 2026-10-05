# blackbox-rs

Board support crate for the **Blackbox** board — STM32H743XI (Cortex-M7 @ 399.36 MHz, rev.Y), built on [Embassy](https://embassy.dev).

`blackbox_rs::init()` brings the whole board up at its design clocks and returns ready-to-use drivers:

- **11 LEDs**, **13 buttons**, **4 endless encoders** (absolute angle via ADC1)
- **GT9147 touchscreen** (I2C)
- **320×240 RGB565 display** — LTDC double-buffered, [`embedded-graphics`](https://crates.io/crates/embedded-graphics) draw target
- **CS42528 audio** — SAI1 I2S stereo out (headphone DACs), 48 kHz

The rev.Y cache erratum, SDRAM timings and the LTDC pin map are handled inside the crate.

![Debug screen captured from the panel framebuffer](docs/debug-screen.png)

## Quick start

```toml
[dependencies]
blackbox-rs = "0.1"
embassy-executor = { version = "0.10", features = ["platform-cortex-m", "executor-thread"] }
embassy-time = "0.5"
```

```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use blackbox_rs::buttons::Button;
use defmt_rtt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut board = blackbox_rs::init().await; // clocks, SDRAM, every peripheral, codec + SAI

    loop {
        if board.buttons.is_pressed(Button::Play) {
            defmt::info!("play!");
        }
        board.display.swap().await; // present at vblank
    }
}
```

`init()` returns a `Board { display, leds, buttons, knobs, touch, i2c, codec_ok, audio }`.

A binary must also supply its own `#[panic_handler]` (and, for a visible fault, `HardFault` and `DefaultHandler` exception handlers). On an SD-installed unit there is no probe to read a panic message, so call `blackbox_rs::leds::panic_blink()` from all three; the examples share one implementation in `examples/common/fault.rs`. The crate links at `0x08040000` through its own `memory.x` (see Memory layout), and your binary needs the usual `-Tlink.x` rustflag from `.cargo/config.toml`.

## Using the peripherals

```rust
board.leds.set(0, true);                     // or board.leds.only(3)

for b in Button::ALL {                       // typed, no magic indices
    if board.buttons.is_pressed(b) { defmt::info!("{}", b.label()); }
}

let r = board.knobs.read(Knob::Tl);          // absolute angle from two wipers
defmt::info!("TL = {} deg", r.angle_deg());

if let Some(tp) = board.touch.poll(&mut board.i2c) {
    defmt::info!("touch {},{}", tp.x, tp.y);
}

// Display — embedded-graphics on the back buffer, then swap
use embedded_graphics::{prelude::*, pixelcolor::Rgb565, primitives::*};
Circle::new(Point::new(160, 120), 40)
    .into_styled(PrimitiveStyle::with_fill(Rgb565::CYAN))
    .draw(&mut board.display.target()).ok();
board.display.swap().await;
board.display.set_backlight(20);             // percent, capped at 35%

// Audio — write interleaved L/R 24-bit samples; see examples/audio_tone.rs and
// examples/common/tone.rs for a sine task
let buf = [0u32; blackbox_rs::audio::HALF_BUFFER_LEN];
board.audio.write(&buf).await.unwrap();
```

`Button` and `Knob` have `::ALL`, `.index()` and `.label()`.

## Install from the SD card

This is the default route and needs no debug probe. It works the way 1010music's own firmware updates do: the stock installer reads `BLACKBOX.bin` from the card and writes it to flash.

**Original Blackbox only, not the Blackbox 2.** This project is unofficial: it is not affiliated with, endorsed by or supported by 1010music, and running modified firmware is at your own risk. Keep a copy of the stock `BLACKBOX.bin` on a computer before you start; it is available from <https://1010music.com/downloads>.

Tools, once:

```sh
rustup target add thumbv7em-none-eabihf
rustup component add llvm-tools
cargo install cargo-binutils
# and `just` from your package manager
```

Build and check an image:

```sh
just sd boot_probe   # recommended first image: clocks and LEDs only
just sd demo         # full board: controls and panel, no audio
```

`just sd <example>` first deletes any `out/BLACKBOX.bin`, builds the flat image under a temporary name and runs `tools/check_image.py` on it (linked at `0x08040000`, sane vectors, at most 768K, no embassy-stm32 flash code, the startup cleanup in `src/boot.rs` linked in). Only an image that passes becomes `out/BLACKBOX.bin`, and the recipe prints the example name and size; on a failure no `out/BLACKBOX.bin` exists.

Install: copy `out/BLACKBOX.bin` to the root of the microSD card, put the card in the unit, and power on while holding **BACK + INFO**. To return to stock, do the same with 1010music's own `BLACKBOX.bin`.

Status: this route has been verified on the host (build, image check, emulator boot test) but has not yet been run on real hardware. Images are capped at 768K (the end of flash bank 1); whether the installer writes past that is untested. The evidence behind the route is in [`docs/superpowers/specs/2026-10-05-sd-boot-design.md`](docs/superpowers/specs/2026-10-05-sd-boot-design.md).

`just test` runs the image-check unit tests, builds the demo image under `target/boot-test/` and boots it in an emulator from dirty installer state; it never touches `out/BLACKBOX.bin`. It needs a `.venv` with `unicorn==2.1.4`.

### Memory layout

1010music's installer lives in flash at `0x08000000`-`0x0803FFFF`. It is not part of any downloadable firmware file, so if it is overwritten there is no SD-card route back to stock. This crate therefore links at `0x08040000` (`memory.x`: 768K of flash, 512K AXI SRAM) and its `build.rs` puts that `memory.x` on the linker path. A downstream `memory.x` that links lower will overwrite the installer when flashed with a probe. A downstream crate has no `just sd`: run `python3 tools/check_image.py <elf> <bin>` from this repository on your own ELF and flat image, and install only if it exits 0. Never link below `0x08040000`.

Because the installer starts the image rather than the chip's reset, `src/boot.rs` runs a `__pre_init` that returns the core to a clean state (stack, interrupts, MPU, D-cache, peripheral resets) before `main`.

### Probe route (secondary)

**Untested with the installer present.** `.cargo/config.toml` still sets a `probe-rs run --chip STM32H743XI` runner, and the image links at `0x08040000` either way, so the build never targets the installer's region. But a reset boots the installer at `0x08000000`, not this image, and whether it then starts a probe-flashed image has not been tried.

```sh
cargo install probe-rs-tools

cargo run --release --example boot_probe
cargo run --release --example demo
cargo run --release --example audio_tone  # audio only: clocks + codec + SAI
```

Do not use a probe on a unit you want to keep stock-restorable until flash read-out protection has been checked: if it is set, a probe write needs a mass erase first, which removes the installer for good, and it cannot be downloaded. If read-out protection is off, read out and keep a copy of the installer region (`0x08000000`-`0x0803FFFF`) before writing anything. Do not commit or share that copy. Whether `probe-rs` erases only the sectors it writes has not been checked on a unit either.

### LED language

The LEDs report startup and failure with no probe attached.

- LED 10 alone is the "started" LED (`leds::started()`): lit first thing in `main`, before the clock bring-up, which waits on oscillators with no timeout. All dark means the image never reached `main`; LED 10 alone means it stopped in clock bring-up. It goes out when the stage LEDs take over.
- `init()` lights one stage LED per step reached: LED 0 clocks, 1 SDRAM, 2 display, 3 codec, 4 touch, 5 audio (SAI). A hang leaves the count of what was reached. LEDs 3 and 4 mean the step ran, not that it worked: they light whether or not the codec or touch answered (`board.codec_ok` and the touch log say which).
- In `demo` the stage LEDs go dark after 1 s, because the LEDs then follow the buttons.
- All 11 LEDs fast-blinking together is a panic, hard fault or other unhandled exception (`leds::panic_blink()`). The rate follows whatever clock is running, so it says nothing about when the fault happened.
- `boot_probe` shows the silicon revision on LEDs 0-3 for 3 s, then chases one LED across all 11 forever:

| LED | Revision |
|-----|----------|
| 0 | Y (what this crate was written for) |
| 1 | V |
| 2 | Z |
| 3 | X |
| 0-3 together | unknown |

## Hardware notes

- **rev.Y erratum ES0392** — D-cache stays off; the MPU marks SDRAM + D2 SRAM non-cacheable so LTDC/SAI DMA stay coherent without maintenance. The crate uses embassy-stm32 feature `stm32h743xi` (not `stm32h743v`, which hangs ADC power-up).
- **Backlight capped at 35%** (`display::MAX_BACKLIGHT_PCT`) — boost-regulator thermal limit; `set_backlight` clamps.
- **Touch** latches its I2C address at the chip's own power-on from INT (PG12): `0x5D` operational / `0x14` degraded. It re-latches only on a real power cycle — if the log shows `@ 0x14`, power-cycle the board.
- Clocks: HSE 6.144 MHz → PLL1 399.36 MHz sysclk, PLL2 12.288 MHz SAI (256 × 48 kHz), PLL3 6.4 MHz LTDC.

| Block | Pins / notes |
|-------|--------------|
| Display LTDC | 25 pins AF14; panel power **PK7**; R2 not routed |
| Audio SAI1 | PE2 MCLK, PE5 SCK, PE4 FS, PB2 SD (AF6); CS42528 @ 0x4C |
| SDRAM FMC | AF12; 2× IS42S16160J, 64 MB @ 0xC000_0000 |
| I2C1 | PB6 SCL / PB7 SDA (AF4), 400 kHz; codec + touch share it |
| Touch GT9147 | INT **PG12** |
| Buttons ×13 | active-low; PA0/PA1/PC2/PC3 need the SYSCFG dual-pad fix |
| LEDs ×11 / Knobs ×4 | LEDs active-high; knobs = 8 wipers on ADC1, `atan2` angle |

## License

MIT OR Apache-2.0
