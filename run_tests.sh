#!/bin/bash

set -euxo pipefail

cargo nextest run --all-features

#export MIRIFLAGS="-Zmiri-strict-provenance"
cargo +nightly miri test --all-features

export ASAN_OPTIONS="detect_odr_violation=0 detect_leaks=0"

# Run address sanitizer
RUSTFLAGS="-Z sanitizer=address" \
cargo +nightly test --test test_bytes --test test_buf --test test_buf_mut

# Run thread sanitizer
export RUSTFLAGS="-Z sanitizer=thread"
exec cargo +nightly -Zbuild-std test \
    --target aarch64-apple-darwin \
    --test test_bytes --test test_buf --test test_buf_mut
