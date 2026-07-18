---
title: Adapt buffers to limits, chains, and std IO
description: Bound reads, sequence buffers, and bridge Buf or BufMut to std IO traits.
---

# Adapt buffers to limits, chains, and std IO

Adapters keep the underlying buffer type while changing how it is consumed or
integrated. Use `take` to prevent a nested parser from reading past a declared
field, `chain` to present two buffers as one logical stream, and `reader` or
`writer` only for APIs that require `std::io`. The IO adapters are gated behind
the default `std` feature; they are unavailable with `default-features = false`.

```rust
use bytes::{Buf, BufMut};

let mut field = (&b"cat!"[..]).take(3);
assert_eq!(field.copy_to_bytes(3), b"cat"[..]);
let rest = field.into_inner();
assert_eq!(rest, b"!"[..]);
```

These wrappers advance their inner cursors. Do not read directly through an
adapter's `get_ref` while it is in use because that bypasses its state, such as
a `Take` limit. For buffer construction and parsing fundamentals, return to
[`BufMut`](buf-mut.md) and [`Buf`](buf.md); for ownership conversions, see
[`Patterns`](patterns.md).

## `Buf::take`

Usage: `buf.take(limit) -> Take<_>`. The adapter exposes at most `limit` bytes
and its `remaining` is the minimum of the inner remaining count and that limit.
It consumes the inner buffer as it is read; `into_inner` returns the remainder.
[View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/buf_impl.rs#L2399)

## `Take`

Usage: `Take<T>` is the value returned by `Buf::take`. It limits the visible
readable bytes without copying and advances the inner buffer for every read.
`Take::advance` panics above its remaining limit, so a nested parser cannot
silently consume the next field. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/take.rs#L13)

## `Chain`

Usage: `first.chain(second) -> Chain<_, _>`. A chain consumes `first` fully
before `second` and may have more remaining bytes than its current `chunk`.
When copying crosses the boundary it builds a new `Bytes`; when the requested
length lies wholly on one side it delegates to that side. Validate total
`remaining()` before requesting a fixed amount. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/chain.rs#L30)

## `Buf::reader`

Usage: `buf.reader() -> Reader<_>`, requiring feature `std`. It implements
`std::io::Read` and adapts reads to `Buf`; its `read` returns up to the smaller
of destination length and remaining bytes. The source documents no `Err` from
the adaptation itself, but callers still handle the standard `io::Result`.
Avoid mutating the wrapped buffer directly while reading. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/buf_impl.rs#L2453)

## `Reader`

Usage: `Reader<B>` owns a `Buf` and implements `std::io::Read` and
`std::io::BufRead` when feature `std` is enabled. `into_inner` returns the
advanced buffer; use it to continue a parser after an IO consumer finishes.
Direct reads through `get_ref` bypass adapter state and are inadvisable.
[View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/reader.rs#L11)

```rust
use bytes::{Buf, Bytes};
use std::io::Read;

let mut reader = Bytes::from_static(b"ok").reader();
let mut dst = [0; 2];
reader.read_exact(&mut dst).unwrap();
assert_eq!(&dst, b"ok");
```

## `BufMut::writer`

Usage: `buf.writer() -> Writer<_>`, requiring feature `std`. It implements
`std::io::Write`; each `write` accepts at most the destination's current
`remaining_mut`, writes that prefix, and returns its count. It can therefore
short-write a fixed-capacity destination, so `write_all` remains appropriate.
`flush` is a no-op. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/buf_mut.rs#L1317)

## `Writer`

Usage: `Writer<B>` owns a `BufMut` and exposes `std::io::Write` when feature
`std` is enabled. `into_inner` recovers the buffer after writing. Its `write`
method may return a short count for a fixed-capacity destination, while `flush`
does nothing because this adapter has no external sink. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/writer.rs#L11)
