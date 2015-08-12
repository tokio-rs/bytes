#![crate_name = "bytes"]
#![deny(warnings)]

pub mod alloc;
pub mod buf;
pub mod str;

pub use buf::{
    Buf,
    BufExt,
    MutBuf,
    MutBufExt,
    ByteBuf,
    MutByteBuf,
    RingBuf,
    ROByteBuf,
    SliceBuf,
    MutSliceBuf,
    Source,
    Sink,
    Take,
};
pub use str::{
    ByteStr,
    Bytes,
    Rope,
    RopeBuf,
    SeqByteStr,
    SmallByteStr,
    SmallByteStrBuf,
    ToBytes,
};

use std::u32;

pub mod traits {
    //! All traits are re-exported here to allow glob imports.
    pub use {Buf, BufExt, MutBuf, MutBufExt, ByteStr, ToBytes};
}

const MAX_CAPACITY: usize = u32::MAX as usize;


/*
 *
 * ===== BufError  =====
 *
 */

#[derive(Copy, Clone, Debug)]
pub enum BufError {
    Underflow,
    Overflow,
}
