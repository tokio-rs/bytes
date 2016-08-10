use bytes::{Bytes};
use std::ops;
use std::io::Cursor;

/*
 *
 * ===== Small immutable set of bytes =====
 *
 */

#[cfg(target_pointer_width = "64")]
const MAX_LEN: usize = 7;

#[cfg(target_pointer_width = "32")]
const MAX_LEN: usize = 3;

#[derive(Clone, Copy)]
pub struct Small {
    len: u8,
    bytes: [u8; MAX_LEN],
}

impl Small {
    pub fn empty() -> Small {
        use std::mem;

        Small {
            len: 0,
            bytes: unsafe { mem::zeroed() }
        }
    }

    pub fn from_slice(bytes: &[u8]) -> Option<Small> {
        use std::{mem, ptr};

        if bytes.len() > MAX_LEN {
            return None;
        }

        let mut ret = Small {
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

    pub fn buf(&self) -> Cursor<&[u8]> {
        Cursor::new(self.as_ref())
    }

    pub fn slice(&self, begin: usize, end: usize) -> Bytes {
        Bytes::from(&self.as_ref()[begin..end])
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }}

impl AsRef<[u8]> for Small {
    fn as_ref(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

impl ops::Index<usize> for Small {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        assert!(index < self.len());
        &self.bytes[index]
    }
}
