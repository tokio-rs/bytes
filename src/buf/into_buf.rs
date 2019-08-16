use super::{Buf};
use crate::BytesMut;

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

    /// Returns number of bytes that `Buf` is going to hold.
    fn len(&self) -> usize;

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
}

impl<T: Buf> IntoBuf for T {
    type Buf = Self;

    fn len(&self) -> usize {
        <Self as Buf>::remaining(self)
    }

    fn into_buf(self) -> Self {
        self
    }
}

impl<'a> IntoBuf for &'a str {
    type Buf = &'a [u8];

    fn len(&self) -> usize {
        str::len(self)
    }

    fn into_buf(self) -> Self::Buf {
        self.as_bytes()
    }
}

impl IntoBuf for Vec<u8> {
    type Buf = BytesMut;

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn into_buf(self) -> Self::Buf {
        self.into()
    }
}

impl<'a> IntoBuf for &'a Vec<u8> {
    type Buf = &'a [u8];

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn into_buf(self) -> Self::Buf {
        self.as_slice()
    }
}

// Kind of annoying... but this impl is required to allow passing `&'static
// [u8]` where for<'a> &'a T: IntoBuf is required.
impl<'a> IntoBuf for &'a &'static [u8] {
    type Buf = &'static [u8];

    fn len(&self) -> usize {
        <&[u8]>::len(self)
    }

    fn into_buf(self) -> Self::Buf {
        *self
    }
}

impl<'a> IntoBuf for &'a &'static str {
    type Buf = &'static [u8];

    fn len(&self) -> usize {
        str::len(self)
    }

    fn into_buf(self) -> Self::Buf {
        self.as_bytes().into_buf()
    }
}

impl IntoBuf for String {
    type Buf = BytesMut;

    fn len(&self) -> usize {
        String::len(self)
    }

    fn into_buf(self) -> Self::Buf {
        self.into_bytes().into_buf()
    }
}

impl<'a> IntoBuf for &'a String {
    type Buf = &'a [u8];

    fn len(&self) -> usize {
        String::len(self)
    }

    fn into_buf(self) -> Self::Buf {
        self.as_bytes().into_buf()
    }
}

impl IntoBuf for u8 {
    type Buf = Option<[u8; 1]>;

    fn len(&self) -> usize {
        core::mem::size_of::<u8>()
    }

    fn into_buf(self) -> Self::Buf {
        Some([self])
    }
}

impl IntoBuf for i8 {
    type Buf = Option<[u8; 1]>;

    fn len(&self) -> usize {
        core::mem::size_of::<i8>()
    }

    fn into_buf(self) -> Self::Buf {
        Some([self as u8; 1])
    }
}
