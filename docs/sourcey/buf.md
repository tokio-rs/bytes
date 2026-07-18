---
title: Parse byte streams with Buf
description: Consume bytes through Buf without assuming contiguous or sufficient input.
---

# Parse byte streams with Buf

`Buf` represents readable bytes plus a cursor. It is intentionally broader
than a slice: a chained buffer can have several chunks, so a correct decoder
uses `remaining()` for availability and treats `chunk()` as only the current
contiguous portion. Read helpers advance the cursor. This makes a generic
parser possible for `Bytes`, slices, chained buffers, and custom buffers.

```rust
use bytes::Buf;

fn read_tag(input: &mut impl Buf) -> Option<u8> {
    (input.remaining() >= 1).then(|| input.get_u8())
}

let mut input = &b"\x2a body"[..];
assert_eq!(read_tag(&mut input), Some(42));
```

Use [`Bytes`](bytes.md) when fields must be retained as immutable values, and
use [`Adapters`](adapters.md) for bounded parsing or `std::io::Read` interop.
For output counterparts, see [`BufMut`](buf-mut.md). The trait's operations are
infallible, but insufficient input commonly causes a panic rather than an IO
error, so a length prefix must be checked before a fixed-width read.

## `Buf`

Usage: accept `&mut impl Buf` or a generic `B: Buf`. Implementations expose
readable bytes and a cursor; their storage need not be contiguous. The trait
contract ties cursor-changing calls to `remaining`, `chunk`, and `advance`.
Prefer consuming its provided methods over implementing a parser against
`AsRef<[u8]>`. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/buf_impl.rs#L122)

## `Buf::remaining`

Usage: `buf.remaining() -> usize`. It reports bytes between the cursor and end
and is at least `chunk().len()`. Its value should change only after a
cursor-changing operation. Check it before consuming a field; zero means no
further data is available. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/buf_impl.rs#L148)

## `Buf::chunk`

Usage: `buf.chunk() -> &[u8]`. It gives the current contiguous bytes, which
may be shorter than `remaining()` for disjoint storage. It must be empty if
and only if no bytes remain and should not panic; do not index it for a field
whose length has only been checked against a larger logical message. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/buf_impl.rs#L181)

## `Buf::advance`

Usage: `buf.advance(cnt)`. It moves the cursor forward so the next chunk starts
later. A call with zero is a no-op; an implementation may panic for
`cnt > remaining()`, otherwise it must behave as advancing to the end. Only
advance validated protocol lengths. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/buf_impl.rs#L255)

## `Buf::get_u8`

Usage: `buf.get_u8() -> u8`. It returns the next byte and advances one
position. It panics when no bytes remain, which is why a decoder checks
`remaining() >= 1` or chooses a `try_get_*` method for recoverable malformed
input handling. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/buf_impl.rs#L320)

## `Buf::copy_to_bytes`

Usage: `buf.copy_to_bytes(len) -> Bytes`. It consumes `len` bytes into an
immutable `Bytes`. A concrete `Bytes` can override this efficiently, but the
default allocates a `BytesMut`, transfers the bytes, and freezes it. It panics
when `len > remaining()`, so validate framing first. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/buf/buf_impl.rs#L2363)
