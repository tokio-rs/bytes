use super::{Buf};

use std::io;

/// Conversion into a `Buf`
///
/// An `IntoBuf` implementation defines how to convert a value into a `Buf`.
/// This is common for types that represent byte storage of some kind. `IntoBuf`
/// may be implemented directly for types or on references for those types.
///
/// # Examples
///
/// ```
/// use bytes::{Buf, IntoBuf};
///
/// let bytes = b"\x00\x01hello world";
/// let mut buf = bytes.into_buf();
///
/// assert_eq!(1, buf.get_u16());
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
    /// use bytes::{Buf, IntoBuf};
    ///
    /// let bytes = b"\x00\x01hello world";
    /// let mut buf = bytes.into_buf();
    ///
    /// assert_eq!(1, buf.get_u16());
    ///
    /// let mut rest = [0; 11];
    /// buf.copy_to_slice(&mut rest);
    ///
    /// assert_eq!(b"hello world", &rest);
    /// ```
    fn into_buf(self) -> Self::Buf;

    /// Returns the number of bytes that would contained in the buffer created by calling
    /// `into_buf()` with `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Buf, IntoBuf};
    ///
    /// let bytes = b"\x00\x01hello world";
    /// let r: &'static [u8] = bytes;
    ///
    /// assert_eq!(13, IntoBuf::len(&r));
    ///
    /// let mut buf = bytes.into_buf();
    ///
    /// assert_eq!(13, bytes.len());
    ///
    /// ```
    fn len(&self) -> usize;
}

impl<T: Buf> IntoBuf for T {
    type Buf = Self;

    fn into_buf(self) -> Self {
        self
    }

    fn len(&self) -> usize {
        <Self as Buf>::remaining(self)
    }
}

impl<'a> IntoBuf for &'a [u8] {
    type Buf = io::Cursor<&'a [u8]>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(self)
    }

    fn len(&self) -> usize {
        <[u8]>::len(self)
    }
}

impl<'a> IntoBuf for &'a str {
    type Buf = io::Cursor<&'a [u8]>;

    fn into_buf(self) -> Self::Buf {
        self.as_bytes().into_buf()
    }

    fn len(&self) -> usize {
        str::len(self)
    }
}

impl IntoBuf for Vec<u8> {
    type Buf = io::Cursor<Vec<u8>>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(self)
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }
}

impl<'a> IntoBuf for &'a Vec<u8> {
    type Buf = io::Cursor<&'a [u8]>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(&self[..])
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }
}

// Kind of annoying... but this impl is required to allow passing `&'static
// [u8]` where for<'a> &'a T: IntoBuf is required.
impl<'a> IntoBuf for &'a &'static [u8] {
    type Buf = io::Cursor<&'static [u8]>;

    fn into_buf(self) -> Self::Buf {
        io::Cursor::new(self)
    }

    fn len(&self) -> usize {
        <&[u8]>::len(self)
    }
}

impl<'a> IntoBuf for &'a &'static str {
    type Buf = io::Cursor<&'static [u8]>;

    fn into_buf(self) -> Self::Buf {
        self.as_bytes().into_buf()
    }

    fn len(&self) -> usize {
        str::len(self)
    }
}

impl IntoBuf for String {
    type Buf = io::Cursor<Vec<u8>>;

    fn into_buf(self) -> Self::Buf {
        self.into_bytes().into_buf()
    }

    fn len(&self) -> usize {
        String::len(self)
    }
}

impl<'a> IntoBuf for &'a String {
    type Buf = io::Cursor<&'a [u8]>;

    fn into_buf(self) -> Self::Buf {
        self.as_bytes().into_buf()
    }

    fn len(&self) -> usize {
        String::len(self)
    }
}

impl IntoBuf for u8 {
    type Buf = Option<[u8; 1]>;

    fn into_buf(self) -> Self::Buf {
        Some([self])
    }

    fn len(&self) -> usize {
        1
    }
}

impl IntoBuf for i8 {
    type Buf = Option<[u8; 1]>;

    fn into_buf(self) -> Self::Buf {
        Some([self as u8; 1])
    }

    fn len(&self) -> usize {
        1
    }
}
