#!/bin/bash

set -ex

cmd="${1:-test}"

# Install cargo-hack for feature flag test
curl -LsSf https://github.com/taiki-e/cargo-hack/releases/latest/download/cargo-hack-x86_64-unknown-linux-gnu.tar.gz | tar xzf - -C ~/.cargo/bin

# Run with each feature
# * --each-feature includes both default/no-default features
# * --optional-deps is needed for serde feature
cargo hack "${cmd}" --each-feature --optional-deps
# Run with all features
cargo "${cmd}" --all-features

cargo doc --no-deps --all-features

if [[ "${RUST_VERSION}" == "nightly"* ]]; then
    # Check benchmarks
    cargo check --benches

    # Check minimal versions
    cargo clean
    cargo update -Zminimal-versions
    cargo check --all-features
fi
