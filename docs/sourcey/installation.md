---
title: Install bytes and select features
description: Add bytes to a Rust crate and choose std, serde, or extra-platforms deliberately.
---

# Install bytes and select features

For ordinary Rust applications, add `bytes = "1"`. The default feature set
contains `std`, so `Buf::reader`, `BufMut::writer`, and the `Reader`/`Writer`
types are available. The core containers and buffer traits work without that
feature, which is useful for embedded code and other `no_std` crates.

```toml
[dependencies]
bytes = "1"
```

```rust
use bytes::{BufMut, BytesMut};

let mut out = BytesMut::new();
out.put_slice(b"ready");
assert_eq!(&out[..], b"ready");
```

For `no_std`, turn off defaults. This crate uses `alloc`, so the consumer must
provide the usual allocation environment. On a target without atomic CAS, add
`extra-platforms`; it enables the crate's optional `portable-atomic`
dependency with the required CAS support. The MSRV then depends on
`portable-atomic`, not only the crate's declared Rust version.

```toml
[dependencies]
bytes = { version = "1", default-features = false, features = ["extra-platforms"] }
```

Serde is separate and disabled by default. Enable it only where `Bytes` or
`BytesMut` must participate in a serde data model; the resulting MSRV depends
on `serde`. It does not enable `std` IO adapters.

```toml
[dependencies]
bytes = { version = "1", features = ["serde"] }
```

The type-specific guides are [`Bytes`](bytes.md), [`BytesMut`](bytes-mut.md),
[`Buf`](buf.md), and [`BufMut`](buf-mut.md). Read [`Adapters`](adapters.md)
before importing `std::io` traits, and use [`Patterns`](patterns.md) for the
usual mutable-to-immutable message handoff.

## Feature checklist

`std` is enabled by default and gates the `Reader` and `Writer` adapters plus
other standard-library integrations. With `default-features = false`, do not
call `reader()` or `writer()` and do not import their types. `serde` only gates
the crate's serde implementation. `extra-platforms` is an optional dependency
for `no_std` targets lacking atomic compare-and-swap; it is not a general
performance flag. Keep Cargo feature choices at the application boundary so
libraries do not unexpectedly force `std` on downstream users.

## Verify the dependency

Use a small buffer operation to confirm the selected feature set in the real
target configuration. A successful compile of `BytesMut::new` and `put_slice`
checks the core API; a call to `.reader()` intentionally checks `std`. The
examples in this guide use public APIs compatible with `bytes = "1"` and do
not rely on a particular allocator or platform.
