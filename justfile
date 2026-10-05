# Build an installable out/BLACKBOX.bin from an example and verify it.
sd example="demo":
    mkdir -p out
    cargo objcopy --release --example {{example}} -- -O binary out/BLACKBOX.bin
    python3 tools/check_image.py target/thumbv7em-none-eabihf/release/examples/{{example}} out/BLACKBOX.bin
