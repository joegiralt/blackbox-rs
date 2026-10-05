from check_image import check_bin, has_flash_code, LIMIT
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
print("ok")
