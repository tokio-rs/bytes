use {alloc, ByteBuf, MutBufExt, ByteStr, ROByteBuf, Rope, Bytes, ToBytes};
use std::ops;

pub struct SeqByteStr {
    mem: alloc::MemRef,
    pos: u32,
    len: u32,
}

impl SeqByteStr {
    /// Create a new `SeqByteStr` from a byte slice.
    ///
    /// The contents of the byte slice will be copied.
    pub fn from_slice(bytes: &[u8]) -> SeqByteStr {
        let mut buf = ByteBuf::mut_with_capacity(bytes.len());

        if let Err(e) = buf.write(bytes) {
            panic!("failed to copy bytes from slice; err={:?}", e);
        }

        buf.flip().to_seq_byte_str()
    }

    /// Creates a new `SeqByteStr` from a `MemRef`, an offset, and a length.
    ///
    /// This function is unsafe as there are no guarantees that the given
    /// arguments are valid.
    pub unsafe fn from_mem_ref(mem: alloc::MemRef, pos: u32, len: u32) -> SeqByteStr {
        SeqByteStr {
            mem: mem,
            pos: pos,
            len: len,
        }
    }
}

impl ByteStr for SeqByteStr {
    type Buf = ROByteBuf;

    fn buf(&self) -> ROByteBuf {
        unsafe {
            let pos = self.pos;
            let lim = pos + self.len;

            ROByteBuf::from_mem_ref(self.mem.clone(), lim, pos, lim)
        }
    }

    fn concat<B: ByteStr+'static>(&self, other: &B) -> Bytes {
        Rope::of(self.clone()).concat(other)
    }

    fn len(&self) -> usize {
        self.len as usize
    }

    fn slice(&self, begin: usize, end: usize) -> Bytes {
        if begin >= end || begin >= self.len() {
            return Bytes::empty()
        }

        let bytes = unsafe {
            SeqByteStr::from_mem_ref(
                self.mem.clone(),
                self.pos + begin as u32,
                (end - begin) as u32)
        };

        Bytes::of(bytes)
    }
}

impl ToBytes for SeqByteStr {
    fn to_bytes(self) -> Bytes {
        Bytes::of(self)
    }
}

impl ops::Index<usize> for SeqByteStr {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        assert!(index < self.len());

        unsafe {
            &*self.mem.ptr()
                .offset(index as isize + self.pos as isize)
        }
    }
}

impl Clone for SeqByteStr {
    fn clone(&self) -> SeqByteStr {
        SeqByteStr {
            mem: self.mem.clone(),
            pos: self.pos,
            len: self.len,
        }
    }
}
