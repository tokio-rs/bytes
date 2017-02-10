use {IntoBuf, ByteBuf, SliceBuf};

use std::{cmp, fmt, mem, ops, slice, ptr};
use std::cell::{Cell, UnsafeCell};
use std::sync::Arc;

/// A reference counted slice of bytes.
///
/// A `Bytes` is an immutable sequence of bytes. Given that it is guaranteed to
/// be immutable, `Bytes` is `Sync`, `Clone` is shallow (ref count increment),
/// and all operations only update views into the underlying data without
/// requiring any copies.
pub struct Bytes {
    inner: Inner,
}

/// A unique reference to a slice of bytes.
///
/// A `BytesMut` is a unique handle to a slice of bytes allowing mutation of
/// the underlying bytes.
pub struct BytesMut {
    inner: Inner
}

struct Inner {
    data: Data,

    // If this pointer is set, then the the BytesMut is backed by an Arc
    arc: Cell<usize>,
}

#[repr(C)]
#[derive(Eq, PartialEq, Clone, Copy)]
struct Data {
    // Pointer to the start of the memory owned by this BytesMut
    ptr: *mut u8,

    // Number of bytes that have been initialized
    len: usize,

    // Total number of bytes owned by this BytesMut
    cap: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Kind {
    Vec,
    Arc,
    Inline,
    Static,
}

type Shared = Arc<UnsafeCell<Vec<u8>>>;

#[cfg(target_pointer_width = "64")]
const INLINE_CAP: usize = 8 * 3;

#[cfg(target_pointer_width = "32")]
const INNER_CAP: usize = 4 * 3;

const KIND_MASK: usize = 3;
const KIND_INLINE: usize = 1;
const KIND_STATIC: usize = 2;


/*
 *
 * ===== Bytes =====
 *
 */

impl Bytes {
    /// Creates a new empty `Bytes`
    #[inline]
    pub fn new() -> Bytes {
        Bytes {
            inner: Inner {
                data: Data {
                    ptr: ptr::null_mut(),
                    len: 0,
                    cap: 0,
                },
                arc: Cell::new(0),
            }
        }
    }

    /// Creates a new `Bytes` and copy the given slice into it.
    #[inline]
    pub fn from_slice<T: AsRef<[u8]>>(bytes: T) -> Bytes {
        BytesMut::from_slice(bytes).freeze()
    }

    /// Creates a new `Bytes` from a static slice.
    ///
    /// This is a zero copy function
    #[inline]
    pub fn from_static(bytes: &'static [u8]) -> Bytes {
        Bytes {
            inner: Inner {
                data: Data {
                    ptr: bytes.as_ptr() as *mut u8,
                    len: bytes.len(),
                    cap: bytes.len(),
                },
                arc: Cell::new(KIND_STATIC),
            }
        }
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

        unsafe {
            ret.inner.set_end(end);
            ret.inner.set_start(start);
        }

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
        Bytes { inner: self.inner.split_off(at) }
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
        Bytes { inner: self.inner.drain_to(at) }
    }

    /// Attempt to convert into a `BytesMut` handle.
    ///
    /// This will only succeed if there are no other outstanding references to
    /// the underlying chunk of memory.
    pub fn try_mut(mut self) -> Result<BytesMut, Bytes> {
        if self.inner.is_mut_safe() {
            Ok(BytesMut { inner: self.inner })
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
        Bytes { inner: self.inner.shallow_clone() }
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
        self.inner.as_ref()
    }
}

impl From<BytesMut> for Bytes {
    fn from(src: BytesMut) -> Bytes {
        src.freeze()
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
        self.inner.as_ref() == other.inner.as_ref()
    }
}

impl Eq for Bytes {
}

impl fmt::Debug for Bytes {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner.as_ref(), fmt)
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
    #[inline]
    pub fn with_capacity(cap: usize) -> BytesMut {
        if cap <= INLINE_CAP {
            BytesMut {
                inner: Inner {
                    data: Data {
                        ptr: ptr::null_mut(),
                        len: 0,
                        cap: 0,
                    },
                    arc: Cell::new(KIND_INLINE),
                }
            }
        } else {
            BytesMut::from(Vec::with_capacity(cap))
        }
    }

    /// Creates a new `BytesMut` and copy the given slice into it.
    #[inline]
    pub fn from_slice<T: AsRef<[u8]>>(bytes: T) -> BytesMut {
        let b = bytes.as_ref();

        if b.len() <= INLINE_CAP {
            unsafe {
                let len = b.len();
                let mut data: [u8; INLINE_CAP] = mem::uninitialized();
                data[0..len].copy_from_slice(b);

                BytesMut {
                    inner: Inner {
                        data: mem::transmute(data),
                        arc: Cell::new(KIND_INLINE | (len << 2)),
                    }
                }
            }
        } else {
            let buf = ByteBuf::from_slice(b);
            buf.into_inner()
        }
    }

    /// Returns the number of bytes contained in this `BytesMut`.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the value contains no bytes
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the total byte capacity of this `BytesMut`
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Return an immutable handle to the bytes
    #[inline]
    pub fn freeze(self) -> Bytes {
        Bytes { inner: self.inner }
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
        BytesMut { inner: self.inner.split_off(at) }
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
        BytesMut { inner: self.inner.drain_to(at) }
    }

    /// Returns the inner contents of this `BytesMut` as a slice.
    pub fn as_slice(&self) -> &[u8] {
        self.as_ref()
    }

    /// Returns the inner contents of this `BytesMut` as a mutable slice
    ///
    /// This a slice of bytes that have been initialized
    pub fn as_mut(&mut self) -> &mut [u8] {
        self.inner.as_mut()
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
        self.inner.set_len(len);
    }

    /// Returns the inner contents of this `BytesMut` as a mutable slice
    ///
    /// This a slice of all bytes, including uninitialized memory
    #[inline]
    pub unsafe fn as_raw(&mut self) -> &mut [u8] {
        self.inner.as_raw()
    }
}

/*
 *
 * ===== Inner =====
 *
 */

impl Inner {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        if self.is_inline() {
            unsafe {
                slice::from_raw_parts(&self.data as *const _ as *const u8, self.inline_len())
            }
        } else {
            unsafe { slice::from_raw_parts(self.data.ptr, self.data.len) }
        }
    }

    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        debug_assert!(self.kind() != Kind::Static);

        if self.is_inline() {
            unsafe {
                slice::from_raw_parts_mut(&mut self.data as *mut _ as *mut u8, self.inline_len())
            }
        } else {
            unsafe { slice::from_raw_parts_mut(self.data.ptr, self.data.len) }
        }
    }

    #[inline]
    unsafe fn as_raw(&mut self) -> &mut [u8] {
        debug_assert!(self.kind() != Kind::Static);

        if self.is_inline() {
            slice::from_raw_parts_mut(&mut self.data as *mut _ as *mut u8, INLINE_CAP)
        } else {
            slice::from_raw_parts_mut(self.data.ptr, self.data.cap)
        }
    }

    #[inline]
    fn len(&self) -> usize {
        if self.is_inline() {
            self.inline_len()
        } else {
            self.data.len
        }
    }

    #[inline]
    fn inline_len(&self) -> usize {
        self.arc.get() >> 2
    }

    #[inline]
    unsafe fn set_len(&mut self, len: usize) {
        if self.is_inline() {
            assert!(len <= INLINE_CAP);
            self.arc.set(len << 2 | KIND_INLINE);
        } else {
            assert!(len <= self.data.cap);
            self.data.len = len;
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        if self.is_inline() {
            INLINE_CAP
        } else {
            self.data.cap
        }
    }

    fn split_off(&mut self, at: usize) -> Inner {
        let mut other = self.shallow_clone();

        unsafe {
            other.set_start(at);
            self.set_end(at);
        }

        return other
    }

    fn drain_to(&mut self, at: usize) -> Inner {
        let mut other = self.shallow_clone();

        unsafe {
            other.set_end(at);
            self.set_start(at);
        }

        return other
    }

    /// Changes the starting index of this window to the index specified.
    ///
    /// # Panics
    ///
    /// This method will panic if `start` is out of bounds for the underlying
    /// slice.
    unsafe fn set_start(&mut self, start: usize) {
        debug_assert!(self.is_shared());

        if self.is_inline() {
            if start == 0 {
                return;
            }

            let len = self.inline_len();

            if len <= start {
                assert!(start <= INLINE_CAP);

                // Set the length to zero
                self.arc.set(KIND_INLINE);
            } else {
                debug_assert!(start <= INLINE_CAP);

                let new_len = len - start;

                let dst = &self.data as *const Data as *mut Data as *mut u8;
                let src = (dst as *const u8).offset(start as isize);

                ptr::copy(src, dst, new_len);

                self.arc.set((new_len << 2) | KIND_INLINE);
            }
        } else {
            assert!(start <= self.data.cap);

            self.data.ptr = self.data.ptr.offset(start as isize);

            // TODO: This could probably be optimized with some bit fiddling
            if self.data.len >= start {
                self.data.len -= start;
            } else {
                self.data.len = 0;
            }

            self.data.cap -= start;
        }
    }

    /// Changes the end index of this window to the index specified.
    ///
    /// # Panics
    ///
    /// This method will panic if `start` is out of bounds for the underlying
    /// slice.
    unsafe fn set_end(&mut self, end: usize) {
        debug_assert!(self.is_shared());

        if self.is_inline() {
            assert!(end <= INLINE_CAP);
            let new_len = cmp::min(self.inline_len(), end);

            self.arc.set((new_len << 2) | KIND_INLINE);
        } else {
            assert!(end <= self.data.cap);
            debug_assert!(self.is_shared());

            self.data.cap = end;
            self.data.len = cmp::min(self.data.len, end);
        }
    }

    /// Checks if it is safe to mutate the memory
    fn is_mut_safe(&mut self) -> bool {
        match self.kind() {
            Kind::Static => false,
            Kind::Arc => {
                unsafe {
                    let arc: &mut Shared = mem::transmute(&mut self.arc);
                    Arc::get_mut(arc).is_some()
                }
            }
            Kind::Vec | Kind::Inline => true,
        }
    }

    /// Increments the ref count. This should only be done if it is known that
    /// it can be done safely. As such, this fn is not public, instead other
    /// fns will use this one while maintaining the guarantees.
    fn shallow_clone(&self) -> Inner {
        match self.kind() {
            Kind::Vec => {
                unsafe {
                    // Promote this `Bytes` to an arc, and clone it
                    let v = Vec::from_raw_parts(
                        self.data.ptr,
                        self.data.len,
                        self.data.cap);

                    let a = Arc::new(v);
                    self.arc.set(mem::transmute(a.clone()));

                    Inner {
                        data: self.data,
                        arc: Cell::new(mem::transmute(a)),
                    }
                }
            }
            Kind::Arc => {
                unsafe {
                    let arc: &Shared = mem::transmute(&self.arc);

                    Inner {
                        data: self.data,
                        arc: Cell::new(mem::transmute(arc.clone())),
                    }
                }
            }
            Kind::Inline | Kind::Static => {
                Inner {
                    data: self.data,
                    arc: Cell::new(self.arc.get()),
                }
            }
        }
    }

    #[inline]
    fn kind(&self) -> Kind {
        let arc = self.arc.get();

        if arc == 0 {
            return Kind::Vec
        }

        let kind = arc & KIND_MASK;

        match kind {
            0 => Kind::Arc,
            KIND_INLINE => Kind::Inline,
            KIND_STATIC => Kind::Static,
            _ => unreachable!(),
        }
    }

    #[inline]
    fn is_inline(&self) -> bool {
        self.arc.get() & KIND_MASK == KIND_INLINE
    }

    #[inline]
    fn is_shared(&self) -> bool {
        self.kind() != Kind::Vec
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        match self.kind() {
            Kind::Vec => {
                unsafe {
                    // Not shared, manually free
                    let _ = Vec::from_raw_parts(
                        self.data.ptr,
                        self.data.len,
                        self.data.cap);
                }
            }
            Kind::Arc => {
                unsafe {
                    let _: Arc<UnsafeCell<Vec<u8>>> = mem::transmute(self.arc.get());
                }
            }
            _ => {}
        }
    }
}

unsafe impl Send for Inner {}

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
        self.inner.as_ref()
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
    fn from(mut src: Vec<u8>) -> BytesMut {
        let len = src.len();
        let cap = src.capacity();
        let ptr = src.as_mut_ptr();

        mem::forget(src);

        BytesMut {
            inner: Inner {
                data: Data {
                    ptr: ptr,
                    len: len,
                    cap: cap,
                },
                arc: Cell::new(0),
            },
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
        self.inner.as_ref() == other.inner.as_ref()
    }
}

impl Eq for BytesMut {
}

impl fmt::Debug for BytesMut {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.inner.as_ref(), fmt)
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
        self.inner.as_ref() == other
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

impl Clone for BytesMut {
    fn clone(&self) -> BytesMut {
        BytesMut::from_slice(self.as_ref())
    }
}
