---
title: Bytes ownership and framing patterns
description: Choose freeze, mutable recovery, slicing, and bounded parsing for real message workflows.
---

# Bytes ownership and framing patterns

The common lifecycle is mutable construction, immutable publication, and
cursor-based consumption. A `BytesMut` lets one producer append a frame; its
`freeze` conversion makes the same contents immutable. `Bytes` is then cheap
to clone or split into field views. If the last immutable owner needs to edit
the contents again, `try_into_mut` can recover a mutable buffer without a copy.
This page focuses on the ownership checks that determine whether that handoff
is possible.

```rust
use bytes::{BufMut, BytesMut};

let mut draft = BytesMut::with_capacity(16);
draft.put_slice(b"ping");
let published = draft.freeze();
assert_eq!(&published[..], b"ping");
```

For parsing, check lengths with [`Buf`](buf.md), isolate a payload using
[`Buf::take`](adapters.md), and copy or retain the resulting bytes according to
their lifetime. For structural field views, [`Bytes::slice`](bytes.md) is
usually preferable to copying. For encoder capacity and raw spare memory,
consult [`BytesMut`](bytes-mut.md) and [`BufMut`](buf-mut.md).

## Publish a finished frame

Use `freeze` when mutations are complete and the message will cross an API or
task boundary. This is the normal way to prevent accidental later changes
while keeping the allocation available to immutable handles. Do not call it
until all expected fields are initialized; after conversion, construct another
`BytesMut` for new output rather than attempting mutation through `Bytes`.

## `BytesMut::freeze`

Usage: `bytes_mut.freeze() -> Bytes`. It consumes the mutable handle and
returns immutable `Bytes` reusing its underlying storage rather than copying.
The resulting `Bytes` can be cloned and sent according to its normal trait
bounds; the old `BytesMut` no longer exists. Freeze only completed data because
the public immutable length is the current initialized length. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes_mut.rs#L259)

## Recover mutation only when unique

When a buffer has no other immutable handles, mutation recovery can avoid a
copy. This is useful for a retry path that receives an outbound `Bytes` back
before it has been cloned. It is not a guarantee for every `Bytes` origin:
static and owner-backed values cannot become a `BytesMut` this way. Treat an
`Err` as the normal signal to allocate or select a separate mutable workspace.

## `Bytes::try_into_mut`

Usage: `bytes.try_into_mut() -> Result<BytesMut, Bytes>`. It returns `Ok` with
the same contents without copying only when the entire original buffer is
unique. If any sharing prevents that, it returns the original `Bytes` in `Err`.
It always fails for buffers made with `from_static` or `from_owner`, even if a
handle appears unshared. [View pinned source](https://github.com/tokio-rs/bytes/blob/d5c8ad3227afe459c09f1d0d85455abf00f0381a/src/bytes.rs#L620)

```rust
use bytes::Bytes;

let bytes = Bytes::from_static(b"fixed");
assert!(bytes.try_into_mut().is_err());

let owned = Bytes::from(Vec::from(&b"edit"[..]));
let mut editable = owned.try_into_mut().expect("unique Vec-backed Bytes");
editable[0] = b'E';
assert_eq!(&editable[..], b"Edit");
```

## Practical fallback

Do not discard the `Err(Bytes)` case. Preserve it for sending unchanged, or
make a new `BytesMut` and append the old bytes plus changes. That makes the
copy explicit and keeps shared observers valid. In a protocol implementation,
favor a clear ownership boundary over an optimization that assumes uniqueness.
