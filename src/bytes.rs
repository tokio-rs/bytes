use {IntoBuf, BufMut};

use std::{cmp, fmt, mem, ops, slice, ptr};
use std::cell::{Cell, UnsafeCell};
use std::io::Cursor;
use std::sync::Arc;

/// A reference counted contiguous slice of memory.
///
/// `Bytes` is an efficient container for storing and operating on continguous
/// slices of memory. It is intended for use primarily in networking code, but
/// could have applications elsewhere as well.
///
/// `Bytes` values facilitate zero-copy network programming by allowing multiple
/// `Bytes` objects to point to the same underlying memory. This is managed by
/// using a reference count to track when the memory is no longer needed and can
/// be freed.
///
/// ```
/// use bytes::Bytes;
///
/// let mem = Bytes::from_slice(b"Hello world");
/// let a = mem.slice(0, 5);
///
/// assert_eq!(&a[..], b"Hello");
///
/// let b = mem.drain_to(6);
///
/// assert_eq!(&mem[..], b"world");
/// assert_eq!(&b[..], b"Hello ");
/// ```
///
/// # Memory layout
///
/// The `Bytes` struct itself is fairly small, limited to a pointer to the
/// memory and 4 `usize` fields used to track information about which segment of
/// the underlying memory the `Bytes` handle has access to.
///
/// The memory layout looks like this:
///
/// ```text
/// +-------+
/// | Bytes |
/// +-------+
///  /      \_____
/// |              \
/// v               v
/// +-----+------------------------------------+
/// | Arc |         |      Data     |          |
/// +-----+------------------------------------+
/// ```
///
/// `Bytes` keeps both a pointer to the shared `Arc` containing the full memory
/// slice and a pointer to the start of the region visible by the handle.
/// `Bytes` also tracks the length of its view into the memory.
///
/// # Sharing
///
/// The memory itself is reference counted, and multiple `Bytes` objects may
/// point to the same region. Each `Bytes` handle point to different sections within
/// the memory region, and `Bytes` handle may or may not have overlapping views
/// into the memory.
///
///
/// ```text
///
///    Arc ptrs                   +---------+
///    ________________________ / | Bytes 2 |
///   /                           +---------+
///  /          +-----------+     |         |
/// |_________/ |  Bytes 1  |     |         |
/// |           +-----------+     |         |
/// |           |           | ___/ data     | tail
/// |      data |      tail |/              |
/// v           v           v               v
/// +-----+---------------------------------+-----+
/// | Arc |     |           |               |     |
/// +-----+---------------------------------+-----+
/// ```
///
/// # Mutating
///
/// While `Bytes` handles may potentially represent overlapping views of the
/// underlying memory slice and may not be mutated, `BytesMut` handles are
/// guaranteed to be the only handle able to view that slice of memory. As such,
/// `BytesMut` handles are able to mutate the underlying memory. Note that
/// holding a unique view to a region of memory does not mean that there are not
/// other `Bytes` and `BytesMut` handles with disjoint views of the underlying
/// memory.
///
/// # Inline bytes.
///
/// As an opitmization, when the slice referenced by a `Bytes` or `BytesMut`
/// handle is small enough [1], `Bytes` will avoid the allocation by inlining
/// the slice directly in the handle. In this case, a clone is no longer
/// "shallow" and the data will be copied.
///
/// [1] Small enough: 24 bytes on 64 bit systems, 12 on 32 bit systems.
///
pub struct Bytes {
    inner: Inner,
}

/// A unique reference to a continuous slice of memory.
///
/// `BytesMut` represents a unique view into a potentially shared memory region.
/// Given the uniqueness guarantee, owners of `BytesMut` handles are able to
/// mutate the memory.
///
/// For more detail, see [Bytes](struct.Bytes.html).
///
/// ```
/// use bytes::{BytesMut, BufMut};
///
/// let mut buf = BytesMut::with_capacity(64);
///
/// buf.put_u8(b'h');
/// buf.put_u8(b'e');
/// buf.put_str("llo");
///
/// assert_eq!(&buf[..], b"hello");
///
/// // Freeze the buffer so that it can be shared
/// let a = buf.freeze();
///
/// // This does not allocate, instead `b` points to the same memory.
/// let b = a.clone();
///
/// assert_eq!(&a[..], b"hello");
/// assert_eq!(&b[..], b"hello");
/// ```
pub struct BytesMut {
    inner: Inner
}

struct Inner {
    data: UnsafeCell<Data>,

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

const INLINE_START_OFFSET: usize = 16;
const INLINE_START_MASK: usize = 0xff << INLINE_START_OFFSET;
const INLINE_LEN_OFFSET: usize = 8;
const INLINE_LEN_MASK: usize = 0xff << INLINE_LEN_OFFSET;

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
                data: UnsafeCell::new(Data {
                    ptr: ptr::null_mut(),
                    len: 0,
                    cap: 0,
                }),
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
                data: UnsafeCell::new(Data {
                    ptr: bytes.as_ptr() as *mut u8,
                    len: bytes.len(),
                    cap: bytes.len(),
                }),
                arc: Cell::new(KIND_STATIC),
            }
        }
    }

    /// Returns the number of bytes contained in this `Bytes`.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns the total byte capacity of this `Bytes`
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
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
        let ret = self.clone();

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
    pub fn split_off(&self, at: usize) -> Bytes {
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
    pub fn drain_to(&self, at: usize) -> Bytes {
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
    type Buf = Cursor<Self>;

    fn into_buf(self) -> Self::Buf {
        Cursor::new(self)
    }
}

impl<'a> IntoBuf for &'a Bytes {
    type Buf = Cursor<Self>;

    fn into_buf(self) -> Self::Buf {
        Cursor::new(self)
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
    ///
    /// The returned `BytesMut` will be able to hold at least `capacity` bytes
    /// without reallocating. If `capacity` is under `3 * size:of::<usize>()`,
    /// then `BytesMut` will not allocate.
    ///
    /// It is important to note that this function does not specify the length
    /// of the returned `BytesMut`, but only the capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BytesMut, BufMut};
    ///
    /// let mut bytes = BytesMut::with_capacity(64);
    ///
    /// // `bytes` contains no data, even though there is capacity
    /// assert_eq!(bytes.len(), 0);
    ///
    /// bytes.copy_from_slice(b"hello world");
    ///
    /// assert_eq!(&bytes[..], b"hello world");
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> BytesMut {
        if capacity <= INLINE_CAP {
            BytesMut {
                inner: Inner {
                    data: UnsafeCell::new(Data {
                        ptr: ptr::null_mut(),
                        len: 0,
                        cap: 0,
                    }),
                    arc: Cell::new(KIND_INLINE),
                }
            }
        } else {
            BytesMut::from(Vec::with_capacity(capacity))
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

                let a = KIND_INLINE | (len << INLINE_LEN_OFFSET);

                BytesMut {
                    inner: Inner {
                        data: mem::transmute(data),
                        arc: Cell::new(a),
                    }
                }
            }
        } else {
            let mut buf = BytesMut::with_capacity(bytes.as_ref().len());
            buf.copy_from_slice(bytes.as_ref());
            buf
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
    /// This is an O(1) operation [1] that just increases the reference count
    /// and sets a few indexes.
    ///
    /// [1] Inlined bytes are copied
    ///
    /// # Panics
    ///
    /// Panics if `at > capacity`
    pub fn split_off(&self, at: usize) -> Bytes {
        Bytes { inner: self.inner.split_off(at) }
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[0, at)`, and the returned
    /// `BytesMut` contains elements `[at, capacity)`.
    ///
    /// This is an O(1) operation [1] that just increases the reference count
    /// and sets a few indexes.
    ///
    /// [1] Inlined bytes are copied
    ///
    /// # Panics
    ///
    /// Panics if `at > capacity`
    pub fn split_off_mut(&mut self, at: usize) -> BytesMut {
        BytesMut { inner: self.inner.split_off(at) }
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned `Bytes`
    /// contains elements `[0, at)`.
    ///
    /// This is an O(1) operation [1] that just increases the reference count
    /// and sets a few indexes.
    ///
    /// [1] Inlined bytes are copied.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    pub fn drain_to(&self, at: usize) -> Bytes {
        Bytes { inner: self.inner.drain_to(at) }
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned `BytesMut`
    /// contains elements `[0, at)`.
    ///
    /// This is an O(1) operation [1] that just increases the reference count and
    /// sets a few indexes.
    ///
    /// [1] Inlined bytes are copied.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    pub fn drain_to_mut(&mut self, at: usize) -> BytesMut {
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

impl BufMut for BytesMut {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.capacity() - self.len()
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        let new_len = self.len() + cnt;
        self.set_len(new_len);
    }

    #[inline]
    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        let len = self.len();
        &mut self.as_raw()[len..]
    }

    #[inline]
    fn copy_from_slice(&mut self, src: &[u8]) {
        assert!(self.remaining_mut() >= src.len());

        let len = src.len();

        unsafe {
            self.bytes_mut()[..len].copy_from_slice(src);
            self.advance_mut(len);
        }
    }
}

impl IntoBuf for BytesMut {
    type Buf = Cursor<Self>;

    fn into_buf(self) -> Self::Buf {
        Cursor::new(self)
    }
}

impl<'a> IntoBuf for &'a BytesMut {
    type Buf = Cursor<&'a BytesMut>;

    fn into_buf(self) -> Self::Buf {
        Cursor::new(self)
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
                data: UnsafeCell::new(Data {
                    ptr: ptr,
                    len: len,
                    cap: cap,
                }),
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
 * ===== Inner =====
 *
 */

impl Inner {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        if self.is_inline() {
            unsafe {
                slice::from_raw_parts(self.inline_ptr(), self.inline_len())
            }
        } else {
            unsafe {
                let d = &*self.data.get();
                slice::from_raw_parts(d.ptr, d.len)
            }
        }
    }

    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        debug_assert!(self.kind() != Kind::Static);

        if self.is_inline() {
            unsafe {
                slice::from_raw_parts_mut(self.inline_ptr(), self.inline_len())
            }
        } else {
            unsafe {
                let d = &*self.data.get();
                slice::from_raw_parts_mut(d.ptr, d.len)
            }
        }
    }

    #[inline]
    unsafe fn as_raw(&mut self) -> &mut [u8] {
        debug_assert!(self.kind() != Kind::Static);

        if self.is_inline() {
            slice::from_raw_parts_mut(self.inline_ptr(), self.inline_capacity())
        } else {
            let d = &*self.data.get();
            slice::from_raw_parts_mut(d.ptr, d.cap)
        }
    }

    #[inline]
    fn len(&self) -> usize {
        if self.is_inline() {
            self.inline_len()
        } else {
            unsafe { (*self.data.get()).len }
        }
    }

    #[inline]
    unsafe fn inline_ptr(&self) -> *mut u8 {
        (self.data.get() as *mut u8).offset(self.inline_start() as isize)
    }

    #[inline]
    fn inline_start(&self) -> usize {
        (self.arc.get() & INLINE_START_MASK) >> INLINE_START_OFFSET
    }

    #[inline]
    fn set_inline_start(&self, start: usize) {
        debug_assert!(start <= INLINE_START_MASK);

        let v = (self.arc.get() & !INLINE_START_MASK) |
            (start << INLINE_START_OFFSET);

        self.arc.set(v);
    }

    #[inline]
    fn inline_len(&self) -> usize {
        (self.arc.get() & INLINE_LEN_MASK) >> INLINE_LEN_OFFSET
    }

    #[inline]
    fn set_inline_len(&self, len: usize) {
        debug_assert!(len <= INLINE_LEN_MASK);

        let v = (self.arc.get() & !INLINE_LEN_MASK) |
            (len << INLINE_LEN_OFFSET);

        self.arc.set(v);
    }

    #[inline]
    fn inline_capacity(&self) -> usize {
        INLINE_CAP - self.inline_start()
    }

    #[inline]
    unsafe fn set_len(&mut self, len: usize) {
        if self.is_inline() {
            assert!(len <= self.inline_capacity());
            self.set_inline_len(len);
        } else {
            let d = &mut *self.data.get();
            assert!(len <= d.cap);
            d.len = len;
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        if self.is_inline() {
            self.inline_capacity()
        } else {
            unsafe { (*self.data.get()).cap }
        }
    }

    fn split_off(&self, at: usize) -> Inner {
        let other = self.shallow_clone();

        unsafe {
            other.set_start(at);
            self.set_end(at);
        }

        return other
    }

    fn drain_to(&self, at: usize) -> Inner {
        let other = self.shallow_clone();

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
    unsafe fn set_start(&self, start: usize) {
        debug_assert!(self.is_shared());

        if start == 0 {
            return;
        }

        if self.is_inline() {
            assert!(start <= self.inline_capacity());

            let old_start = self.inline_start();
            let old_len = self.inline_len();

            self.set_inline_start(old_start + start);

            if old_len >= start {
                self.set_inline_len(old_len - start);
            } else {
                self.set_inline_len(0);
            }
        } else {
            let d = &mut *self.data.get();

            assert!(start <= d.cap);

            d.ptr = d.ptr.offset(start as isize);

            // TODO: This could probably be optimized with some bit fiddling
            if d.len >= start {
                d.len -= start;
            } else {
                d.len = 0;
            }

            d.cap -= start;
        }
    }

    /// Changes the end index of this window to the index specified.
    ///
    /// # Panics
    ///
    /// This method will panic if `start` is out of bounds for the underlying
    /// slice.
    unsafe fn set_end(&self, end: usize) {
        debug_assert!(self.is_shared());

        if self.is_inline() {
            assert!(end <= self.inline_capacity());
            let new_len = cmp::min(self.inline_len(), end);
            self.set_inline_len(new_len);
        } else {
            let d = &mut *self.data.get();

            assert!(end <= d.cap);

            d.cap = end;
            d.len = cmp::min(d.len, end);
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
                    let d = &*self.data.get();

                    // Promote this `Bytes` to an arc, and clone it
                    let v = Vec::from_raw_parts(d.ptr, d.len, d.cap);

                    let a = Arc::new(v);
                    self.arc.set(mem::transmute(a.clone()));

                    Inner {
                        data: UnsafeCell::new(*d),
                        arc: Cell::new(mem::transmute(a)),
                    }
                }
            }
            Kind::Arc => {
                unsafe {
                    let arc: &Shared = mem::transmute(&self.arc);

                    Inner {
                        data: UnsafeCell::new(*self.data.get()),
                        arc: Cell::new(mem::transmute(arc.clone())),
                    }
                }
            }
            Kind::Inline => {
                let len = self.inline_len();

                unsafe {
                    let mut data: Data = mem::uninitialized();

                    let dst = &mut data as *mut _ as *mut u8;
                    let src = self.inline_ptr();

                    ptr::copy_nonoverlapping(src, dst, len);

                    let mut a = KIND_INLINE;
                    a |= len << INLINE_LEN_OFFSET;

                    Inner {
                        data: UnsafeCell::new(data),
                        arc: Cell::new(a),
                    }
                }
            }
            Kind::Static => {
                Inner {
                    data: unsafe { UnsafeCell::new(*self.data.get()) },
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
                    let d = *self.data.get();
                    // Not shared, manually free
                    let _ = Vec::from_raw_parts(d.ptr, d.len, d.cap);
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
