//! Provides abstractions for working with bytes.

#![deny(warnings, missing_docs)]

extern crate byteorder;

mod buf;
mod bytes;

pub use buf::{
    Buf,
    BufMut,
    IntoBuf,
    Source,
    Sink,
    Reader,
    Writer,
};
pub use bytes::{Bytes, BytesMut};
