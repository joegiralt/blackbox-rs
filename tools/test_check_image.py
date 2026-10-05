from check_image import check_bin, check_elf, check_startup, has_flash_code, LIMIT
import struct

def image(sp, reset, size=1024):
    return struct.pack("<II", sp, reset) + bytes(size - 8)

assert check_bin(image(0x24080000, 0x08040299)) == []
assert check_bin(image(0x20020000, 0x080403C1)) == []          # stock 3.1.9's own values
assert check_bin(image(0x08000000, 0x08040299))                # SP not in RAM
assert check_bin(image(0x24080001, 0x08040299))                # SP not 4-aligned
assert check_bin(image(0x24080000, 0x08040298))                # reset lacks Thumb bit
assert check_bin(image(0x24080000, 0x08000299))                # reset below the image
assert check_bin(image(0x24080000, 0x08040299 + 1024))         # reset past the image
assert check_bin(image(0x24080000, 0x08040299, LIMIT + 4))     # too large
assert check_bin(b"\0" * 4)                                    # truncated
assert has_flash_code({"_ZN13embassy_stm325flash5Flash5write17h0E": 1})
assert not has_flash_code({"_ZN13embassy_stm323rcc4init17h0E": 1})
assert has_flash_code({"_ZN71_$LT$embassy_stm32..flash..Flash$u20$as$u20$embedded_storage..Storage$GT$5write17h0E": 1})

def bare_elf(paddr):  # ELF32 LE, one PT_LOAD, no sections
    hdr = b"\x7fELF\x01\x01\x01" + bytes(9) + struct.pack("<HHIIIIIHHHHHH", 2, 40, 1, 0, 52, 0, 0, 52, 32, 1, 40, 0, 0)
    return hdr + struct.pack("<IIIIIIII", 1, 0, 0, paddr, 4, 4, 5, 4) + bytes(4)

assert any("symbol" in m for m in check_elf(bare_elf(0x08040000)))
assert any("0x08000000" in m for m in check_elf(bare_elf(0x08000000)))   # over the installer

ours = {"__pre_init": 0x08040651, "__behind_installer": 0x0804081F}
assert check_startup(ours) == []                                # DefaultPreInit gc'd
assert check_startup(dict(ours, DefaultPreInit=0x08040701)) == []
assert check_startup({"__pre_init": 0x08040651})                # no __behind_installer
assert check_startup(dict(ours, DefaultPreInit=0x08040651))     # cortex-m-rt's empty default
print("ok")
