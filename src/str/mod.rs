mod bytes;
mod rope;
mod seq;
mod small;

pub use self::bytes::Bytes;
pub use self::rope::{Rope, RopeBuf};
pub use self::seq::SeqByteStr;
pub use self::small::{SmallByteStr, SmallByteStrBuf};

use {Buf};
use std::{cmp, fmt, ops};
use std::any::Any;

/// An immutable sequence of bytes. Operations will not mutate the original
/// value. Since only immutable access is permitted, operations do not require
/// copying (though, sometimes copying will happen as an optimization).
pub trait ByteStr : Clone + Sized + Send + Sync + Any + ToBytes + ops::Index<usize, Output=u8> + 'static {

    // Until HKT lands, the buf must be bound by 'static
    type Buf: Buf+'static;

    /// Returns a read-only `Buf` for accessing the byte contents of the
    /// `ByteStr`.
    fn buf(&self) -> Self::Buf;

    /// Returns a new `Bytes` value representing the concatenation of `self`
    /// with the given `Bytes`.
    fn concat<B: ByteStr+'static>(&self, other: &B) -> Bytes;

    /// Returns the number of bytes in the ByteStr
    fn len(&self) -> usize;

    /// Returns true if the length of the `ByteStr` is 0
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a new ByteStr value containing the byte range between `begin`
    /// (inclusive) and `end` (exclusive)
    fn slice(&self, begin: usize, end: usize) -> Bytes;

    /// Returns a new ByteStr value containing the byte range starting from
    /// `begin` (inclusive) to the end of the byte str.
    ///
    /// Equivalent to `bytes.slice(begin, bytes.len())`
    fn slice_from(&self, begin: usize) -> Bytes {
        self.slice(begin, self.len())
    }

    /// Returns a new ByteStr value containing the byte range from the start up
    /// to `end` (exclusive).
    ///
    /// Equivalent to `bytes.slice(0, end)`
    fn slice_to(&self, end: usize) -> Bytes {
        self.slice(0, end)
    }

    /// Divides the value into two `Bytes` at the given index.
    ///
    /// The first will contain all bytes from `[0, mid]` (excluding the index
    /// `mid` itself) and the second will contain all indices from `[mid, len)`
    /// (excluding the index `len` itself).
    ///
    /// Panics if `mid > len`.
    fn split_at(&self, mid: usize) -> (Bytes, Bytes) {
        (self.slice_to(mid), self.slice_from(mid))
    }
}

macro_rules! impl_parteq {
    ($ty:ty) => {
        impl<B: ByteStr> cmp::PartialEq<B> for $ty {
            fn eq(&self, other: &B) -> bool {
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

            fn ne(&self, other: &B) -> bool {
                return !self.eq(other)
            }
        }
    }
}

impl_parteq!(SeqByteStr);
impl_parteq!(SmallByteStr);
impl_parteq!(Bytes);
impl_parteq!(Rope);

macro_rules! impl_eq {
    ($ty:ty) => {
        impl cmp::Eq for $ty {}
    }
}

impl_eq!(Bytes);

/*
 *
 * ===== ToBytes =====
 *
 */

pub trait ToBytes {
    /// Consumes the value and returns a `Bytes` instance containing
    /// identical bytes
    fn to_bytes(self) -> Bytes;
}

impl<'a> ToBytes for &'a [u8] {
    fn to_bytes(self) -> Bytes {
        Bytes::from_slice(self)
    }
}

impl<'a> ToBytes for &'a Vec<u8> {
    fn to_bytes(self) -> Bytes {
        (&self[..]).to_bytes()
    }
}



/*
 *
 * ===== Internal utilities =====
 *
 */

fn debug<B: ByteStr>(bytes: &B, name: &str, fmt: &mut fmt::Formatter) -> fmt::Result {
    let mut buf = bytes.buf();

    try!(write!(fmt, "{}[len={}; ", name, bytes.len()));

    let mut rem = 128;

    while let Some(byte) = buf.read_byte() {
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

fn is_ascii(byte: u8) -> bool {
    match byte {
        10 | 13 | 32...126 => true,
        _ => false,
    }
}
