//! Types related to [`Utf8Bytes`].
//!
//! See [`Utf8Bytes`] for more info.

use core::borrow::Borrow;
use core::convert::TryFrom;
use core::ops::{Deref, RangeBounds};
use core::str::Utf8Error;
use core::{cmp, fmt, hash};

use alloc::{boxed::Box, string::String, vec::Vec};

use crate::{Buf, Bytes};

/// A wrapper over [`Bytes`] that guarantees UTF-8 validity, meaning it can be
/// treated as a string.
///
/// See the documentation for [`Bytes`] for more information about its
/// implementation.
///
/// `Utf8Bytes` supports most of the same operations as `Bytes`, such as
/// slicing, splitting, and advancing the internal cursor, except that all
/// such operations require that provided indices be on char boundaries or else
/// will panic, the same as [`str`] slices. This is not generally a concern so
/// long as indices are obtained from UTF-8-aware string operations, such as
/// [`str::char_indices()`]. See also [`str::is_char_boundary()`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Utf8Bytes {
    inner: Bytes,
}

impl Utf8Bytes {
    /// Creates a new empty `Utf8Bytes`.
    ///
    /// This will not allocate and the returned `Utf8Bytes` handle will be empty.
    #[inline]
    #[cfg(not(all(loom, test)))]
    pub const fn new() -> Self {
        // SAFETY: an empty string is always UTF-8
        unsafe { Utf8Bytes::from_utf8_unchecked(Bytes::new()) }
    }

    /// Creates a new empty `Utf8Bytes`.
    #[cfg(all(loom, test))]
    pub fn new() -> Self {
        // SAFETY: an empty string is always UTF-8
        unsafe { Utf8Bytes::from_utf8_unchecked(Bytes::new()) }
    }

    /// Creates a new `Utf8Bytes` from a static string.
    ///
    /// The returned `Utf8Bytes` will point directly to the static string. There is
    /// no allocating or copying.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Utf8Bytes;
    ///
    /// let b = Utf8Bytes::from_static("hello");
    /// assert_eq!(b, "hello");
    /// ```
    #[inline]
    #[cfg(not(all(loom, test)))]
    pub const fn from_static(string: &'static str) -> Self {
        let bytes = Bytes::from_static(string.as_bytes());
        // SAFETY: bytes was created from a UTF-8 string
        unsafe { Self::from_utf8_unchecked(bytes) }
    }

    /// Creates a new `Utf8Bytes` from a static string.
    #[cfg(all(loom, test))]
    pub fn from_static(string: &'static str) -> Self {
        let bytes = Bytes::from_static(string.as_bytes());
        // SAFETY: bytes was created from a UTF-8 string
        unsafe { Self::from_utf8_unchecked(bytes) }
    }

    /// Convert `Bytes` to `Utf8Bytes`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the argument is not valid UTF-8.
    #[inline]
    pub fn from_utf8(bytes: Bytes) -> Result<Self, FromUtf8Error> {
        match core::str::from_utf8(&bytes) {
            // SAFETY: verified that b is valid UTF-8
            Ok(_) => Ok(unsafe { Utf8Bytes::from_utf8_unchecked(bytes) }),
            Err(error) => Err(FromUtf8Error { bytes, error }),
        }
    }

    /// Convert [`Bytes`] to [`Utf8Bytes`] without checking that it contains
    /// valid UTF-8.
    ///
    /// # Safety
    ///
    /// The argument must contain valid UTF-8.
    #[inline]
    pub const unsafe fn from_utf8_unchecked(bytes: Bytes) -> Self {
        Utf8Bytes { inner: bytes }
    }

    /// Unwrap `Utf8Bytes` into its underlying `Bytes`.
    pub fn into_bytes(self) -> Bytes {
        self.inner
    }

    /// Create [Utf8Bytes] with a buffer whose lifetime is controlled
    /// via an explicit owner.
    ///
    /// See [`Bytes::from_owner`] for more details.
    pub fn from_owner<T>(owner: T) -> Self
    where
        T: AsRef<str> + Send + 'static,
    {
        struct OwnerWrapper<T>(T);
        impl<T: AsRef<str>> AsRef<[u8]> for OwnerWrapper<T> {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref().as_bytes()
            }
        }

        let bytes = Bytes::from_owner(OwnerWrapper(owner));
        // SAFETY: bytes was created from an owner whose AsRef<[u8]> impl always returns valid UTF-8
        unsafe { Utf8Bytes::from_utf8_unchecked(bytes) }
    }

    /// Get a `&str` reference from this `Utf8Bytes`.
    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: guaranteed sound by type-level invariant
        unsafe { core::str::from_utf8_unchecked(&self.inner) }
    }

    /// Returns the number of bytes contained in this `Bytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Utf8Bytes;
    ///
    /// let b = Utf8Bytes::from("hello");
    /// assert_eq!(b.len(), 5);
    /// ```
    #[inline]
    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the `Utf8Bytes` has a length of 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Utf8Bytes;
    ///
    /// let b = Utf8Bytes::new();
    /// assert!(b.is_empty());
    /// ```
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Creates `Utf8Bytes` instance from slice, by copying it.
    pub fn copy_from_slice(data: &str) -> Self {
        let bytes = Bytes::copy_from_slice(data.as_bytes());
        // SAFETY: bytes was created from a UTF-8 string
        unsafe { Utf8Bytes::from_utf8_unchecked(bytes) }
    }

    /// Returns a slice of self for the provided range.
    ///
    /// This will increment the reference count for the underlying memory and
    /// return a new `Utf8Bytes` handle set to the slice.
    ///
    /// This operation is `O(1)`.
    ///
    /// # Panics
    ///
    /// Requires that `begin` and `end` are both char boundaries, otherwise
    /// slicing will panic.
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        use core::ops::Bound;

        let len = self.len();

        let begin = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.checked_add(1).expect("out of range"),
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n.checked_add(1).expect("out of range"),
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };

        assert!(
            begin <= end,
            "range start must not be greater than end: {:?} <= {:?}",
            begin,
            end,
        );
        assert!(
            end <= len,
            "range end out of bounds: {:?} <= {:?}",
            end,
            len,
        );

        assert!(
            self.is_char_boundary(begin),
            "range start is not a char boundary: {:?}",
            begin,
        );
        assert!(
            self.is_char_boundary(end),
            "range end is not a char boundary: {:?}",
            end,
        );

        let bytes = self.inner.slice(begin..end);
        // SAFETY: bytes is a slice into a UTF-8 string where whose start and end
        //         have been verified to be char boundaries.
        unsafe { Utf8Bytes::from_utf8_unchecked(bytes) }
    }

    /// Returns a slice of self that is equivalent to the given `subset`.
    ///
    /// This operation is `O(1)`.
    ///
    /// See [`Bytes::slice_ref()`] for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Utf8Bytes;
    ///
    /// let bytes = Utf8Bytes::from("012345678");
    /// let as_slice = bytes.as_ref();
    /// let subset = &as_slice[2..6];
    /// let subslice = bytes.slice_ref(&subset);
    /// assert_eq!(subslice, "2345");
    /// ```
    ///
    /// # Panics
    ///
    /// Requires that the given `sub` slice is in fact contained within the
    /// `Utf8Bytes` buffer; otherwise this function will panic.
    pub fn slice_ref(&self, subset: &str) -> Self {
        let bytes = self.inner.slice_ref(subset.as_bytes());
        // SAFETY: bytes has the same data as `subset`, which is valid UTF-8
        unsafe { Utf8Bytes::from_utf8_unchecked(bytes) }
    }

    /// Splits the string into two at the given index.
    ///
    /// Afterwards `self` contains elements `[0, at)`, and the returned `Utf8Bytes`
    /// contains elements `[at, len)`. It's guaranteed that the memory does not
    /// move, that is, the address of `self` does not change, and the address of
    /// the returned slice is `at` bytes after that.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Utf8Bytes;
    ///
    /// let mut a = Utf8Bytes::from("hello world");
    /// let b = a.split_off(5);
    ///
    /// assert_eq!(a, "hello");
    /// assert_eq!(b, " world");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at` is not on a char boundary.
    #[must_use = "consider Utf8Bytes::truncate if you don't need the other half"]
    pub fn split_off(&mut self, at: usize) -> Self {
        assert!(
            at <= self.len(),
            "split_off out of bounds: {:?} <= {:?}",
            at,
            self.len(),
        );
        assert!(
            self.is_char_boundary(at),
            "split_off not on char boundary: {:?}",
            at,
        );

        // SAFETY: `at` is verified to be on a char boundary
        unsafe {
            let bytes = self.as_mut_bytes().split_off(at);
            Utf8Bytes::from_utf8_unchecked(bytes)
        }
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned
    /// `Utf8Bytes` contains elements `[0, at)`.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Utf8Bytes;
    ///
    /// let mut a = Utf8Bytes::from("hello world");
    /// let b = a.split_to(5);
    ///
    /// assert_eq!(a, " world");
    /// assert_eq!(b, "hello");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at` is not on a char boundary.
    #[must_use = "consider Utf8Bytes::advance if you don't need the other half"]
    pub fn split_to(&mut self, at: usize) -> Self {
        assert!(
            at <= self.len(),
            "split_to out of bounds: {:?} <= {:?}",
            at,
            self.len(),
        );
        assert!(
            self.is_char_boundary(at),
            "split_to not on char boundary: {:?}",
            at,
        );

        // SAFETY: `at` is verified to be on a char boundary
        unsafe {
            let bytes = self.as_mut_bytes().split_to(at);
            Utf8Bytes::from_utf8_unchecked(bytes)
        }
    }

    /// Shortens the buffer, keeping the first `len` bytes and dropping the
    /// rest.
    ///
    /// If `len` is greater than the buffer's current length, this has no
    /// effect.
    ///
    /// The [split_off](`Self::split_off()`) method can emulate `truncate`, but this causes the
    /// excess bytes to be returned instead of dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Utf8Bytes;
    ///
    /// let mut buf = Utf8Bytes::from("hello world");
    /// buf.truncate(5);
    /// assert_eq!(buf, "hello");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `len` is not on a char boundary.
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        assert!(
            len >= self.len() || self.is_char_boundary(len),
            "truncate not on char boundary: {:?}",
            len
        );
        unsafe { self.as_mut_bytes().truncate(len) }
    }

    /// Advance the internal cursor of the `Utf8Bytes`.
    ///
    /// Like [`Buf::advance()`], but ensures that the cursor can only be
    /// advanced to char boundaries.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Utf8Bytes;
    ///
    /// let mut buf = Utf8Bytes::from("hello world");
    ///
    /// assert_eq!(buf, "hello world");
    ///
    /// buf.advance(6);
    ///
    /// assert_eq!(buf, "world");
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if `cnt > self.remaining()` or if `cnt` is not on
    /// a char boundary.
    pub fn advance(&mut self, cnt: usize) {
        assert!(
            cnt <= self.len(),
            "cannot advance past `remaining`: {:?} <= {:?}",
            cnt,
            self.len(),
        );
        assert!(
            self.is_char_boundary(cnt),
            "advance not on char boundary: {:?}",
            cnt,
        );
        // SAFETY: verified that `cnt` is on a char boundary.
        unsafe { self.as_mut_bytes().advance(cnt) }
    }

    // private

    /// Returns `&mut self.inner`.
    ///
    /// # Safety
    /// Mutating the inner `Bytes` is unsafe; for example, it could be advanced
    /// to the middle of a codepoint, leaving an invalid string.
    unsafe fn as_mut_bytes(&mut self) -> &mut Bytes {
        &mut self.inner
    }
}

impl Deref for Utf8Bytes {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for Utf8Bytes {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Debug for Utf8Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl hash::Hash for Utf8Bytes {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        self.as_str().hash(state);
    }
}

impl Borrow<str> for Utf8Bytes {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

// impl Eq

impl PartialEq<str> for Utf8Bytes {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialOrd<str> for Utf8Bytes {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other)
    }
}

impl PartialEq<Utf8Bytes> for str {
    fn eq(&self, other: &Utf8Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Utf8Bytes> for str {
    fn partial_cmp(&self, other: &Utf8Bytes) -> Option<cmp::Ordering> {
        <str as PartialOrd<str>>::partial_cmp(self, other)
    }
}

impl PartialEq<[u8]> for Utf8Bytes {
    fn eq(&self, other: &[u8]) -> bool {
        self.as_bytes() == other
    }
}

impl PartialOrd<[u8]> for Utf8Bytes {
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(other)
    }
}

impl PartialEq<Utf8Bytes> for [u8] {
    fn eq(&self, other: &Utf8Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Utf8Bytes> for [u8] {
    fn partial_cmp(&self, other: &Utf8Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other.as_bytes())
    }
}

impl PartialEq<String> for Utf8Bytes {
    fn eq(&self, other: &String) -> bool {
        *self == other[..]
    }
}

impl PartialOrd<String> for Utf8Bytes {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl PartialEq<Utf8Bytes> for String {
    fn eq(&self, other: &Utf8Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Utf8Bytes> for String {
    fn partial_cmp(&self, other: &Utf8Bytes) -> Option<cmp::Ordering> {
        <str as PartialOrd<str>>::partial_cmp(self.as_str(), other)
    }
}

impl PartialEq<Vec<u8>> for Utf8Bytes {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == other[..]
    }
}

impl PartialOrd<Vec<u8>> for Utf8Bytes {
    fn partial_cmp(&self, other: &Vec<u8>) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(&other[..])
    }
}

impl PartialEq<Utf8Bytes> for Vec<u8> {
    fn eq(&self, other: &Utf8Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Utf8Bytes> for Vec<u8> {
    fn partial_cmp(&self, other: &Utf8Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other.as_bytes())
    }
}

impl PartialEq<Utf8Bytes> for &str {
    fn eq(&self, other: &Utf8Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Utf8Bytes> for &str {
    fn partial_cmp(&self, other: &Utf8Bytes) -> Option<cmp::Ordering> {
        <str as PartialOrd<str>>::partial_cmp(self, other)
    }
}

impl PartialEq<Utf8Bytes> for &[u8] {
    fn eq(&self, other: &Utf8Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Utf8Bytes> for &[u8] {
    fn partial_cmp(&self, other: &Utf8Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other.as_bytes())
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for Utf8Bytes
where
    Utf8Bytes: PartialEq<T>,
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a, T: ?Sized> PartialOrd<&'a T> for Utf8Bytes
where
    Utf8Bytes: PartialOrd<T>,
{
    fn partial_cmp(&self, other: &&'a T) -> Option<cmp::Ordering> {
        self.partial_cmp(&**other)
    }
}

// impl From

impl Default for Utf8Bytes {
    #[inline]
    fn default() -> Utf8Bytes {
        Utf8Bytes::new()
    }
}

impl From<&'static str> for Utf8Bytes {
    fn from(slice: &'static str) -> Utf8Bytes {
        Utf8Bytes::from_static(slice)
    }
}

impl From<Box<str>> for Utf8Bytes {
    fn from(slice: Box<str>) -> Utf8Bytes {
        let bytes = Bytes::from(slice.into_boxed_bytes());
        // SAFETY: bytes was created from a UTF-8 string
        unsafe { Utf8Bytes::from_utf8_unchecked(bytes) }
    }
}

impl From<String> for Utf8Bytes {
    fn from(s: String) -> Utf8Bytes {
        let bytes = Bytes::from(s.into_bytes());
        // SAFETY: bytes was created from a UTF-8 string
        unsafe { Utf8Bytes::from_utf8_unchecked(bytes) }
    }
}

impl From<Utf8Bytes> for String {
    fn from(string: Utf8Bytes) -> String {
        let bytes: Vec<u8> = string.into_bytes().into();
        // SAFETY: bytes was created from a UTF-8 string
        unsafe { String::from_utf8_unchecked(bytes) }
    }
}

impl From<Utf8Bytes> for Bytes {
    fn from(string: Utf8Bytes) -> Self {
        string.into_bytes()
    }
}

impl TryFrom<Bytes> for Utf8Bytes {
    type Error = FromUtf8Error;

    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        Utf8Bytes::from_utf8(bytes)
    }
}

impl TryFrom<Vec<u8>> for Utf8Bytes {
    type Error = FromUtf8Error;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Utf8Bytes::from_utf8(bytes.into())
    }
}

// error types

/// A possible error value when converting a `Bytes` to a `Utf8Bytes`.
///
/// See [`Utf8Bytes::from_utf8()`] for more info.
//
// based on `std::string::FromUtf8Error`
#[derive(Debug, PartialEq, Eq)]
pub struct FromUtf8Error {
    bytes: Bytes,
    error: Utf8Error,
}

impl FromUtf8Error {
    /// Get the byte slice that was attempted to convert to a `Utf8Bytes`.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the `Bytes` that was attempted to convert to a `Utf8Bytes`.
    #[must_use = "`self` will be dropped if the result is not used"]
    pub fn into_bytes(self) -> Bytes {
        self.bytes
    }

    /// Fetch a `Utf8Error` to get more details about the conversion failure.
    #[must_use]
    pub fn utf8_error(&self) -> Utf8Error {
        self.error
    }
}

impl fmt::Display for FromUtf8Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FromUtf8Error {}
