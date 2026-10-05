# SD-card install: a Rust image the stock installer accepts

Date: 2026-10-05. Status: draft for owner review (revised after two adversarial reviews and a build spike).

## Purpose

blackbox-rs should be a base anyone can build Blackbox firmware on, in Rust, and install the
way 1010music firmware is installed: copy `BLACKBOX.bin` to the microSD card, hold BACK + INFO
at power-on. No debug probe, no opened case, and always a way back to stock.

Today the crate links at `0x08000000` and flashes over SWD. Run as-is it overwrites 1010music's
installer, which is the only SD-card route back to stock and is not downloadable. This spec
changes that and proves the SD route on real hardware. It adds no drivers.

Applies to the original Blackbox only (STM32H743XI), not the Blackbox 2.

## What is known

| Fact | Source | Confidence |
|---|---|---|
| App loads at `0x08040000`; the installer occupies `0x08000000`–`0x0803FFFF` and is not in `BLACKBOX.bin` | blackbox-mod, custom-blackbox-fw (independent; both patch stock 3.1.9 at that base) | High |
| The installer is its own program with its own screen ("Blackbox Installer", "Looking for File", "Erasing") | custom-blackbox-fw `docs/hardware.md`; the stock app has no `.bin` string | High |
| Stock `BLACKBOX.bin` is a raw image: vector table at offset 0, no header, 728,696 bytes, ends `0x080F1E78` | Read from stock 3.1.9, SHA-256 `281ae303…f1341d` | Verified |
| The installer takes images with changed and appended bytes and no checksum fix-up | Both mod projects install such images | High |
| The installer accepts a program that is not the Blackbox app at all | 1010music's own "Gamechanger" (Doom, 2021) ships as a 389,884-byte `BLACKBOX.BIN` installed the same way. It shares no non-zero word with stock in its first 1 KB and has a different tail, so there is no header, magic or trailer the installer could be checking | Verified from 1010music's file |
| Reaching the installer does not depend on the installed app | Neither stock nor Gamechanger contains the installer's screen text, a `.bin` filename or a flash-unlock key. Owners returned from Gamechanger to stock with BACK + INFO, so the installer reads the buttons before any app runs | High |
| An app that fails to start is a state 1010music expected owners to be in | Gamechanger's notes: without its `doom` folder on the card "it won't boot up" | First-party statement |
| The installer does not pin the initial SP: 3.1.2 starts on AXI SRAM (`0x240609B0`), 3.1.9 on DTCM (`0x20020000`) | custom-blackbox-fw `docs/re-notes.md` | High |
| The stock app sets `VTOR`, the MPU and both caches itself at start | Stock reset handler and `main` | Verified |
| embassy-stm32 0.6 switches to HSI and stops the PLLs before reconfiguring clocks | `rcc/h.rs` | Verified |
| The crate relinks at `0x08040000` with the changes below: `demo` becomes a 66,208-byte flat image, vector table at `0x08040000`, a library-defined `pre_init` links, stack top `0x24080000` (AXI SRAM, where Gamechanger and eight of nine stock builds put theirs) | Throwaway build spike, 2026-10-05 | Verified on host |
| Chip, pins, clocks, SDRAM, display, touch, codec | This crate, tested on its author's unit over SWD, written for rev.Y silicon | Author's claim |

Unknown, and only the hardware can answer:

- the CPU state the installer leaves at the jump (SysTick, caches, MPU, running peripherals).
  The design assumes the worst and clears all of it;
- whether the installer consults anything persistent an app could corrupt. The images here
  never write flash or option bytes, and the build check enforces that. The clock config
  (`rcc.ls` off) keeps embassy from resetting the backup domain when nothing has configured
  the RTC or LSE; that holds by configuration, not by the check, and embassy still sets
  `PWR_CR1.DBP`;
- whether it writes past `0x08100000` (no published image does; out of scope);
- the silicon revision of the owner's unit (`boot_probe` reports it);
- whether flash read-out protection is set, and where the SWD pads are (needed only for the
  backstop).

## Design

### One memory layout

- Drop embassy-stm32's `memory-x` feature. The crate ships its own `memory.x`, emitted by
  `build.rs`, with `FLASH ORIGIN = 0x08040000`, `LENGTH = 768K` (to `0x08100000`, the end of
  flash bank 1). Every published image ends below that address. Raising the limit is a later
  test, not part of this spec.
- RAM regions stay as embassy's H743 layout has them.
- There is no layout that links below `0x08040000`. A probe user flashes the same image to the
  same address and the installer starts it, so neither route can touch the installer.

### Starting behind the installer

The app does not start from reset state and cannot reset its way there (a reset returns to the
installer). In `pre_init`, before any crate code:

- load the main stack pointer and set `VTOR` to `0x08040000` (cortex-m-rt `set-sp`,
  `set-vtor`); the stock app loads its own SP, so the installer may not;
- select the main stack in privileged mode (`CONTROL = 0`), as the stock reset handler does;
- stop SysTick and clear its pending bit (it is not an NVIC interrupt);
- mask every NVIC interrupt and clear pending ones;
- disable the MPU and clear all 16 regions;
- flush and disable the D-cache, which the crate assumes is off;
- pulse every peripheral reset in `RCC` (AHB1–4, APB1–4), so nothing the installer started
  (display, SD, DMA) is still running;
- unmask interrupts (`PRIMASK`, `FAULTMASK`, `BASEPRI`) last, in case the installer jumped
  with them masked; embassy's timers never fire otherwise.

A host test runs the built image in an emulator from its reset vector with all of this state
deliberately dirty and checks it is clean on arrival at `main`.

Clocks need nothing extra: embassy's init already handles a running PLL.

### Seeing a failure

A dark screen must not be the only symptom. LEDs need only GPIO, so:

- `init()` lights one more LED as each stage completes (clocks, SDRAM, display, I2C and codec,
  touch, audio). A hang is then readable from the panel as "stopped after stage N".
- The examples replace `panic-probe` with a panic handler that blinks all LEDs. A panic with
  no probe attached is otherwise a silent halt.

### Packaging

- `just sd [example]` builds release and converts the ELF to a flat `out/BLACKBOX.bin`
  (cargo-binutils `objcopy -O binary`).
- The recipe then runs `tools/check_image.py`, which fails the build unless: the ELF's lowest
  loaded address is exactly `0x08040000`; the initial SP is in RAM; the reset vector is inside
  the image; the image is no larger than 768K; the ELF holds no symbol from
  `embassy_stm32::flash` (an image that cannot write flash cannot damage the installer or
  anything it reads). It prints the size next to stock's 728,696.

### Two images

1. `boot_probe` (new example): no SDRAM, display, codec or audio. It runs `pre_init`, brings
   up clocks, shows the silicon revision (`DBGMCU` `REV_ID`) as an LED pattern, then chases
   the LEDs. It answers one question: does the installer start a Rust image.
2. `demo` (existing), with the tone removed from the SD build: half-scale 440 Hz into
   headphones at power-on is not a safe first sound.

## Hardware test ladder

Run on the owner's unit, in order. Stop at the first failure.

0. Note the firmware version the unit runs now and copy the whole SD card to the computer.
   Keep stock `BLACKBOX.bin` on the computer, never only on the card.
1. Install stock 3.1.9 through the installer. Record what its screen shows.
2. **Gate: the owner decides** whether to continue on the evidence above or wait for a tested
   backstop (see below). The evidence supports continuing: the installer demonstrably runs
   before, and independently of, whatever app is installed.
3. Install `boot_probe`. Pass: LEDs chase within 2 s of power-on; revision pattern recorded.
4. Power off; power on holding BACK + INFO. Pass: the installer appears.
5. Install `demo`. Pass: all stage LEDs lit and the screen drawn within 3 s of power-on;
   buttons, knobs and touch respond.
6. Power off; BACK + INFO again; install stock 3.1.9. Pass: the unit is back to stock.

Success is 3–6 passing. The README then documents the SD route as the default.

## Failure handling

- **Installer refuses or fails the file:** it prints "Erasing" straight after finding the
  file, so assume the old app is already gone. The installer itself is untouched: put stock on
  the card and install again.
- **Image installs, LEDs stay dark, installer still appears:** restore stock; the startup
  assumptions are wrong.
- **Image installs and the installer no longer appears:** the unit needs SWD. This is the
  case the gate exists for.

## What a backstop actually requires

A probe in a drawer is not a backstop. It becomes one only after, on the opened unit:

- the SWD pads are found (lead: the teardown video 1010music staff link from their own
  forum, https://www.youtube.com/watch?v=1XRyD7wd4Uw; not yet reviewed);
- read-out protection is checked. If it is set, writing over SWD first requires a mass erase,
  which destroys the installer for good;
- if it is not set, the first action is to read out and keep the installer.

Also unexplored: the chip's built-in USB loader (BOOT0), which is how the PreenFM3 recovers.

## Out of scope

SD card, MIDI, USB, audio input, the six other codec outputs, images past `0x08100000`, and
any port of Chimera. Each is its own spec after this one passes.

## Repository rules

No 1010music firmware, patched images, installer dumps or disassembly listings are committed.
The stock image is downloaded by the user from 1010music and stays out of the repo.
