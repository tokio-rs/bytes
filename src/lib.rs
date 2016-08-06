#![crate_name = "bytes"]
#![deny(warnings)]

extern crate stable_heap;

pub mod alloc;
pub mod buf;
pub mod str;

pub use buf::{
    Buf,
    MutBuf,
    ByteBuf,
    MutByteBuf,
    RingBuf,
    ROByteBuf,
    Take,
    ReadExt,
    WriteExt,
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

const MAX_CAPACITY: usize = u32::MAX as usize;
