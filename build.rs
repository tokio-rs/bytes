use std::env;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(bytes_has_atomic_ptr)");

    let target_atomics = env::var("CARGO_CFG_TARGET_HAS_ATOMIC").unwrap_or_default();
    if target_atomics.split(',').any(|atomic| atomic == "ptr") {
        println!("cargo:rustc-cfg=bytes_has_atomic_ptr");
    }
}
