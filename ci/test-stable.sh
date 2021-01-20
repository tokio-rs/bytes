#!/bin/bash

set -ex

cmd="${1:-test}"

# Install cargo-hack for feature flag test
cargo install cargo-hack

# Run with each feature
# * --each-feature includes both default/no-default features
# * --optional-deps is needed for serde feature
cargo hack "${cmd}" --each-feature --optional-deps
# Run with all features
cargo "${cmd}" --all-features

cargo doc --no-deps --all-features

if [[ "${RUST_VERSION}" == "nightly"* ]]; then
    # Check for no_std environment.
    rustup target add thumbv7m-none-eabi
    rustup target add thumbv6m-none-eabi
    cargo build --no-default-features --target thumbv7m-none-eabi
    RUSTFLAGS="--cfg bytes_unstable -Dwarnings" cargo build --no-default-features --target thumbv6m-none-eabi

    # Check benchmarks
    cargo check --benches

    # Check minimal versions
    cargo clean
    cargo update -Zminimal-versions
    cargo check --all-features
fi
