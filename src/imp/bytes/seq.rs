//! Immutable set of bytes sequential in memory.

use {MutBuf, Bytes};
use buf::{AppendBuf};
use std::ops;
use std::io::Cursor;
use std::sync::Arc;

pub struct Seq {
    mem: Arc<Box<[u8]>>,
    pos: usize,
    len: usize,
}

impl Seq {
    /// Creates a new `SeqByteStr` from a `MemRef`, an offset, and a length.
    ///
    /// This function is unsafe as there are no guarantees that the given
    /// arguments are valid.
    pub fn new(mem: Arc<Box<[u8]>>, pos: usize, len: usize) -> Seq {
        Seq {
            mem: mem,
            pos: pos,
            len: len,
        }
    }

    pub fn from_slice(bytes: &[u8]) -> Bytes {
        let mut buf = AppendBuf::with_capacity(bytes.len() as u32);

        buf.copy_from(bytes);
        buf.into()
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn slice(&self, begin: usize, end: usize) -> Bytes {
        use super::Kind;

        assert!(begin <= end && end <= self.len(), "invalid range");

        let seq = Seq::new(
            self.mem.clone(),
            self.pos + begin,
            end - begin);

        Bytes { kind: Kind::Seq(seq) }
    }

    pub fn buf(&self) -> Cursor<&[u8]> {
        Cursor::new(self.as_slice())
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.mem[self.pos..self.pos+self.len]
    }
}

impl ops::Index<usize> for Seq {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        assert!(index < self.len());
        self.mem.index(index + self.pos as usize)
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
