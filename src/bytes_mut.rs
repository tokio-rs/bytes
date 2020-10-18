use core::alloc::Layout;
use core::iter::{FromIterator, Iterator};
use core::mem::{self};
use core::ops::{Deref, DerefMut};
use core::ptr::{self, NonNull};
use core::{cmp, fmt, hash, isize, slice, usize};

use alloc::{
    borrow::{Borrow, BorrowMut},
    string::String,
    vec::Vec,
};

use crate::buf::IntoIter;
use crate::bytes::Vtable;
#[allow(unused)]
use crate::loom::sync::atomic::AtomicMut;
use crate::loom::sync::atomic::{self, AtomicPtr, AtomicUsize, Ordering};
use crate::{Buf, BufMut, Bytes};

/// A unique reference to a contiguous slice of memory.
///
/// `BytesMut` represents a unique view into a potentially shared memory region.
/// Given the uniqueness guarantee, owners of `BytesMut` handles are able to
/// mutate the memory.
///
/// `BytesMut` can be thought of as containing a `buf: Arc<Vec<u8>>`, an offset
/// into `buf`, a slice length, and a guarantee that no other `BytesMut` for the
/// same `buf` overlaps with its slice. That guarantee means that a write lock
/// is not required.
///
/// # Growth
///
/// `BytesMut`'s `BufMut` implementation will implicitly grow its buffer as
/// necessary. However, explicitly reserving the required space up-front before
/// a series of inserts will be more efficient.
///
/// # Examples
///
/// ```
/// use bytes::{BytesMut, BufMut};
///
/// let mut buf = BytesMut::with_capacity(64);
///
/// buf.put_u8(b'h');
/// buf.put_u8(b'e');
/// buf.put(&b"llo"[..]);
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
    ptr: NonNull<u8>,
    len: usize,
    cap: usize,
    data: *mut Shared,
}
struct Shared {
    capacity: usize,
    refcount: AtomicUsize,
}

/// The max original capacity value which is reused when a `BytesMut` which
/// had been previously split into a smaller size is resized.
/// The `BytesMut` will try to regain the original capacity that was available
/// before the `split` - but the capacity will be bounded by this value.
const MAX_ORIGINAL_CAPACITY: usize = 64 * 1024;

/*
 *
 * ===== BytesMut =====
 *
 */

impl BytesMut {
    /// Creates a new `BytesMut` with the specified capacity.
    ///
    /// The returned `BytesMut` will be able to hold at least `capacity` bytes
    /// without reallocating.
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
        unsafe {
            if capacity == 0 {
                BytesMut {
                    data: ptr::null_mut(),
                    ptr: NonNull::dangling(),
                    len: 0,
                    cap: 0,
                }
            } else {
                let shared = Shared::allocate_for_size(capacity)
                    .expect("Fallible allocations are not yet supported");
                BytesMut {
                    data: shared,
                    len: 0,
                    ptr: (*shared).data_ptr_mut(),
                    cap: (*shared).capacity,
                }
            }
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
        self.len
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
        self.len == 0
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
        self.cap
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
    /// b.put(&b"hello world"[..]);
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
        if self.data.is_null() {
            return Bytes::new();
        }

        let ptr = self.ptr.as_ptr();
        let len = self.len;
        let data = AtomicPtr::new(self.data as _);
        mem::forget(self);
        unsafe { Bytes::with_vtable(ptr, len, data, &SHARED_VTABLE) }
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
    #[must_use = "consider BytesMut::truncate if you don't need the other half"]
    pub fn split_off(&mut self, at: usize) -> BytesMut {
        assert!(
            at <= self.capacity(),
            "split_off out of bounds: {:?} <= {:?}",
            at,
            self.capacity(),
        );

        if self.data.is_null() {
            debug_assert_eq!(at, 0, "Empty Bytes is only splittable at the front");
            return BytesMut::new();
        }

        unsafe {
            let mut other = self.shallow_clone();
            other.set_start(at);
            self.set_end(at);
            other
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
    /// let other = buf.split();
    ///
    /// assert!(buf.is_empty());
    /// assert_eq!(1013, buf.capacity());
    ///
    /// assert_eq!(other, b"hello world"[..]);
    /// ```
    #[must_use = "consider BytesMut::advance(len()) if you don't need the other half"]
    pub fn split(&mut self) -> BytesMut {
        let len = self.len();
        self.split_to(len)
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
    #[must_use = "consider BytesMut::advance if you don't need the other half"]
    pub fn split_to(&mut self, at: usize) -> BytesMut {
        assert!(
            at <= self.len(),
            "split_to out of bounds: {:?} <= {:?}",
            at,
            self.len(),
        );

        if self.data.is_null() {
            debug_assert_eq!(at, 0, "Empty Bytes is only splittable at the front");
            return BytesMut::new();
        }

        unsafe {
            let mut other = self.shallow_clone();
            other.set_end(at);
            self.set_start(at);
            other
        }
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
        if len <= self.len() {
            unsafe {
                self.set_len(len);
            }
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
        let len = self.len();
        if new_len > len {
            let additional = new_len - len;
            self.reserve(additional);
            unsafe {
                let dst = self.bytes_mut().as_mut_ptr();
                ptr::write_bytes(dst, value, additional);
                self.set_len(new_len);
            }
        } else {
            self.truncate(new_len);
        }
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
    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        debug_assert!(len <= self.cap, "set_len out of bounds");
        self.len = len;
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
    /// let other = buf.split();
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
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let len = self.len();
        let rem = self.capacity() - len;

        if additional <= rem {
            // The handle can already store at least `additional` more bytes, so
            // there is no further work needed to be done.
            return;
        }

        self.reserve_inner(additional);
    }

    // In separate function to allow the short-circuits in `reserve` to
    // be inline-able. Significant helps performance.
    fn reserve_inner(&mut self, additional: usize) {
        let len = self.len();

        let shared: *mut Shared = self.data as _;

        // Reserving involves abandoning the currently shared buffer and
        // allocating a new shared state with the requested capacity.
        //
        // Compute the new capacity
        let mut new_cap = len.checked_add(additional).expect("overflow");

        unsafe {
            let mut original_capacity = 0;

            if !shared.is_null() {
                original_capacity = (*shared).capacity;

                // First, try to reclaim the buffer. This is possible if the current
                // handle is the only outstanding handle pointing to the buffer.
                // However, before doing the work of copying data, check to make
                // sure that the old shared state has enough capacity.
                if (*shared).is_unique() && (*shared).capacity >= new_cap {
                    // The capacity is sufficient, reclaim the buffer
                    let ptr = (*shared).data_ptr_mut();

                    ptr::copy(self.ptr.as_ptr(), ptr.as_ptr(), len);

                    self.ptr = ptr;
                    self.cap = (*shared).capacity;

                    return;
                }
            }

            // The backing storage capacity is not sufficient. The reserve request is
            // asking for more than the initial buffer capacity. Allocate more
            // than requested if `new_cap` is not much bigger than the current
            // capacity.
            //
            // The reservation strategy follows what `std` does:
            // https://github.com/rust-lang/rust/blob/834821e3b666f77bb7caf1ed88ed662c395fbc11/library/alloc/src/raw_vec.rs#L401-L417
            //
            // - The old capacity is at least doubled
            let double = self.cap.checked_shl(1).unwrap_or(new_cap);
            // - If this is not sufficient for what the user asked for, the higher
            //   value is taken.
            new_cap = cmp::max(double, new_cap);
            // - We want to allocate at least 8 bytes, because tiny allocations
            //   are not efficient
            new_cap = cmp::max(8, new_cap);
            // - If we had an existing backing storage, we want to obtain at
            //   least the same capacity again that this one had. This is specific
            //   to BytesMut and not found in Vecs. The use-case is that BytesMut
            //   which are used over time to split off chunks and then gain new
            //   input data do not oscillate too much in size.
            //   This functionality is however bounded by `MAX_ORIGINAL_CAPACITY`.
            //   A `BytesMut` will not automatically reserve more capacity than this.
            //   TODO: Do we really want to keep this? This seems like a footgun
            //   where people splitting `BytesMut` and reusing them for different
            //   purposes might accidentally keep around big memory allocations.
            new_cap = cmp::max(cmp::min(original_capacity, MAX_ORIGINAL_CAPACITY), new_cap);

            // Create a new backing storage
            let shared = Shared::allocate_for_size(new_cap)
                .expect("Fallible allocations are not yet supported");

            // Copy data over into the new storage
            if self.len != 0 {
                ptr::copy_nonoverlapping(
                    self.ptr.as_ptr(),
                    (*shared).data_ptr_mut().as_ptr(),
                    self.len,
                );
            }

            // Release the shared handle.
            // This must be done *after* the bytes are copied.
            Shared::release_refcount(self.data);

            // Update self
            self.data = shared;
            self.ptr = (*shared).data_ptr_mut();
            self.cap = (*shared).capacity;
        }
    }

    /// Appends given bytes to this `BytesMut`.
    ///
    /// If this `BytesMut` object does not have enough capacity, it is resized
    /// first.
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
        let cnt = extend.len();
        self.reserve(cnt);

        unsafe {
            let dst = self.maybe_uninit_bytes();
            // Reserved above
            debug_assert!(dst.len() >= cnt);

            ptr::copy_nonoverlapping(extend.as_ptr(), dst.as_mut_ptr() as *mut u8, cnt);
        }

        unsafe {
            self.advance_mut(cnt);
        }
    }

    /// Absorbs a `BytesMut` that was previously split off.
    ///
    /// If the two `BytesMut` objects were previously contiguous, i.e., if
    /// `other` was created by calling `split_off` on this `BytesMut`, then
    /// this is an `O(1)` operation that just decreases a reference
    /// count and sets a few indices. Otherwise this method degenerates to
    /// `self.extend_from_slice(other.as_ref())`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BytesMut;
    ///
    /// let mut buf = BytesMut::with_capacity(64);
    /// buf.extend_from_slice(b"aaabbbcccddd");
    ///
    /// let split = buf.split_off(6);
    /// assert_eq!(b"aaabbb", &buf[..]);
    /// assert_eq!(b"cccddd", &split[..]);
    ///
    /// buf.unsplit(split);
    /// assert_eq!(b"aaabbbcccddd", &buf[..]);
    /// ```
    pub fn unsplit(&mut self, other: BytesMut) {
        if self.is_empty() {
            *self = other;
            return;
        }

        if let Err(other) = self.try_unsplit(other) {
            self.extend_from_slice(other.as_ref());
        }
    }

    // private

    // This method is only used in combination with the serde feature at the moment.
    // BytesMut doesn't expose a public `From<Vec<u8>>` implementation in order
    // not provide the wrong impression that the conversion is free.
    // Even if one implementation `BytesMut` would have a free conversion,
    // a future change of the allocation strategy might remove this guarantee.
    #[inline]
    #[cfg(feature = "serde")]
    pub(crate) fn from_vec(vec: Vec<u8>) -> BytesMut {
        BytesMut::from(&vec[..])
    }

    #[inline]
    fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    #[inline]
    fn as_slice_mut(&mut self) -> &mut [u8] {
        // TODO:
        // The pointer is always valid, but might not backed by real storage
        // in case of an empty BytesMut.
        // This shouldn't matter due to the array length being 0 and the pointer
        // never being accessed. However it is still not fully clear if this is
        // valid within Rusts's UB rules.
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    unsafe fn set_start(&mut self, start: usize) {
        // Setting the start to 0 is a no-op, so return early if this is the
        // case.
        if start == 0 {
            return;
        }

        debug_assert!(start <= self.cap, "internal: set_start out of bounds");

        // Updating the start of the view is setting `ptr` to point to the
        // new start and updating the `len` field to reflect the new length
        // of the view.
        self.ptr = vptr(self.ptr.as_ptr().offset(start as isize));

        if self.len >= start {
            self.len -= start;
        } else {
            self.len = 0;
        }

        self.cap -= start;
    }

    unsafe fn set_end(&mut self, end: usize) {
        assert!(end <= self.cap, "set_end out of bounds");

        self.cap = end;
        self.len = cmp::min(self.len, end);
    }

    fn try_unsplit(&mut self, other: BytesMut) -> Result<(), BytesMut> {
        if other.is_empty() {
            return Ok(());
        }

        // Note that the following comparison will also yield true for 2 empty
        // BytesMut. This will however be fine - it will simply combine them
        // into a single empty BytesMut.
        let ptr = unsafe { self.ptr.as_ptr().offset(self.len as isize) };
        if ptr == other.ptr.as_ptr() && self.data == other.data {
            // Contiguous blocks, just combine directly
            self.len += other.len;
            self.cap += other.cap;
            Ok(())
        } else {
            Err(other)
        }
    }

    /// Makes an exact shallow clone of `self`.
    ///
    /// The kind of `self` doesn't matter, but this is unsafe
    /// because the clone will have the same offsets. You must
    /// be sure the returned value to the user doesn't allow
    /// two views into the same range.
    #[inline]
    unsafe fn shallow_clone(&mut self) -> BytesMut {
        Shared::increment_refcount(self.data);
        ptr::read(self)
    }

    #[inline]
    fn maybe_uninit_bytes(&mut self) -> &mut [mem::MaybeUninit<u8>] {
        unsafe {
            let ptr = self.ptr.as_ptr().offset(self.len as isize);
            let len = self.cap - self.len;

            slice::from_raw_parts_mut(ptr as *mut mem::MaybeUninit<u8>, len)
        }
    }
}

impl Drop for BytesMut {
    fn drop(&mut self) {
        unsafe {
            Shared::release_refcount(self.data);
        }
    }
}

impl Buf for BytesMut {
    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        self.as_slice()
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        assert!(
            cnt <= self.remaining(),
            "cannot advance past `remaining`: {:?} <= {:?}",
            cnt,
            self.remaining(),
        );
        unsafe {
            self.set_start(cnt);
        }
    }

    fn to_bytes(&mut self) -> crate::Bytes {
        self.split().freeze()
    }
}

unsafe impl BufMut for BytesMut {
    #[inline]
    fn remaining_mut(&self) -> usize {
        usize::MAX - self.len()
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        let new_len = self.len() + cnt;
        assert!(
            new_len <= self.cap,
            "new_len = {}; capacity = {}",
            new_len,
            self.cap
        );
        self.len = new_len;
    }

    #[inline]
    fn bytes_mut(&mut self) -> &mut [mem::MaybeUninit<u8>] {
        if self.capacity() == self.len() {
            self.reserve(64);
        }
        self.maybe_uninit_bytes()
    }

    // Specialize these methods so they can skip checking `remaining_mut`
    // and `advance_mut`.

    fn put<T: crate::Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        while src.has_remaining() {
            let s = src.bytes();
            let l = s.len();
            self.extend_from_slice(s);
            src.advance(l);
        }
    }

    fn put_slice(&mut self, src: &[u8]) {
        self.extend_from_slice(src);
    }
}

impl AsRef<[u8]> for BytesMut {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Deref for BytesMut {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}

impl AsMut<[u8]> for BytesMut {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_slice_mut()
    }
}

impl DerefMut for BytesMut {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        self.as_mut()
    }
}

impl<'a> From<&'a [u8]> for BytesMut {
    fn from(src: &'a [u8]) -> BytesMut {
        let mut bytes = BytesMut::with_capacity(src.len());
        bytes.extend_from_slice(src);
        bytes
    }
}

impl<'a> From<&'a str> for BytesMut {
    fn from(src: &'a str) -> BytesMut {
        BytesMut::from(src.as_bytes())
    }
}

impl From<BytesMut> for Bytes {
    fn from(src: BytesMut) -> Bytes {
        src.freeze()
    }
}

impl PartialEq for BytesMut {
    fn eq(&self, other: &BytesMut) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl PartialOrd for BytesMut {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl Ord for BytesMut {
    fn cmp(&self, other: &BytesMut) -> cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl Eq for BytesMut {}

impl Default for BytesMut {
    #[inline]
    fn default() -> BytesMut {
        BytesMut::new()
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
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
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
    type IntoIter = IntoIter<BytesMut>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self)
    }
}

impl<'a> IntoIterator for &'a BytesMut {
    type Item = &'a u8;
    type IntoIter = core::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_ref().into_iter()
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
            self.reserve(1);
            self.put_u8(b);
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

impl FromIterator<u8> for BytesMut {
    fn from_iter<T: IntoIterator<Item = u8>>(into_iter: T) -> Self {
        let mut bytes = BytesMut::new();
        bytes.extend(into_iter);
        bytes
    }
}

impl<'a> FromIterator<&'a u8> for BytesMut {
    fn from_iter<T: IntoIterator<Item = &'a u8>>(into_iter: T) -> Self {
        BytesMut::from_iter(into_iter.into_iter().map(|b| *b))
    }
}

/*
 *
 * ===== Inner =====
 *
 */

impl Shared {
    unsafe fn allocate_for_size(size: usize) -> Result<*mut Shared, ()> {
        let combined_layout = Shared::layout_for_size(size);
        let alloc_res = alloc::alloc::alloc(combined_layout) as *mut Shared;
        if alloc_res.is_null() {
            return Err(());
        }

        let result = &mut *alloc_res;
        result.capacity = size;
        result.refcount.store(1, Ordering::Relaxed);

        Ok(alloc_res)
    }

    fn is_unique(&self) -> bool {
        // The goal is to check if the current handle is the only handle
        // that currently has access to the buffer. This is done by
        // checking if the `ref_count` is currently 1.
        //
        // The `Acquire` ordering synchronizes with the `Release` as
        // part of the `fetch_sub` in `release_shared`. The `fetch_sub`
        // operation guarantees that any mutations done in other threads
        // are ordered before the `ref_count` is decremented. As such,
        // this `Acquire` will guarantee that those mutations are
        // visible to the current thread.
        self.refcount.load(Ordering::Acquire) == 1
    }

    fn layout_for_size(size: usize) -> Layout {
        let layout = Layout::new::<Shared>();
        let total_size = layout
            .size()
            .checked_add(size)
            .expect("Overflow during layout calculation");
        let combined_layout =
            Layout::from_size_align(total_size, layout.align()).expect("Error calculating layout");
        combined_layout
    }

    unsafe fn increment_refcount(self_ptr: *mut Self) {
        debug_assert!(!self_ptr.is_null());
        let old_size = (*self_ptr).refcount.fetch_add(1, Ordering::Relaxed);

        if old_size > isize::MAX as usize {
            crate::abort();
        }
    }

    unsafe fn release_refcount(self_ptr: *mut Self) {
        if self_ptr.is_null() {
            return;
        }

        if (*self_ptr).refcount.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }

        // The shared state should be released

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
        atomic::fence(Ordering::Acquire);

        alloc::alloc::dealloc(
            self_ptr as *mut u8,
            Self::layout_for_size((*self_ptr).capacity),
        )
    }

    unsafe fn data_ptr_mut(&mut self) -> NonNull<u8> {
        let mut end_addr = self as *const Shared as usize;
        end_addr += mem::size_of::<Shared>();
        NonNull::new_unchecked(end_addr as *mut u8)
    }
}

/*
#[test]
fn test_original_capacity_to_repr() {
    assert_eq!(original_capacity_to_repr(0), 0);

    let max_width = 32;

    for width in 1..(max_width + 1) {
        let cap = 1 << width - 1;

        let expected = if width < MIN_ORIGINAL_CAPACITY_WIDTH {
            0
        } else if width < MAX_ORIGINAL_CAPACITY_WIDTH {
            width - MIN_ORIGINAL_CAPACITY_WIDTH
        } else {
            MAX_ORIGINAL_CAPACITY_WIDTH - MIN_ORIGINAL_CAPACITY_WIDTH
        };

        assert_eq!(original_capacity_to_repr(cap), expected);

        if width > 1 {
            assert_eq!(original_capacity_to_repr(cap + 1), expected);
        }

        //  MIN_ORIGINAL_CAPACITY_WIDTH must be bigger than 7 to pass tests below
        if width == MIN_ORIGINAL_CAPACITY_WIDTH + 1 {
            assert_eq!(original_capacity_to_repr(cap - 24), expected - 1);
            assert_eq!(original_capacity_to_repr(cap + 76), expected);
        } else if width == MIN_ORIGINAL_CAPACITY_WIDTH + 2 {
            assert_eq!(original_capacity_to_repr(cap - 1), expected - 1);
            assert_eq!(original_capacity_to_repr(cap - 48), expected - 1);
        }
    }
}

#[test]
fn test_original_capacity_from_repr() {
    assert_eq!(0, original_capacity_from_repr(0));

    let min_cap = 1 << MIN_ORIGINAL_CAPACITY_WIDTH;

    assert_eq!(min_cap, original_capacity_from_repr(1));
    assert_eq!(min_cap * 2, original_capacity_from_repr(2));
    assert_eq!(min_cap * 4, original_capacity_from_repr(3));
    assert_eq!(min_cap * 8, original_capacity_from_repr(4));
    assert_eq!(min_cap * 16, original_capacity_from_repr(5));
    assert_eq!(min_cap * 32, original_capacity_from_repr(6));
    assert_eq!(min_cap * 64, original_capacity_from_repr(7));
}
*/

unsafe impl Send for BytesMut {}
unsafe impl Sync for BytesMut {}

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
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
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
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
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
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
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

impl PartialEq<BytesMut> for &[u8] {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for &[u8] {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
    }
}

impl PartialEq<BytesMut> for &str {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for &str {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
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

fn vptr(ptr: *mut u8) -> NonNull<u8> {
    if cfg!(debug_assertions) {
        NonNull::new(ptr).expect("data pointer should be non-null")
    } else {
        unsafe { NonNull::new_unchecked(ptr) }
    }
}

// ===== impl SharedVtable =====

static SHARED_VTABLE: Vtable = Vtable {
    clone: shared_v_clone,
    drop: shared_v_drop,
};

unsafe fn shared_v_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    let shared = data.load(Ordering::Relaxed) as *mut Shared;
    Shared::increment_refcount(shared);

    let data = AtomicPtr::new(shared as _);
    Bytes::with_vtable(ptr, len, data, &SHARED_VTABLE)
}

unsafe fn shared_v_drop(data: &mut AtomicPtr<()>, _ptr: *const u8, _len: usize) {
    data.with_mut(|shared| {
        Shared::release_refcount(*shared as *mut Shared);
    });
}

// compile-fails

/// ```compile_fail
/// use bytes::BytesMut;
/// #[deny(unused_must_use)]
/// {
///     let mut b1 = BytesMut::from("hello world");
///     b1.split_to(6);
/// }
/// ```
fn _split_to_must_use() {}

/// ```compile_fail
/// use bytes::BytesMut;
/// #[deny(unused_must_use)]
/// {
///     let mut b1 = BytesMut::from("hello world");
///     b1.split_off(6);
/// }
/// ```
fn _split_off_must_use() {}

/// ```compile_fail
/// use bytes::BytesMut;
/// #[deny(unused_must_use)]
/// {
///     let mut b1 = BytesMut::from("hello world");
///     b1.split();
/// }
/// ```
fn _split_must_use() {}

// fuzz tests
#[cfg(all(test, loom))]
mod fuzz {
    use loom::sync::Arc;
    use loom::thread;

    use super::BytesMut;
    use crate::Bytes;

    #[test]
    fn bytes_mut_cloning_frozen() {
        loom::model(|| {
            let a = BytesMut::from(&b"abcdefgh"[..]).split().freeze();
            let addr = a.as_ptr() as usize;

            // test the Bytes::clone is Sync by putting it in an Arc
            let a1 = Arc::new(a);
            let a2 = a1.clone();

            let t1 = thread::spawn(move || {
                let b: Bytes = (*a1).clone();
                assert_eq!(b.as_ptr() as usize, addr);
            });

            let t2 = thread::spawn(move || {
                let b: Bytes = (*a2).clone();
                assert_eq!(b.as_ptr() as usize, addr);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }
}
