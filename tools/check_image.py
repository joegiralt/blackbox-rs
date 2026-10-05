"""Gate for SD-installable images: linked at BASE, sane vectors, fits, no flash writes,
and the crate's startup cleanup linked in."""
import struct
import sys

BASE = 0x08040000
LIMIT = 768 * 1024
STOCK_SIZE = 728696


def check_bin(b):
    if len(b) < 8:
        return ["image shorter than the vector table header"]
    sp, reset = struct.unpack_from("<II", b)
    out = []
    if not (0x20000000 < sp <= 0x20020000 or 0x24000000 < sp <= 0x24080000) or sp % 4:
        out.append(f"initial SP {sp:#010x} is not an aligned RAM address")
    if not reset & 1:
        out.append(f"reset vector {reset:#010x} lacks the Thumb bit")
    if not BASE <= reset < BASE + len(b):
        out.append(f"reset vector {reset:#010x} outside the image at {BASE:#010x}")
    if len(b) > LIMIT:
        out.append(f"{len(b)} bytes exceeds {LIMIT}")
    return out


def _sections(elf):
    shoff, = struct.unpack_from("<I", elf, 0x20)
    shentsize, shnum = struct.unpack_from("<HH", elf, 0x2E)
    return [struct.unpack_from("<IIIIIIIIII", elf, shoff + i * shentsize) for i in range(shnum)]


def elf_loads(elf):
    phoff, = struct.unpack_from("<I", elf, 0x1C)
    phentsize, phnum = struct.unpack_from("<HH", elf, 0x2A)
    loads = []
    for i in range(phnum):
        p_type, _, _, paddr, filesz = struct.unpack_from("<IIIII", elf, phoff + i * phentsize)
        if p_type == 1 and filesz:
            loads.append(paddr)
    return loads


def has_symtab(elf):
    return any(s[1] == 2 for s in _sections(elf))


def elf_symbols(elf):
    secs = _sections(elf)
    syms = {}
    for _, kind, _, _, off, size, link, _, _, _ in secs:
        if kind != 2:  # SHT_SYMTAB
            continue
        stroff = secs[link][4]
        for o in range(off, off + size, 16):
            name, value = struct.unpack_from("<II", elf, o)
            end = elf.index(b"\0", stroff + name)
            syms[elf[stroff + name:end].decode("ascii", "replace")] = value
    return syms


def has_flash_code(symbols):
    return any("13embassy_stm325flash" in n or "embassy_stm32..flash" in n for n in symbols)


def check_startup(symbols):
    """`__pre_init` must be the crate's, not cortex-m-rt's PROVIDEd empty default (an override
    that wins only through linker archive order). The default is gc'd when unused."""
    out = []
    if "__behind_installer" not in symbols:
        out.append("no __behind_installer: the startup cleanup is not linked")
    pre = symbols.get("__pre_init")
    if pre is None or pre == symbols.get("DefaultPreInit"):
        out.append("__pre_init is cortex-m-rt's empty default, not the startup cleanup")
    return out


def check_elf(elf):
    out = []
    base = min(elf_loads(elf), default=None)
    if base != BASE:
        out.append(f"linked at {base:#010x}, must be {BASE:#010x}" if base is not None
                   else "no loadable segments")
    if not has_symtab(elf):
        out.append("no symbol table, cannot verify absence of flash code")
    else:
        symbols = elf_symbols(elf)
        if has_flash_code(symbols):
            out.append("links embassy-stm32 flash code")
        out += check_startup(symbols)
    return out


if __name__ == "__main__":
    elf, binf = (open(p, "rb").read() for p in sys.argv[1:3])
    print(f"{len(binf)} bytes (stock {STOCK_SIZE})")
    fails = check_elf(elf) + check_bin(binf)
    for f in fails:
        print(f)
    sys.exit(bool(fails))
