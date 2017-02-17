use {IntoBuf, BufMut};

use std::{cmp, fmt, mem, hash, ops, slice, ptr};
use std::borrow::Borrow;
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
/// let mem = Bytes::from(&b"Hello world"[..]);
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
/// buf.put(b'h');
/// buf.put(b'e');
/// buf.put("llo");
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
    ///
    /// This will not allocate and the returned `Bytes` handle will be empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let b = Bytes::new();
    /// assert_eq!(&b[..], b"");
    /// ```
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

    /// Creates a new `Bytes` from a static slice.
    ///
    /// The returned `Bytes` will point directly to the static slice. There is
    /// no allocating or copying.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let b = Bytes::from_static(b"hello");
    /// assert_eq!(&b[..], b"hello");
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let b = Bytes::from(&b"hello"[..]);
    /// assert_eq!(b.len(), 5);
    /// ```
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the `Bytes` has a length of 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let b = Bytes::new();
    /// assert!(b.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a slice of self for the index range `[begin..end)`.
    ///
    /// This will increment the reference count for the underlying memory and
    /// return a new `Bytes` handle set to the slice.
    ///
    /// This operation is `O(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let a = Bytes::from(&b"hello world"[..]);
    /// let b = a.slice(2, 5);
    ///
    /// assert_eq!(&b[..], b"llo");
    /// ```
    ///
    /// # Panics
    ///
    /// Requires that `begin <= end` and `end <= self.len()`, otherwise slicing
    /// will panic.
    pub fn slice(&self, begin: usize, end: usize) -> Bytes {
        let ret = self.clone();

        unsafe {
            ret.inner.set_end(end);
            ret.inner.set_start(begin);
        }

        ret
    }

    /// Returns a slice of self for the index range `[begin..self.len())`.
    ///
    /// This will increment the reference count for the underlying memory and
    /// return a new `Bytes` handle set to the slice.
    ///
    /// This operation is `O(1)` and is equivalent to `self.slice(begin,
    /// self.len())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let a = Bytes::from(&b"hello world"[..]);
    /// let b = a.slice_from(6);
    ///
    /// assert_eq!(&b[..], b"world");
    /// ```
    ///
    /// # Panics
    ///
    /// Requires that `begin <= self.len()`, otherwise slicing will panic.
    pub fn slice_from(&self, begin: usize) -> Bytes {
        self.slice(begin, self.len())
    }

    /// Returns a slice of self for the index range `[0..end)`.
    ///
    /// This will increment the reference count for the underlying memory and
    /// return a new `Bytes` handle set to the slice.
    ///
    /// This operation is `O(1)` and is equivalent to `self.slice(0, end)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let a = Bytes::from(&b"hello world"[..]);
    /// let b = a.slice_to(5);
    ///
    /// assert_eq!(&b[..], b"hello");
    /// ```
    ///
    /// # Panics
    ///
    /// Requires that `end <= self.len()`, otherwise slicing will panic.
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
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let a = Bytes::from(&b"hello world"[..]);
    /// let b = a.split_off(5);
    ///
    /// assert_eq!(&a[..], b"hello");
    /// assert_eq!(&b[..], b" world");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    pub fn split_off(&self, at: usize) -> Bytes {
        Bytes { inner: self.inner.split_off(at) }
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned
    /// `Bytes` contains elements `[0, at)`.
    ///
    /// This is an O(1) operation that just increases the reference count and
    /// sets a few indexes.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let a = Bytes::from(&b"hello world"[..]);
    /// let b = a.drain_to(5);
    ///
    /// assert_eq!(&a[..], b" world");
    /// assert_eq!(&b[..], b"hello");
    /// ```
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
    /// the underlying chunk of memory. `Bytes` handles that contain inlined
    /// bytes will always be convertable to `BytesMut`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let a = Bytes::from(&b"Mary had a little lamb, little lamb, little lamb..."[..]);
    ///
    /// // Create a shallow clone
    /// let b = a.clone();
    ///
    /// // This will fail because `b` shares a reference with `a`
    /// let a = a.try_mut().unwrap_err();
    ///
    /// drop(b);
    ///
    /// // This will succeed
    /// let mut a = a.try_mut().unwrap();
    ///
    /// a[0] = b'b';
    ///
    /// assert_eq!(&a[..4], b"bary");
    /// ```
    pub fn try_mut(mut self) -> Result<BytesMut, Bytes> {
        if self.inner.is_mut_safe() {
            Ok(BytesMut { inner: self.inner })
        } else {
            Err(self)
        }
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

impl From<String> for Bytes {
    fn from(src: String) -> Bytes {
        BytesMut::from(src).freeze()
    }
}

impl<'a> From<&'a [u8]> for Bytes {
    fn from(src: &'a [u8]) -> Bytes {
        BytesMut::from(src).freeze()
    }
}

impl<'a> From<&'a str> for Bytes {
    fn from(src: &'a str) -> Bytes {
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

impl hash::Hash for Bytes {
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
        let s: &[u8] = self.as_ref();
        s.hash(state);
    }
}

impl Borrow<[u8]> for Bytes {
    fn borrow(&self) -> &[u8] {
        self.as_ref()
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
    /// bytes.put(&b"hello world"[..]);
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

    /// Returns the number of bytes contained in this `BytesMut`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let b = BytesMut::from(&b"hello"[..]);
    /// assert_eq!(b.len(), 5);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the `BytesMut` has a length of 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let b = BytesMut::with_capacity(64);
    /// assert!(b.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of bytes the `BytesMut` can hold without reallocating.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let b = BytesMut::with_capacity(64);
    /// assert_eq!(b.capacity(), 64);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Convert `self` into an immutable `Bytes`
    ///
    /// The conversion is zero cost and is used to indicate that the slice
    /// referenced by the handle will no longer be mutated. Once the conversion
    /// is done, the handle can be cloned and shared across threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BytesMut, BufMut};
    /// use std::thread;
    ///
    /// let mut b = BytesMut::with_capacity(64);
    /// b.put("hello world");
    /// let b1 = b.freeze();
    /// let b2 = b1.clone();
    ///
    /// let th = thread::spawn(move || {
    ///     assert_eq!(&b1[..], b"hello world");
    /// });
    ///
    /// assert_eq!(&b2[..], b"hello world");
    /// th.join().unwrap();
    /// ```
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
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut a = BytesMut::from(&b"hello world"[..]);
    /// let b = a.split_off(5);
    ///
    /// a[0] = b'j';
    ///
    /// assert_eq!(&a[..], b"jello");
    /// assert_eq!(&b[..], b" world");
    /// ```
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
    /// This is an O(1) operation that just increases the reference count
    /// and sets a few indexes.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut a = BytesMut::from(&b"hello world"[..]);
    /// let mut b = a.split_off_mut(5);
    ///
    /// a[0] = b'j';
    /// b[0] = b'!';
    ///
    /// assert_eq!(&a[..], b"jello");
    /// assert_eq!(&b[..], b"!world");
    /// ```
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
    /// This is an O(1) operation that just increases the reference count and
    /// sets a few indexes.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut a = BytesMut::from(&b"hello world"[..]);
    /// let b = a.drain_to(5);
    ///
    /// a[0] = b'!';
    ///
    /// assert_eq!(&a[..], b"!world");
    /// assert_eq!(&b[..], b"hello");
    /// ```
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
    /// This is an O(1) operation that just increases the reference count and
    /// sets a few indexes.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut a = BytesMut::from(&b"hello world"[..]);
    /// let mut b = a.drain_to_mut(5);
    ///
    /// a[0] = b'!';
    /// b[0] = b'j';
    ///
    /// assert_eq!(&a[..], b"!world");
    /// assert_eq!(&b[..], b"jello");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at > len`
    pub fn drain_to_mut(&mut self, at: usize) -> BytesMut {
        BytesMut { inner: self.inner.drain_to(at) }
    }

    /// Sets the length of the buffer
    ///
    /// This will explicitly set the size of the buffer without actually
    /// modifying the data, so it is up to the caller to ensure that the data
    /// has been initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut b = BytesMut::from(&b"hello world"[..]);
    ///
    /// unsafe {
    ///     b.set_len(5);
    /// }
    ///
    /// assert_eq!(&b[..], b"hello");
    ///
    /// unsafe {
    ///     b.set_len(11);
    /// }
    ///
    /// assert_eq!(&b[..], b"hello world");
    /// ```
    ///
    /// # Panics
    ///
    /// This method will panic if `len` is out of bounds for the underlying
    /// slice or if it comes after the `end` of the configured window.
    pub unsafe fn set_len(&mut self, len: usize) {
        self.inner.set_len(len)
    }

    /// Reserves capacity for at least `additional` more bytes to be inserted
    /// into the given `BytesMut`.
    ///
    /// More than `additional` bytes may be reserved in order to avoid frequent
    /// reallocations. A call to `reserve` may result in an allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut b = BytesMut::from(&b"hello"[..]);
    /// b.reserve(64);
    /// assert!(b.capacity() >= 69);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    /// Attempts to reclaim ownership of the full buffer.
    ///
    /// Returns `true` if the reclaim is successful.
    ///
    /// If the `BytesMut` handle is the only outstanding handle pointing to the
    /// memory slice, the handle's view will be set to the full memory slice,
    /// enabling reusing buffer space without allocating.
    ///
    /// Any data in the `BytesMut` handle will be copied to the start of the
    /// memory region.
    ///
    /// ```rust
    /// use bytes::BytesMut;
    ///
    /// let mut bytes = BytesMut::from(
    ///     "Lorem ipsum dolor sit amet, consectetur adipiscing elit.");
    ///
    /// // Create a new handle to the shared memory region
    /// let a = bytes.drain_to(5);
    ///
    /// // Attempting to reclaim here will fail due to `a` still being in
    /// // existence.
    /// assert!(!bytes.try_reclaim());
    /// assert_eq!(bytes.capacity(), 51);
    ///
    /// // Dropping the handle will allow reclaim to succeed.
    /// drop(a);
    /// assert!(bytes.try_reclaim());
    /// assert_eq!(bytes.capacity(), 56);
    /// ```
    pub fn try_reclaim(&mut self) -> bool {
        self.inner.try_reclaim()
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
        self.inner.set_len(new_len);
    }

    #[inline]
    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        let len = self.len();
        &mut self.inner.as_raw()[len..]
    }

    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
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
        self.inner.as_mut()
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

impl From<String> for BytesMut {
    fn from(src: String) -> BytesMut {
        BytesMut::from(src.into_bytes())
    }
}

impl<'a> From<&'a [u8]> for BytesMut {
    fn from(src: &'a [u8]) -> BytesMut {
        if src.len() <= INLINE_CAP {
            unsafe {
                let len = src.len();
                let mut data: [u8; INLINE_CAP] = mem::uninitialized();
                data[0..len].copy_from_slice(src);

                let a = KIND_INLINE | (len << INLINE_LEN_OFFSET);

                BytesMut {
                    inner: Inner {
                        data: mem::transmute(data),
                        arc: Cell::new(a),
                    }
                }
            }
        } else {
            let mut buf = BytesMut::with_capacity(src.len());
            buf.put(src.as_ref());
            buf
        }
    }
}

impl<'a> From<&'a str> for BytesMut {
    fn from(src: &'a str) -> BytesMut {
        BytesMut::from(src.as_bytes())
    }
}

impl From<Bytes> for BytesMut {
    fn from(src: Bytes) -> BytesMut {
        src.try_mut()
            .unwrap_or_else(|src| BytesMut::from(&src[..]))
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

impl hash::Hash for BytesMut {
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
        let s: &[u8] = self.as_ref();
        s.hash(state);
    }
}

impl Borrow<[u8]> for BytesMut {
    fn borrow(&self) -> &[u8] {
        self.as_ref()
    }
}

impl fmt::Write for BytesMut {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        BufMut::put(self, s);
        Ok(())
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self, args)
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
    fn reserve(&mut self, additional: usize) {
        let len = self.len();
        let rem = self.capacity() - len;

        if additional <= rem {
            // Nothing more to do
            return;
        }

        match self.kind() {
            Kind::Vec => {
                unsafe {
                    let d = &mut *self.data.get();

                    // Promote this `Bytes` to an arc, and clone it
                    let mut v = Vec::from_raw_parts(d.ptr, d.len, d.cap);
                    v.reserve(additional);

                    // Update the info
                    d.ptr = v.as_mut_ptr();
                    d.len = v.len();
                    d.cap = v.capacity();

                    // Drop the vec reference
                    mem::forget(v);
                }
            }
            Kind::Arc => {
                unsafe {
                    // Compute the new capacity
                    let new_cap = len + additional;

                    // Create a new vector to store the data
                    let mut v = Vec::with_capacity(new_cap);

                    // Copy the bytes
                    v.extend_from_slice(self.as_ref());

                    let d = &mut *self.data.get();

                    d.ptr = v.as_mut_ptr();
                    d.len = v.len();
                    d.cap = v.capacity();

                    mem::forget(v);

                    // Drop the arc reference
                    let _: Arc<UnsafeCell<Vec<u8>>> = mem::transmute(self.arc.get());

                    self.arc.set(0);
                }
            }
            Kind::Inline => {
                let new_cap = len + additional;

                unsafe {
                    if new_cap <= INLINE_CAP {
                        let dst = &mut self.data as *mut _ as *mut u8;
                        let src = self.inline_ptr();

                        ptr::copy(src, dst, len);

                        let mut a = KIND_INLINE;
                        a |= len << INLINE_LEN_OFFSET;

                        self.arc.set(a);
                    } else {
                        let mut v = Vec::with_capacity(new_cap);

                        // Copy the bytes
                        v.extend_from_slice(self.as_ref());

                        let d = &mut *self.data.get();

                        d.ptr = v.as_mut_ptr();
                        d.len = v.len();
                        d.cap = v.capacity();

                        mem::forget(v);

                        self.arc.set(0);
                    }
                }
            }
            Kind::Static => unreachable!(),
        }
    }

    /// This must take `&mut self` in order to be able to copy memory in the
    /// inline case.
    #[inline]
    fn try_reclaim(&mut self) -> bool {
        match self.kind() {
            Kind::Inline => {
                if self.inline_start() > 0 {
                    // Shift the data back to the front
                    unsafe {
                        let len = self.inline_len();
                        let dst = &mut self.data as *mut _ as *mut u8;
                        let src = self.inline_ptr();

                        ptr::copy(src, dst, len);

                        let mut a = KIND_INLINE;
                        a |= len << INLINE_LEN_OFFSET;

                        self.arc.set(a);
                    }
                }

                true
            }
            Kind::Arc => {
                unsafe {
                    let arc: &mut Shared = mem::transmute(&mut self.arc);

                    // Check if mut safe
                    if Arc::get_mut(arc).is_none() {
                        return false;
                    }

                    let v = &mut *arc.get();
                    let d = &mut *self.data.get();

                    let len = v.len();
                    let ptr = v.as_mut_ptr();

                    ptr::copy(d.ptr, ptr, len);

                    d.ptr = ptr;
                    d.len = len;
                    d.cap = v.capacity();

                    true
                }
            }
            Kind::Vec => {
                true
            }
            Kind::Static => unreachable!(),
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

impl PartialEq<str> for BytesMut {
    fn eq(&self, other: &str) -> bool {
        &**self == other.as_bytes()
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

impl PartialEq<String> for BytesMut {
    fn eq(&self, other: &String) -> bool {
        *self == &other[..]
    }
}

impl PartialEq<BytesMut> for Vec<u8> {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialEq<BytesMut> for String {
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

impl<'a> PartialEq<BytesMut> for &'a str {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialEq<[u8]> for Bytes {
    fn eq(&self, other: &[u8]) -> bool {
        self.inner.as_ref() == other
    }
}

impl PartialEq<str> for Bytes {
    fn eq(&self, other: &str) -> bool {
        self.inner.as_ref() == other.as_bytes()
    }
}

impl PartialEq<Bytes> for [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialEq<Bytes> for str {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialEq<Vec<u8>> for Bytes {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == &other[..]
    }
}

impl PartialEq<String> for Bytes {
    fn eq(&self, other: &String) -> bool {
        *self == &other[..]
    }
}

impl PartialEq<Bytes> for Vec<u8> {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialEq<Bytes> for String {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl<'a> PartialEq<Bytes> for &'a [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl<'a> PartialEq<Bytes> for &'a str {
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
        BytesMut::from(&self[..])
    }
}
