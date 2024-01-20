use crate::Buf;

use crate::buf::cursor::BufCursor;
use alloc::boxed::Box;

/// Read bytes from an arbitrary position within a buffer.
///
/// This is an extension over the standard [Buf] type that allows seeking to
/// arbitrary positions within the buffer for reads. The buffer may return
/// chunks of bytes at each position of undetermined length, to allow for
/// buffer implementations that are formed using non-contiguous memory.
///
/// It is always true that a [SeekBuf] is also a [Buf], but not always the
/// inverse.
///
/// Many types that implement [Buf] may also implement [SeekBuf], including:
///     - `&[u8]`
///     - [alloc::collections::VecDeque]
///
/// # Examples
///
/// ```
/// use bytes::{Buf, SeekBufExt};
///
/// let buf = b"try to find the T in the haystack".as_slice();
///
/// let remaining = buf.remaining();
///
/// assert!(buf.cursor().find(|&&b| b == b'Q').is_none());
/// assert!(buf.cursor().find(|&&b| b == b'T').is_some());
///
/// // No bytes in the buffer were consumed while using the cursor.
/// assert_eq!(remaining, buf.remaining());
/// ```
pub trait SeekBuf: Buf {
    /// Returns a chunk of unspecified length (but not exceeding
    /// [Self::remaining]) starting at the specified inclusive index position.
    ///
    /// This method can be alternately thought of as equivalent to the
    /// `[start..]` range indexing operation.
    ///
    /// # Implementer notes
    ///
    /// Implementations of [Self::chunk_from] should return [None] if the
    /// `start` index is out of range for optimal performance. Implementations
    /// that return values of empty slices are functionally equivalent, but may
    /// hit slower code paths during use.
    ///
    /// Note that the `end` argument is an **inclusive** bound.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::SeekBuf;
    ///
    /// let buf = b"hello world".as_slice();
    ///
    /// assert_eq!(buf.chunk_from(6), Some(b"world".as_slice()));
    /// assert_eq!(buf.chunk_from(100), None);
    /// ```
    fn chunk_from(&self, start: usize) -> Option<&[u8]>;

    /// Returns a chunk of unspecified length (but not exceeding
    /// [Self::remaining]) ending at the specified exclusive index position.
    ///
    /// This method can be alternately thought of as equivalent to the
    /// `[..end]` range indexing operation.
    ///
    /// # Implementer notes
    ///
    /// Implementations of [Self::chunk_to] should return [None] if the `end`
    /// index is out of range for optimal performance. Implementations that
    /// return values of empty slices are functionally equivalent, but may hit
    /// slower code paths during use.
    ///
    /// An identity of this function is that any call with an `end` argument of
    /// zero will unconditionally return `Some(&[])`, rather than `None`, since
    /// a sub-slice of zero length is a valid chunk of any buffer of any length.
    ///
    /// Note that the `end` argument is an **exclusive** bound.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::SeekBuf;
    ///
    /// let buf = b"hello world".as_slice();
    ///
    /// assert_eq!(buf.chunk_to(5), Some(b"hello".as_slice()));
    /// assert_eq!(buf.chunk_to(100), None);
    ///
    /// // It may not be intuitive, but an identity of `chunk_to` is that when
    /// // passed an `end` of zero, it will always return an empty slice instead
    /// // of `None`.
    /// assert_eq!([].as_slice().chunk_to(0), Some([].as_slice()));
    /// ```
    fn chunk_to(&self, end: usize) -> Option<&[u8]>;
}

/// SeekBufExt provides additional functionality for any type that implements
/// the [SeekBuf] trait.
///
/// Methods within this trait are not implemented directly on the [SeekBuf]
/// trait in order to ensure that [SeekBuf] remains object-safe.
pub trait SeekBufExt: SeekBuf {
    /// Returns a new [BufCursor] that can iterate over the current buffer.
    /// [Self] is borrowed immutably while the cursor is active.
    ///
    /// ```
    /// use bytes::SeekBufExt;
    ///
    /// let buf = b"hello world".as_slice();
    ///
    /// let mut cursor = buf.cursor();
    ///
    /// assert_eq!(cursor.next(), Some(&b'h'));
    /// assert_eq!(cursor.next(), Some(&b'e'));
    /// assert_eq!(cursor.next(), Some(&b'l'));
    /// assert_eq!(cursor.next(), Some(&b'l'));
    /// assert_eq!(cursor.next(), Some(&b'o'));
    ///
    /// let mut sub_cursor = cursor.cursor();
    ///
    /// assert_eq!(sub_cursor.next(), Some(&b' '));
    /// assert_eq!(cursor.next(), Some(&b' '));
    /// ```
    fn cursor(&self) -> BufCursor<'_, Self> {
        BufCursor::new(self)
    }
}

impl<T: SeekBuf + ?Sized> SeekBufExt for T {}

macro_rules! deref_forward_seek_buf {
    () => {
        #[inline]
        fn chunk_from(&self, start: usize) -> Option<&[u8]> {
            (**self).chunk_from(start)
        }

        #[inline]
        fn chunk_to(&self, end: usize) -> Option<&[u8]> {
            (**self).chunk_to(end)
        }
    };
}

impl<T: SeekBuf + ?Sized> SeekBuf for &mut T {
    deref_forward_seek_buf!();
}

impl<T: SeekBuf + ?Sized> SeekBuf for Box<T> {
    deref_forward_seek_buf!();
}

impl SeekBuf for &[u8] {
    #[inline]
    fn chunk_from(&self, start: usize) -> Option<&[u8]> {
        self.get(start..)
    }

    #[inline]
    fn chunk_to(&self, end: usize) -> Option<&[u8]> {
        self.get(..end)
    }
}

// The existence of this function makes the compiler catch if the SeekBuf
// trait is "object-safe" or not.
fn _assert_trait_object(_b: &dyn SeekBuf) {}
