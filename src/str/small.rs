use {Bytes, Rope};
use traits::{Buf, MutBuf, ByteStr, ToBytes};
use std::{cmp, ops};

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
        use std::{mem, ptr};

        if bytes.len() > MAX_LEN {
            return None;
        }

        let mut ret = SmallByteStr {
            len: bytes.len() as u8,
            bytes: unsafe { mem::zeroed() },
        };

        // Copy the memory
        unsafe {
            ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                ret.bytes.as_mut_ptr(),
                bytes.len());
        }

        Some(ret)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
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

    fn slice(&self, begin: usize, end: usize) -> Bytes {
        Bytes::from_slice(&self.as_slice()[begin..end])
    }
}

impl ToBytes for SmallByteStr {
    fn to_bytes(self) -> Bytes {
        Bytes::of(self)
    }
}

impl ops::Index<usize> for SmallByteStr {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        assert!(index < self.len());
        &self.bytes[index]
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
