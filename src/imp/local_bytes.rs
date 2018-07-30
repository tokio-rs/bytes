use buf::Iter;
use debug;
use imp::{Inner, RefCount, Shared, SharedPtr, INLINE_CAP, KIND_INLINE};
use {Buf, BufMut, IntoBuf};

use std::borrow::{Borrow, BorrowMut};
use std::cell::UnsafeCell;
use std::io::Cursor;
use std::iter::{FromIterator, Iterator};
use std::sync::atomic::Ordering;
use std::{cmp, fmt, hash, mem, ops};

/// A reference counted contiguous slice of memory.
///
/// `LocalBytes` is an efficient container for storing and operating on contiguous
/// slices of memory. It is intended for use primarily in networking code, but
/// could have applications elsewhere as well.
///
/// `LocalBytes` values facilitate zero-copy network programming by allowing multiple
/// `LocalBytes` objects to point to the same underlying memory. This is managed by
/// using a reference count to track when the memory is no longer needed and can
/// be freed.
///
/// ```
/// use bytes::LocalBytes;
///
/// let mut mem = LocalBytes::from(&b"Hello world"[..]);
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
/// The `LocalBytes` struct itself is fairly small, limited to a pointer to the
/// memory and 4 `usize` fields used to track information about which segment of
/// the underlying memory the `LocalBytes` handle has access to.
///
/// The memory layout looks like this:
///
/// ```text
/// +------------+
/// | LocalBytes |
/// +------------+
///  /           \_____
/// |                  \
/// v                   v
/// +----+-----------------------------------------+
/// | Rc |              |      Data     |          |
/// +----+-----------------------------------------+
/// ```
///
/// `LocalBytes` keeps both a pointer to the shared `Arc` containing the full memory
/// slice and a pointer to the start of the region visible by the handle.
/// `LocalBytes` also tracks the length of its view into the memory.
///
/// # Sharing
///
/// The memory itself is reference counted, and multiple `LocalBytes` objects may
/// point to the same region. Each `LocalBytes` handle point to different sections within
/// the memory region, and `LocalBytes` handle may or may not have overlapping views
/// into the memory.
///
///
/// ```text
///
///    Rc ptrs                         +--------------+
///    ______________________________/ | LocalBytes 2 |
///   /                                +--------------+
///  /          +----------------+     |              |
/// |_________/ |  LocalBytes 1  |     |              |
/// |           +----------------+     |              |
/// |           |                | ___/ data          | tail
/// |      data |           tail |/                   |
/// v           v                v                    v
/// +----+--------------------------------------------+-----+
/// | Rc |      |                |                    |     |
/// +----+--------------------------------------------+-----+
/// ```
///
/// # Mutating
///
/// While `LocalBytes` handles may potentially represent overlapping views of the
/// underlying memory slice and may not be mutated, `LocalBytesMut` handles are
/// guaranteed to be the only handle able to view that slice of memory. As such,
/// `LocalBytesMut` handles are able to mutate the underlying memory. Note that
/// holding a unique view to a region of memory does not mean that there are no
/// other `LocalBytes` and `LocalBytesMut` handles with disjoint views of the underlying
/// memory.
///
/// # Inline bytes
///
/// As an optimization, when the slice referenced by a `LocalBytes` or `LocalBytesMut`
/// handle is small enough [^1], `with_capacity` will avoid the allocation
/// by inlining the slice directly in the handle. In this case, a clone is no
/// longer "shallow" and the data will be copied.  Converting from a `Vec` will
/// never use inlining.
///
/// [^1]: Small enough: 31 bytes on 64 bit systems, 15 on 32 bit systems.
///
pub struct LocalBytes {
    inner: Inner<UnsafeCell<*mut Shared<usize>>>,
}

/// A unique reference to a contiguous slice of memory.
///
/// `LocalBytesMut` represents a unique view into a potentially shared memory region.
/// Given the uniqueness guarantee, owners of `LocalBytesMut` handles are able to
/// mutate the memory. It is similar to a `Vec<u8>` but with less copies and
/// allocations.
///
/// For more detail, see [LocalBytes](struct.LocalBytes.html).
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
/// use bytes::{LocalBytesMut, BufMut};
///
/// let mut buf = LocalBytesMut::with_capacity(64);
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
pub struct LocalBytesMut {
    inner: Inner<UnsafeCell<*mut Shared<usize>>>,
}

/*
 *
 * ===== LocalBytes =====
 *
 */

impl LocalBytes {
    /// Creates a new `LocalBytes` with the specified capacity.
    ///
    /// The returned `LocalBytes` will be able to hold at least `capacity` bytes
    /// without reallocating. If `capacity` is under `4 * size_of::<usize>() - 1`,
    /// then `LocalBytesMut` will not allocate.
    ///
    /// It is important to note that this function does not specify the length
    /// of the returned `LocalBytes`, but only the capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let mut bytes = LocalBytes::with_capacity(64);
    ///
    /// // `bytes` contains no data, even though there is capacity
    /// assert_eq!(bytes.len(), 0);
    ///
    /// bytes.extend_from_slice(&b"hello world"[..]);
    ///
    /// assert_eq!(&bytes[..], b"hello world");
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> LocalBytes {
        LocalBytes {
            inner: Inner::with_capacity(capacity),
        }
    }

    /// Creates a new empty `LocalBytes`.
    ///
    /// This will not allocate and the returned `LocalBytes` handle will be empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let b = LocalBytes::new();
    /// assert_eq!(&b[..], b"");
    /// ```
    #[inline]
    pub fn new() -> LocalBytes {
        LocalBytes::with_capacity(0)
    }

    /// Creates a new `LocalBytes` from a static slice.
    ///
    /// The returned `LocalBytes` will point directly to the static slice. There is
    /// no allocating or copying.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let b = LocalBytes::from_static(b"hello");
    /// assert_eq!(&b[..], b"hello");
    /// ```
    #[inline]
    pub fn from_static(bytes: &'static [u8]) -> LocalBytes {
        LocalBytes {
            inner: Inner::from_static(bytes),
        }
    }

    /// Returns the number of bytes contained in this `LocalBytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let b = LocalBytes::from(&b"hello"[..]);
    /// assert_eq!(b.len(), 5);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the `LocalBytes` has a length of 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let b = LocalBytes::new();
    /// assert!(b.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Return true if the `LocalBytes` uses inline allocation
    ///
    /// # Examples
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// assert!(LocalBytes::with_capacity(4).is_inline());
    /// assert!(!LocalBytes::from(Vec::with_capacity(4)).is_inline());
    /// assert!(!LocalBytes::with_capacity(1024).is_inline());
    /// ```
    pub fn is_inline(&self) -> bool {
        self.inner.is_inline()
    }

    /// Returns a slice of self for the index range `[begin..end)`.
    ///
    /// This will increment the reference count for the underlying memory and
    /// return a new `LocalBytes` handle set to the slice.
    ///
    /// This operation is `O(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let a = LocalBytes::from(&b"hello world"[..]);
    /// let b = a.slice(2, 5);
    ///
    /// assert_eq!(&b[..], b"llo");
    /// ```
    ///
    /// # Panics
    ///
    /// Requires that `begin <= end` and `end <= self.len()`, otherwise slicing
    /// will panic.
    pub fn slice(&self, begin: usize, end: usize) -> LocalBytes {
        assert!(begin <= end);
        assert!(end <= self.len());

        if end - begin <= INLINE_CAP {
            return LocalBytes::from(&self[begin..end]);
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
    /// return a new `LocalBytes` handle set to the slice.
    ///
    /// This operation is `O(1)` and is equivalent to `self.slice(begin,
    /// self.len())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let a = LocalBytes::from(&b"hello world"[..]);
    /// let b = a.slice_from(6);
    ///
    /// assert_eq!(&b[..], b"world");
    /// ```
    ///
    /// # Panics
    ///
    /// Requires that `begin <= self.len()`, otherwise slicing will panic.
    pub fn slice_from(&self, begin: usize) -> LocalBytes {
        self.slice(begin, self.len())
    }

    /// Returns a slice of self for the index range `[0..end)`.
    ///
    /// This will increment the reference count for the underlying memory and
    /// return a new `LocalBytes` handle set to the slice.
    ///
    /// This operation is `O(1)` and is equivalent to `self.slice(0, end)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let a = LocalBytes::from(&b"hello world"[..]);
    /// let b = a.slice_to(5);
    ///
    /// assert_eq!(&b[..], b"hello");
    /// ```
    ///
    /// # Panics
    ///
    /// Requires that `end <= self.len()`, otherwise slicing will panic.
    pub fn slice_to(&self, end: usize) -> LocalBytes {
        self.slice(0, end)
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[0, at)`, and the returned `LocalBytes`
    /// contains elements `[at, len)`.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let mut a = LocalBytes::from(&b"hello world"[..]);
    /// let b = a.split_off(5);
    ///
    /// assert_eq!(&a[..], b"hello");
    /// assert_eq!(&b[..], b" world");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    pub fn split_off(&mut self, at: usize) -> LocalBytes {
        assert!(at <= self.len());

        if at == self.len() {
            return LocalBytes::new();
        }

        if at == 0 {
            return mem::replace(self, LocalBytes::new());
        }

        LocalBytes {
            inner: self.inner.split_off(at),
        }
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned
    /// `LocalBytes` contains elements `[0, at)`.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let mut a = LocalBytes::from(&b"hello world"[..]);
    /// let b = a.split_to(5);
    ///
    /// assert_eq!(&a[..], b" world");
    /// assert_eq!(&b[..], b"hello");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    pub fn split_to(&mut self, at: usize) -> LocalBytes {
        assert!(at <= self.len());

        if at == self.len() {
            return mem::replace(self, LocalBytes::new());
        }

        if at == 0 {
            return LocalBytes::new();
        }

        LocalBytes {
            inner: self.inner.split_to(at),
        }
    }

    #[deprecated(since = "0.4.1", note = "use split_to instead")]
    #[doc(hidden)]
    pub fn drain_to(&mut self, at: usize) -> LocalBytes {
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
    /// use bytes::LocalBytes;
    ///
    /// let mut buf = LocalBytes::from(&b"hello world"[..]);
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
    /// having `LocalBytes` implement `Buf`.
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
    /// use bytes::LocalBytes;
    ///
    /// let mut buf = LocalBytes::from(&b"hello world"[..]);
    /// buf.clear();
    /// assert!(buf.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Attempts to convert into a `LocalBytesMut` handle.
    ///
    /// This will only succeed if there are no other outstanding references to
    /// the underlying chunk of memory. `LocalBytes` handles that contain inlined
    /// bytes will always be convertable to `LocalBytesMut`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let a = LocalBytes::from(&b"Mary had a little lamb, little lamb, little lamb..."[..]);
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
    pub fn try_mut(mut self) -> Result<LocalBytesMut, LocalBytes> {
        if self.inner.is_mut_safe() {
            Ok(LocalBytesMut { inner: self.inner })
        } else {
            Err(self)
        }
    }

    /// Acquires a mutable reference to the owned form of the data.
    ///
    /// Clones the data if it is not already owned.
    pub fn to_mut(&mut self) -> &mut LocalBytesMut {
        if !self.inner.is_mut_safe() {
            let new = LocalBytes::from(&self[..]);
            *self = new;
        }
        unsafe { &mut *(self as *mut LocalBytes as *mut LocalBytesMut) }
    }

    /// Appends given bytes to this object.
    ///
    /// If this `LocalBytes` object has not enough capacity, it is resized first.
    /// If it is shared (`refcount > 1`), it is copied first.
    ///
    /// This operation can be less effective than the similar operation on
    /// `LocalBytesMut`, especially on small additions.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let mut buf = LocalBytes::from("aabb");
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

        let result = match mem::replace(self, LocalBytes::new()).try_mut() {
            Ok(mut bytes_mut) => {
                bytes_mut.extend_from_slice(extend);
                bytes_mut
            }
            Err(bytes) => {
                let mut bytes_mut = LocalBytesMut::with_capacity(new_cap);
                bytes_mut.put_slice(&bytes);
                bytes_mut.put_slice(extend);
                bytes_mut
            }
        };

        mem::replace(self, result.freeze());
    }

    /// Combine splitted LocalBytes objects back as contiguous.
    ///
    /// If `LocalBytes` objects were not contiguous originally, they will be extended.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytes;
    ///
    /// let mut buf = LocalBytes::with_capacity(64);
    /// buf.extend_from_slice(b"aaabbbcccddd");
    ///
    /// let splitted = buf.split_off(6);
    /// assert_eq!(b"aaabbb", &buf[..]);
    /// assert_eq!(b"cccddd", &splitted[..]);
    ///
    /// buf.unsplit(splitted);
    /// assert_eq!(b"aaabbbcccddd", &buf[..]);
    /// ```
    pub fn unsplit(&mut self, other: LocalBytes) {
        if self.is_empty() {
            *self = other;
            return;
        }

        if let Err(other_inner) = self.inner.try_unsplit(other.inner) {
            self.extend_from_slice(other_inner.as_ref());
        }
    }
}

impl IntoBuf for LocalBytes {
    type Buf = Cursor<Self>;

    fn into_buf(self) -> Self::Buf {
        Cursor::new(self)
    }
}

impl<'a> IntoBuf for &'a LocalBytes {
    type Buf = Cursor<Self>;

    fn into_buf(self) -> Self::Buf {
        Cursor::new(self)
    }
}

impl Clone for LocalBytes {
    fn clone(&self) -> LocalBytes {
        LocalBytes {
            inner: unsafe { self.inner.shallow_clone(false) },
        }
    }
}

impl AsRef<[u8]> for LocalBytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl ops::Deref for LocalBytes {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl From<LocalBytesMut> for LocalBytes {
    fn from(src: LocalBytesMut) -> LocalBytes {
        src.freeze()
    }
}

impl From<Vec<u8>> for LocalBytes {
    /// Convert a `Vec` into a `LocalBytes`
    ///
    /// This constructor may be used to avoid the inlining optimization used by
    /// `with_capacity`.  A `LocalBytes` constructed this way will always store its
    /// data on the heap.
    fn from(src: Vec<u8>) -> LocalBytes {
        LocalBytesMut::from(src).freeze()
    }
}

impl From<String> for LocalBytes {
    fn from(src: String) -> LocalBytes {
        LocalBytesMut::from(src).freeze()
    }
}

impl<'a> From<&'a [u8]> for LocalBytes {
    fn from(src: &'a [u8]) -> LocalBytes {
        LocalBytesMut::from(src).freeze()
    }
}

impl<'a> From<&'a str> for LocalBytes {
    fn from(src: &'a str) -> LocalBytes {
        LocalBytesMut::from(src).freeze()
    }
}

impl FromIterator<u8> for LocalBytesMut {
    fn from_iter<T: IntoIterator<Item = u8>>(into_iter: T) -> Self {
        let iter = into_iter.into_iter();
        let (min, maybe_max) = iter.size_hint();

        let mut out = LocalBytesMut::with_capacity(maybe_max.unwrap_or(min));

        for i in iter {
            out.reserve(1);
            out.put(i);
        }

        out
    }
}

impl FromIterator<u8> for LocalBytes {
    fn from_iter<T: IntoIterator<Item = u8>>(into_iter: T) -> Self {
        LocalBytesMut::from_iter(into_iter).freeze()
    }
}

impl PartialEq for LocalBytes {
    fn eq(&self, other: &LocalBytes) -> bool {
        self.inner.as_ref() == other.inner.as_ref()
    }
}

impl PartialOrd for LocalBytes {
    fn partial_cmp(&self, other: &LocalBytes) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other.inner.as_ref())
    }
}

impl Ord for LocalBytes {
    fn cmp(&self, other: &LocalBytes) -> cmp::Ordering {
        self.inner.as_ref().cmp(other.inner.as_ref())
    }
}

impl Eq for LocalBytes {}

impl Default for LocalBytes {
    #[inline]
    fn default() -> LocalBytes {
        LocalBytes::new()
    }
}

impl fmt::Debug for LocalBytes {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&debug::BsDebug(&self.inner.as_ref()), fmt)
    }
}

impl hash::Hash for LocalBytes {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        let s: &[u8] = self.as_ref();
        s.hash(state);
    }
}

impl Borrow<[u8]> for LocalBytes {
    fn borrow(&self) -> &[u8] {
        self.as_ref()
    }
}

impl IntoIterator for LocalBytes {
    type Item = u8;
    type IntoIter = Iter<Cursor<LocalBytes>>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_buf().iter()
    }
}

impl<'a> IntoIterator for &'a LocalBytes {
    type Item = u8;
    type IntoIter = Iter<Cursor<&'a LocalBytes>>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_buf().iter()
    }
}

impl Extend<u8> for LocalBytes {
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

        let mut bytes_mut = match mem::replace(self, LocalBytes::new()).try_mut() {
            Ok(bytes_mut) => bytes_mut,
            Err(bytes) => {
                let mut bytes_mut = LocalBytesMut::with_capacity(bytes.len() + lower);
                bytes_mut.put_slice(&bytes);
                bytes_mut
            }
        };

        bytes_mut.extend(iter);

        mem::replace(self, bytes_mut.freeze());
    }
}

impl<'a> Extend<&'a u8> for LocalBytes {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = &'a u8>,
    {
        self.extend(iter.into_iter().map(|b| *b))
    }
}

/*
 *
 * ===== LocalBytesMut =====
 *
 */

impl LocalBytesMut {
    /// Creates a new `LocalBytesMut` with the specified capacity.
    ///
    /// The returned `LocalBytesMut` will be able to hold at least `capacity` bytes
    /// without reallocating. If `capacity` is under `4 * size_of::<usize>() - 1`,
    /// then `LocalBytesMut` will not allocate.
    ///
    /// It is important to note that this function does not specify the length
    /// of the returned `LocalBytesMut`, but only the capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{LocalBytesMut, BufMut};
    ///
    /// let mut bytes = LocalBytesMut::with_capacity(64);
    ///
    /// // `bytes` contains no data, even though there is capacity
    /// assert_eq!(bytes.len(), 0);
    ///
    /// bytes.put(&b"hello world"[..]);
    ///
    /// assert_eq!(&bytes[..], b"hello world");
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> LocalBytesMut {
        LocalBytesMut {
            inner: Inner::with_capacity(capacity),
        }
    }

    /// Creates a new `LocalBytesMut` with default capacity.
    ///
    /// Resulting object has length 0 and unspecified capacity.
    /// This function does not allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{LocalBytesMut, BufMut};
    ///
    /// let mut bytes = LocalBytesMut::new();
    ///
    /// assert_eq!(0, bytes.len());
    ///
    /// bytes.reserve(2);
    /// bytes.put_slice(b"xy");
    ///
    /// assert_eq!(&b"xy"[..], &bytes[..]);
    /// ```
    #[inline]
    pub fn new() -> LocalBytesMut {
        LocalBytesMut::with_capacity(0)
    }

    /// Returns the number of bytes contained in this `LocalBytesMut`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytesMut;
    ///
    /// let b = LocalBytesMut::from(&b"hello"[..]);
    /// assert_eq!(b.len(), 5);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the `LocalBytesMut` has a length of 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytesMut;
    ///
    /// let b = LocalBytesMut::with_capacity(64);
    /// assert!(b.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Return true if the `LocalBytesMut` uses inline allocation
    ///
    /// # Examples
    /// ```
    /// use bytes::LocalBytesMut;
    ///
    /// assert!(LocalBytesMut::with_capacity(4).is_inline());
    /// assert!(!LocalBytesMut::from(Vec::with_capacity(4)).is_inline());
    /// assert!(!LocalBytesMut::with_capacity(1024).is_inline());
    /// ```
    pub fn is_inline(&self) -> bool {
        self.inner.is_inline()
    }

    /// Returns the number of bytes the `LocalBytesMut` can hold without reallocating.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytesMut;
    ///
    /// let b = LocalBytesMut::with_capacity(64);
    /// assert_eq!(b.capacity(), 64);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Converts `self` into an immutable `LocalBytes`.
    ///
    /// The conversion is zero cost and is used to indicate that the slice
    /// referenced by the handle will no longer be mutated.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{LocalBytesMut, BufMut};
    /// use std::thread;
    ///
    /// let mut b = LocalBytesMut::with_capacity(64);
    /// b.put("hello world");
    /// let b1 = b.freeze();
    /// let b2 = b1.clone();
    ///
    /// assert_eq!(&b1[..], b"hello world");
    ///
    /// assert_eq!(&b2[..], b"hello world");
    /// ```
    #[inline]
    pub fn freeze(self) -> LocalBytes {
        LocalBytes { inner: self.inner }
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[0, at)`, and the returned
    /// `LocalBytesMut` contains elements `[at, capacity)`.
    ///
    /// This is an `O(1)` operation that just increases the reference count
    /// and sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytesMut;
    ///
    /// let mut a = LocalBytesMut::from(&b"hello world"[..]);
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
    pub fn split_off(&mut self, at: usize) -> LocalBytesMut {
        LocalBytesMut {
            inner: self.inner.split_off(at),
        }
    }

    /// Removes the bytes from the current view, returning them in a new
    /// `LocalBytesMut` handle.
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
    /// use bytes::{LocalBytesMut, BufMut};
    ///
    /// let mut buf = LocalBytesMut::with_capacity(1024);
    /// buf.put(&b"hello world"[..]);
    ///
    /// let other = buf.take();
    ///
    /// assert!(buf.is_empty());
    /// assert_eq!(1013, buf.capacity());
    ///
    /// assert_eq!(other, b"hello world"[..]);
    /// ```
    pub fn take(&mut self) -> LocalBytesMut {
        let len = self.len();
        self.split_to(len)
    }

    #[deprecated(since = "0.4.1", note = "use take instead")]
    #[doc(hidden)]
    pub fn drain(&mut self) -> LocalBytesMut {
        self.take()
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned `LocalBytesMut`
    /// contains elements `[0, at)`.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytesMut;
    ///
    /// let mut a = LocalBytesMut::from(&b"hello world"[..]);
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
    pub fn split_to(&mut self, at: usize) -> LocalBytesMut {
        LocalBytesMut {
            inner: self.inner.split_to(at),
        }
    }

    #[deprecated(since = "0.4.1", note = "use split_to instead")]
    #[doc(hidden)]
    pub fn drain_to(&mut self, at: usize) -> LocalBytesMut {
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
    /// use bytes::LocalBytesMut;
    ///
    /// let mut buf = LocalBytesMut::from(&b"hello world"[..]);
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
    /// having `LocalBytesMut` implement `Buf`.
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
    /// use bytes::LocalBytesMut;
    ///
    /// let mut buf = LocalBytesMut::from(&b"hello world"[..]);
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
    /// use bytes::LocalBytesMut;
    ///
    /// let mut buf = LocalBytesMut::new();
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
    /// use bytes::LocalBytesMut;
    ///
    /// let mut b = LocalBytesMut::from(&b"hello world"[..]);
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
    /// into the given `LocalBytesMut`.
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
    /// use bytes::LocalBytesMut;
    ///
    /// let mut buf = LocalBytesMut::from(&b"hello"[..]);
    /// buf.reserve(64);
    /// assert!(buf.capacity() >= 69);
    /// ```
    ///
    /// In the following example, the existing buffer is reclaimed.
    ///
    /// ```
    /// use bytes::{LocalBytesMut, BufMut};
    ///
    /// let mut buf = LocalBytesMut::with_capacity(128);
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
    /// If this `LocalBytesMut` object has not enough capacity, it is resized first.
    /// So unlike `put_slice` operation, `extend_from_slice` does not panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytesMut;
    ///
    /// let mut buf = LocalBytesMut::with_capacity(0);
    /// buf.extend_from_slice(b"aaabbb");
    /// buf.extend_from_slice(b"cccddd");
    ///
    /// assert_eq!(b"aaabbbcccddd", &buf[..]);
    /// ```
    pub fn extend_from_slice(&mut self, extend: &[u8]) {
        self.reserve(extend.len());
        self.put_slice(extend);
    }

    /// Combine splitted LocalBytesMut objects back as contiguous.
    ///
    /// If `LocalBytesMut` objects were not contiguous originally, they will be extended.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::LocalBytesMut;
    ///
    /// let mut buf = LocalBytesMut::with_capacity(64);
    /// buf.extend_from_slice(b"aaabbbcccddd");
    ///
    /// let splitted = buf.split_off(6);
    /// assert_eq!(b"aaabbb", &buf[..]);
    /// assert_eq!(b"cccddd", &splitted[..]);
    ///
    /// buf.unsplit(splitted);
    /// assert_eq!(b"aaabbbcccddd", &buf[..]);
    /// ```
    pub fn unsplit(&mut self, other: LocalBytesMut) {
        if self.is_empty() {
            *self = other;
            return;
        }

        if let Err(other_inner) = self.inner.try_unsplit(other.inner) {
            self.extend_from_slice(other_inner.as_ref());
        }
    }
}

impl BufMut for LocalBytesMut {
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

impl IntoBuf for LocalBytesMut {
    type Buf = Cursor<Self>;

    fn into_buf(self) -> Self::Buf {
        Cursor::new(self)
    }
}

impl<'a> IntoBuf for &'a LocalBytesMut {
    type Buf = Cursor<&'a LocalBytesMut>;

    fn into_buf(self) -> Self::Buf {
        Cursor::new(self)
    }
}

impl AsRef<[u8]> for LocalBytesMut {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl ops::Deref for LocalBytesMut {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}

impl AsMut<[u8]> for LocalBytesMut {
    fn as_mut(&mut self) -> &mut [u8] {
        self.inner.as_mut()
    }
}

impl ops::DerefMut for LocalBytesMut {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        self.inner.as_mut()
    }
}

impl From<Vec<u8>> for LocalBytesMut {
    /// Convert a `Vec` into a `LocalBytesMut`
    ///
    /// This constructor may be used to avoid the inlining optimization used by
    /// `with_capacity`.  A `LocalBytesMut` constructed this way will always store
    /// its data on the heap.
    fn from(src: Vec<u8>) -> LocalBytesMut {
        LocalBytesMut {
            inner: Inner::from_vec(src),
        }
    }
}

impl From<String> for LocalBytesMut {
    fn from(src: String) -> LocalBytesMut {
        LocalBytesMut::from(src.into_bytes())
    }
}

impl<'a> From<&'a [u8]> for LocalBytesMut {
    fn from(src: &'a [u8]) -> LocalBytesMut {
        let len = src.len();

        if len == 0 {
            LocalBytesMut::new()
        } else if len <= INLINE_CAP {
            unsafe {
                let mut inner: Inner<UnsafeCell<*mut Shared<usize>>> = mem::uninitialized();

                // Set inline mask
                inner.arc = UnsafeCell::new(KIND_INLINE as *mut Shared<usize>);
                inner.set_inline_len(len);
                inner.as_raw()[0..len].copy_from_slice(src);

                LocalBytesMut { inner }
            }
        } else {
            LocalBytesMut::from(src.to_vec())
        }
    }
}

impl<'a> From<&'a str> for LocalBytesMut {
    fn from(src: &'a str) -> LocalBytesMut {
        LocalBytesMut::from(src.as_bytes())
    }
}

impl From<LocalBytes> for LocalBytesMut {
    fn from(src: LocalBytes) -> LocalBytesMut {
        src.try_mut()
            .unwrap_or_else(|src| LocalBytesMut::from(&src[..]))
    }
}

impl PartialEq for LocalBytesMut {
    fn eq(&self, other: &LocalBytesMut) -> bool {
        self.inner.as_ref() == other.inner.as_ref()
    }
}

impl PartialOrd for LocalBytesMut {
    fn partial_cmp(&self, other: &LocalBytesMut) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other.inner.as_ref())
    }
}

impl Ord for LocalBytesMut {
    fn cmp(&self, other: &LocalBytesMut) -> cmp::Ordering {
        self.inner.as_ref().cmp(other.inner.as_ref())
    }
}

impl Eq for LocalBytesMut {}

impl Default for LocalBytesMut {
    #[inline]
    fn default() -> LocalBytesMut {
        LocalBytesMut::new()
    }
}

impl fmt::Debug for LocalBytesMut {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&debug::BsDebug(&self.inner.as_ref()), fmt)
    }
}

impl hash::Hash for LocalBytesMut {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        let s: &[u8] = self.as_ref();
        s.hash(state);
    }
}

impl Borrow<[u8]> for LocalBytesMut {
    fn borrow(&self) -> &[u8] {
        self.as_ref()
    }
}

impl BorrowMut<[u8]> for LocalBytesMut {
    fn borrow_mut(&mut self) -> &mut [u8] {
        self.as_mut()
    }
}

impl fmt::Write for LocalBytesMut {
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

impl Clone for LocalBytesMut {
    fn clone(&self) -> LocalBytesMut {
        LocalBytesMut::from(&self[..])
    }
}

impl IntoIterator for LocalBytesMut {
    type Item = u8;
    type IntoIter = Iter<Cursor<LocalBytesMut>>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_buf().iter()
    }
}

impl<'a> IntoIterator for &'a LocalBytesMut {
    type Item = u8;
    type IntoIter = Iter<Cursor<&'a LocalBytesMut>>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_buf().iter()
    }
}

impl Extend<u8> for LocalBytesMut {
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

impl<'a> Extend<&'a u8> for LocalBytesMut {
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

impl PartialEq<[u8]> for LocalBytesMut {
    fn eq(&self, other: &[u8]) -> bool {
        &**self == other
    }
}

impl PartialOrd<[u8]> for LocalBytesMut {
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other)
    }
}

impl PartialEq<LocalBytesMut> for [u8] {
    fn eq(&self, other: &LocalBytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<LocalBytesMut> for [u8] {
    fn partial_cmp(&self, other: &LocalBytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<str> for LocalBytesMut {
    fn eq(&self, other: &str) -> bool {
        &**self == other.as_bytes()
    }
}

impl PartialOrd<str> for LocalBytesMut {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other.as_bytes())
    }
}

impl PartialEq<LocalBytesMut> for str {
    fn eq(&self, other: &LocalBytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<LocalBytesMut> for str {
    fn partial_cmp(&self, other: &LocalBytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<Vec<u8>> for LocalBytesMut {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == &other[..]
    }
}

impl PartialOrd<Vec<u8>> for LocalBytesMut {
    fn partial_cmp(&self, other: &Vec<u8>) -> Option<cmp::Ordering> {
        (**self).partial_cmp(&other[..])
    }
}

impl PartialEq<LocalBytesMut> for Vec<u8> {
    fn eq(&self, other: &LocalBytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<LocalBytesMut> for Vec<u8> {
    fn partial_cmp(&self, other: &LocalBytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<String> for LocalBytesMut {
    fn eq(&self, other: &String) -> bool {
        *self == &other[..]
    }
}

impl PartialOrd<String> for LocalBytesMut {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other.as_bytes())
    }
}

impl PartialEq<LocalBytesMut> for String {
    fn eq(&self, other: &LocalBytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<LocalBytesMut> for String {
    fn partial_cmp(&self, other: &LocalBytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for LocalBytesMut
where
    LocalBytesMut: PartialEq<T>,
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a, T: ?Sized> PartialOrd<&'a T> for LocalBytesMut
where
    LocalBytesMut: PartialOrd<T>,
{
    fn partial_cmp(&self, other: &&'a T) -> Option<cmp::Ordering> {
        self.partial_cmp(*other)
    }
}

impl<'a> PartialEq<LocalBytesMut> for &'a [u8] {
    fn eq(&self, other: &LocalBytesMut) -> bool {
        *other == *self
    }
}

impl<'a> PartialOrd<LocalBytesMut> for &'a [u8] {
    fn partial_cmp(&self, other: &LocalBytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a> PartialEq<LocalBytesMut> for &'a str {
    fn eq(&self, other: &LocalBytesMut) -> bool {
        *other == *self
    }
}

impl<'a> PartialOrd<LocalBytesMut> for &'a str {
    fn partial_cmp(&self, other: &LocalBytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<[u8]> for LocalBytes {
    fn eq(&self, other: &[u8]) -> bool {
        self.inner.as_ref() == other
    }
}

impl PartialOrd<[u8]> for LocalBytes {
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other)
    }
}

impl PartialEq<LocalBytes> for [u8] {
    fn eq(&self, other: &LocalBytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<LocalBytes> for [u8] {
    fn partial_cmp(&self, other: &LocalBytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<str> for LocalBytes {
    fn eq(&self, other: &str) -> bool {
        self.inner.as_ref() == other.as_bytes()
    }
}

impl PartialOrd<str> for LocalBytes {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other.as_bytes())
    }
}

impl PartialEq<LocalBytes> for str {
    fn eq(&self, other: &LocalBytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<LocalBytes> for str {
    fn partial_cmp(&self, other: &LocalBytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<Vec<u8>> for LocalBytes {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == &other[..]
    }
}

impl PartialOrd<Vec<u8>> for LocalBytes {
    fn partial_cmp(&self, other: &Vec<u8>) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(&other[..])
    }
}

impl PartialEq<LocalBytes> for Vec<u8> {
    fn eq(&self, other: &LocalBytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<LocalBytes> for Vec<u8> {
    fn partial_cmp(&self, other: &LocalBytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<String> for LocalBytes {
    fn eq(&self, other: &String) -> bool {
        *self == &other[..]
    }
}

impl PartialOrd<String> for LocalBytes {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.inner.as_ref().partial_cmp(other.as_bytes())
    }
}

impl PartialEq<LocalBytes> for String {
    fn eq(&self, other: &LocalBytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<LocalBytes> for String {
    fn partial_cmp(&self, other: &LocalBytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a> PartialEq<LocalBytes> for &'a [u8] {
    fn eq(&self, other: &LocalBytes) -> bool {
        *other == *self
    }
}

impl<'a> PartialOrd<LocalBytes> for &'a [u8] {
    fn partial_cmp(&self, other: &LocalBytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a> PartialEq<LocalBytes> for &'a str {
    fn eq(&self, other: &LocalBytes) -> bool {
        *other == *self
    }
}

impl<'a> PartialOrd<LocalBytes> for &'a str {
    fn partial_cmp(&self, other: &LocalBytes) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for LocalBytes
where
    LocalBytes: PartialEq<T>,
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a, T: ?Sized> PartialOrd<&'a T> for LocalBytes
where
    LocalBytes: PartialOrd<T>,
{
    fn partial_cmp(&self, other: &&'a T) -> Option<cmp::Ordering> {
        self.partial_cmp(&**other)
    }
}

impl PartialEq<LocalBytesMut> for LocalBytes {
    fn eq(&self, other: &LocalBytesMut) -> bool {
        &other[..] == &self[..]
    }
}

impl PartialEq<LocalBytes> for LocalBytesMut {
    fn eq(&self, other: &LocalBytes) -> bool {
        &other[..] == &self[..]
    }
}

/*
 *
 * ===== usize =====
 *
 */

impl RefCount for usize {
    #[inline]
    fn new(val: usize) -> Self {
        val
    }

    #[inline]
    fn fetch_inc(&mut self, _: Ordering) -> usize {
        let val = *self;
        *self += 1;
        val
    }

    #[inline]
    fn release_shared(ptr: *mut Shared<Self>) {
        unsafe {
            if (*ptr).ref_count == 1 {
                drop(Box::from_raw(ptr));
                return;
            }
            (*ptr).ref_count -= 1;
        }
    }

    #[inline]
    fn load(&self, _: Ordering) -> usize {
        *self
    }
}

impl SharedPtr for UnsafeCell<*mut Shared<usize>> {
    type RefCount = usize;

    #[inline]
    fn new(ptr: *mut Shared<Self::RefCount>) -> Self {
        UnsafeCell::new(ptr)
    }

    #[inline]
    fn get_mut(&mut self) -> &mut *mut Shared<Self::RefCount> {
        unsafe { &mut *self.get() }
    }

    #[inline]
    fn load(&self, _: Ordering) -> *mut Shared<Self::RefCount> {
        unsafe { *self.get() }
    }

    #[inline]
    fn store(&self, ptr: *mut Shared<Self::RefCount>, _: Ordering) {
        unsafe {
            *self.get() = ptr;
        }
    }

    #[inline]
    fn compare_and_swap(
        &self,
        current: *mut Shared<Self::RefCount>,
        new: *mut Shared<Self::RefCount>,
        _: Ordering,
    ) -> *mut Shared<Self::RefCount> {
        unsafe {
            *self.get() = new;
        }
        current
    }
}
