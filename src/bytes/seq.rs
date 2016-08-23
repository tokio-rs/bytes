//! Immutable set of bytes sequential in memory.

use {alloc, MutByteBuf, MutBuf};
use bytes::{Bytes};
use std::ops;
use std::io::Cursor;

pub struct Seq {
    mem: alloc::MemRef,
    pos: u32,
    len: u32,
}

impl Seq {
    pub fn from_slice(bytes: &[u8]) -> Bytes {
        let mut buf = MutByteBuf::with_capacity(bytes.len());

        buf.copy_from(bytes);
        buf.flip().into()
    }

    /// Creates a new `SeqByteStr` from a `MemRef`, an offset, and a length.
    ///
    /// This function is unsafe as there are no guarantees that the given
    /// arguments are valid.
    pub unsafe fn from_mem_ref(mem: alloc::MemRef, pos: u32, len: u32) -> Seq {
        Seq {
            mem: mem,
            pos: pos,
            len: len,
        }
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn slice(&self, begin: usize, end: usize) -> Bytes {
        use super::Kind;

        assert!(begin <= end && end <= self.len(), "invalid range");

        let seq = unsafe {
            Seq::from_mem_ref(
                self.mem.clone(),
                self.pos + begin as u32,
                (end - begin) as u32)
        };

        Bytes { kind: Kind::Seq(seq) }
    }

    pub fn buf(&self) -> Cursor<&[u8]> {
        Cursor::new(self.as_slice())
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { &self.mem.bytes()[self.pos as usize..self.pos as usize + self.len as usize] }
    }
}

impl ops::Index<usize> for Seq {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        assert!(index < self.len());
        unsafe { self.mem.bytes().index(index + self.pos as usize) }
    }
}

impl Clone for Seq {
    fn clone(&self) -> Seq {
        Seq {
            mem: self.mem.clone(),
            pos: self.pos,
            len: self.len,
        }
    }
}
