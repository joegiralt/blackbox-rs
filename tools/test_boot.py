"""Boot the built image from its reset vector with the installer's worst-case leftovers;
on arrival at `main` the stack, privilege, masks, SysTick, NVIC, MPU, cache and RCC are clean."""
import struct
import sys

from unicorn import Uc, UC_ARCH_ARM, UC_MODE_THUMB, UC_MODE_MCLASS, UC_HOOK_MEM_WRITE
from unicorn.arm_const import (UC_CPU_ARM_CORTEX_M7, UC_ARM_REG_CONTROL, UC_ARM_REG_MSP,
                               UC_ARM_REG_PC, UC_ARM_REG_PRIMASK, UC_ARM_REG_PSP, UC_ARM_REG_SP)

from check_image import elf_symbols

example = sys.argv[1]
image = open("out/BLACKBOX.bin", "rb").read()
sym = elf_symbols(open(f"target/thumbv7em-none-eabihf/release/examples/{example}", "rb").read())

uc = Uc(UC_ARCH_ARM, UC_MODE_THUMB | UC_MODE_MCLASS)
uc.ctl_set_cpu_model(UC_CPU_ARM_CORTEX_M7)
uc.mem_map(0x08000000, 0x200000); uc.mem_write(0x08040000, image)
uc.mem_map(0x24000000, 0x80000);  uc.mem_map(0x20000000, 0x20000)
uc.mem_map(0xE000E000, 0x1000)    # SCS as plain RAM
uc.mem_map(0x58024000, 0x1000)    # RCC as plain RAM


def mem32(addr):
    return struct.unpack("<I", uc.mem_read(addr, 4))[0]


def poke32(addr, value):
    uc.mem_write(addr, struct.pack("<I", value))


# What the installer might leave behind.
uc.reg_write(UC_ARM_REG_MSP, 0x20001000)
uc.reg_write(UC_ARM_REG_PSP, 0x20002000)
uc.reg_write(UC_ARM_REG_CONTROL, 2)
uc.reg_write(UC_ARM_REG_PRIMASK, 1)
poke32(0xE000E010, 7)             # SysTick CSR: running, interrupting
poke32(0xE000ED94, 5)             # MPU CTRL: ENABLE | PRIVDEFENA
poke32(0xE000ED14, 0x00030200)    # CCR: DC | IC | STKALIGN
poke32(0xE000ED80, 0xF003E019)    # CCSIDR for the set/way loops

writes = []
uc.hook_add(UC_HOOK_MEM_WRITE,
            lambda _uc, _acc, addr, size, value, _d: writes.append((addr, value & ((1 << 8 * size) - 1))))

main = sym["main"] & ~1                                   # ELF Thumb symbols carry bit 0
uc.emu_start(struct.unpack_from("<I", image, 4)[0] | 1, main, count=20_000_000)
pc = uc.reg_read(UC_ARM_REG_PC)

assert pc == main
assert uc.reg_read(UC_ARM_REG_CONTROL) & 3 == 0
assert uc.reg_read(UC_ARM_REG_PRIMASK) == 0
assert uc.reg_read(UC_ARM_REG_SP) > 0x24000000            # on our stack, not the installer's
assert mem32(0xE000ED08) == 0x08040000                     # VTOR
assert mem32(0xE000E010) == 0                              # SysTick stopped
assert (0xE000ED04, 1 << 25) in writes                     # PENDSTCLR
for n in range(8):
    assert (0xE000E180 + 4 * n, 0xFFFFFFFF) in writes      # NVIC ICER
    assert (0xE000E280 + 4 * n, 0xFFFFFFFF) in writes      # NVIC ICPR
assert mem32(0xE000ED94) == 0                              # MPU off
assert [v for a, v in writes if a == 0xE000ED98] == list(range(16))   # RNR 0..15
assert mem32(0xE000ED14) & (1 << 16) == 0                  # D-cache off
rstr = [0x7C, 0x80, 0x84, 0x88, 0x8C, 0x90, 0x94, 0x98, 0x9C]
for off in rstr:
    vals = [v for a, v in writes if a == 0x58024400 + off]
    assert len(vals) >= 2 and vals[0] != 0 and vals[-1] == 0   # pulsed, released
assert not any(v & (1 << 31) for a, v in writes if a == 0x5802447C)   # never AHB3 CPURST
print("ok")
