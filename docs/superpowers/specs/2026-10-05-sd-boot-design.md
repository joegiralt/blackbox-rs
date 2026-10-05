# SD-card install: a Rust image the stock updater accepts

Date: 2026-10-05. Status: draft for owner review.

## Purpose

blackbox-rs should be a base anyone can build Blackbox firmware on, in Rust, and install the
way 1010music firmware is installed: copy `BLACKBOX.bin` to the microSD card, hold BACK + INFO
at power-on. No debug probe, no opened case, and always a way back to stock.

Today the crate links at `0x08000000` and flashes over SWD. Run as-is it overwrites 1010music's
bootloader, which is the only SD-card route back to stock. This spec changes that and proves
the SD route on real hardware. It adds no drivers.

Applies to the original Blackbox only (STM32H743XI), not the Blackbox 2.

## What is known

| Fact | Source | Confidence |
|---|---|---|
| App loads at `0x08040000`; bootloader occupies `0x08000000`–`0x0803FFFF` | blackbox-mod, custom-blackbox-fw (both patch stock 3.1.9 at that base) | High |
| Stock `BLACKBOX.bin` is a raw image: vector table at offset 0 (SP `0x20020000`, reset `0x080403C1`), no header, 728,696 bytes | Read from stock 3.1.9, SHA-256 `281ae303…f1341d` | Verified |
| The updater accepts images with changed and appended bytes, with no checksum fix-up | blackbox-mod's `patch.py` writes the file directly and its users install it | High |
| The updater lives in the bootloader, not the app | The stock app has plain ASCII strings (`preset.xml`) but no `.bin` or `BLACKBOX.bin` | Inference |
| Chip, pins, clocks, SDRAM, display, touch, codec | This crate, tested on its author's unit over SWD | Author's claim |

Unknown: whether the updater checks anything a patched stock image keeps but a fresh image
lacks; the largest image it accepts; whether BACK + INFO still reaches the updater when the
installed app hangs.

## Design

### One memory layout

- Drop embassy-stm32's `memory-x` feature. The crate ships its own `memory.x`, emitted by
  `build.rs`, with `FLASH ORIGIN = 0x08040000`, `LENGTH = 1792K` (to the end of the 2 MB part).
  RAM regions stay as embassy's H743 layout has them.
- There is no layout that links below `0x08040000`. A probe user flashes the same image to the
  same address and the bootloader starts it, so neither route can touch the bootloader.

### Starting behind a bootloader

The app no longer starts from reset state. In a `pre_init` hook, before any crate code:

- set `VTOR` to `0x08040000` (cortex-m-rt `set-vtor`);
- mask every NVIC interrupt and clear pending ones;
- confirm embassy's H7 clock init copes with a PLL already driving the system clock; if it does
  not, switch to HSI and reset `RCC` to its reset values first.

### Packaging

- `just sd [example]` builds release and converts the ELF to a flat `out/BLACKBOX.bin`
  (cargo-binutils `objcopy -O binary`).
- The recipe then runs `tools/check_image.py`, which fails the build unless: the ELF's lowest
  loaded address is exactly `0x08040000`; the initial SP is in RAM; the reset vector is inside
  the image; the image fits the flash region. It prints the size next to stock's 728,696 bytes.
- The first image stays smaller than stock, so size is not a variable in the first test.

### First image

The existing `demo` example: screen, LEDs, buttons, knobs, 440 Hz tone. It logs over
defmt-RTT, which writes to a RAM buffer and does not block with no probe attached.

## Hardware test ladder

Run on the owner's unit, in order. Stop at the first failure.

1. Install stock 3.1.9 through the updater. Proves the procedure on this unit at no risk.
2. **Gate: the owner decides** whether to continue on the evidence above or wait until an SWD
   probe is on hand.
3. Install the Rust image. Pass: the demo screen appears.
4. Power off; power on holding BACK + INFO. Pass: the updater appears.
5. Install stock 3.1.9. Pass: the unit is back to stock.

Success is all of 3–5 passing. The README then documents the SD route as the default.

## Failure handling

- **Updater rejects the image:** the unit stays on stock. Compare what the updater could be
  checking (vector-table values, size, padding) against stock and retry.
- **Image installs, demo does not appear, updater still appears:** restore stock, debug the
  startup assumptions.
- **Image installs and the updater no longer appears:** the unit needs SWD to recover. This is
  the case step 2 exists for.

## Out of scope

SD card, MIDI, USB, audio input, the six other codec outputs, and any port of Chimera. Each is
its own spec after this one passes.

## Repository rules

No 1010music firmware, patched images, or disassembly listings are committed. The stock image
is downloaded by the user from 1010music and stays out of the repo.
