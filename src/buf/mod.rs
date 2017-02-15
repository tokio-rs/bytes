pub mod take;

use {Bytes, Take, TakeMut};
use byteorder::ByteOrder;
use std::{cmp, io, ptr, usize};

/// A trait for values that provide sequential read access to bytes.
pub trait Buf {

    /// Returns the number of bytes that can be accessed from the Buf
    fn remaining(&self) -> usize;

    /// Returns a slice starting at the current Buf position and of length
    /// between 0 and `Buf::remaining()`.
    fn bytes(&self) -> &[u8];

    /// Advance the internal cursor of the Buf
    fn advance(&mut self, cnt: usize);

    /// Returns true if there are any more bytes to consume
    fn has_remaining(&self) -> bool {
        self.remaining() > 0
    }

    /// Copies bytes from `self` into `dst`
    ///
    /// # Panics
    ///
    /// The function panics if `self` does not contain enough bytes to fill
    /// `dst`.
    fn copy_to<S: Sink + ?Sized>(&mut self, dst: &mut S) where Self: Sized {
        dst.sink(self);
    }

    /// Copies bytes from the `Buf` into the given slice and advance the cursor by
    /// the number of bytes copied.
    ///
    /// ```
    /// use std::io::Cursor;
    /// use bytes::Buf;
    ///
    /// let mut buf = Cursor::new(b"hello world");
    /// let mut dst = [0; 5];
    ///
    /// buf.copy_to_slice(&mut dst);
    /// assert_eq!(b"hello", &dst);
    /// assert_eq!(6, buf.remaining());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if `self.remaining() < dst.len()`
    fn copy_to_slice(&mut self, dst: &mut [u8]) {
        let mut off = 0;

        assert!(self.remaining() >= dst.len());

        while off < dst.len() {
            let cnt;

            unsafe {
                let src = self.bytes();
                cnt = cmp::min(src.len(), dst.len() - off);

                ptr::copy_nonoverlapping(
                    src.as_ptr(), dst[off..].as_mut_ptr(), cnt);

                off += src.len();
            }

            self.advance(cnt);
        }
    }

    /// Gets an unsigned 8 bit integer from the `Buf` without advancing the
    /// buffer cursor
    fn peek_u8(&self) -> Option<u8> {
        if self.has_remaining() {
            Some(self.bytes()[0])
        } else {
            None
        }
    }

    /// Gets an unsigned 8 bit integer from the `Buf`.
    fn get_u8(&mut self) -> u8 {
        let mut buf = [0; 1];
        self.copy_to_slice(&mut buf);
        buf[0]
    }

    /// Gets a signed 8 bit integer from the `Buf`.
    fn get_i8(&mut self) -> i8 {
        let mut buf = [0; 1];
        self.copy_to_slice(&mut buf);
        buf[0] as i8
    }

    /// Gets an unsigned 16 bit integer from the `Buf`
    fn get_u16<T: ByteOrder>(&mut self) -> u16 {
        let mut buf = [0; 2];
        self.copy_to_slice(&mut buf);
        T::read_u16(&buf)
    }

    /// Gets a signed 16 bit integer from the `Buf`
    fn get_i16<T: ByteOrder>(&mut self) -> i16 {
        let mut buf = [0; 2];
        self.copy_to_slice(&mut buf);
        T::read_i16(&buf)
    }

    /// Gets an unsigned 32 bit integer from the `Buf`
    fn get_u32<T: ByteOrder>(&mut self) -> u32 {
        let mut buf = [0; 4];
        self.copy_to_slice(&mut buf);
        T::read_u32(&buf)
    }

    /// Gets a signed 32 bit integer from the `Buf`
    fn get_i32<T: ByteOrder>(&mut self) -> i32 {
        let mut buf = [0; 4];
        self.copy_to_slice(&mut buf);
        T::read_i32(&buf)
    }

    /// Gets an unsigned 64 bit integer from the `Buf`
    fn get_u64<T: ByteOrder>(&mut self) -> u64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf);
        T::read_u64(&buf)
    }

    /// Gets a signed 64 bit integer from the `Buf`
    fn get_i64<T: ByteOrder>(&mut self) -> i64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf);
        T::read_i64(&buf)
    }

    /// Gets an unsigned n-bytes integer from the `Buf`
    fn get_uint<T: ByteOrder>(&mut self, nbytes: usize) -> u64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf[..nbytes]);
        T::read_uint(&buf[..nbytes], nbytes)
    }

    /// Gets a signed n-bytes integer from the `Buf`
    fn get_int<T: ByteOrder>(&mut self, nbytes: usize) -> i64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf[..nbytes]);
        T::read_int(&buf[..nbytes], nbytes)
    }

    /// Gets a IEEE754 single-precision (4 bytes) floating point number from
    /// the `Buf`
    fn get_f32<T: ByteOrder>(&mut self) -> f32 {
        let mut buf = [0; 4];
        self.copy_to_slice(&mut buf);
        T::read_f32(&buf)
    }

    /// Gets a IEEE754 double-precision (8 bytes) floating point number from
    /// the `Buf`
    fn get_f64<T: ByteOrder>(&mut self) -> f64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf);
        T::read_f64(&buf)
    }

    /// Creates a "by reference" adaptor for this instance of Buf
    fn by_ref(&mut self) -> &mut Self where Self: Sized {
        self
    }

    /// Create an adapter which will limit at most `limit` bytes from it.
    fn take(self, limit: usize) -> Take<Self> where Self: Sized {
        take::new(self, limit)
    }

    /// Return a `Reader` for the value. Allows using a `Buf` as an `io::Read`
    fn reader(self) -> Reader<Self> where Self: Sized {
        Reader::new(self)
    }
}

/// A trait for values that provide sequential write access to bytes.
pub trait BufMut {

    /// Returns the number of bytes that can be written to the BufMut
    fn remaining_mut(&self) -> usize;

    /// Advance the internal cursor of the BufMut
    unsafe fn advance_mut(&mut self, cnt: usize);

    /// Returns true iff there is any more space for bytes to be written
    fn has_remaining_mut(&self) -> bool {
        self.remaining_mut() > 0
    }

    /// Returns a mutable slice starting at the current BufMut position and of
    /// length between 0 and `BufMut::remaining()`.
    ///
    /// The returned byte slice may represent uninitialized memory.
    unsafe fn bytes_mut(&mut self) -> &mut [u8];

    /// Copies bytes from `src` into `self`
    ///
    /// # Panics
    ///
    /// Panics if `self` does not have enough capacity to copy all the data
    /// from `src`
    fn copy_from<S: Source>(&mut self, src: S) where Self: Sized {
        src.source(self);
    }

    /// Copies bytes from the given slice into the `BufMut` and advance the
    /// cursor by the number of bytes written.
    /// Returns the number of bytes written.
    ///
    /// ```
    /// use bytes::BufMut;
    /// use std::io::Cursor;
    ///
    /// let mut dst = [0; 6];
    ///
    /// {
    ///     let mut buf = Cursor::new(&mut dst);
    ///     buf.copy_from_slice(b"hello");
    ///
    ///     assert_eq!(1, buf.remaining_mut());
    /// }
    ///
    /// assert_eq!(b"hello\0", &dst);
    /// ```
    fn copy_from_slice(&mut self, src: &[u8]) {
        let mut off = 0;

        assert!(self.remaining_mut() >= src.len(), "buffer overflow");

        while off < src.len() {
            let cnt;

            unsafe {
                let dst = self.bytes_mut();
                cnt = cmp::min(dst.len(), src.len() - off);

                ptr::copy_nonoverlapping(
                    src[off..].as_ptr(),
                    dst.as_mut_ptr(),
                    cnt);

                off += cnt;

            }

            unsafe { self.advance_mut(cnt); }
        }
    }

    /// Writes the given string into self.
    ///
    /// # Panics
    ///
    /// The function panics if `self` does not have enough remaining capacity
    /// to write the full string.
    fn put_str(&mut self, src: &str) {
        self.copy_from_slice(src.as_bytes());
    }

    /// Writes an unsigned 8 bit integer to the BufMut.
    fn put_u8(&mut self, n: u8) {
        self.copy_from_slice(&[n])
    }

    /// Writes a signed 8 bit integer to the BufMut.
    fn put_i8(&mut self, n: i8) {
        self.copy_from_slice(&[n as u8])
    }

    /// Writes an unsigned 16 bit integer to the BufMut.
    fn put_u16<T: ByteOrder>(&mut self, n: u16) {
        let mut buf = [0; 2];
        T::write_u16(&mut buf, n);
        self.copy_from_slice(&buf)
    }

    /// Writes a signed 16 bit integer to the BufMut.
    fn put_i16<T: ByteOrder>(&mut self, n: i16) {
        let mut buf = [0; 2];
        T::write_i16(&mut buf, n);
        self.copy_from_slice(&buf)
    }

    /// Writes an unsigned 32 bit integer to the BufMut.
    fn put_u32<T: ByteOrder>(&mut self, n: u32) {
        let mut buf = [0; 4];
        T::write_u32(&mut buf, n);
        self.copy_from_slice(&buf)
    }

    /// Writes a signed 32 bit integer to the BufMut.
    fn put_i32<T: ByteOrder>(&mut self, n: i32) {
        let mut buf = [0; 4];
        T::write_i32(&mut buf, n);
        self.copy_from_slice(&buf)
    }

    /// Writes an unsigned 64 bit integer to the BufMut.
    fn put_u64<T: ByteOrder>(&mut self, n: u64) {
        let mut buf = [0; 8];
        T::write_u64(&mut buf, n);
        self.copy_from_slice(&buf)
    }

    /// Writes a signed 64 bit integer to the BufMut.
    fn put_i64<T: ByteOrder>(&mut self, n: i64) {
        let mut buf = [0; 8];
        T::write_i64(&mut buf, n);
        self.copy_from_slice(&buf)
    }

    /// Writes an unsigned n-bytes integer to the BufMut.
    ///
    /// If the given integer is not representable in the given number of bytes,
    /// this method panics. If `nbytes > 8`, this method panics.
    fn put_uint<T: ByteOrder>(&mut self, n: u64, nbytes: usize) {
        let mut buf = [0; 8];
        T::write_uint(&mut buf, n, nbytes);
        self.copy_from_slice(&buf[0..nbytes])
    }

    /// Writes a signed n-bytes integer to the BufMut.
    ///
    /// If the given integer is not representable in the given number of bytes,
    /// this method panics. If `nbytes > 8`, this method panics.
    fn put_int<T: ByteOrder>(&mut self, n: i64, nbytes: usize) {
        let mut buf = [0; 8];
        T::write_int(&mut buf, n, nbytes);
        self.copy_from_slice(&buf[0..nbytes])
    }

    /// Writes a IEEE754 single-precision (4 bytes) floating point number to
    /// the BufMut.
    fn put_f32<T: ByteOrder>(&mut self, n: f32) {
        let mut buf = [0; 4];
        T::write_f32(&mut buf, n);
        self.copy_from_slice(&buf)
    }

    /// Writes a IEEE754 double-precision (8 bytes) floating point number to
    /// the BufMut.
    fn put_f64<T: ByteOrder>(&mut self, n: f64) {
        let mut buf = [0; 8];
        T::write_f64(&mut buf, n);
        self.copy_from_slice(&buf)
    }

    /// Creates a "by reference" adaptor for this instance of BufMut
    fn by_ref(&mut self) -> &mut Self where Self: Sized {
        self
    }

    /// Create an adapter which will limit at most `limit` bytes from it.
    fn take_mut(self, limit: usize) -> TakeMut<Self> where Self: Sized {
        take::new_mut(self, limit)
    }

    /// Return a `Write` for the value. Allows using a `BufMut` as an
    /// `io::Write`
    fn writer(self) -> Writer<Self> where Self: Sized {
        Writer::new(self)
    }
}

/*
 *
 * ===== IntoBuf =====
 *
 */

/// Conversion into a `Buf`
///
/// Usually, `IntoBuf` is implemented on references of types and not directly
/// on the types themselves. For example, `IntoBuf` is implemented for `&'a
/// Vec<u8>` and not `Vec<u8>` directly.
pub trait IntoBuf {
    /// The `Buf` type that `self` is being converted into
    type Buf: Buf;

    /// Creates a `Buf` from a value.
    fn into_buf(self) -> Self::Buf;
}

impl<'a> IntoBuf for &'a [u8] {
    type Buf = io::Cursor<&'a [u8]>;

    /// Creates a buffer from a value
    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(self)
    }
}

// Kind of annoying...
impl<'a> IntoBuf for &'a &'static [u8] {
    type Buf = io::Cursor<&'static [u8]>;

    /// Creates a buffer from a value
    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(self)
    }
}

impl IntoBuf for Vec<u8> {
    type Buf = io::Cursor<Vec<u8>>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(self)
    }
}

impl<'a> IntoBuf for &'a Vec<u8> {
    type Buf = io::Cursor<&'a [u8]>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(&self[..])
    }
}

impl IntoBuf for () {
    type Buf = io::Cursor<&'static [u8]>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(&[])
    }
}

impl<'a> IntoBuf for &'a () {
    type Buf = io::Cursor<&'static [u8]>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(&[])
    }
}

/*
 *
 * ===== Sink / Source =====
 *
 */


/// A value that writes bytes from itself into a `BufMut`.
pub trait Source {
    /// Copy data from self into destination buffer
    fn source<B: BufMut>(self, buf: &mut B);
}

impl<'a> Source for &'a [u8] {
    fn source<B: BufMut>(self, buf: &mut B) {
        buf.copy_from_slice(self);
    }
}

impl Source for u8 {
    fn source<B: BufMut>(self, buf: &mut B) {
        let src = [self];
        buf.copy_from_slice(&src);
    }
}

impl Source for Bytes {
    fn source<B: BufMut>(self, buf: &mut B) {
        Source::source(self.as_ref(), buf);
    }
}

impl<'a> Source for &'a Bytes {
    fn source<B: BufMut>(self, buf: &mut B) {
        Source::source(self.as_ref(), buf);
    }
}

impl<'a, T: Buf> Source for &'a mut T {
    fn source<B: BufMut>(mut self, buf: &mut B) {
        assert!(buf.remaining_mut() >= self.remaining());

        while self.has_remaining() {
            let l;

            unsafe {
                let s = self.bytes();
                let d = buf.bytes_mut();
                l = cmp::min(s.len(), d.len());

                ptr::copy_nonoverlapping(
                    s.as_ptr(),
                    d.as_mut_ptr(),
                    l);
            }

            self.advance(l);
            unsafe { buf.advance_mut(l); }
        }
    }
}

/// A value that copies bytes from a `Buf` into itself
pub trait Sink {
    /// Copy bytes from `buf` into `self`
    fn sink<B: Buf>(&mut self, buf: &mut B);
}

impl Sink for [u8] {
    fn sink<B: Buf>(&mut self, buf: &mut B) {
        buf.copy_to_slice(self);
    }
}

impl<T: BufMut> Sink for T {
    fn sink<B: Buf>(&mut self, buf: &mut B) {
        Source::source(buf, self)
    }
}

/*
 *
 * ===== Read / Write =====
 *
 */

/// Adapts a `Buf` to the `io::Read` trait
pub struct Reader<B> {
    buf: B,
}

impl<B: Buf> Reader<B> {
    /// Return a `Reader` for the given `buf`
    pub fn new(buf: B) -> Reader<B> {
        Reader { buf: buf }
    }

    /// Gets a reference to the underlying buf.
    pub fn get_ref(&self) -> &B {
        &self.buf
    }

    /// Gets a mutable reference to the underlying buf.
    pub fn get_mut(&mut self) -> &mut B {
        &mut self.buf
    }

    /// Unwraps this `Reader`, returning the underlying `Buf`
    pub fn into_inner(self) -> B {
        self.buf
    }
}

impl<B: Buf + Sized> io::Read for Reader<B> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        let len = cmp::min(self.buf.remaining(), dst.len());

        Buf::copy_to(&mut self.buf, &mut dst[0..len]);
        Ok(len)
    }
}

/// Adapts a `BufMut` to the `io::Write` trait
pub struct Writer<B> {
    buf: B,
}

impl<B: BufMut> Writer<B> {
    /// Return a `Writer` for teh given `buf`
    pub fn new(buf: B) -> Writer<B> {
        Writer { buf: buf }
    }

    /// Gets a reference to the underlying buf.
    pub fn get_ref(&self) -> &B {
        &self.buf
    }

    /// Gets a mutable reference to the underlying buf.
    pub fn get_mut(&mut self) -> &mut B {
        &mut self.buf
    }

    /// Unwraps this `Writer`, returning the underlying `BufMut`
    pub fn into_inner(self) -> B {
        self.buf
    }
}

impl<B: BufMut + Sized> io::Write for Writer<B> {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        let n = cmp::min(self.buf.remaining_mut(), src.len());

        self.buf.copy_from(&src[0..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/*
 *
 * ===== Buf impls =====
 *
 */

impl<'a, T: Buf> Buf for &'a mut T {
    fn remaining(&self) -> usize {
        (**self).remaining()
    }

    fn bytes(&self) -> &[u8] {
        (**self).bytes()
    }

    fn advance(&mut self, cnt: usize) {
        (**self).advance(cnt)
    }
}

impl<'a, T: BufMut> BufMut for &'a mut T {
    fn remaining_mut(&self) -> usize {
        (**self).remaining_mut()
    }

    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        (**self).bytes_mut()
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        (**self).advance_mut(cnt)
    }
}

impl<T: AsRef<[u8]>> Buf for io::Cursor<T> {
    fn remaining(&self) -> usize {
        let len = self.get_ref().as_ref().len();
        let pos = self.position();

        if pos >= len as u64 {
            return 0;
        }

        len - pos as usize
    }

    fn bytes(&self) -> &[u8] {
        let pos = self.position() as usize;
        &(self.get_ref().as_ref())[pos..]
    }

    fn advance(&mut self, cnt: usize) {
        let pos = self.position() as usize;
        let pos = cmp::min(self.get_ref().as_ref().len(), pos + cnt);
        self.set_position(pos as u64);
    }
}

impl<T: AsMut<[u8]> + AsRef<[u8]>> BufMut for io::Cursor<T> {
    fn remaining_mut(&self) -> usize {
        self.remaining()
    }

    /// Advance the internal cursor of the BufMut
    unsafe fn advance_mut(&mut self, cnt: usize) {
        let pos = self.position() as usize;
        let pos = cmp::min(self.get_mut().as_mut().len(), pos + cnt);
        self.set_position(pos as u64);
    }

    /// Returns a mutable slice starting at the current BufMut position and of
    /// length between 0 and `BufMut::remaining()`.
    ///
    /// The returned byte slice may represent uninitialized memory.
    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        let pos = self.position() as usize;
        &mut (self.get_mut().as_mut())[pos..]
    }
}

impl BufMut for Vec<u8> {
    fn remaining_mut(&self) -> usize {
        usize::MAX - self.len()
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        let len = self.len() + cnt;

        if len > self.capacity() {
            // Reserve additional
            // TODO: Should this case panic?
            let cap = self.capacity();
            self.reserve(cap - len);
        }

        self.set_len(len);
    }

    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
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
