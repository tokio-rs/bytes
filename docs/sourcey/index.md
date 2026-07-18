---
title: Bytes Field Guide
description: Task-oriented guidance for choosing and using bytes buffers in Rust.
---

# Bytes Field Guide

`bytes` supplies byte containers and cursor-based traits for network parsers,
framing code, and binary encoders. Start with the ownership boundary: build a
message in [`BytesMut`](bytes-mut.md), then publish it as immutable
[`Bytes`](bytes.md). Consume input through [`Buf`](buf.md), produce output
through [`BufMut`](buf-mut.md), and use [`adapters`](adapters.md) only when an
API specifically wants `std::io::Read` or `std::io::Write`.

The central distinction is between stored bytes and a cursor. `Bytes` and
`BytesMut` are contiguous buffers. `Buf` and `BufMut` describe a current read
or write position and can work over non-contiguous storage. A parser should
therefore check the readable length before consuming a fixed header; it should
not assume `chunk()` contains the entire message. An encoder should reserve or
use a growable destination before calling fixed-capacity operations.

```rust
use bytes::{Buf, BufMut, BytesMut};

let mut frame = BytesMut::with_capacity(16);
frame.put_u8(3);
frame.put_slice(b"cat");
let mut input = frame.freeze();

let len = input.get_u8() as usize;
assert_eq!(input.remaining(), len);
assert_eq!(input.copy_to_bytes(len), b"cat"[..]);
```

Use [`Installation and features`](installation.md) before selecting a target:
the default `std` feature enables IO adapters; `serde` is optional and only
adds serialization support; and `extra-platforms` supports `no_std` targets
without atomic compare-and-swap. See [`Patterns`](patterns.md) for handoff and
reuse decisions.

## Choose a workflow

For a request body or outbound frame, create `BytesMut`, append fields with
`BufMut`, then call `freeze`. For a shared inbound message, retain `Bytes` and
use `slice`, `split_to`, or `split_off` to make cheap views. For a generic
decoder, accept `impl Buf` and consume only verified bytes. For integration
with a synchronous `std::io` API, wrap the trait value with `reader` or
`writer`; those adapters do not perform syscalls themselves.

Each guide links source observations to the pinned upstream revision
`d5c8ad3227afe459c09f1d0d85455abf00f0381a`. The pages are supplemental to
rustdoc: follow the exact API section when panics, ownership, or allocation
behavior matters.

## Navigation

- [`Installation and features`](installation.md): Cargo entries and feature gates.
- [`Immutable Bytes`](bytes.md): shared message views and splitting.
- [`Mutable BytesMut`](bytes-mut.md): growth, appending, and raw spare capacity.
- [`Reading with Buf`](buf.md): parsing cursor operations.
- [`Writing with BufMut`](buf-mut.md): generic encoders and initialization rules.
- [`Adapters`](adapters.md): limits, chaining, and `std::io` bridges.
- [`Patterns`](patterns.md): mutation handoff and frame construction.
