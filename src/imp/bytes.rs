use buf::Iter;
use debug;
use imp::{Inner, RefCount, Shared, SharedPtr, INLINE_CAP, KIND_INLINE};
use {Buf, BufMut, IntoBuf};

use std::borrow::{Borrow, BorrowMut};
use std::io::Cursor;
use std::iter::{FromIterator, Iterator};
use std::sync::atomic::Ordering::{Acquire, Release};
use std::sync::atomic::{self, AtomicPtr, AtomicUsize, Ordering};
use std::{cmp, fmt, hash, mem, ops};

/// A reference counted contiguous slice of memory.
///
/// `Bytes` is an efficient container for storing and operating on contiguous
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
/// let mut mem = Bytes::from(&b"Hello world"[..]);
/// let a = mem.slice(0, 5);
///
/// assert_eq!(&a[..], b"Hello");
///
/// let b = mem.split_to(6);
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
/// holding a unique view to a region of memory does not mean that there are no
/// other `Bytes` and `BytesMut` handles with disjoint views of the underlying
/// memory.
///
/// # Inline bytes
///
/// As an optimization, when the slice referenced by a `Bytes` or `BytesMut`
/// handle is small enough [^1], `with_capacity` will avoid the allocation
/// by inlining the slice directly in the handle. In this case, a clone is no
/// longer "shallow" and the data will be copied.  Converting from a `Vec` will
/// never use inlining.
///
/// [^1]: Small enough: 31 bytes on 64 bit systems, 15 on 32 bit systems.
///
pub struct Bytes {
    inner: Inner<AtomicPtr<Shared<AtomicUsize>>>,
}

/// A unique reference to a contiguous slice of memory.
///
/// `BytesMut` represents a unique view into a potentially shared memory region.
/// Given the uniqueness guarantee, owners of `BytesMut` handles are able to
/// mutate the memory. It is similar to a `Vec<u8>` but with less copies and
/// allocations.
///
/// For more detail, see [Bytes](struct.Bytes.html).
///
/// # Growth
///
/// One key difference from `Vec<u8>` is that most operations **do not
/// implicitly grow the buffer**. This means that calling `my_bytes.put("hello
/// world");` could panic if `my_bytes` does not have enough capacity. Before
/// writing to the buffer, ensure that there is enough remaining capacity by
/// calling `my_bytes.remaining_mut()`. In general, avoiding calls to `reserve`
/// is preferable.
///
/// The only exception is `extend` which implicitly reserves required capacity.
///
/// # Examples
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
    inner: Inner<AtomicPtr<Shared<AtomicUsize>>>,
}

/*
 *
 * ===== Bytes =====
 *
 */

impl Bytes {
    /// Creates a new `Bytes` with the specified capacity.
    ///
    /// The returned `Bytes` will be able to hold at least `capacity` bytes
    /// without reallocating. If `capacity` is under `4 * size_of::<usize>() - 1`,
    /// then `BytesMut` will not allocate.
    ///
    /// It is important to note that this function does not specify the length
    /// of the returned `Bytes`, but only the capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut bytes = Bytes::with_capacity(64);
    ///
    /// // `bytes` contains no data, even though there is capacity
    /// assert_eq!(bytes.len(), 0);
    ///
    /// bytes.extend_from_slice(&b"hello world"[..]);
    ///
    /// assert_eq!(&bytes[..], b"hello world");
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> Bytes {
        Bytes {
            inner: Inner::with_capacity(capacity),
        }
    }

    /// Creates a new empty `Bytes`.
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
        Bytes::with_capacity(0)
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
            inner: Inner::from_static(bytes),
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
    #[inline]
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
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Return true if the `Bytes` uses inline allocation
    ///
    /// # Examples
    /// ```
    /// use bytes::Bytes;
    ///
    /// assert!(Bytes::with_capacity(4).is_inline());
    /// assert!(!Bytes::from(Vec::with_capacity(4)).is_inline());
    /// assert!(!Bytes::with_capacity(1024).is_inline());
    /// ```
    pub fn is_inline(&self) -> bool {
        self.inner.is_inline()
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
        assert!(begin <= end);
        assert!(end <= self.len());

        if end - begin <= INLINE_CAP {
            return Bytes::from(&self[begin..end]);
        }

        let mut ret = self.clone();

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
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut a = Bytes::from(&b"hello world"[..]);
    /// let b = a.split_off(5);
    ///
    /// assert_eq!(&a[..], b"hello");
    /// assert_eq!(&b[..], b" world");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    pub fn split_off(&mut self, at: usize) -> Bytes {
        assert!(at <= self.len());

        if at == self.len() {
            return Bytes::new();
        }

        if at == 0 {
            return mem::replace(self, Bytes::new());
        }

        Bytes {
            inner: self.inner.split_off(at),
        }
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned
    /// `Bytes` contains elements `[0, at)`.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut a = Bytes::from(&b"hello world"[..]);
    /// let b = a.split_to(5);
    ///
    /// assert_eq!(&a[..], b" world");
    /// assert_eq!(&b[..], b"hello");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    pub fn split_to(&mut self, at: usize) -> Bytes {
        assert!(at <= self.len());

        if at == self.len() {
            return mem::replace(self, Bytes::new());
        }

        if at == 0 {
            return Bytes::new();
        }

        Bytes {
            inner: self.inner.split_to(at),
        }
    }

    #[deprecated(since = "0.4.1", note = "use split_to instead")]
    #[doc(hidden)]
    pub fn drain_to(&mut self, at: usize) -> Bytes {
        self.split_to(at)
    }

    /// Shortens the buffer, keeping the first `len` bytes and dropping the
    /// rest.
    ///
    /// If `len` is greater than the buffer's current length, this has no
    /// effect.
    ///
    /// The [`split_off`] method can emulate `truncate`, but this causes the
    /// excess bytes to be returned instead of dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut buf = Bytes::from(&b"hello world"[..]);
    /// buf.truncate(5);
    /// assert_eq!(buf, b"hello"[..]);
    /// ```
    ///
    /// [`split_off`]: #method.split_off
    pub fn truncate(&mut self, len: usize) {
        self.inner.truncate(len);
    }

    /// Shortens the buffer, dropping the first `cnt` bytes and keeping the
    /// rest.
    ///
    /// This is the same function as `Buf::advance`, and in the next breaking
    /// release of `bytes`, this implementation will be removed in favor of
    /// having `Bytes` implement `Buf`.
    ///
    /// # Panics
    ///
    /// This function panics if `cnt` is greater than `self.len()`
    #[inline]
    pub fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.len(), "cannot advance past `remaining`");
        unsafe {
            self.inner.set_start(cnt);
        }
    }

    /// Clears the buffer, removing all data.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut buf = Bytes::from(&b"hello world"[..]);
    /// buf.clear();
    /// assert!(buf.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Attempts to convert into a `BytesMut` handle.
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

    /// Acquires a mutable reference to the owned form of the data.
    ///
    /// Clones the data if it is not already owned.
    pub fn to_mut(&mut self) -> &mut BytesMut {
        if !self.inner.is_mut_safe() {
            let new = Bytes::from(&self[..]);
            *self = new;
        }
        unsafe { &mut *(self as *mut Bytes as *mut BytesMut) }
    }

    /// Appends given bytes to this object.
    ///
    /// If this `Bytes` object has not enough capacity, it is resized first.
    /// If it is shared (`refcount > 1`), it is copied first.
    ///
    /// This operation can be less effective than the similar operation on
    /// `BytesMut`, especially on small additions.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut buf = Bytes::from("aabb");
    /// buf.extend_from_slice(b"ccdd");
    /// buf.extend_from_slice(b"eeff");
    ///
    /// assert_eq!(b"aabbccddeeff", &buf[..]);
    /// ```
    pub fn extend_from_slice(&mut self, extend: &[u8]) {
        if extend.is_empty() {
            return;
        }

        let new_cap = self
            .len()
            .checked_add(extend.len())
            .expect("capacity overflow");

        let result = match mem::replace(self, Bytes::new()).try_mut() {
            Ok(mut bytes_mut) => {
                bytes_mut.extend_from_slice(extend);
                bytes_mut
            }
            Err(bytes) => {
                let mut bytes_mut = BytesMut::with_capacity(new_cap);
                bytes_mut.put_slice(&bytes);
                bytes_mut.put_slice(extend);
                bytes_mut
            }
        };

        mem::replace(self, result.freeze());
    }

    /// Combine splitted Bytes objects back as contiguous.
    ///
    /// If `Bytes` objects were not contiguous originally, they will be extended.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut buf = Bytes::with_capacity(64);
    /// buf.extend_from_slice(b"aaabbbcccddd");
    ///
    /// let splitted = buf.split_off(6);
    /// assert_eq!(b"aaabbb", &buf[..]);
    /// assert_eq!(b"cccddd", &splitted[..]);
    ///
    /// buf.unsplit(splitted);
    /// assert_eq!(b"aaabbbcccddd", &buf[..]);
    /// ```
    pub fn unsplit(&mut self, other: Bytes) {
        if self.is_empty() {
            *self = other;
            return;
        }

        if let Err(other_inner) = self.inner.try_unsplit(other.inner) {
            self.extend_from_slice(other_inner.as_ref());
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
        Bytes {
            inner: unsafe { self.inner.shallow_clone(false) },
        }
    }
}

impl AsRef<[u8]> for Bytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl ops::Deref for Bytes {
    type Target = [u8];

    #[inline]
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
    /// Convert a `Vec` into a `Bytes`
    ///
    /// This constructor may be used to avoid the inlining optimization used by
    /// `with_capacity`.  A `Bytes` constructed this way will always store its
    /// data on the heap.
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

impl FromIterator<u8> for BytesMut {
    fn from_iter<T: IntoIterator<Item = u8>>(into_iter: T) -> Self {
        let iter = into_iter.into_iter();
        let (min, maybe_max) = iter.size_hint();

        let mut out = BytesMut::with_capacity(maybe_max.unwrap_or(min));

        for i in iter {
            out.reserve(1);
            out.put(i);
        }

        out
    }
}

impl FromIterator<u8> for Bytes {
    fn from_iter<T: IntoIterator<Item = u8>>(into_iter: T) -> Self {
        BytesMut::from_iter(into_iter).freeze()
    }
}

impl PartialEq for Bytes {
    fn eq(&self, other: &Bytes) -> bool {
        self.inner.as_ref() == other.inner.as_ref()
    }
}

impl PartialOrd for Bytes {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other.inner.as_ref())
    }
}

impl Ord for Bytes {
    fn cmp(&self, other: &Bytes) -> cmp::Ordering {
        self.inner.as_ref().cmp(other.inner.as_ref())
    }
}

impl Eq for Bytes {}

impl Default for Bytes {
    #[inline]
    fn default() -> Bytes {
        Bytes::new()
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&debug::BsDebug(&self.inner.as_ref()), fmt)
    }
}

impl hash::Hash for Bytes {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        let s: &[u8] = self.as_ref();
        s.hash(state);
    }
}

impl Borrow<[u8]> for Bytes {
    fn borrow(&self) -> &[u8] {
        self.as_ref()
    }
}

impl IntoIterator for Bytes {
    type Item = u8;
    type IntoIter = Iter<Cursor<Bytes>>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_buf().iter()
    }
}

impl<'a> IntoIterator for &'a Bytes {
    type Item = u8;
    type IntoIter = Iter<Cursor<&'a Bytes>>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_buf().iter()
    }
}

impl Extend<u8> for Bytes {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = u8>,
    {
        let iter = iter.into_iter();

        let (lower, upper) = iter.size_hint();

        // Avoid possible conversion into mut if there's nothing to add
        if let Some(0) = upper {
            return;
        }

        let mut bytes_mut = match mem::replace(self, Bytes::new()).try_mut() {
            Ok(bytes_mut) => bytes_mut,
            Err(bytes) => {
                let mut bytes_mut = BytesMut::with_capacity(bytes.len() + lower);
                bytes_mut.put_slice(&bytes);
                bytes_mut
            }
        };

        bytes_mut.extend(iter);

        mem::replace(self, bytes_mut.freeze());
    }
}

impl<'a> Extend<&'a u8> for Bytes {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = &'a u8>,
    {
        self.extend(iter.into_iter().map(|b| *b))
    }
}

/*
 *
 * ===== BytesMut =====
 *
 */

impl BytesMut {
    /// Creates a new `BytesMut` with the specified capacity.
    ///
    /// The returned `BytesMut` will be able to hold at least `capacity` bytes
    /// without reallocating. If `capacity` is under `4 * size_of::<usize>() - 1`,
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
        BytesMut {
            inner: Inner::with_capacity(capacity),
        }
    }

    /// Creates a new `BytesMut` with default capacity.
    ///
    /// Resulting object has length 0 and unspecified capacity.
    /// This function does not allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BytesMut, BufMut};
    ///
    /// let mut bytes = BytesMut::new();
    ///
    /// assert_eq!(0, bytes.len());
    ///
    /// bytes.reserve(2);
    /// bytes.put_slice(b"xy");
    ///
    /// assert_eq!(&b"xy"[..], &bytes[..]);
    /// ```
    #[inline]
    pub fn new() -> BytesMut {
        BytesMut::with_capacity(0)
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
        self.inner.is_empty()
    }

    /// Return true if the `BytesMut` uses inline allocation
    ///
    /// # Examples
    /// ```
    /// use bytes::BytesMut;
    ///
    /// assert!(BytesMut::with_capacity(4).is_inline());
    /// assert!(!BytesMut::from(Vec::with_capacity(4)).is_inline());
    /// assert!(!BytesMut::with_capacity(1024).is_inline());
    /// ```
    pub fn is_inline(&self) -> bool {
        self.inner.is_inline()
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

    /// Converts `self` into an immutable `Bytes`.
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
    /// This is an `O(1)` operation that just increases the reference count
    /// and sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut a = BytesMut::from(&b"hello world"[..]);
    /// let mut b = a.split_off(5);
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
    /// Panics if `at > capacity`.
    pub fn split_off(&mut self, at: usize) -> BytesMut {
        BytesMut {
            inner: self.inner.split_off(at),
        }
    }

    /// Removes the bytes from the current view, returning them in a new
    /// `BytesMut` handle.
    ///
    /// Afterwards, `self` will be empty, but will retain any additional
    /// capacity that it had before the operation. This is identical to
    /// `self.split_to(self.len())`.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BytesMut, BufMut};
    ///
    /// let mut buf = BytesMut::with_capacity(1024);
    /// buf.put(&b"hello world"[..]);
    ///
    /// let other = buf.take();
    ///
    /// assert!(buf.is_empty());
    /// assert_eq!(1013, buf.capacity());
    ///
    /// assert_eq!(other, b"hello world"[..]);
    /// ```
    pub fn take(&mut self) -> BytesMut {
        let len = self.len();
        self.split_to(len)
    }

    #[deprecated(since = "0.4.1", note = "use take instead")]
    #[doc(hidden)]
    pub fn drain(&mut self) -> BytesMut {
        self.take()
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned `BytesMut`
    /// contains elements `[0, at)`.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut a = BytesMut::from(&b"hello world"[..]);
    /// let mut b = a.split_to(5);
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
    /// Panics if `at > len`.
    pub fn split_to(&mut self, at: usize) -> BytesMut {
        BytesMut {
            inner: self.inner.split_to(at),
        }
    }

    #[deprecated(since = "0.4.1", note = "use split_to instead")]
    #[doc(hidden)]
    pub fn drain_to(&mut self, at: usize) -> BytesMut {
        self.split_to(at)
    }

    /// Shortens the buffer, keeping the first `len` bytes and dropping the
    /// rest.
    ///
    /// If `len` is greater than the buffer's current length, this has no
    /// effect.
    ///
    /// The [`split_off`] method can emulate `truncate`, but this causes the
    /// excess bytes to be returned instead of dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut buf = BytesMut::from(&b"hello world"[..]);
    /// buf.truncate(5);
    /// assert_eq!(buf, b"hello"[..]);
    /// ```
    ///
    /// [`split_off`]: #method.split_off
    pub fn truncate(&mut self, len: usize) {
        self.inner.truncate(len);
    }

    /// Shortens the buffer, dropping the first `cnt` bytes and keeping the
    /// rest.
    ///
    /// This is the same function as `Buf::advance`, and in the next breaking
    /// release of `bytes`, this implementation will be removed in favor of
    /// having `BytesMut` implement `Buf`.
    ///
    /// # Panics
    ///
    /// This function panics if `cnt` is greater than `self.len()`
    #[inline]
    pub fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.len(), "cannot advance past `remaining`");
        unsafe {
            self.inner.set_start(cnt);
        }
    }

    /// Clears the buffer, removing all data.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut buf = BytesMut::from(&b"hello world"[..]);
    /// buf.clear();
    /// assert!(buf.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Resizes the buffer so that `len` is equal to `new_len`.
    ///
    /// If `new_len` is greater than `len`, the buffer is extended by the
    /// difference with each additional byte set to `value`. If `new_len` is
    /// less than `len`, the buffer is simply truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut buf = BytesMut::new();
    ///
    /// buf.resize(3, 0x1);
    /// assert_eq!(&buf[..], &[0x1, 0x1, 0x1]);
    ///
    /// buf.resize(2, 0x2);
    /// assert_eq!(&buf[..], &[0x1, 0x1]);
    ///
    /// buf.resize(4, 0x3);
    /// assert_eq!(&buf[..], &[0x1, 0x1, 0x3, 0x3]);
    /// ```
    pub fn resize(&mut self, new_len: usize, value: u8) {
        self.inner.resize(new_len, value);
    }

    /// Sets the length of the buffer.
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
    /// Before allocating new buffer space, the function will attempt to reclaim
    /// space in the existing buffer. If the current handle references a small
    /// view in the original buffer and all other handles have been dropped,
    /// and the requested capacity is less than or equal to the existing
    /// buffer's capacity, then the current view will be copied to the front of
    /// the buffer and the handle will take ownership of the full buffer.
    ///
    /// # Examples
    ///
    /// In the following example, a new buffer is allocated.
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut buf = BytesMut::from(&b"hello"[..]);
    /// buf.reserve(64);
    /// assert!(buf.capacity() >= 69);
    /// ```
    ///
    /// In the following example, the existing buffer is reclaimed.
    ///
    /// ```
    /// use bytes::{BytesMut, BufMut};
    ///
    /// let mut buf = BytesMut::with_capacity(128);
    /// buf.put(&[0; 64][..]);
    ///
    /// let ptr = buf.as_ptr();
    /// let other = buf.take();
    ///
    /// assert!(buf.is_empty());
    /// assert_eq!(buf.capacity(), 64);
    ///
    /// drop(other);
    /// buf.reserve(128);
    ///
    /// assert_eq!(buf.capacity(), 128);
    /// assert_eq!(buf.as_ptr(), ptr);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`.
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    /// Appends given bytes to this object.
    ///
    /// If this `BytesMut` object has not enough capacity, it is resized first.
    /// So unlike `put_slice` operation, `extend_from_slice` does not panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut buf = BytesMut::with_capacity(0);
    /// buf.extend_from_slice(b"aaabbb");
    /// buf.extend_from_slice(b"cccddd");
    ///
    /// assert_eq!(b"aaabbbcccddd", &buf[..]);
    /// ```
    pub fn extend_from_slice(&mut self, extend: &[u8]) {
        self.reserve(extend.len());
        self.put_slice(extend);
    }

    /// Combine splitted BytesMut objects back as contiguous.
    ///
    /// If `BytesMut` objects were not contiguous originally, they will be extended.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut buf = BytesMut::with_capacity(64);
    /// buf.extend_from_slice(b"aaabbbcccddd");
    ///
    /// let splitted = buf.split_off(6);
    /// assert_eq!(b"aaabbb", &buf[..]);
    /// assert_eq!(b"cccddd", &splitted[..]);
    ///
    /// buf.unsplit(splitted);
    /// assert_eq!(b"aaabbbcccddd", &buf[..]);
    /// ```
    pub fn unsplit(&mut self, other: BytesMut) {
        if self.is_empty() {
            *self = other;
            return;
        }

        if let Err(other_inner) = self.inner.try_unsplit(other.inner) {
            self.extend_from_slice(other_inner.as_ref());
        }
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

        // This call will panic if `cnt` is too big
        self.inner.set_len(new_len);
    }

    #[inline]
    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        let len = self.len();

        // This will never panic as `len` can never become invalid
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

    #[inline]
    fn put_u8(&mut self, n: u8) {
        self.inner.put_u8(n);
    }

    #[inline]
    fn put_i8(&mut self, n: i8) {
        self.put_u8(n as u8);
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
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl ops::Deref for BytesMut {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}

impl AsMut<[u8]> for BytesMut {
    fn as_mut(&mut self) -> &mut [u8] {
        self.inner.as_mut()
    }
}

impl ops::DerefMut for BytesMut {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        self.inner.as_mut()
    }
}

impl From<Vec<u8>> for BytesMut {
    /// Convert a `Vec` into a `BytesMut`
    ///
    /// This constructor may be used to avoid the inlining optimization used by
    /// `with_capacity`.  A `BytesMut` constructed this way will always store
    /// its data on the heap.
    fn from(src: Vec<u8>) -> BytesMut {
        BytesMut {
            inner: Inner::from_vec(src),
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
        let len = src.len();

        if len == 0 {
            BytesMut::new()
        } else if len <= INLINE_CAP {
            unsafe {
                let mut inner: Inner<AtomicPtr<Shared<AtomicUsize>>> = mem::uninitialized();

                // Set inline mask
                inner.arc = AtomicPtr::new(KIND_INLINE as *mut Shared<AtomicUsize>);
                inner.set_inline_len(len);
                inner.as_raw()[0..len].copy_from_slice(src);

                BytesMut { inner }
            }
        } else {
            BytesMut::from(src.to_vec())
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
        src.try_mut().unwrap_or_else(|src| BytesMut::from(&src[..]))
    }
}

impl PartialEq for BytesMut {
    fn eq(&self, other: &BytesMut) -> bool {
        self.inner.as_ref() == other.inner.as_ref()
    }
}

impl PartialOrd for BytesMut {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other.inner.as_ref())
    }
}

impl Ord for BytesMut {
    fn cmp(&self, other: &BytesMut) -> cmp::Ordering {
        self.inner.as_ref().cmp(other.inner.as_ref())
    }
}

impl Eq for BytesMut {}

impl Default for BytesMut {
    #[inline]
    fn default() -> BytesMut {
        BytesMut::new()
    }
}

impl fmt::Debug for BytesMut {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&debug::BsDebug(&self.inner.as_ref()), fmt)
    }
}

impl hash::Hash for BytesMut {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        let s: &[u8] = self.as_ref();
        s.hash(state);
    }
}

impl Borrow<[u8]> for BytesMut {
    fn borrow(&self) -> &[u8] {
        self.as_ref()
    }
}

impl BorrowMut<[u8]> for BytesMut {
    fn borrow_mut(&mut self) -> &mut [u8] {
        self.as_mut()
    }
}

impl fmt::Write for BytesMut {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.remaining_mut() >= s.len() {
            self.put_slice(s.as_bytes());
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }

    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self, args)
    }
}

impl Clone for BytesMut {
    fn clone(&self) -> BytesMut {
        BytesMut::from(&self[..])
    }
}

impl IntoIterator for BytesMut {
    type Item = u8;
    type IntoIter = Iter<Cursor<BytesMut>>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_buf().iter()
    }
}

impl<'a> IntoIterator for &'a BytesMut {
    type Item = u8;
    type IntoIter = Iter<Cursor<&'a BytesMut>>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_buf().iter()
    }
}

impl Extend<u8> for BytesMut {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = u8>,
    {
        let iter = iter.into_iter();

        let (lower, _) = iter.size_hint();
        self.reserve(lower);

        for b in iter {
            unsafe {
                self.bytes_mut()[0] = b;
                self.advance_mut(1);
            }
        }
    }
}

impl<'a> Extend<&'a u8> for BytesMut {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = &'a u8>,
    {
        self.extend(iter.into_iter().map(|b| *b))
    }
}

/*
 *
 * ===== PartialEq / PartialOrd =====
 *
 */

impl PartialEq<[u8]> for BytesMut {
    fn eq(&self, other: &[u8]) -> bool {
        &**self == other
    }
}

impl PartialOrd<[u8]> for BytesMut {
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other)
    }
}

impl PartialEq<BytesMut> for [u8] {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for [u8] {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<str> for BytesMut {
    fn eq(&self, other: &str) -> bool {
        &**self == other.as_bytes()
    }
}

impl PartialOrd<str> for BytesMut {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other.as_bytes())
    }
}

impl PartialEq<BytesMut> for str {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for str {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<Vec<u8>> for BytesMut {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == &other[..]
    }
}

impl PartialOrd<Vec<u8>> for BytesMut {
    fn partial_cmp(&self, other: &Vec<u8>) -> Option<cmp::Ordering> {
        (**self).partial_cmp(&other[..])
    }
}

impl PartialEq<BytesMut> for Vec<u8> {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for Vec<u8> {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<String> for BytesMut {
    fn eq(&self, other: &String) -> bool {
        *self == &other[..]
    }
}

impl PartialOrd<String> for BytesMut {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other.as_bytes())
    }
}

impl PartialEq<BytesMut> for String {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for String {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for BytesMut
where
    BytesMut: PartialEq<T>,
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a, T: ?Sized> PartialOrd<&'a T> for BytesMut
where
    BytesMut: PartialOrd<T>,
{
    fn partial_cmp(&self, other: &&'a T) -> Option<cmp::Ordering> {
        self.partial_cmp(*other)
    }
}

impl<'a> PartialEq<BytesMut> for &'a [u8] {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl<'a> PartialOrd<BytesMut> for &'a [u8] {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a> PartialEq<BytesMut> for &'a str {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl<'a> PartialOrd<BytesMut> for &'a str {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<[u8]> for Bytes {
    fn eq(&self, other: &[u8]) -> bool {
        self.inner.as_ref() == other
    }
}

impl PartialOrd<[u8]> for Bytes {
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other)
    }
}

impl PartialEq<Bytes> for [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for [u8] {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<str> for Bytes {
    fn eq(&self, other: &str) -> bool {
        self.inner.as_ref() == other.as_bytes()
    }
}

impl PartialOrd<str> for Bytes {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other.as_bytes())
    }
}

impl PartialEq<Bytes> for str {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for str {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<Vec<u8>> for Bytes {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == &other[..]
    }
}

impl PartialOrd<Vec<u8>> for Bytes {
    fn partial_cmp(&self, other: &Vec<u8>) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(&other[..])
    }
}

impl PartialEq<Bytes> for Vec<u8> {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for Vec<u8> {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<String> for Bytes {
    fn eq(&self, other: &String) -> bool {
        *self == &other[..]
    }
}

impl PartialOrd<String> for Bytes {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other.as_bytes())
    }
}

impl PartialEq<Bytes> for String {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for String {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a> PartialEq<Bytes> for &'a [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl<'a> PartialOrd<Bytes> for &'a [u8] {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a> PartialEq<Bytes> for &'a str {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl<'a> PartialOrd<Bytes> for &'a str {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for Bytes
where
    Bytes: PartialEq<T>,
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a, T: ?Sized> PartialOrd<&'a T> for Bytes
where
    Bytes: PartialOrd<T>,
{
    fn partial_cmp(&self, other: &&'a T) -> Option<cmp::Ordering> {
        self.partial_cmp(&**other)
    }
}

impl PartialEq<BytesMut> for Bytes {
    fn eq(&self, other: &BytesMut) -> bool {
        &other[..] == &self[..]
    }
}

impl PartialEq<Bytes> for BytesMut {
    fn eq(&self, other: &Bytes) -> bool {
        &other[..] == &self[..]
    }
}

/*
 *
 * ===== AtomicUsize =====
 *
 */

impl RefCount for AtomicUsize {
    #[inline]
    fn new(val: usize) -> Self {
        AtomicUsize::new(val)
    }

    #[inline]
    fn fetch_inc(&mut self, order: Ordering) -> usize {
        self.fetch_add(1, order)
    }

    #[inline]
    fn release_shared(ptr: *mut Shared<Self>) {
        // `Shared` storage... follow the drop steps from Arc.
        unsafe {
            if (*ptr).ref_count.fetch_sub(1, Release) != 1 {
                return;
            }

            // This fence is needed to prevent reordering of use of the data and
            // deletion of the data.  Because it is marked `Release`, the decreasing
            // of the reference count synchronizes with this `Acquire` fence. This
            // means that use of the data happens before decreasing the reference
            // count, which happens before this fence, which happens before the
            // deletion of the data.
            //
            // As explained in the [Boost documentation][1],
            //
            // > It is important to enforce any possible access to the object in one
            // > thread (through an existing reference) to *happen before* deleting
            // > the object in a different thread. This is achieved by a "release"
            // > operation after dropping a reference (any access to the object
            // > through this reference must obviously happened before), and an
            // > "acquire" operation before deleting the object.
            //
            // [1]: (www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html)
            atomic::fence(Acquire);

            // Drop the data
            drop(Box::from_raw(ptr));
        }
    }

    #[inline]
    fn load(&self, order: Ordering) -> usize {
        self.load(order)
    }
}

impl SharedPtr for AtomicPtr<Shared<AtomicUsize>> {
    type RefCount = AtomicUsize;

    #[inline]
    fn new(ptr: *mut Shared<Self::RefCount>) -> Self {
        AtomicPtr::new(ptr)
    }

    #[inline]
    fn get_mut(&mut self) -> &mut *mut Shared<Self::RefCount> {
        self.get_mut()
    }

    #[inline]
    fn load(&self, order: Ordering) -> *mut Shared<Self::RefCount> {
        self.load(order)
    }

    #[inline]
    fn store(&self, ptr: *mut Shared<Self::RefCount>, order: Ordering) {
        self.store(ptr, order);
    }

    #[inline]
    fn compare_and_swap(
        &self,
        current: *mut Shared<Self::RefCount>,
        new: *mut Shared<Self::RefCount>,
        order: Ordering,
    ) -> *mut Shared<Self::RefCount> {
        self.compare_and_swap(current, new, order)
    }
}
