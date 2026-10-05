elf := "target/thumbv7em-none-eabihf/release/examples"

# Build an installable out/BLACKBOX.bin from an example; no file by that name unless it passes.
sd example="demo": (image example "out/BLACKBOX.bin")
    @echo "{{example}}: out/BLACKBOX.bin, $(stat -c %s out/BLACKBOX.bin) bytes"

# Image-check unit tests, then boot the demo image in an emulator from dirty installer state.
# Leaves out/BLACKBOX.bin alone.
test:
    python3 tools/test_check_image.py
    just image demo target/boot-test/demo.bin
    .venv/bin/python tools/test_boot.py demo target/boot-test/demo.bin

# Build an example's flat image at `out`, kept only if tools/check_image.py passes it.
[private]
image example out:
    rm -f {{out}}
    mkdir -p $(dirname {{out}})
    cargo objcopy --release --example {{example}} -- -O binary {{out}}.tmp
    python3 tools/check_image.py {{elf}}/{{example}} {{out}}.tmp || { rm -f {{out}}.tmp; exit 1; }
    mv {{out}}.tmp {{out}}
