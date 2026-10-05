# Hardware test, October 2026: SD-card install

The ladder from `docs/superpowers/specs/2026-10-05-sd-boot-design.md`, run by the owner on
their unit. Run the steps in order and stop at the first failure. Fill in each result line.

Build images with `just sd boot_probe` and `just sd demo`. Each writes `out/BLACKBOX.bin` only
if the image passes the check. Copy it to the root of the card.

## What you will see

LEDs are numbered 0-10 in board order (`src/leds.rs`).

| Panel | Meaning |
|---|---|
| All LEDs dark | The image never reached `main`: the installer did not start it, or the startup cleanup hung. Restore stock. |
| LED 10 only | `main` ran; stopped in embassy's clock bring-up (HSE or PLL never became ready). |
| LED 0 only, for 3 s (`boot_probe`) | Revision Y, what the crate was written for. |
| LED 1 only, for 3 s (`boot_probe`) | Revision V. |
| LED 2 only, for 3 s (`boot_probe`) | Revision Z. |
| LED 3 only, for 3 s (`boot_probe`) | Revision X. |
| LEDs 0-3 together, for 3 s (`boot_probe`) | Unknown revision; note the pattern. |
| One LED chasing across all 11, 100 ms per step (`boot_probe`) | The image runs, timers work. Pass for step 3. |
| LEDs 0-5 lit, then dark after 1 s, then lighting with buttons (`demo`) | All six stages reached; the LEDs now follow buttons 0-10. |
| Stage LEDs stopped at N (`demo`: LEDs 0..N lit, nothing else) | LED N is the last stage reached (0 clocks, 1 SDRAM, 2 display, 3 codec, 4 touch, 5 audio/SAI); it hung in the work after it. LEDs 3 and 4 mean the step was reached, not that the codec or touch answered. |
| All 11 LEDs fast-blinking together | Panic, hard fault or other unhandled exception. The rate follows whatever clock was running, so it does not tell you when the fault happened. |
| The sequence restarting every few seconds | A watchdog, probably started by the installer; the app cannot stop it. |

In `demo`, LED 10 lights first and goes out as LED 0 lights.

## Ladder

### 0. Back up

Note the firmware version the unit runs now and copy the whole SD card to the computer. Keep
stock `BLACKBOX.bin` on the computer, never only on the card.

Result: 2026-10-05. Unit runs stock 3.0.9 (TOOLS screen). Card held stock 3.0.9 `BLACKBOX.bin` (691,452 bytes, SHA-256 `e68b5345…`), kept on the card as `BLACKBOX-stock-3.0.9.bin`. Whole card (2,184 files, 5.0 GB) copied to the computer and compared by count, size and time.

### 1. Stock through the installer

Install stock 3.1.9 with BACK + INFO held at power-on. Record what the installer's screen
shows.

Result: skipped by the owner's decision. The way back is 3.0.9, the version the unit ran, not 3.1.9.

### 2. Gate

The owner decides whether to continue on the evidence in the spec or wait for a tested
backstop.

Result: 2026-10-05. Owner said go on the evidence, no probe on hand.

### 3. `boot_probe`

Install `boot_probe`. Pass: the started LED (LED 10) lights at once, a revision LED within
about 1 s, the chase after 3 s. Record the revision LED.

Result: PASS, 2026-10-05, image `boot_probe` at 803cb06 (19,528 bytes, SHA-256 `3d36cb60…`). Installer showed "Erasing", then "installing new software", then a factory test menu (MIDI TRS loopback, clock, audio loopback, sine wave, pot input, touch; INFO steps through them). After a power cycle with no buttons held: revision LED KEYS (LED 1 = revision V, `REV_ID` 0x2003), then the chase. Screen dark, as expected. LED order on the panel: PADS, KEYS, SEQS, SONG, FX, MIX, PSET, TOOLS, REC, STOP, PLAY (LED 10, the started LED, is PLAY).

### 4. Back to the installer

Power off; power on holding BACK + INFO. Pass: the installer appears.

Result: PASS, 2026-10-05. With `boot_probe` installed, BACK + INFO at power-on brought up the installer ("installer", then "Erasing").

### 5. `demo`

Install `demo`. Pass: all six stage LEDs light and the screen is drawn within 3 s of
power-on. The stage LEDs stay lit for only 1 s, then go dark and follow the buttons, so
watch the panel from power-on. Buttons, knobs and touch respond; still responsive after
5 minutes running.

Result:

### 6. Back to stock

Power off; BACK + INFO; install stock 3.1.9. Pass: the unit is back to stock.

Result:
