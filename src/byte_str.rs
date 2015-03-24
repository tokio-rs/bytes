use {alloc, Bytes, ByteBuf, ROByteBuf, Rope};
use traits::{Buf, MutBuf, MutBufExt, ByteStr, ToBytes};
use std::{cmp, ops};

/*
 *
 * ===== SeqByteStr =====
 *
 */

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

    fn index(&self, index: &usize) -> &u8 {
        assert!(*index < self.len());

        unsafe {
            &*self.mem.ptr()
                .offset(*index as isize + self.pos as isize)
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

/*
 *
 * ===== SmallByteStr =====
 *
 */

#[cfg(target_pointer_width = "64")]
const MAX_LEN: usize = 7;

#[cfg(target_pointer_width = "32")]
const MAX_LEN: usize = 3;

#[derive(Clone, Copy)]
pub struct SmallByteStr {
    len: u8,
    bytes: [u8; MAX_LEN],
}

impl SmallByteStr {
    pub fn zero() -> SmallByteStr {
        use std::mem;

        SmallByteStr {
            len: 0,
            bytes: unsafe { mem::zeroed() }
        }
    }

    pub fn from_slice(bytes: &[u8]) -> Option<SmallByteStr> {
        use std::mem;
        use std::slice::bytes;

        if bytes.len() > MAX_LEN {
            return None;
        }

        let mut ret = SmallByteStr {
            len: bytes.len() as u8,
            bytes: unsafe { mem::zeroed() },
        };

        // Copy the memory
        bytes::copy_memory(&mut ret.bytes, bytes);

        Some(ret)
    }
}

impl ByteStr for SmallByteStr {
    type Buf = SmallByteStrBuf;

    fn buf(&self) -> SmallByteStrBuf {
        SmallByteStrBuf { small: self.clone() }
    }

    fn concat<B: ByteStr+'static>(&self, other: &B) -> Bytes {
        Rope::of(self.clone()).concat(other)
    }

    fn len(&self) -> usize {
        self.len as usize
    }

    fn slice(&self, _begin: usize, _end: usize) -> Bytes {
        unimplemented!();
    }

    fn split_at(&self, _mid: usize) -> (Bytes, Bytes) {
        unimplemented!();
    }
}

impl ToBytes for SmallByteStr {
    fn to_bytes(self) -> Bytes {
        Bytes::of(self)
    }
}

impl ops::Index<usize> for SmallByteStr {
    type Output = u8;

    fn index(&self, index: &usize) -> &u8 {
        assert!(*index < self.len());
        &self.bytes[*index]
    }
}

#[derive(Clone)]
#[allow(missing_copy_implementations)]
pub struct SmallByteStrBuf {
    small: SmallByteStr,
}

impl SmallByteStrBuf {
    fn len(&self) -> usize {
        (self.small.len & 0x0F) as usize
    }

    fn pos(&self) -> usize {
        (self.small.len >> 4) as usize
    }
}

impl Buf for SmallByteStrBuf {
    fn remaining(&self) -> usize {
        self.len() - self.pos()
    }

    fn bytes(&self) -> &[u8] {
        &self.small.bytes[self.pos()..self.len()]
    }

    fn advance(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.remaining());
        self.small.len += (cnt as u8) << 4;
    }
}

#[test]
pub fn test_size_of() {
    use std::mem;
    assert_eq!(mem::size_of::<SmallByteStr>(), mem::size_of::<usize>());
}
