use {IntoBuf, ByteBuf, SliceBuf};

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::{cmp, fmt, ops};

/// A reference counted slice of bytes.
///
/// A `Bytes` is an immutable sequence of bytes. Given that it is guaranteed to
/// be immutable, `Bytes` is `Sync`, `Clone` is shallow (ref count increment),
/// and all operations only update views into the underlying data without
/// requiring any copies.
#[derive(Eq)]
pub struct Bytes {
    inner: BytesMut,
}

/// A unique reference to a slice of bytes.
///
/// A `BytesMut` is a unique handle to a slice of bytes allowing mutation of
/// the underlying bytes.
pub struct BytesMut {
    mem: Mem,
    pos: usize,
    len: usize,
    cap: usize,
}

struct Mem {
    inner: Arc<UnsafeCell<Box<[u8]>>>,
}

/*
 *
 * ===== Bytes =====
 *
 */

impl Bytes {
    /// Creates a new `Bytes` and copy the given slice into it.
    pub fn from_slice<T: AsRef<[u8]>>(bytes: T) -> Bytes {
        BytesMut::from_slice(bytes).freeze()
    }

    /// Returns the number of bytes contained in this `Bytes`.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the value contains no bytes
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the inner contents of this `Bytes` as a slice.
    pub fn as_slice(&self) -> &[u8] {
        self.as_ref()
    }

    /// Extracts a new `Bytes` referencing the bytes from range [start, end).
    pub fn slice(&self, start: usize, end: usize) -> Bytes {
        let mut ret = self.clone();

        ret.inner
            .set_end(end)
            .set_start(start);

        ret
    }

    /// Extracts a new `Bytes` referencing the bytes from range [start, len).
    pub fn slice_from(&self, start: usize) -> Bytes {
        self.slice(start, self.len())
    }

    /// Extracts a new `Bytes` referencing the bytes from range [0, end).
    pub fn slice_to(&self, end: usize) -> Bytes {
        self.slice(0, end)
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[0, at)`, and the returned `Bytes`
    /// contains elements `[at, len)`.
    ///
    /// This is an O(1) operation that just increases the reference count and
    /// sets a few indexes.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    pub fn split_off(&mut self, at: usize) -> Bytes {
        self.inner.split_off(at).freeze()
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned
    /// `Bytes` contains elements `[0, at)`.
    ///
    /// This is an O(1) operation that just increases the reference count and
    /// sets a few indexes.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    pub fn drain_to(&mut self, at: usize) -> Bytes {
        self.inner.drain_to(at).freeze()
    }

    /// Attempt to convert into a `BytesMut` handle.
    ///
    /// This will only succeed if there are no other outstanding references to
    /// the underlying chunk of memory.
    pub fn try_mut(mut self) -> Result<BytesMut, Bytes> {
        if self.inner.mem.is_mut_safe() {
            Ok(self.inner)
        } else {
            Err(self)
        }
    }

    /// Consumes handle, returning a new mutable handle
    ///
    /// The function attempts to avoid copying, however if it is unable to
    /// obtain a unique reference to the underlying data, a new buffer is
    /// allocated and the data is copied to it.
    pub fn into_mut(self) -> BytesMut {
        self.try_mut().unwrap_or_else(BytesMut::from_slice)
    }
}

impl IntoBuf for Bytes {
    type Buf = SliceBuf<Self>;

    fn into_buf(self) -> Self::Buf {
        SliceBuf::new(self)
    }
}

impl<'a> IntoBuf for &'a Bytes {
    type Buf = SliceBuf<Self>;

    fn into_buf(self) -> Self::Buf {
        SliceBuf::new(self)
    }
}

impl Clone for Bytes {
    fn clone(&self) -> Bytes {
        Bytes { inner: self.inner.clone() }
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl ops::Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(src: Vec<u8>) -> Bytes {
        BytesMut::from(src).freeze()
    }
}

impl<'a> From<&'a [u8]> for Bytes {
    fn from(src: &'a [u8]) -> Bytes {
        BytesMut::from(src).freeze()
    }
}

impl PartialEq for Bytes {
    fn eq(&self, other: &Bytes) -> bool {
        self.inner == other.inner
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, fmt)
    }
}

unsafe impl Sync for Bytes {}

/*
 *
 * ===== BytesMut =====
 *
 */

impl BytesMut {
    /// Create a new `BytesMut` with the specified capacity.
    pub fn with_capacity(cap: usize) -> BytesMut {
        BytesMut {
            mem: Mem::with_capacity(cap),
            pos: 0,
            len: 0,
            cap: cap,
        }
    }

    /// Creates a new `BytesMut` and copy the given slice into it.
    pub fn from_slice<T: AsRef<[u8]>>(bytes: T) -> BytesMut {
        let buf = ByteBuf::from_slice(bytes);
        buf.into_inner()
    }

    /// Returns the number of bytes contained in this `BytesMut`.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the value contains no bytes
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the total byte capacity of this `BytesMut`
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Return an immutable handle to the bytes
    pub fn freeze(self) -> Bytes {
        Bytes { inner: self }
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[0, at)`, and the returned
    /// `BytesMut` contains elements `[at, capacity)`.
    ///
    /// This is an O(1) operation that just increases the reference count and
    /// sets a few indexes.
    ///
    /// # Panics
    ///
    /// Panics if `at > capacity`
    pub fn split_off(&mut self, at: usize) -> BytesMut {
        let mut other = self.clone();

        other.set_start(at);
        self.set_end(at);

        return other
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned `BytesMut`
    /// contains elements `[0, at)`.
    ///
    /// This is an O(1) operation that just increases the reference count and
    /// sets a few indexes.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    pub fn drain_to(&mut self, at: usize) -> BytesMut {
        let mut other = self.clone();

        other.set_end(at);
        self.set_start(at);

        return other
    }

    /// Returns the inner contents of this `BytesMut` as a slice.
    pub fn as_slice(&self) -> &[u8] {
        self.as_ref()
    }

    /// Returns the inner contents of this `BytesMut` as a mutable slice
    ///
    /// This a slice of bytes that have been initialized
    pub fn as_mut(&mut self) -> &mut [u8] {
        let end = self.pos + self.len;
        &mut self.mem.as_mut()[self.pos..end]
    }

    /// Sets the length of the buffer
    ///
    /// This will explicitly set the size of the buffer without actually
    /// modifying the data, so it is up to the caller to ensure that the data
    /// has been initialized.
    ///
    /// # Panics
    ///
    /// This method will panic if `len` is out of bounds for the underlying
    /// slice or if it comes after the `end` of the configured window.
    pub unsafe fn set_len(&mut self, len: usize) {
        assert!(len <= self.cap);
        self.len = len;
    }

    /// Returns the inner contents of this `BytesMut` as a mutable slice
    ///
    /// This a slice of all bytes, including uninitialized memory
    pub unsafe fn as_raw(&mut self) -> &mut [u8] {
        let end = self.pos + self.cap;
        &mut self.mem.as_mut()[self.pos..end]
    }

    /// Changes the starting index of this window to the index specified.
    ///
    /// Returns the windows back to chain multiple calls to this method.
    ///
    /// # Panics
    ///
    /// This method will panic if `start` is out of bounds for the underlying
    /// slice.
    fn set_start(&mut self, start: usize) -> &mut BytesMut {
        assert!(start <= self.cap);
        self.pos += start;

        if self.len >= start {
            self.len -= start;
        } else {
            self.len = 0;
        }

        self.cap -= start;
        self
    }

    /// Changes the end index of this window to the index specified.
    ///
    /// Returns the windows back to chain multiple calls to this method.
    ///
    /// # Panics
    ///
    /// This method will panic if `start` is out of bounds for the underlying
    /// slice.
    fn set_end(&mut self, end: usize) -> &mut BytesMut {
        assert!(end <= self.cap);
        self.cap = end;
        self.len = cmp::min(self.len, end);
        self
    }

    /// Increments the ref count. This should only be done if it is known that
    /// it can be done safely. As such, this fn is not public, instead other
    /// fns will use this one while maintaining the guarantees.
    fn clone(&self) -> BytesMut {
         BytesMut {
            mem: self.mem.clone(),
            .. *self
        }
    }
}

impl IntoBuf for BytesMut {
    type Buf = SliceBuf<Self>;

    fn into_buf(self) -> Self::Buf {
        SliceBuf::new(self)
    }
}

impl<'a> IntoBuf for &'a BytesMut {
    type Buf = SliceBuf<&'a BytesMut>;

    fn into_buf(self) -> Self::Buf {
        SliceBuf::new(self)
    }
}

impl AsRef<[u8]> for BytesMut {
    fn as_ref(&self) -> &[u8] {
        let end = self.pos + self.len;
        &self.mem.as_ref()[self.pos..end]
    }
}

impl ops::Deref for BytesMut {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}

impl ops::DerefMut for BytesMut {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.as_mut()
    }
}

impl From<Vec<u8>> for BytesMut {
    fn from(src: Vec<u8>) -> BytesMut {
        let len = src.len();
        let cap = src.capacity();

        BytesMut {
            mem: Mem::from_vec(src),
            pos: 0,
            len: len,
            cap: cap,
        }
    }
}

impl<'a> From<&'a [u8]> for BytesMut {
    fn from(src: &'a [u8]) -> BytesMut {
        BytesMut::from_slice(src)
    }
}

impl PartialEq for BytesMut {
    fn eq(&self, other: &BytesMut) -> bool {
        **self == **other
    }
}

impl Eq for BytesMut {
}

impl fmt::Debug for BytesMut {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_ref(), fmt)
    }
}

unsafe impl Send for BytesMut {}

/*
 *
 * ===== Mem =====
 *
 */

impl Mem {
    fn with_capacity(cap: usize) -> Mem {
        let mut vec = Vec::with_capacity(cap);
        unsafe { vec.set_len(cap); }

        Mem { inner: Arc::new(UnsafeCell::new(vec.into_boxed_slice())) }
    }

    fn from_vec(mut vec: Vec<u8>) -> Mem {
        let cap = vec.capacity();
        unsafe { vec.set_len(cap); }

        Mem { inner: Arc::new(UnsafeCell::new(vec.into_boxed_slice())) }
    }

    fn as_ref(&self) -> &[u8] {
        unsafe { &*self.inner.get() }
    }

    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *self.inner.get() }
    }

    fn is_mut_safe(&mut self) -> bool {
        Arc::get_mut(&mut self.inner).is_some()
    }

    fn clone(&self) -> Mem {
        Mem { inner: self.inner.clone() }
    }
}

/*
 *
 * ===== PartialEq =====
 *
 */

impl PartialEq<[u8]> for BytesMut {
    fn eq(&self, other: &[u8]) -> bool {
        &**self == other
    }
}

impl PartialEq<BytesMut> for [u8] {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialEq<Vec<u8>> for BytesMut {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == &other[..]
    }
}

impl PartialEq<BytesMut> for Vec<u8> {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for BytesMut
    where BytesMut: PartialEq<T>
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a> PartialEq<BytesMut> for &'a [u8] {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialEq<[u8]> for Bytes {
    fn eq(&self, other: &[u8]) -> bool {
        self.inner == *other
    }
}

impl PartialEq<Bytes> for [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialEq<Vec<u8>> for Bytes {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == &other[..]
    }
}

impl PartialEq<Bytes> for Vec<u8> {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl<'a> PartialEq<Bytes> for &'a [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for Bytes
    where Bytes: PartialEq<T>
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}
