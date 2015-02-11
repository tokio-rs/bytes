#![crate_name = "bytes"]
#![unstable]

#![feature(core)]
#![feature(alloc)]

pub use byte_buf::{ByteBuf, ROByteBuf, MutByteBuf};
pub use byte_str::{SeqByteStr, SmallByteStr, SmallByteStrBuf};
pub use bytes::Bytes;
pub use ring::{RingBuf, RingBufReader, RingBufWriter};
pub use rope::Rope;
pub use slice::{SliceBuf, MutSliceBuf};

use std::{cmp, io, ops, ptr, u32};

extern crate core;

mod alloc;
mod byte_buf;
mod byte_str;
mod bytes;
mod ring;
mod rope;
mod slice;

pub mod traits {
    pub use {Buf, BufExt, MutBuf, MutBufExt, ByteStr};
}

const MAX_CAPACITY: usize = u32::MAX as usize;

/// A trait for values that provide random and sequential access to bytes.
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
    /// assert_eq!(b"hello", dst);
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

                ptr::copy_nonoverlapping_memory(
                    dst[off..].as_mut_ptr(), src.as_ptr(), cnt);

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

pub trait BufExt {

    /// Read bytes from this Buf into the given sink and advance the cursor by
    /// the number of bytes read.
    fn read<S: Sink>(&mut self, dst: S) -> Result<usize, S::Error>;
}

// TODO: Remove Sized
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
    /// assert_eq!(b"hello\0", dst);
    /// ```
    fn write_slice(&mut self, src: &[u8]) -> usize {
        let mut off = 0;
        let len = cmp::min(src.len(), self.remaining());

        while off < len {
            let mut cnt;

            unsafe {
                let dst = self.mut_bytes();
                cnt = cmp::min(dst.len(), len - off);

                ptr::copy_nonoverlapping_memory(
                    dst.as_mut_ptr(), src[off..].as_ptr(), cnt);

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

pub trait ByteStr : Clone + Sized + Send + Sync + ops::Index<usize, Output=u8> {

    // Until HKT lands, the buf must be bound by 'static
    type Buf: Buf+'static;

    /// Returns a read-only `Buf` for accessing the byte contents of the
    /// `ByteStr`.
    fn buf(&self) -> Self::Buf;

    /// Returns a new `Bytes` value representing the concatenation of `self`
    /// with the given `Bytes`.
    fn concat<B: ByteStr+'static>(&self, other: B) -> Bytes;

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

    /// Consumes the value and returns a `Bytes` instance containing
    /// identical bytes
    fn to_bytes(self) -> Bytes;
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

/// An value that reads bytes from a Buf into itself
pub trait Sink {
    type Error;

    fn sink<B: Buf>(self, buf: &mut B) -> Result<usize, Self::Error>;
}

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

        let rem = buf.remaining();
        let cap = self.capacity();
        let len = rem - cap;

        // Ensure that the vec is big enough
        if cap < rem {
            self.reserve(len);
        }

        unsafe {
            {
                let dst = self.as_mut_slice();
                buf.read_slice(slice::from_raw_parts_mut(dst.as_mut_ptr(), rem));
            }

            self.set_len(rem);
        }

        Ok(len)
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
        Ok(buf.write_slice(self.as_slice()))
    }
}

impl<'a> Source for &'a Bytes {
    type Error = BufError;

    fn fill<B: MutBuf>(self, _buf: &mut B) -> Result<usize, BufError> {
        unimplemented!();
    }
}

impl<'a> Source for &'a mut (io::Read+'a) {
    type Error = io::Error;

    fn fill<B: MutBuf>(self, _buf: &mut B) -> Result<usize, io::Error> {
        unimplemented!();
    }
}

impl<'a> Source for &'a mut (Iterator<Item=u8>+'a) {
    type Error = BufError;

    fn fill<B: MutBuf>(self, _buf: &mut B) -> Result<usize, BufError> {
        unimplemented!();
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

/*
 *
 * ===== BufError / BufResult =====
 *
 */

#[derive(Copy, Debug)]
pub enum BufError {
    Underflow,
    Overflow,
}

pub type BufResult<T> = Result<T, BufError>;
