#![crate_name = "bytes"]
#![deny(warnings)]

#[macro_use]
extern crate log;
extern crate byteorder;

// Implementation in here
mod imp;

pub mod alloc;

pub use imp::buf::{Buf, MutBuf};
pub use imp::bytes::Bytes;

pub mod buf {
    pub use imp::buf::{
        Source,
        Sink,
        Reader,
        ReadExt,
        Writer,
        WriteExt,
        Fmt,
    };
    pub use imp::buf::append::AppendBuf;
    pub use imp::buf::block::{BlockBuf, BlockBufCursor};
    pub use imp::buf::byte::{ByteBuf, MutByteBuf};
    pub use imp::buf::ring::RingBuf;
    pub use imp::buf::take::Take;

    pub use imp::bytes::BytesBuf;
}

use std::u32;

const MAX_CAPACITY: usize = u32::MAX as usize;
