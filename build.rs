use std::{env, fs, path::PathBuf};

fn main() {
    // Own memory.x (not embassy's): the installer below 0x08040000 must never be overwritten.
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::copy("memory.x", out.join("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
}
