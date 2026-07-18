---
title: Work with immutable Bytes
description: Create shared byte views, split message fields, and understand immutable buffer boundaries.
---

# Work with immutable Bytes

`Bytes` is a contiguous immutable byte buffer with cheap cloning and slicing.
Use it when a message is complete and may be shared by tasks or retained after
parsing. Build it from static data, copied input, a vector, or by freezing a
[`BytesMut`](bytes-mut.md). Its view operations adjust handles rather than
rewriting payload bytes, but they must receive valid ranges.

```rust
use bytes::Bytes;

let message = Bytes::from_static(b"GET / HTTP/1.1");
let method = message.slice(..3);
assert_eq!(method, b"GET"[..]);
```

For a consuming parser, prefer [`Buf`](buf.md) methods. For named regions that
must coexist, keep the original `Bytes` and call `slice`. To physically divide
one handle into a prefix and suffix, use `split_to` or `split_off`. See
[`Patterns`](patterns.md) to attempt a zero-copy return to `BytesMut` when the
immutable handle is uniquely owned.

## `Bytes::new`

Usage: `Bytes::new() -> Bytes`. It returns an empty immutable buffer and is
appropriate for an explicit empty payload. It contains no bytes; do not call
consuming `Buf` getters without checking `remaining()`. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes.rs#L151)

## `Bytes::from_static`

Usage: `Bytes::from_static(bytes: &'static [u8]) -> Bytes`. The result points
at the supplied static slice without allocation or copying. The input must
really have `'static` lifetime, such as `b"token"`; use `copy_from_slice` for
borrowed request data. Static storage has no reference count, so slicing a
`Bytes::from_static` value does not increment one. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes.rs#L182)

## `Bytes::copy_from_slice`

Usage: `Bytes::copy_from_slice(data: &[u8]) -> Bytes`. It owns an independent
copy of the slice, so the caller may reuse or mutate the original afterward.
This deliberately allocates and copies; it is not a borrowing conversion.
[View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes.rs#L347)

## `Bytes::slice`

Usage: `bytes.slice(range) -> Bytes`. A non-empty valid range backed by
reference-counted storage returns an `O(1)` view and increments that storage's
reference count. A non-empty slice of `Bytes::from_static` is also `O(1)`, but
uses static storage and has no reference count to increment. It panics unless
`begin <= end` and `end <= bytes.len()`; validate protocol offsets before
passing them. An empty range is valid but returns an independent empty `Bytes`,
not a shared backing-storage handle. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes.rs#L373)

## `Bytes::split_off`

Usage: `bytes.split_off(at) -> Bytes`. Afterward `bytes` is `[0, at)` and the
returned handle is `[at, len)`. It is an inexpensive handle split, including
the zero and end boundaries, but panics when `at > len`. Use `truncate` when
the suffix is intentionally discarded. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes.rs#L472)

## `Bytes::split_to`

Usage: `bytes.split_to(at) -> Bytes`. It returns `[0, at)` and leaves `bytes`
as `[at, len)`, which fits a parser that removes a validated prefix. The work
is `O(1)` apart from reference-count bookkeeping. It accepts zero and `len`,
but panics when `at > len`; use `advance` through [`Buf`](buf.md) when the
prefix is not needed. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes.rs#L523)
