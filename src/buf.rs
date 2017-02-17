use {Bytes};
use byteorder::ByteOrder;
use std::{cmp, io, ptr, usize};

/// Read bytes from a buffer.
///
/// A buffer stores bytes in memory such that read operations are infallible.
/// The underlying storage may or may not be in contiguous memory. A `Buf` value
/// is a cursor into the buffer. Reading from `Buf` advances the cursor
/// position.
///
/// The simplest `Buf` is a `Cursor` wrapping a `[u8]`.
///
/// ```
/// use bytes::Buf;
/// use std::io::Cursor;
///
/// let mut buf = Cursor::new(b"hello world");
///
/// assert_eq!(b'h', buf.get_u8());
/// assert_eq!(b'e', buf.get_u8());
/// assert_eq!(b'l', buf.get_u8());
///
/// let mut rest = [0; 8];
/// buf.copy_to_slice(&mut rest);
///
/// assert_eq!(&rest[..], b"lo world");
/// ```
pub trait Buf {
    /// Returns the number of bytes between the current position and the end of
    /// the buffer.
    ///
    /// This value is greater than or equal to the length of the slice returned
    /// by `bytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Buf;
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"hello world");
    ///
    /// assert_eq!(buf.remaining(), 11);
    ///
    /// buf.get_u8();
    ///
    /// assert_eq!(buf.remaining(), 10);
    /// ```
    fn remaining(&self) -> usize;

    /// Returns a slice starting at the current position and of length between 0
    /// and `Buf::remaining()`.
    ///
    /// This is a lower level function. Most operations are done with other
    /// functions.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Buf;
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"hello world");
    ///
    /// assert_eq!(buf.bytes(), b"hello world");
    ///
    /// buf.advance(6);
    ///
    /// assert_eq!(buf.bytes(), b"world");
    /// ```
    fn bytes(&self) -> &[u8];

    /// Advance the internal cursor of the Buf
    ///
    /// The next call to `bytes` will return a slice starting `cnt` bytes
    /// further into the underlying buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Buf;
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"hello world");
    ///
    /// assert_eq!(buf.bytes(), b"hello world");
    ///
    /// buf.advance(6);
    ///
    /// assert_eq!(buf.bytes(), b"world");
    /// ```
    ///
    /// # Panics
    ///
    /// This function can panic if `cnt > self.remaining()`.
    fn advance(&mut self, cnt: usize);

    /// Returns true if there are any more bytes to consume
    ///
    /// This is equivalent to `self.remaining() != 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Buf;
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"a");
    ///
    /// assert!(buf.has_remaining());
    ///
    /// buf.get_u8();
    ///
    /// assert!(!buf.has_remaining());
    /// ```
    fn has_remaining(&self) -> bool {
        self.remaining() > 0
    }

    /// Copies bytes from `self` into `dst`.
    ///
    /// The cursor is advanced by the number of bytes copied. `self` must have
    /// enough remaining bytes to fill `dst`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Buf;
    /// use std::io::Cursor;
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

    /// Gets an unsigned 8 bit integer from `self`.
    ///
    /// The current position is advanced by 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Buf;
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x08 hello");
    /// assert_eq!(8, buf.get_u8());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is no more remaining data in `self`.
    fn get_u8(&mut self) -> u8 {
        let mut buf = [0; 1];
        self.copy_to_slice(&mut buf);
        buf[0]
    }

    /// Gets a signed 8 bit integer from `self`.
    ///
    /// The current position is advanced by 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Buf;
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x08 hello");
    /// assert_eq!(8, buf.get_i8());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is no more remaining data in `self`.
    fn get_i8(&mut self) -> i8 {
        let mut buf = [0; 1];
        self.copy_to_slice(&mut buf);
        buf[0] as i8
    }

    /// Gets an unsigned 16 bit integer from `self` in the specified byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x08\x09 hello");
    /// assert_eq!(0x0809, buf.get_u16::<BigEndian>());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_u16<T: ByteOrder>(&mut self) -> u16 {
        let mut buf = [0; 2];
        self.copy_to_slice(&mut buf);
        T::read_u16(&buf)
    }

    /// Gets a signed 16 bit integer from `self` in the specified byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x08\x09 hello");
    /// assert_eq!(0x0809, buf.get_i16::<BigEndian>());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_i16<T: ByteOrder>(&mut self) -> i16 {
        let mut buf = [0; 2];
        self.copy_to_slice(&mut buf);
        T::read_i16(&buf)
    }

    /// Gets an unsigned 32 bit integer from `self` in the specified byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x08\x09\xA0\xA1 hello");
    /// assert_eq!(0x0809A0A1, buf.get_u32::<BigEndian>());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_u32<T: ByteOrder>(&mut self) -> u32 {
        let mut buf = [0; 4];
        self.copy_to_slice(&mut buf);
        T::read_u32(&buf)
    }

    /// Gets a signed 32 bit integer from `self` in the specified byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x08\x09\xA0\xA1 hello");
    /// assert_eq!(0x0809A0A1, buf.get_i32::<BigEndian>());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_i32<T: ByteOrder>(&mut self) -> i32 {
        let mut buf = [0; 4];
        self.copy_to_slice(&mut buf);
        T::read_i32(&buf)
    }

    /// Gets an unsigned 64 bit integer from `self` in the specified byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x01\x02\x03\x04\x05\x06\x07\x08 hello");
    /// assert_eq!(0x0102030405060708, buf.get_u64::<BigEndian>());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_u64<T: ByteOrder>(&mut self) -> u64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf);
        T::read_u64(&buf)
    }

    /// Gets a signed 64 bit integer from `self` in the specified byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x01\x02\x03\x04\x05\x06\x07\x08 hello");
    /// assert_eq!(0x0102030405060708, buf.get_i64::<BigEndian>());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_i64<T: ByteOrder>(&mut self) -> i64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf);
        T::read_i64(&buf)
    }

    /// Gets an unsigned n-byte integer from `self` in the specified byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x01\x02\x03 hello");
    /// assert_eq!(0x010203, buf.get_uint::<BigEndian>(3));
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_uint<T: ByteOrder>(&mut self, nbytes: usize) -> u64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf[..nbytes]);
        T::read_uint(&buf[..nbytes], nbytes)
    }

    /// Gets a signed n-byte integer from `self` in the specified byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x01\x02\x03 hello");
    /// assert_eq!(0x010203, buf.get_int::<BigEndian>(3));
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_int<T: ByteOrder>(&mut self, nbytes: usize) -> i64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf[..nbytes]);
        T::read_int(&buf[..nbytes], nbytes)
    }

    /// Gets an IEEE754 single-precision (4 bytes) floating point number from
    /// `self` in the specified byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x3F\x99\x99\x9A hello");
    /// assert_eq!(1.2f32, buf.get_f32::<BigEndian>());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_f32<T: ByteOrder>(&mut self) -> f32 {
        let mut buf = [0; 4];
        self.copy_to_slice(&mut buf);
        T::read_f32(&buf)
    }

    /// Gets an IEEE754 double-precision (8 bytes) floating point number from
    /// `self` in the specified byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BigEndian};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"\x3F\xF3\x33\x33\x33\x33\x33\x33 hello");
    /// assert_eq!(1.2f64, buf.get_f64::<BigEndian>());
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining data in `self`.
    fn get_f64<T: ByteOrder>(&mut self) -> f64 {
        let mut buf = [0; 8];
        self.copy_to_slice(&mut buf);
        T::read_f64(&buf)
    }

    /// Creates an adaptor which will read at most `limit` bytes from `self`.
    ///
    /// This function returns a new instance of `Buf` which will read at most
    /// `limit` bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BufMut};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new("hello world").take(5);
    /// let mut dst = vec![];
    ///
    /// dst.put(&mut buf);
    /// assert_eq!(dst, b"hello");
    ///
    /// let mut buf = buf.into_inner();
    /// dst.clear();
    /// dst.put(&mut buf);
    /// assert_eq!(dst, b" world");
    /// ```
    fn take(self, limit: usize) -> Take<Self>
        where Self: Sized
    {
        Take {
            inner: self,
            limit: limit,
        }
    }

    /// Creates a "by reference" adaptor for this instance of `Buf`.
    ///
    /// The returned adaptor also implements `Buf` and will simply borrow `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, BufMut};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new("hello world");
    /// let mut dst = vec![];
    ///
    /// {
    ///     let mut reference = buf.by_ref();
    ///     dst.put(&mut reference.take(5));
    ///     assert_eq!(dst, b"hello");
    /// } // drop our &mut reference so we can use `buf` again
    ///
    /// dst.clear();
    /// dst.put(&mut buf);
    /// assert_eq!(dst, b" world");
    /// ```
    fn by_ref(&mut self) -> &mut Self where Self: Sized {
        self
    }

    /// Creates an adaptor which implements the `Read` trait for `self`.
    ///
    /// This function returns a new value which implements `Read` by adapting
    /// the `Read` trait functions to the `Buf` trait functions. Given that
    /// `Buf` operations are infallible, none of the `Read` functions will
    /// return with `Err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, IntoBuf, Bytes};
    /// use std::io::Read;
    ///
    /// let buf = Bytes::from("hello world").into_buf();
    ///
    /// let mut reader = buf.reader();
    /// let mut dst = [0; 1024];
    ///
    /// let num = reader.read(&mut dst).unwrap();
    ///
    /// assert_eq!(11, num);
    /// assert_eq!(&dst[..11], b"hello world");
    /// ```
    fn reader(self) -> Reader<Self> where Self: Sized {
        Reader { buf: self }
    }
}

/// A trait for values that provide sequential write access to bytes.
///
/// Write bytes to a buffer
///
/// A buffer stores bytes in memory such that write operations are infallible.
/// The underlying storage may or may not be in contiguous memory. A `BufMut`
/// value is a cursor into the buffer. Writing to `BufMut` advances the cursor
/// position.
///
/// The simplest `BufMut` is a `Vec<u8>`.
///
/// ```
/// use bytes::BufMut;
///
/// let mut buf = vec![];
///
/// buf.put("hello world");
///
/// assert_eq!(buf, b"hello world");
/// ```
pub trait BufMut {
    /// Returns the number of bytes that can be written from the current
    /// position until the end of the buffer is reached.
    ///
    /// This value is greater than or equal to the length of the slice returned
    /// by `bytes_mut`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    /// use std::io::Cursor;
    ///
    /// let mut dst = [0; 10];
    /// let mut buf = Cursor::new(&mut dst[..]);
    ///
    /// assert_eq!(10, buf.remaining_mut());
    /// buf.put("hello");
    ///
    /// assert_eq!(5, buf.remaining_mut());
    /// ```
    fn remaining_mut(&self) -> usize;

    /// Advance the internal cursor of the BufMut
    ///
    /// The next call to `bytes_mut` will return a slice starting `cnt` bytes
    /// further into the underlying buffer.
    ///
    /// This function is unsafe because there is no guarantee that the bytes
    /// being advanced to have been initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = Vec::with_capacity(16);
    ///
    /// unsafe {
    ///     buf.bytes_mut()[0] = b'h';
    ///     buf.bytes_mut()[1] = b'e';
    ///
    ///     buf.advance_mut(2);
    ///
    ///     buf.bytes_mut()[0] = b'l';
    ///     buf.bytes_mut()[1..3].copy_from_slice(b"lo");
    ///
    ///     buf.advance_mut(3);
    /// }
    ///
    /// assert_eq!(5, buf.len());
    /// assert_eq!(buf, b"hello");
    /// ```
    ///
    /// # Panics
    ///
    /// This function can panic if `cnt > self.remaining_mut()`.
    unsafe fn advance_mut(&mut self, cnt: usize);

    /// Returns true if there is space in `self` for more bytes.
    ///
    /// This is equivalent to `self.remaining_mut() != 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    /// use std::io::Cursor;
    ///
    /// let mut dst = [0; 5];
    /// let mut buf = Cursor::new(&mut dst);
    ///
    /// assert!(buf.has_remaining_mut());
    ///
    /// buf.put("hello");
    ///
    /// assert!(!buf.has_remaining_mut());
    /// ```
    fn has_remaining_mut(&self) -> bool {
        self.remaining_mut() > 0
    }

    /// Returns a mutable slice starting at the current BufMut position and of
    /// length between 0 and `BufMut::remaining_mut()`.
    ///
    /// This is a lower level function. Most operations are done with other
    /// functions.
    ///
    /// The returned byte slice may represent uninitialized memory.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = Vec::with_capacity(16);
    ///
    /// unsafe {
    ///     buf.bytes_mut()[0] = b'h';
    ///     buf.bytes_mut()[1] = b'e';
    ///
    ///     buf.advance_mut(2);
    ///
    ///     buf.bytes_mut()[0] = b'l';
    ///     buf.bytes_mut()[1..3].copy_from_slice(b"lo");
    ///
    ///     buf.advance_mut(3);
    /// }
    ///
    /// assert_eq!(5, buf.len());
    /// assert_eq!(buf, b"hello");
    /// ```
    unsafe fn bytes_mut(&mut self) -> &mut [u8];

    /// Transfer bytes into `self` from `src` and advance the cursor by the
    /// number of bytes written.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![];
    ///
    /// buf.put(b'h');
    /// buf.put(&b"ello"[..]);
    /// buf.put(" world");
    ///
    /// assert_eq!(buf, b"hello world");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `self` does not have enough capacity to contain `src`.
    fn put<S: Source>(&mut self, src: S) where Self: Sized {
        src.copy_to_buf(self);
    }

    /// Transfer bytes into `self` from `src` and advance the cursor by the
    /// number of bytes written.
    ///
    /// `self` must have enough remaining capacity to contain all of `src`.
    ///
    /// ```
    /// use bytes::BufMut;
    /// use std::io::Cursor;
    ///
    /// let mut dst = [0; 6];
    ///
    /// {
    ///     let mut buf = Cursor::new(&mut dst);
    ///     buf.put_slice(b"hello");
    ///
    ///     assert_eq!(1, buf.remaining_mut());
    /// }
    ///
    /// assert_eq!(b"hello\0", &dst);
    /// ```
    fn put_slice(&mut self, src: &[u8]) {
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

    /// Writes an unsigned 16 bit integer to `self` in the specified byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_u16::<BigEndian>(0x0809);
    /// assert_eq!(buf, b"\x08\x09");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u16<T: ByteOrder>(&mut self, n: u16) {
        let mut buf = [0; 2];
        T::write_u16(&mut buf, n);
        self.put_slice(&buf)
    }

    /// Writes a signed 16 bit integer to `self` in the specified byte order.
    ///
    /// The current position is advanced by 2.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_i16::<BigEndian>(0x0809);
    /// assert_eq!(buf, b"\x08\x09");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i16<T: ByteOrder>(&mut self, n: i16) {
        let mut buf = [0; 2];
        T::write_i16(&mut buf, n);
        self.put_slice(&buf)
    }

    /// Writes an unsigned 32 bit integer to `self` in the specified byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_u32::<BigEndian>(0x0809A0A1);
    /// assert_eq!(buf, b"\x08\x09\xA0\xA1");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u32<T: ByteOrder>(&mut self, n: u32) {
        let mut buf = [0; 4];
        T::write_u32(&mut buf, n);
        self.put_slice(&buf)
    }

    /// Writes a signed 32 bit integer to `self` in the specified byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_i32::<BigEndian>(0x0809A0A1);
    /// assert_eq!(buf, b"\x08\x09\xA0\xA1");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i32<T: ByteOrder>(&mut self, n: i32) {
        let mut buf = [0; 4];
        T::write_i32(&mut buf, n);
        self.put_slice(&buf)
    }

    /// Writes an unsigned 64 bit integer to `self` in the specified byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_u64::<BigEndian>(0x0102030405060708);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_u64<T: ByteOrder>(&mut self, n: u64) {
        let mut buf = [0; 8];
        T::write_u64(&mut buf, n);
        self.put_slice(&buf)
    }

    /// Writes a signed 64 bit integer to `self` in the specified byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_i64::<BigEndian>(0x0102030405060708);
    /// assert_eq!(buf, b"\x01\x02\x03\x04\x05\x06\x07\x08");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_i64<T: ByteOrder>(&mut self, n: i64) {
        let mut buf = [0; 8];
        T::write_i64(&mut buf, n);
        self.put_slice(&buf)
    }

    /// Writes an unsigned n-byte integer to `self` in the specified byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_uint::<BigEndian>(0x010203, 3);
    /// assert_eq!(buf, b"\x01\x02\x03");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_uint<T: ByteOrder>(&mut self, n: u64, nbytes: usize) {
        let mut buf = [0; 8];
        T::write_uint(&mut buf, n, nbytes);
        self.put_slice(&buf[0..nbytes])
    }

    /// Writes a signed n-byte integer to `self` in the specified byte order.
    ///
    /// The current position is advanced by `nbytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_int::<BigEndian>(0x010203, 3);
    /// assert_eq!(buf, b"\x01\x02\x03");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_int<T: ByteOrder>(&mut self, n: i64, nbytes: usize) {
        let mut buf = [0; 8];
        T::write_int(&mut buf, n, nbytes);
        self.put_slice(&buf[0..nbytes])
    }

    /// Writes  an IEEE754 single-precision (4 bytes) floating point number to
    /// `self` in the specified byte order.
    ///
    /// The current position is advanced by 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_f32::<BigEndian>(1.2f32);
    /// assert_eq!(buf, b"\x3F\x99\x99\x9A");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_f32<T: ByteOrder>(&mut self, n: f32) {
        let mut buf = [0; 4];
        T::write_f32(&mut buf, n);
        self.put_slice(&buf)
    }

    /// Writes  an IEEE754 double-precision (8 bytes) floating point number to
    /// `self` in the specified byte order.
    ///
    /// The current position is advanced by 8.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, BigEndian};
    ///
    /// let mut buf = vec![];
    /// buf.put_f64::<BigEndian>(1.2f64);
    /// assert_eq!(buf, b"\x3F\xF3\x33\x33\x33\x33\x33\x33");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if there is not enough remaining capacity in
    /// `self`.
    fn put_f64<T: ByteOrder>(&mut self, n: f64) {
        let mut buf = [0; 8];
        T::write_f64(&mut buf, n);
        self.put_slice(&buf)
    }

    /// Creates a "by reference" adaptor for this instance of `BufMut`.
    ///
    /// The returned adapter also implements `BufMut` and will simply borrow
    /// `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    /// use std::io;
    ///
    /// let mut buf = vec![];
    ///
    /// {
    ///     let mut reference = buf.by_ref();
    ///
    ///     // Adapt reference to `std::io::Write`.
    ///     let mut writer = reference.writer();
    ///
    ///     // Use the buffer as a writter
    ///     io::Write::write(&mut writer, &b"hello world"[..]).unwrap();
    /// } // drop our &mut reference so that we can use `buf` again
    ///
    /// assert_eq!(buf, &b"hello world"[..]);
    /// ```
    fn by_ref(&mut self) -> &mut Self where Self: Sized {
        self
    }

    /// Creates an adaptor which implements the `Write` trait for `self`.
    ///
    /// This function returns a new value which implements `Write` by adapting
    /// the `Write` trait functions to the `BufMut` trait functions. Given that
    /// `BufMut` operations are infallible, none of the `Write` functions will
    /// return with `Err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    /// use std::io::Write;
    ///
    /// let mut buf = vec![].writer();
    ///
    /// let num = buf.write(&b"hello world"[..]).unwrap();
    /// assert_eq!(11, num);
    ///
    /// let buf = buf.into_inner();
    ///
    /// assert_eq!(*buf, b"hello world"[..]);
    /// ```
    fn writer(self) -> Writer<Self> where Self: Sized {
        Writer { buf: self }
    }
}

/*
 *
 * ===== IntoBuf =====
 *
 */

/// Conversion into a `Buf`
///
/// An `IntoBuf` implementation defines how to convert a value into a `Buf`.
/// This is common for types that represent byte storage of some kind. `IntoBuf`
/// may be implemented directly for types or on references for those types.
///
/// # Examples
///
/// ```
/// use bytes::{Buf, IntoBuf, BigEndian};
///
/// let bytes = b"\x00\x01hello world";
/// let mut buf = bytes.into_buf();
///
/// assert_eq!(1, buf.get_u16::<BigEndian>());
///
/// let mut rest = [0; 11];
/// buf.copy_to_slice(&mut rest);
///
/// assert_eq!(b"hello world", &rest);
/// ```
pub trait IntoBuf {
    /// The `Buf` type that `self` is being converted into
    type Buf: Buf;

    /// Creates a `Buf` from a value.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, IntoBuf, BigEndian};
    ///
    /// let bytes = b"\x00\x01hello world";
    /// let mut buf = bytes.into_buf();
    ///
    /// assert_eq!(1, buf.get_u16::<BigEndian>());
    ///
    /// let mut rest = [0; 11];
    /// buf.copy_to_slice(&mut rest);
    ///
    /// assert_eq!(b"hello world", &rest);
    /// ```
    fn into_buf(self) -> Self::Buf;
}

impl<'a> IntoBuf for &'a [u8] {
    type Buf = io::Cursor<&'a [u8]>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(self)
    }
}

impl<'a> IntoBuf for &'a str {
    type Buf = io::Cursor<&'a [u8]>;

    fn into_buf(self) -> Self::Buf {
        self.as_bytes().into_buf()
    }
}

impl IntoBuf for Vec<u8> {
    type Buf = io::Cursor<Vec<u8>>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(self)
    }
}

// Kind of annoying... but this impl is required to allow passing `&'static
// [u8]` where for<'a> &'a T: IntoBuf is required.
impl<'a> IntoBuf for &'a &'static [u8] {
    type Buf = io::Cursor<&'static [u8]>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(self)
    }
}

impl<'a> IntoBuf for &'a &'static str {
    type Buf = io::Cursor<&'static [u8]>;

    fn into_buf(self) -> Self::Buf {
        self.as_bytes().into_buf()
    }
}

impl<'a> IntoBuf for &'a Vec<u8> {
    type Buf = io::Cursor<&'a [u8]>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(&self[..])
    }
}

impl IntoBuf for String {
    type Buf = io::Cursor<Vec<u8>>;

    fn into_buf(self) -> Self::Buf {
        self.into_bytes().into_buf()
    }
}

impl<'a> IntoBuf for &'a String {
    type Buf = io::Cursor<&'a [u8]>;

    fn into_buf(self) -> Self::Buf {
        self.as_bytes().into_buf()
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
 * ===== Source =====
 *
 */


/// A value that writes bytes from itself into a `BufMut`.
///
/// Values that implement `Source` are used as an argument to
/// [`BufMut::put`](trait.BufMut.html#method.put).
///
/// # Examples
///
/// ```
/// use bytes::{BufMut, Source};
///
/// struct Repeat {
///     num: usize,
///     str: String,
/// }
///
/// impl Source for Repeat {
///     fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
///         for _ in 0..self.num {
///             buf.put(&self.str);
///         }
///     }
/// }
///
/// let mut dst = vec![];
/// dst.put(Repeat {
///     num: 3,
///     str: "hello".into(),
/// });
///
/// assert_eq!(*dst, b"hellohellohello"[..]);
/// ```
pub trait Source {
    /// Copy data from self into destination buffer
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, Source};
    ///
    /// let mut dst = vec![];
    ///
    /// "hello".copy_to_buf(&mut dst);
    ///
    /// assert_eq!(*dst, b"hello"[..]);
    /// ```
    ///
    /// # Panics
    ///
    /// This function panis if `buf` does not have enough capacity for `self`.
    fn copy_to_buf<B: BufMut>(self, buf: &mut B);
}

impl Source for Vec<u8> {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(&self[..]);
    }
}

impl<'a> Source for &'a Vec<u8> {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(&self[..]);
    }
}

impl<'a> Source for &'a [u8] {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(self);
    }
}

impl Source for String {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(self.as_bytes());
    }
}

impl<'a> Source for &'a String {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(self.as_bytes());
    }
}

impl<'a> Source for &'a str {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(self.as_bytes());
    }
}

impl Source for u8 {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        let src = [self];
        buf.put_slice(&src);
    }
}

impl Source for i8 {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(&[self as u8])
    }
}

impl Source for Bytes {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        Source::copy_to_buf(self.as_ref(), buf);
    }
}

impl<'a> Source for &'a Bytes {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        Source::copy_to_buf(self.as_ref(), buf);
    }
}

impl<T: Buf> Source for T {
    fn copy_to_buf<B: BufMut>(mut self, buf: &mut B) {
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

/*
 *
 * ===== Take =====
 *
 */

/// A `Buf` adapter which limits the bytes read from an underlying buffer.
///
/// This struct is generally created by calling `take()` on `Buf`. See
/// documentation of [`take()`](trait.Buf.html#method.take) for more details.
pub struct Take<T> {
    inner: T,
    limit: usize,
}

impl<T> Take<T> {
    /// Consumes this `Take`, returning the underlying value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::{Buf, BufMut};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"hello world").take(2);
    /// let mut dst = vec![];
    ///
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"he"[..]);
    ///
    /// let mut buf = buf.into_inner();
    ///
    /// dst.clear();
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"llo world"[..]);
    /// ```
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Gets a reference to the underlying `Buf`.
    ///
    /// It is inadvisable to directly read from the underlying `Buf`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::{Buf, BufMut};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"hello world").take(2);
    ///
    /// assert_eq!(0, buf.get_ref().position());
    /// ```
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying `Buf`.
    ///
    /// It is inadvisable to directly read from the underlying `Buf`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::{Buf, BufMut};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"hello world").take(2);
    /// let mut dst = vec![];
    ///
    /// buf.get_mut().set_position(2);
    ///
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"ll"[..]);
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the maximum number of bytes that can be read.
    ///
    /// # Note
    ///
    /// If the inner `Buf` has fewer bytes than indicated by this method then
    /// that is the actual number of available bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::Buf;
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"hello world").take(2);
    ///
    /// assert_eq!(2, buf.limit());
    /// assert_eq!(b'h', buf.get_u8());
    /// assert_eq!(1, buf.limit());
    /// ```
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Sets the maximum number of bytes that can be read.
    ///
    /// # Note
    ///
    /// If the inner `Buf` has fewer bytes than `lim` then that is the actual
    /// number of available bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::{Buf, BufMut};
    /// use std::io::Cursor;
    ///
    /// let mut buf = Cursor::new(b"hello world").take(2);
    /// let mut dst = vec![];
    ///
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"he"[..]);
    ///
    /// dst.clear();
    ///
    /// buf.set_limit(3);
    /// dst.put(&mut buf);
    /// assert_eq!(*dst, b"llo"[..]);
    /// ```
    pub fn set_limit(&mut self, lim: usize) {
        self.limit = lim
    }
}

impl<T: Buf> Buf for Take<T> {
    fn remaining(&self) -> usize {
        cmp::min(self.inner.remaining(), self.limit)
    }

    fn bytes(&self) -> &[u8] {
        &self.inner.bytes()[..self.limit]
    }

    fn advance(&mut self, cnt: usize) {
        let cnt = cmp::min(cnt, self.limit);
        self.limit -= cnt;
        self.inner.advance(cnt);
    }
}

/*
 *
 * ===== Read / Write =====
 *
 */

/// A `Buf` adapter which implements `io::Read` for the inner value.
///
/// This struct is generally created by calling `reader()` on `Buf`. See
/// documentation of [`reader()`](trait.Buf.html#method.reader) for more
/// details.
pub struct Reader<B> {
    buf: B,
}

impl<B: Buf> Reader<B> {
    /// Gets a reference to the underlying `Buf`.
    ///
    /// It is inadvisable to directly read from the underlying `Buf`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::Buf;
    /// use std::io::{self, Cursor};
    ///
    /// let mut buf = Cursor::new(b"hello world").reader();
    ///
    /// assert_eq!(0, buf.get_ref().position());
    /// ```
    pub fn get_ref(&self) -> &B {
        &self.buf
    }

    /// Gets a mutable reference to the underlying `Buf`.
    ///
    /// It is inadvisable to directly read from the underlying `Buf`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::Buf;
    /// use std::io::{self, Cursor};
    ///
    /// let mut buf = Cursor::new(b"hello world").reader();
    /// let mut dst = vec![];
    ///
    /// buf.get_mut().set_position(2);
    /// io::copy(&mut buf, &mut dst).unwrap();
    ///
    /// assert_eq!(*dst, b"llo world"[..]);
    /// ```
    pub fn get_mut(&mut self) -> &mut B {
        &mut self.buf
    }

    /// Consumes this `Reader`, returning the underlying value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::Buf;
    /// use std::io::{self, Cursor};
    ///
    /// let mut buf = Cursor::new(b"hello world").reader();
    /// let mut dst = vec![];
    ///
    /// io::copy(&mut buf, &mut dst).unwrap();
    ///
    /// let buf = buf.into_inner();
    /// assert_eq!(0, buf.remaining());
    /// ```
    pub fn into_inner(self) -> B {
        self.buf
    }
}

impl<B: Buf + Sized> io::Read for Reader<B> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        let len = cmp::min(self.buf.remaining(), dst.len());

        Buf::copy_to_slice(&mut self.buf, &mut dst[0..len]);
        Ok(len)
    }
}

/// A `BufMut` adapter which implements `io::Write` for the inner value.
///
/// This struct is generally created by calling `writer()` on `BufMut`. See
/// documentation of [`writer()`](trait.BufMut.html#method.writer) for more
/// details.
pub struct Writer<B> {
    buf: B,
}

impl<B: BufMut> Writer<B> {
    /// Gets a reference to the underlying `BufMut`.
    ///
    /// It is inadvisable to directly write to the underlying `BufMut`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::BufMut;
    ///
    /// let mut buf = Vec::with_capacity(1024).writer();
    ///
    /// assert_eq!(1024, buf.get_ref().capacity());
    /// ```
    pub fn get_ref(&self) -> &B {
        &self.buf
    }

    /// Gets a mutable reference to the underlying `BufMut`.
    ///
    /// It is inadvisable to directly write to the underlying `BufMut`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![].writer();
    ///
    /// buf.get_mut().reserve(1024);
    ///
    /// assert_eq!(1024, buf.get_ref().capacity());
    /// ```
    pub fn get_mut(&mut self) -> &mut B {
        &mut self.buf
    }

    /// Consumes this `Writer`, returning the underlying value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::BufMut;
    /// use std::io::{self, Cursor};
    ///
    /// let mut buf = vec![].writer();
    /// let mut src = Cursor::new(b"hello world");
    ///
    /// io::copy(&mut src, &mut buf).unwrap();
    ///
    /// let buf = buf.into_inner();
    /// assert_eq!(*buf, b"hello world"[..]);
    /// ```
    pub fn into_inner(self) -> B {
        self.buf
    }
}

impl<B: BufMut + Sized> io::Write for Writer<B> {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        let n = cmp::min(self.buf.remaining_mut(), src.len());

        self.buf.put(&src[0..n]);
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
