//! Provides abstractions for working with bytes.
//!
//! The `bytes` crate provides an efficient byte buffer structure
//! ([`Bytes`](struct.Bytes.html)) and traits for working with buffer
//! implementations ([`Buf`](trait.Buf.html), [`BufMut`](trait.BufMut.html)).
//!
//! # `Bytes`
//!
//! `Bytes` is an efficient container for storing and operating on continguous
//! slices of memory. It is intended for use primarily in networking code, but
//! could have applications elsewhere as well.
//!
//! `Bytes` values facilitate zero-copy network programming by allowing multiple
//! `Bytes` objects to point to the same underlying memory. This is managed by
//! using a reference count to track when the memory is no longer needed and can
//! be freed.
//!
//! See the [struct docs](struct.Bytes.html) for more details.
//!
//! # `Buf`, `BufMut`
//!
//! These two traits provide read and write access to buffers. The underlying
//! storage may or may not be in contiguous memory. For example, `Bytes` is a
//! buffer that guarantees contiguous memory, but a
//! [rope](https://en.wikipedia.org/wiki/Rope_(data_structure)) stores the bytes
//! in disjoint chunks. `Buf` and `BufMut` maintain cursors tracking the current
//! position in the underlying byte storage. When bytes are read or written, the
//! cursor is advanced.
//!
//! ## Relation with `Read` and `Write`
//!
//! At first glance, it may seem that `Buf` and `BufMut` overlap in
//! functionality with `std::io::Ready` and `std::io::Write`. However, they
//! serve different purposes. A buffer is the value that is provided as an
//! argument to `Read::read` and `Write::write`. `Read` and `Write` may then
//! perform a syscall, which has the potential of failing. Operations on `Buf`
//! and `BufMut` are infallible.
//!
//! # Example
//!
//! ```
//! use bytes::{BytesMut, Buf, BufMut};
//! use std::io::Cursor;
//! use std::thread;
//!
//! // Allocate a buffer capable of holding 1024 bytes.
//! let mut buf = BytesMut::with_capacity(1024);
//!
//! // Write some data
//! buf.put("Hello world");
//! buf.put(b'-');
//! buf.put("goodbye");
//!
//! // Freeze the buffer, enabling concurrent access
//! let b1 = buf.freeze();
//! let b2 = b1.clone();
//!
//! thread::spawn(move || {
//!     assert_eq!(&b1[..], b"Hello world-goodbye");
//! });
//!
//! let mut buf = Cursor::new(b2);
//! assert_eq!(b'H', buf.get_u8());
//! ```

#![deny(warnings, missing_docs)]

extern crate byteorder;

mod buf;
mod bytes;

pub use buf::{
    Buf,
    BufMut,
    IntoBuf,
    Source,
    Reader,
    Writer,
    Take,
};
pub use bytes::{Bytes, BytesMut};
pub use byteorder::{ByteOrder, BigEndian, LittleEndian};
