#![crate_name = "bytes"]
#![deny(warnings)]

extern crate stable_heap;

#[macro_use]
extern crate log;

mod buf;
mod bytes;

pub mod alloc;

pub use buf::{Buf, MutBuf, Source, Sink, ReadExt, WriteExt, Fmt};
pub use buf::append::AppendBuf;
pub use buf::block::{BlockBuf, BlockBufCursor};
pub use buf::byte::{ByteBuf, MutByteBuf};
pub use buf::ring::RingBuf;
pub use buf::take::Take;
pub use bytes::Bytes;

use std::u32;

const MAX_CAPACITY: usize = u32::MAX as usize;
