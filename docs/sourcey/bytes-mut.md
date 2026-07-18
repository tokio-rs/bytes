---
title: Build buffers with BytesMut
description: Allocate, grow, append, merge, and safely initialize mutable byte buffers.
---

# Build buffers with BytesMut

`BytesMut` is the growable, mutable staging buffer for encoders and framed IO.
Its length is initialized data; capacity is reserved storage and is not
readable payload. Append with `extend_from_slice` or the [`BufMut`](buf-mut.md)
methods, then turn the completed value into `Bytes` with `freeze` as described
in [`Patterns`](patterns.md).

```rust
use bytes::{BufMut, BytesMut};

let mut out = BytesMut::with_capacity(8);
out.put_u8(3);
out.extend_from_slice(b"cat");
assert_eq!(&out[..], b"\x03cat");
```

Capacity can be reclaimed or reallocated as ownership and prior splits change.
Do not rely on pointer identity after `reserve`, and keep unsafe initialization
localized. [`Buf`](buf.md) covers reading the resulting bytes; [`Adapters`](adapters.md)
covers a `std` writer when that integration is required.

## `BytesMut::with_capacity`

Usage: `BytesMut::with_capacity(capacity) -> BytesMut`. It allocates room for
the requested capacity while setting length to zero, so `&buf[..]` is empty
until data is written. It is suitable when a likely frame size is known, not
when uninitialized bytes need to be exposed as data. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes_mut.rs#L161)

## `BytesMut::reserve`

Usage: `buf.reserve(additional)`. It ensures space for at least `additional`
more bytes beyond the current length and does nothing when sufficient capacity
already exists. The buffer may move or reclaim storage, and it panics if the
new capacity overflows `usize`. Reserve before a known append to control
allocation timing. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes_mut.rs#L615)

## `BytesMut::extend_from_slice`

Usage: `buf.extend_from_slice(bytes)`. It reserves enough space, copies the
given slice into spare capacity, and advances the initialized length. It is
the safe default for appending borrowed payload data; allocation failures have
the normal Rust allocation behavior. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes_mut.rs#L892)

## `BytesMut::unsplit`

Usage: `buf.unsplit(other)`. When `other` is the contiguous result of an
earlier split and neither side forced reallocation, it rejoins in `O(1)` by
adjusting metadata. Otherwise it appends `other` by copying. An empty receiver
is replaced by `other`, so use this for parser-buffer reunification rather than
as a guaranteed zero-copy operation. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes_mut.rs#L969)

## `BytesMut::spare_capacity_mut`

Usage: `buf.spare_capacity_mut() -> &mut [MaybeUninit<u8>]`. It exposes the
uninitialized tail from `len` to capacity. Write every byte that will be made
visible, then use the documented unsafe length-advance operation; never read
the returned memory before initialization. For ordinary copying, use
`extend_from_slice` instead. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes_mut.rs#L1211)
