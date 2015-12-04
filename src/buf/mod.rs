mod byte;
mod ring;
mod sink;
mod slice;
mod source;
mod take;

pub use self::byte::{ByteBuf, MutByteBuf, ROByteBuf};
pub use self::ring::RingBuf;
pub use self::slice::{SliceBuf, MutSliceBuf};
pub use self::take::Take;

use {BufError, RopeBuf};
use std::{cmp, fmt, io, ptr, usize};

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
    /// Returns the number of bytes read.
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
            let cnt;

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

    /// Returns the number of bytes that can be written to the MutBuf
    fn remaining(&self) -> usize;

    /// Advance the internal cursor of the MutBuf
    unsafe fn advance(&mut self, cnt: usize);

    /// Returns true iff there is any more space for bytes to be written
    fn has_remaining(&self) -> bool {
        self.remaining() > 0
    }

    /// Returns a mutable slice starting at the current MutBuf position and of
    /// length between 0 and `MutBuf::remaining()`.
    ///
    /// The returned byte slice may represent uninitialized memory.
    unsafe fn mut_bytes<'a>(&'a mut self) -> &'a mut [u8];

    /// Write bytes from the given slice into the `MutBuf` and advance the
    /// cursor by the number of bytes written.
    /// Returns the number of bytes written.
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
            let cnt;

            unsafe {
                let dst = self.mut_bytes();
                cnt = cmp::min(dst.len(), len - off);

                ptr::copy_nonoverlapping(
                    src[off..].as_ptr(),
                    dst.as_mut_ptr(),
                    cnt);

                off += cnt;

            }

            unsafe { self.advance(cnt); }
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


impl<'a, R: io::Read+'a> Source for &'a mut R {
    type Error = io::Error;

    fn fill<B: MutBuf>(self, buf: &mut B) -> Result<usize, io::Error> {
        let mut cnt = 0;

        while buf.has_remaining() {
            let i = try!(self.read(unsafe { buf.mut_bytes() }));

            if i == 0 {
                break;
            }

            unsafe { buf.advance(i); }
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

impl Buf for io::Cursor<Vec<u8>> {
    fn remaining(&self) -> usize {
        self.get_ref().len() - self.position() as usize
    }

    fn bytes(&self) -> &[u8] {
        let pos = self.position() as usize;
        &(&self.get_ref())[pos..]
    }

    fn advance(&mut self, cnt: usize) {
        let pos = self.position() as usize;
        let pos = cmp::min(self.get_ref().len(), pos + cnt);
        self.set_position(pos as u64);
    }
}

impl MutBuf for Vec<u8> {
    fn remaining(&self) -> usize {
        usize::MAX - self.len()
    }

    unsafe fn advance(&mut self, cnt: usize) {
        let len = self.len() + cnt;

        if len > self.capacity() {
            // Reserve additional
            // TODO: Should this case panic?
            let cap = self.capacity();
            self.reserve(cap - len);
        }

        self.set_len(len);
    }

    unsafe fn mut_bytes(&mut self) -> &mut [u8] {
        use std::slice;

        if self.capacity() == self.len() {
            self.reserve(64); // Grow the vec
        }

        let cap = self.capacity();
        let len = self.len();

        let ptr = self.as_mut_ptr();
        &mut slice::from_raw_parts_mut(ptr, cap)[len..]
    }
}

impl<'a> Buf for io::Cursor<&'a [u8]> {
    fn remaining(&self) -> usize {
        self.get_ref().len() - self.position() as usize
    }

    fn bytes(&self) -> &[u8] {
        let pos = self.position() as usize;
        &(&self.get_ref())[pos..]
    }

    fn advance(&mut self, cnt: usize) {
        let pos = self.position() as usize;
        let pos = cmp::min(self.get_ref().len(), pos + cnt);
        self.set_position(pos as u64);
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
