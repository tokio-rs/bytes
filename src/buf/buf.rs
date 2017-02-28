use super::{Take, Reader};
use byteorder::ByteOrder;

use std::{cmp, ptr};

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
        super::take::new(self, limit)
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
        super::reader::new(self)
    }
}
