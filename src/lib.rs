#![crate_name = "bytes"]
#![unstable]

#![feature(alloc, convert, core)]

pub use byte_buf::{ByteBuf, ROByteBuf, MutByteBuf};
pub use byte_str::{SeqByteStr, SmallByteStr, SmallByteStrBuf};
pub use bytes::Bytes;
pub use ring::RingBuf;
pub use rope::{Rope, RopeBuf};
pub use slice::{SliceBuf, MutSliceBuf};

use std::{cmp, fmt, io, ops, ptr, u32};
use std::marker::Reflect;

extern crate core;

mod alloc;
mod byte_buf;
mod byte_str;
mod bytes;
mod ring;
mod rope;
mod slice;

pub mod traits {
    //! All traits are re-exported here to allow glob imports.
    pub use {Buf, BufExt, MutBuf, MutBufExt, ByteStr, ToBytes};
}

const MAX_CAPACITY: usize = u32::MAX as usize;

/// A trait for values that provide sequential read access to bytes.
pub trait Buf {

    /// Returns the number of bytes that can be accessed from the Buf
    fn remaining(&self) -> usize;

    /// Returns a slice starting at the current Buf position and of length
    /// between 0 and `Buf::remaining()`.
    fn bytes<'a>(&'a self) -> &'a [u8];

    /// Advance the internal cursor of the Buf
    fn advance(&mut self, cnt: usize);

    /// Returns true if there are any more bytes to consume
    fn has_remaining(&self) -> bool {
        self.remaining() > 0
    }

    /// Read bytes from the `Buf` into the given slice and advance the cursor by
    /// the number of bytes read.
    ///
    /// If there are fewer bytes remaining than is needed to satisfy the
    /// request (aka `dst.len()` > self.remaining()`), then
    /// `Err(BufError::Overflow)` is returned.
    ///
    /// ```
    /// use bytes::{SliceBuf, Buf};
    ///
    /// let mut buf = SliceBuf::wrap(b"hello world");
    /// let mut dst = [0; 5];
    ///
    /// buf.read_slice(&mut dst);
    /// assert_eq!(b"hello", &dst);
    /// assert_eq!(6, buf.remaining());
    /// ```
    fn read_slice(&mut self, dst: &mut [u8]) -> usize {
        let mut off = 0;
        let len = cmp::min(dst.len(), self.remaining());

        while off < len {
            let mut cnt;

            unsafe {
                let src = self.bytes();
                cnt = cmp::min(src.len(), len - off);

                ptr::copy_nonoverlapping(
                    src.as_ptr(), dst[off..].as_mut_ptr(), cnt);

                off += src.len();
            }

            self.advance(cnt);
        }

        len
    }

    /// Read a single byte from the `Buf`
    fn read_byte(&mut self) -> Option<u8> {
        let mut dst = [0];

        if self.read_slice(&mut dst) == 0 {
            return None;
        }

        Some(dst[0])
    }
}

/// An extension trait providing extra functions applicable to all `Buf` values.
pub trait BufExt {

    /// Read bytes from this Buf into the given sink and advance the cursor by
    /// the number of bytes read.
    fn read<S: Sink>(&mut self, dst: S) -> Result<usize, S::Error>;
}

/// A trait for values that provide sequential write access to bytes.
pub trait MutBuf : Sized {

    /// Returns the number of bytes that can be accessed from the Buf
    fn remaining(&self) -> usize;

    /// Advance the internal cursor of the Buf
    fn advance(&mut self, cnt: usize);

    /// Returns true if there are any more bytes to consume
    fn has_remaining(&self) -> bool {
        self.remaining() > 0
    }

    /// Returns a mutable slice starting at the current Buf position and of
    /// length between 0 and `Buf::remaining()`.
    fn mut_bytes<'a>(&'a mut self) -> &'a mut [u8];

    /// Read bytes from this Buf into the given slice and advance the cursor by
    /// the number of bytes read.
    ///
    /// If there are fewer bytes remaining than is needed to satisfy the
    /// request (aka `dst.len()` > self.remaining()`), then
    /// `Err(BufError::Overflow)` is returned.
    ///
    /// ```
    /// use bytes::{MutSliceBuf, Buf, MutBuf};
    ///
    /// let mut dst = [0; 6];
    ///
    /// {
    ///     let mut buf = MutSliceBuf::wrap(&mut dst);
    ///     buf.write_slice(b"hello");
    ///
    ///     assert_eq!(1, buf.remaining());
    /// }
    ///
    /// assert_eq!(b"hello\0", &dst);
    /// ```
    fn write_slice(&mut self, src: &[u8]) -> usize {
        let mut off = 0;
        let len = cmp::min(src.len(), self.remaining());

        while off < len {
            let mut cnt;

            unsafe {
                let dst = self.mut_bytes();
                cnt = cmp::min(dst.len(), len - off);

                ptr::copy_nonoverlapping(
                    src[off..].as_ptr(),
                    dst.as_mut_ptr(),
                    cnt);

                off += cnt;
            }

            self.advance(cnt);
        }

        len
    }

    /// Write a single byte to the `MuBuf`
    fn write_byte(&mut self, byte: u8) -> bool {
        let src = [byte];

        if self.write_slice(&src) == 0 {
            return false;
        }

        true
    }
}

/// An extension trait providing extra functions applicable to all `MutBuf` values.
pub trait MutBufExt {

    /// Write bytes from the given source into the current `MutBuf` and advance
    /// the cursor by the number of bytes written.
    fn write<S: Source>(&mut self, src: S) -> Result<usize, S::Error>;
}

/*
 *
 * ===== ByteStr =====
 *
 */

/// An immutable sequence of bytes. Operations will not mutate the original
/// value. Since only immutable access is permitted, operations do not require
/// copying (though, sometimes copying will happen as an optimization).
pub trait ByteStr : Clone + Sized + Send + Sync + Reflect + ToBytes + ops::Index<usize, Output=u8> + 'static {

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
 * ===== *Ext impls =====
 *
 */

impl<B: Buf> BufExt for B {
    fn read<S: Sink>(&mut self, dst: S) -> Result<usize, S::Error> {
        dst.sink(self)
    }
}

impl<B: MutBuf> MutBufExt for B {
    fn write<S: Source>(&mut self, src: S) -> Result<usize, S::Error> {
        src.fill(self)
    }
}

/*
 *
 * ===== Sink / Source =====
 *
 */

/// A value that reads bytes from a Buf into itself
pub trait Sink {
    type Error;

    fn sink<B: Buf>(self, buf: &mut B) -> Result<usize, Self::Error>;
}

/// A value that writes bytes from itself into a `MutBuf`.
pub trait Source {
    type Error;

    fn fill<B: MutBuf>(self, buf: &mut B) -> Result<usize, Self::Error>;
}

impl<'a> Sink for &'a mut [u8] {
    type Error = BufError;

    fn sink<B: Buf>(self, buf: &mut B) -> Result<usize, BufError> {
        Ok(buf.read_slice(self))
    }
}

impl<'a> Sink for &'a mut Vec<u8> {
    type Error = BufError;

    fn sink<B: Buf>(self, buf: &mut B) -> Result<usize, BufError> {
        use std::slice;

        self.clear();

        let rem = buf.remaining();
        let cap = self.capacity();

        // Ensure that the vec is big enough
        if rem > self.capacity() {
            self.reserve(rem - cap);
        }

        unsafe {
            {
                let dst = &mut self[..];
                let cnt = buf.read_slice(slice::from_raw_parts_mut(dst.as_mut_ptr(), rem));

                debug_assert!(cnt == rem);
            }

            self.set_len(rem);
        }

        Ok(rem)
    }
}

impl<'a> Source for &'a [u8] {
    type Error = BufError;

    fn fill<B: MutBuf>(self, buf: &mut B) -> Result<usize, BufError> {
        Ok(buf.write_slice(self))
    }
}

impl<'a> Source for &'a Vec<u8> {
    type Error = BufError;

    fn fill<B: MutBuf>(self, buf: &mut B) -> Result<usize, BufError> {
        Ok(buf.write_slice(self.as_ref()))
    }
}

impl<'a> Source for &'a Bytes {
    type Error = BufError;

    fn fill<B: MutBuf>(self, dst: &mut B) -> Result<usize, BufError> {
        let mut src = self.buf();
        let mut res = 0;

        while src.has_remaining() && dst.has_remaining() {
            let mut l;

            {
                let s = src.bytes();
                let d = dst.mut_bytes();
                l = cmp::min(s.len(), d.len());

                unsafe {
                    ptr::copy_nonoverlapping(
                        s.as_ptr(),
                        d.as_mut_ptr(),
                        l);
                }
            }

            src.advance(l);
            dst.advance(l);

            res += l;
        }

        Ok(res)
    }
}

impl<'a, R: io::Read+'a> Source for &'a mut R {
    type Error = io::Error;

    fn fill<B: MutBuf>(self, buf: &mut B) -> Result<usize, io::Error> {
        let mut cnt = 0;

        while buf.has_remaining() {
            let i = try!(self.read(buf.mut_bytes()));

            if i == 0 {
                break;
            }

            buf.advance(i);
            cnt += i;
        }

        Ok(cnt)
    }
}

/*
 *
 * ===== Buf impls =====
 *
 */

impl Buf for Box<Buf+'static> {
    fn remaining(&self) -> usize {
        (**self).remaining()
    }

    fn bytes(&self) -> &[u8] {
        (**self).bytes()
    }

    fn advance(&mut self, cnt: usize) {
        (**self).advance(cnt);
    }

    fn read_slice(&mut self, dst: &mut [u8]) -> usize {
        (**self).read_slice(dst)
    }
}

impl fmt::Debug for Box<Buf+'static> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Box<Buf> {{ remaining: {} }}", self.remaining())
    }
}

/*
 *
 * ===== Read impls =====
 *
 */

macro_rules! impl_read {
    ($ty:ty) => {
        impl io::Read for $ty {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if !self.has_remaining() {
                    return Ok(0);
                }

                Ok(self.read_slice(buf))
            }
        }
    }
}

impl_read!(ByteBuf);
impl_read!(ROByteBuf);
impl_read!(RopeBuf);
impl_read!(Box<Buf+'static>);

macro_rules! impl_write {
    ($ty:ty) => {
        impl io::Write for $ty {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                if !self.has_remaining() {
                    return Ok(0);
                }

                Ok(self.write_slice(buf))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
    }
}

impl_write!(MutByteBuf);

/*
 *
 * ===== BufError  =====
 *
 */

#[derive(Copy, Clone, Debug)]
pub enum BufError {
    Underflow,
    Overflow,
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
