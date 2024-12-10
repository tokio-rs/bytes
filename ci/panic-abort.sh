#!/bin/bash

set -ex
RUSTFLAGS="$RUSTFLAGS -Cpanic=abort -Zpanic-abort-tests" cargo test --all-features --test test_bytes_vec_alloc -- --nocapture
