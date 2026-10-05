"""Boot the built image from its reset vector with the installer's worst-case leftovers;
on arrival at `main` the stack, privilege, masks, SysTick, NVIC, MPU, cache and RCC are clean."""
import struct
import sys

from unicorn import UcError, Uc, UC_ARCH_ARM, UC_MODE_THUMB, UC_MODE_MCLASS, UC_HOOK_MEM_WRITE
from unicorn.arm_const import (UC_CPU_ARM_CORTEX_M7, UC_ARM_REG_BASEPRI, UC_ARM_REG_CONTROL,
                               UC_ARM_REG_FAULTMASK, UC_ARM_REG_MSP, UC_ARM_REG_PC,
                               UC_ARM_REG_PRIMASK, UC_ARM_REG_PSP, UC_ARM_REG_SP)

from check_image import elf_symbols

example = sys.argv[1]                                     # usage: test_boot.py <example> [image]
image = open(sys.argv[2] if len(sys.argv) > 2 else "out/BLACKBOX.bin", "rb").read()
sym = elf_symbols(open(f"target/thumbv7em-none-eabihf/release/examples/{example}", "rb").read())

uc = Uc(UC_ARCH_ARM, UC_MODE_THUMB | UC_MODE_MCLASS)
uc.ctl_set_cpu_model(UC_CPU_ARM_CORTEX_M7)
uc.mem_map(0x08000000, 0x200000); uc.mem_write(0x08040000, image)
uc.mem_map(0x24000000, 0x80000);  uc.mem_map(0x20000000, 0x20000)
uc.mem_map(0xE000E000, 0x1000)    # SCS as plain RAM
uc.mem_map(0x58020000, 0x5000)    # GPIO + RCC as plain RAM


def mem32(addr):
    return struct.unpack("<I", uc.mem_read(addr, 4))[0]


def poke32(addr, value):
    uc.mem_write(addr, struct.pack("<I", value))


# What the installer might leave behind.
uc.reg_write(UC_ARM_REG_MSP, 0x20001000)
uc.reg_write(UC_ARM_REG_PSP, 0x20002000)
uc.reg_write(UC_ARM_REG_CONTROL, 2)
uc.reg_write(UC_ARM_REG_PRIMASK, 1)
uc.reg_write(UC_ARM_REG_FAULTMASK, 1)
uc.reg_write(UC_ARM_REG_BASEPRI, 0x80)
poke32(0xE000E010, 7)             # SysTick CSR: running, interrupting
poke32(0xE000ED94, 5)             # MPU CTRL: ENABLE | PRIVDEFENA
poke32(0xE000ED14, 0x00030200)    # CCR: DC | IC | STKALIGN
poke32(0xE000ED80, 0xF003E019)    # CCSIDR for the set/way loops
poke32(0xE000ED24, 0x00070000)    # SHCSR: Mem/Bus/UsageFault enabled
poke32(0xE000ED10, 0x16)          # SCR: SLEEPONEXIT | SLEEPDEEP | SEVONPEND

writes = []
uc.hook_add(UC_HOOK_MEM_WRITE,
            lambda _uc, _acc, addr, size, value, _d: writes.append((addr, value & ((1 << 8 * size) - 1))))

main = sym["main"] & ~1                                   # ELF Thumb symbols carry bit 0
uc.emu_start(struct.unpack_from("<I", image, 4)[0] | 1, main, count=20_000_000)
pc = uc.reg_read(UC_ARM_REG_PC)

assert pc == main
assert uc.reg_read(UC_ARM_REG_CONTROL) & 3 == 0
assert uc.reg_read(UC_ARM_REG_PRIMASK) == 0
assert uc.reg_read(UC_ARM_REG_FAULTMASK) == 0
assert uc.reg_read(UC_ARM_REG_BASEPRI) == 0
assert mem32(0xE000ED24) == 0                              # SHCSR
assert mem32(0xE000ED10) == 0                              # SCR
assert (0xE000EF50, 0) in writes                           # ICIALLU
assert uc.reg_read(UC_ARM_REG_SP) > 0x24000000            # on our stack, not the installer's
assert mem32(0xE000ED08) == 0x08040000                     # VTOR
assert mem32(0xE000E010) == 0                              # SysTick stopped
assert (0xE000ED04, 1 << 25) in writes                     # PENDSTCLR
assert (0xE000ED04, 1 << 27) in writes                     # PENDSVCLR
for n in range(8):
    assert (0xE000E180 + 4 * n, 0xFFFFFFFF) in writes      # NVIC ICER
    assert (0xE000E280 + 4 * n, 0xFFFFFFFF) in writes      # NVIC ICPR
assert mem32(0xE000ED94) == 0                              # MPU off
assert [v for a, v in writes if a == 0xE000ED98] == list(range(16))   # RNR 0..15
assert mem32(0xE000ED14) & (1 << 16) == 0                  # D-cache off
ccsidr = 0xF003E019
sets, ways = ((ccsidr >> 13) & 0x7FFF) + 1, ((ccsidr >> 3) & 0x3FF) + 1
assert sum(a == 0xE000EF74 for a, _ in writes) == sets * ways   # DCCISW, every set and way
rstr = [0x7C, 0x80, 0x84, 0x88, 0x8C, 0x90, 0x94, 0x98, 0x9C]
for off in rstr:
    vals = [v for a, v in writes if a == 0x58024400 + off]
    assert len(vals) >= 2 and vals[0] != 0 and vals[-1] == 0   # pulsed, released
assert not any(v & (1 << 31) for a, v in writes if a == 0x5802447C)   # never AHB3 CPURST
order = [a for a, _ in writes]
last_rstr = max(i for i, a in enumerate(order) if a - 0x58024400 in rstr)
assert all(i > last_rstr for i, a in enumerate(order) if 0xE000E280 <= a < 0xE000E2A0), \
    "NVIC pending bits cleared before the peripherals are reset"

# Before embassy touches the clocks, the "started" LED (10, PA4) is lit with raw writes.
# Anything else main reaches first (DBGMCU, RCC CR, ...) faults on unmapped memory here.
gpioa = 0x58020000
lit = []


def on_led(_uc, _acc, addr, size, value, _d):
    if addr == gpioa and (value >> 8) & 3 == 1:         # MODER: PA4 becomes an output
        lit.append(addr)
        _uc.emu_stop()


uc.hook_add(UC_HOOK_MEM_WRITE, on_led)
try:
    uc.emu_start(main | 1, 0, count=1_000_000)
except UcError as e:
    raise AssertionError(f"started LED not lit before {e} at {uc.reg_read(UC_ARM_REG_PC):#x}")
assert lit, "started LED never lit"
assert mem32(0x580244E0) & 1                               # GPIOA clock on
assert mem32(gpioa + 0x14) & (1 << 4)                      # driven high
assert not mem32(gpioa + 4) & (1 << 4)                     # push-pull


def blinks(entry):
    """From a bare core, `entry` must reach the LEDs. Fresh emulator, GPIO/RCC page as RAM."""
    uc = Uc(UC_ARCH_ARM, UC_MODE_THUMB | UC_MODE_MCLASS)
    uc.ctl_set_cpu_model(UC_CPU_ARM_CORTEX_M7)
    uc.mem_map(0x08000000, 0x200000); uc.mem_write(0x08040000, image)
    uc.mem_map(0x24000000, 0x80000);  uc.mem_map(0x20000000, 0x20000)
    uc.mem_map(0xE000E000, 0x1000)
    uc.mem_map(0x58020000, 0x5000)
    uc.reg_write(UC_ARM_REG_MSP, 0x20001000)
    odr_g = 0x58021800 + 0x14        # GPIOG ODR; BSRR at +0x18 also counts
    hits = []

    def on_write(_uc, _acc, addr, size, value, _d):
        if addr in (odr_g, odr_g + 4):
            hits.append(value)
            if len(hits) >= 2:
                _uc.emu_stop()

    uc.hook_add(UC_HOOK_MEM_WRITE, on_write)
    try:
        uc.emu_start(sym[entry] | 1, 0, count=100_000_000)   # never returns; the hook stops it
    except UcError:
        pass
    assert len(hits) >= 2, f"{entry} must toggle LEDs"


blinks("HardFault")
blinks("DefaultHandler")
print("ok")
