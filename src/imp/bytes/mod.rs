pub mod rope;
pub mod seq;
pub mod small;

use {Buf, IntoBuf};
use self::seq::Seq;
use self::small::Small;
use self::rope::{Rope, RopeBuf};
use std::{cmp, fmt, ops};
use std::io::Cursor;
use std::sync::Arc;

/// An immutable sequence of bytes
#[derive(Clone)]
pub struct Bytes {
    kind: Kind,
}

#[derive(Clone)]
enum Kind {
    Seq(Seq),
    Small(Small),
    Rope(Arc<Rope>),
}

pub struct BytesBuf<'a> {
    kind: BufKind<'a>,
}

enum BufKind<'a> {
    Cursor(Cursor<&'a [u8]>),
    Rope(RopeBuf<'a>),
}

impl Bytes {
    /// Return an empty `Bytes`
    pub fn empty() -> Bytes {
        Bytes { kind: Kind::Small(Small::empty()) }
    }

    pub fn from_slice<T: AsRef<[u8]>>(slice: T) -> Bytes {
        Small::from_slice(slice.as_ref())
            .map(|b| Bytes { kind: Kind::Small(b)})
            .unwrap_or_else(|| Seq::from_slice(slice.as_ref()))
    }

    pub fn from_vec(mem: Vec<u8>) -> Bytes {
        let pos = 0;
        let len = mem.len();

        Small::from_slice(&mem[..])
            .map(|b| Bytes { kind: Kind::Small(b) })
            .unwrap_or_else(|| {
                let seq = Seq::new(Arc::new(mem.into_boxed_slice()), pos, len);
                Bytes { kind: Kind::Seq(seq) }
            })
    }

    /// Creates a new `Bytes` from an `Arc<Box<[u8]>>`, an offset, and a length.
    #[inline]
    pub fn from_boxed(mem: Arc<Box<[u8]>>, pos: usize, len: usize) -> Bytes {
        // Check ranges
        assert!(pos + len <= mem.len(), "invalid arguments");

        Small::from_slice(&mem[pos..pos + len])
            .map(|b| Bytes { kind: Kind::Small(b) })
            .unwrap_or_else(|| {
                let seq = Seq::new(mem, pos, len);
                Bytes { kind: Kind::Seq(seq) }
            })
    }

    pub fn buf(&self) -> BytesBuf {
        let kind = match self.kind {
            Kind::Seq(ref v) => BufKind::Cursor(v.buf()),
            Kind::Small(ref v) => BufKind::Cursor(v.buf()),
            Kind::Rope(ref v) => BufKind::Rope(v.buf()),
        };

        BytesBuf { kind: kind }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        match self.kind {
            Kind::Seq(ref v) => v.len(),
            Kind::Small(ref v) => v.len(),
            Kind::Rope(ref v) => v.len(),
        }
    }

    pub fn concat(&self, other: &Bytes) -> Bytes {
        Rope::concat(self.clone(), other.clone())
    }

    /// Returns a new ByteStr value containing the byte range between `begin`
    /// (inclusive) and `end` (exclusive)
    pub fn slice(&self, begin: usize, end: usize) -> Bytes {
        match self.kind {
            Kind::Seq(ref v) => v.slice(begin, end),
            Kind::Small(ref v) => v.slice(begin, end),
            Kind::Rope(ref v) => v.slice(begin, end),
        }
    }

    /// Returns a new ByteStr value containing the byte range starting from
    /// `begin` (inclusive) to the end of the byte str.
    ///
    /// Equivalent to `bytes.slice(begin, bytes.len())`
    pub fn slice_from(&self, begin: usize) -> Bytes {
        self.slice(begin, self.len())
    }

    /// Returns a new ByteStr value containing the byte range from the start up
    /// to `end` (exclusive).
    ///
    /// Equivalent to `bytes.slice(0, end)`
    pub fn slice_to(&self, end: usize) -> Bytes {
        self.slice(0, end)
    }

    /// Returns the Rope depth
    fn depth(&self) -> u16 {
        match self.kind {
            Kind::Rope(ref r) => r.depth(),
            _ => 0,
        }
    }

    fn into_rope(self) -> Result<Arc<Rope>, Bytes> {
        match self.kind {
            Kind::Rope(r) => Ok(r),
            _ => Err(self),
        }
    }
}

impl<'a> From<&'a [u8]> for Bytes {
    fn from(src: &'a [u8]) -> Bytes {
        Bytes::from_slice(src)
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(src: Vec<u8>) -> Bytes {
        let mem = Arc::new(src.into_boxed_slice());
        let len = mem.len();

        Bytes::from_boxed(mem, 0, len)
    }
}

impl ops::Index<usize> for Bytes {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        match self.kind {
            Kind::Seq(ref v) => v.index(index),
            Kind::Small(ref v) => v.index(index),
            Kind::Rope(ref v) => v.index(index),
        }
    }
}

impl cmp::PartialEq<Bytes> for Bytes {
    fn eq(&self, other: &Bytes) -> bool {
        if self.len() != other.len() {
            return false;
        }

        let mut buf1 = self.buf();
        let mut buf2 = self.buf();

        while buf1.has_remaining() {
            let len;

            {
                let b1 = buf1.bytes();
                let b2 = buf2.bytes();

                len = cmp::min(b1.len(), b2.len());

                if b1[..len] != b2[..len] {
                    return false;
                }
            }

            buf1.advance(len);
            buf2.advance(len);
        }

        true
    }

    fn ne(&self, other: &Bytes) -> bool {
        return !self.eq(other)
    }
}

impl<'a> IntoBuf for &'a Bytes {
    type Buf = BytesBuf<'a>;

    fn into_buf(self) -> Self::Buf {
        self.buf()
    }
}

/*
 *
 * ===== BytesBuf =====
 *
 */

impl<'a> Buf for BytesBuf<'a> {
    fn remaining(&self) -> usize {
        match self.kind {
            BufKind::Cursor(ref v) => v.remaining(),
            BufKind::Rope(ref v) => v.remaining(),
        }
    }

    fn bytes(&self) -> &[u8] {
        match self.kind {
            BufKind::Cursor(ref v) => v.bytes(),
            BufKind::Rope(ref v) => v.bytes(),
        }
    }

    fn advance(&mut self, cnt: usize) {
        match self.kind {
            BufKind::Cursor(ref mut v) => v.advance(cnt),
            BufKind::Rope(ref mut v) => v.advance(cnt),
        }
    }
}


/*
 *
 * ===== Internal utilities =====
 *
 */

impl fmt::Debug for Bytes {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut buf = self.buf();

        try!(write!(fmt, "Bytes[len={}; ", self.len()));

        let mut rem = 128;

        while buf.has_remaining() {
            let byte = buf.read_u8();

            if rem > 0 {
                if is_ascii(byte) {
                    try!(write!(fmt, "{}", byte as char));
                } else {
                    try!(write!(fmt, "\\x{:02X}", byte));
                }

                rem -= 1;
            } else {
                try!(write!(fmt, " ... "));
                break;
            }
        }

        try!(write!(fmt, "]"));

        Ok(())
    }
}

fn is_ascii(byte: u8) -> bool {
    match byte {
        10 | 13 | 32...126 => true,
        _ => false,
    }
}
