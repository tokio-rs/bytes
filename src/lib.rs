#![crate_name = "bytes"]
#![deny(warnings)]

#[macro_use]
extern crate log;
extern crate byteorder;

// Implementation in here
mod imp;
// TODO: delete
mod alloc;

pub use imp::buf::{Buf, MutBuf, IntoBuf};
pub use imp::bytes::Bytes;

pub mod buf {
    //! Traits, helpers, and type definitions for working with buffers.

    pub use imp::buf::{
        Source,
        Sink,
        Reader,
        ReadExt,
        Writer,
        WriteExt,
        Fmt,
    };

    pub use imp::buf::slice::SliceBuf;
    pub use imp::buf::append::AppendBuf;
    pub use imp::buf::block::{BlockBuf, BlockBufCursor};
    pub use imp::buf::bound::{BoundBuf};
    pub use imp::buf::ring::RingBuf;
    pub use imp::buf::take::Take;
    pub use imp::bytes::BytesBuf;
}
