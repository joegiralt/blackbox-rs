# Build an installable out/BLACKBOX.bin from an example and verify it.
sd example="demo":
    mkdir -p out
    cargo objcopy --release --example {{example}} -- -O binary out/BLACKBOX.bin
    python3 tools/check_image.py target/thumbv7em-none-eabihf/release/examples/{{example}} out/BLACKBOX.bin

# Image-check unit tests, then boot the demo image in an emulator from dirty installer state.
test:
    python3 tools/test_check_image.py
    just sd demo
    .venv/bin/python tools/test_boot.py demo
