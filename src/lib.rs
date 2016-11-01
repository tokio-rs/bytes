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
pub use buf::byte::{ByteBuf};
pub use buf::slice::{SliceBuf};
pub use buf::take::{Take, TakeMut};
pub use bytes::{Bytes, BytesMut};
