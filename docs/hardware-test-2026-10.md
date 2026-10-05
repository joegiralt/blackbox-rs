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
| All 11 LEDs fast-blinking together | Panic, hard fault or other unhandled exception. Roughly 2 Hz if it happened before the PLL was up, faster after. |
| The sequence restarting every few seconds | A watchdog, probably started by the installer; the app cannot stop it. |

In `demo`, LED 10 lights first and goes out as LED 0 lights.

## Ladder

### 0. Back up

Note the firmware version the unit runs now and copy the whole SD card to the computer. Keep
stock `BLACKBOX.bin` on the computer, never only on the card.

Result:

### 1. Stock through the installer

Install stock 3.1.9 with BACK + INFO held at power-on. Record what the installer's screen
shows.

Result:

### 2. Gate

The owner decides whether to continue on the evidence in the spec or wait for a tested
backstop.

Result:

### 3. `boot_probe`

Install `boot_probe`. Pass: the started LED (LED 10) lights at once, a revision LED within
about 1 s, the chase after 3 s. Record the revision LED.

Result:

### 4. Back to the installer

Power off; power on holding BACK + INFO. Pass: the installer appears.

Result:

### 5. `demo`

Install `demo`. Pass: all six stage LEDs lit and the screen drawn within 3 s of power-on;
buttons, knobs and touch respond; still responsive after 5 minutes running.

Result:

### 6. Back to stock

Power off; BACK + INFO; install stock 3.1.9. Pass: the unit is back to stock.

Result:
