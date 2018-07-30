use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::{cmp, mem, ptr, slice, usize};

pub mod bytes;
pub mod local_bytes;

// Both `Bytes` and `BytesMut` are backed by `Inner` and functions are delegated
// to `Inner` functions. The `Bytes` and `BytesMut` shims ensure that functions
// that mutate the underlying buffer are only performed when the data range
// being mutated is only available via a single `BytesMut` handle.
//
// # Data storage modes
//
// The goal of `bytes` is to be as efficient as possible across a wide range of
// potential usage patterns. As such, `bytes` needs to be able to handle buffers
// that are never shared, shared on a single thread, and shared across many
// threads. `bytes` also needs to handle both tiny buffers as well as very large
// buffers. For example, [Cassandra](http://cassandra.apache.org) values have
// been known to be in the hundreds of megabyte, and HTTP header values can be a
// few characters in size.
//
// To achieve high performance in these various situations, `Bytes` and
// `BytesMut` use different strategies for storing the buffer depending on the
// usage pattern.
//
// ## Delayed `Arc` allocation
//
// When a `Bytes` or `BytesMut` is first created, there is only one outstanding
// handle referencing the buffer. Since sharing is not yet required, an `Arc`* is
// not used and the buffer is backed by a `Vec<u8>` directly. Using an
// `Arc<Vec<u8>>` requires two allocations, so if the buffer ends up never being
// shared, that allocation is avoided.
//
// When sharing does become necessary (`clone`, `split_to`, `split_off`), that
// is when the buffer is promoted to being shareable. The `Vec<u8>` is moved
// into an `Arc` and both the original handle and the new handle use the same
// buffer via the `Arc`.
//
// * `Arc` is being used to signify an atomically reference counted cell. We
// don't use the `Arc` implementation provided by `std` and instead use our own.
// This ends up simplifying a number of the `unsafe` code snippets.
//
// ## Inlining small buffers
//
// The `Bytes` / `BytesMut` structs require 4 pointer sized fields. On 64 bit
// systems, this ends up being 32 bytes, which is actually a lot of storage for
// cases where `Bytes` is being used to represent small byte strings, such as
// HTTP header names and values.
//
// To avoid any allocation at all in these cases, `Bytes` will use the struct
// itself for storing the buffer, reserving 1 byte for meta data. This means
// that, on 64 bit systems, 31 byte buffers require no allocation at all.
//
// The byte used for metadata stores a 2 bits flag used to indicate that the
// buffer is stored inline as well as 6 bits for tracking the buffer length (the
// return value of `Bytes::len`).
//
// ## Static buffers
//
// `Bytes` can also represent a static buffer, which is created with
// `Bytes::from_static`. No copying or allocations are required for tracking
// static buffers. The pointer to the `&'static [u8]`, the length, and a flag
// tracking that the `Bytes` instance represents a static buffer is stored in
// the `Bytes` struct.
//
// # Struct layout
//
// Both `Bytes` and `BytesMut` are wrappers around `Inner`, which provides the
// data fields as well as all of the function implementations.
//
// The `Inner` struct is carefully laid out in order to support the
// functionality described above as well as being as small as possible. Size is
// important as growing the size of the `Bytes` struct from 32 bytes to 40 bytes
// added as much as 15% overhead in benchmarks using `Bytes` in an HTTP header
// map structure.
//
// The `Inner` struct contains the following fields:
//
// * `ptr: *mut u8`
// * `len: usize`
// * `cap: usize`
// * `arc: AtomicPtr<Shared>`
//
// ## `ptr: *mut u8`
//
// A pointer to start of the handle's buffer view. When backed by a `Vec<u8>`,
// this is always the `Vec`'s pointer. When backed by an `Arc<Vec<u8>>`, `ptr`
// may have been shifted to point somewhere inside the buffer.
//
// When in "inlined" mode, `ptr` is used as part of the inlined buffer.
//
// ## `len: usize`
//
// The length of the handle's buffer view. When backed by a `Vec<u8>`, this is
// always the `Vec`'s length. The slice represented by `ptr` and `len` should
// (ideally) always be initialized memory.
//
// When in "inlined" mode, `len` is used as part of the inlined buffer.
//
// ## `cap: usize`
//
// The capacity of the handle's buffer view. When backed by a `Vec<u8>`, this is
// always the `Vec`'s capacity. The slice represented by `ptr+len` and `cap-len`
// may or may not be initialized memory.
//
// When in "inlined" mode, `cap` is used as part of the inlined buffer.
//
// ## `arc: AtomicPtr<Shared>`
//
// When `Inner` is in allocated mode (backed by Vec<u8> or Arc<Vec<u8>>), this
// will be the pointer to the `Arc` structure tracking the ref count for the
// underlying buffer. When the pointer is null, then the `Arc` has not been
// allocated yet and `self` is the only outstanding handle for the underlying
// buffer.
//
// The lower two bits of `arc` are used to track the storage mode of `Inner`.
// `0b01` indicates inline storage, `0b10` indicates static storage, and `0b11`
// indicates vector storage, not yet promoted to Arc.  Since pointers to
// allocated structures are aligned, the lower two bits of a pointer will always
// be 0. This allows disambiguating between a pointer and the two flags.
//
// When in "inlined" mode, the least significant byte of `arc` is also used to
// store the length of the buffer view (vs. the capacity, which is a constant).
//
// The rest of `arc`'s bytes are used as part of the inline buffer, which means
// that those bytes need to be located next to the `ptr`, `len`, and `cap`
// fields, which make up the rest of the inline buffer. This requires special
// casing the layout of `Inner` depending on if the target platform is bit or
// little endian.
//
// On little endian platforms, the `arc` field must be the first field in the
// struct. On big endian platforms, the `arc` field must be the last field in
// the struct. Since a deterministic struct layout is required, `Inner` is
// annotated with `#[repr(C)]`.
//
// # Thread safety
//
// `Bytes::clone()` returns a new `Bytes` handle with no copying. This is done
// by bumping the buffer ref count and returning a new struct pointing to the
// same buffer. However, the `Arc` structure is lazily allocated. This means
// that if `Bytes` is stored itself in an `Arc` (`Arc<Bytes>`), the `clone`
// function can be called concurrently from multiple threads. This is why an
// `AtomicPtr` is used for the `arc` field vs. a `*const`.
//
// Care is taken to ensure that the need for synchronization is minimized. Most
// operations do not require any synchronization.
//
#[cfg(target_endian = "little")]
#[repr(C)]
struct Inner<P: SharedPtr> {
    // WARNING: Do not access the fields directly unless you know what you are
    // doing. Instead, use the fns. See implementation comment above.
    arc: P,
    ptr: *mut u8,
    len: usize,
    cap: usize,
}

#[cfg(target_endian = "big")]
#[repr(C)]
struct Inner<P: SharedPtr> {
    // WARNING: Do not access the fields directly unless you know what you are
    // doing. Instead, use the fns. See implementation comment above.
    ptr: *mut u8,
    len: usize,
    cap: usize,
    arc: P,
}

trait SharedPtr {
    type RefCount: RefCount;

    fn new(ptr: *mut Shared<Self::RefCount>) -> Self;
    fn get_mut(&mut self) -> &mut *mut Shared<Self::RefCount>;
    fn load(&self, order: Ordering) -> *mut Shared<Self::RefCount>;
    fn store(&self, ptr: *mut Shared<Self::RefCount>, order: Ordering);
    fn compare_and_swap(
        &self,
        current: *mut Shared<Self::RefCount>,
        new: *mut Shared<Self::RefCount>,
        order: Ordering,
    ) -> *mut Shared<Self::RefCount>;
}

trait RefCount: Sized {
    fn new(val: usize) -> Self;
    fn fetch_inc(&mut self, order: Ordering) -> usize;
    fn release_shared(ptr: *mut Shared<Self>);
    fn load(&self, order: Ordering) -> usize;
}

// Thread-safe reference-counted container for the shared storage. This mostly
// the same as `std::sync::Arc` but without the weak counter. The ref counting
// fns are based on the ones found in `std`.
//
// The main reason to use `Shared` instead of `std::sync::Arc` is that it ends
// up making the overall code simpler and easier to reason about. This is due to
// some of the logic around setting `Inner::arc` and other ways the `arc` field
// is used. Using `Arc` ended up requiring a number of funky transmutes and
// other shenanigans to make it work.
struct Shared<C: RefCount> {
    vec: Vec<u8>,
    original_capacity_repr: usize,
    ref_count: C,
}

// Buffer storage strategy flags.
const KIND_ARC: usize = 0b00;
const KIND_INLINE: usize = 0b01;
const KIND_STATIC: usize = 0b10;
const KIND_VEC: usize = 0b11;
const KIND_MASK: usize = 0b11;

// The max original capacity value. Any `Bytes` allocated with a greater initial
// capacity will default to this.
const MAX_ORIGINAL_CAPACITY_WIDTH: usize = 17;
// The original capacity algorithm will not take effect unless the originally
// allocated capacity was at least 1kb in size.
const MIN_ORIGINAL_CAPACITY_WIDTH: usize = 10;
// The original capacity is stored in powers of 2 starting at 1kb to a max of
// 64kb. Representing it as such requires only 3 bits of storage.
const ORIGINAL_CAPACITY_MASK: usize = 0b11100;
const ORIGINAL_CAPACITY_OFFSET: usize = 2;

// When the storage is in the `Vec` representation, the pointer can be advanced
// at most this value. This is due to the amount of storage available to track
// the offset is usize - number of KIND bits and number of ORIGINAL_CAPACITY
// bits.
const VEC_POS_OFFSET: usize = 5;
const MAX_VEC_POS: usize = usize::MAX >> VEC_POS_OFFSET;
const NOT_VEC_POS_MASK: usize = 0b11111;

// Bit op constants for extracting the inline length value from the `arc` field.
const INLINE_LEN_MASK: usize = 0b11111100;
const INLINE_LEN_OFFSET: usize = 2;

// Byte offset from the start of `Inner` to where the inline buffer data
// starts. On little endian platforms, the first byte of the struct is the
// storage flag, so the data is shifted by a byte. On big endian systems, the
// data starts at the beginning of the struct.
#[cfg(target_endian = "little")]
const INLINE_DATA_OFFSET: isize = 1;
#[cfg(target_endian = "big")]
const INLINE_DATA_OFFSET: isize = 0;

#[cfg(target_pointer_width = "64")]
const PTR_WIDTH: usize = 64;
#[cfg(target_pointer_width = "32")]
const PTR_WIDTH: usize = 32;

// Inline buffer capacity. This is the size of `Inner` minus 1 byte for the
// metadata.
#[cfg(target_pointer_width = "64")]
const INLINE_CAP: usize = 4 * 8 - 1;
#[cfg(target_pointer_width = "32")]
const INLINE_CAP: usize = 4 * 4 - 1;

/*
 *
 * ===== Inner =====
 *
 */

impl<P> Inner<P>
where
    P: SharedPtr,
{
    #[inline]
    fn from_static(bytes: &'static [u8]) -> Self {
        let ptr = bytes.as_ptr() as *mut u8;

        Inner {
            // `arc` won't ever store a pointer. Instead, use it to
            // track the fact that the `Bytes` handle is backed by a
            // static buffer.
            arc: P::new(KIND_STATIC as *mut Shared<P::RefCount>),
            ptr,
            len: bytes.len(),
            cap: bytes.len(),
        }
    }

    #[inline]
    fn from_vec(mut src: Vec<u8>) -> Self {
        let len = src.len();
        let cap = src.capacity();
        let ptr = src.as_mut_ptr();

        mem::forget(src);

        let original_capacity_repr = original_capacity_to_repr(cap);
        let arc = (original_capacity_repr << ORIGINAL_CAPACITY_OFFSET) | KIND_VEC;

        Inner {
            arc: P::new(arc as *mut Shared<P::RefCount>),
            ptr,
            len,
            cap,
        }
    }

    #[inline]
    fn with_capacity(capacity: usize) -> Self {
        if capacity <= INLINE_CAP {
            unsafe {
                // Using uninitialized memory is ~30% faster
                let mut inner: Self = mem::uninitialized();
                inner.arc = P::new(KIND_INLINE as *mut Shared<P::RefCount>);
                inner
            }
        } else {
            Inner::from_vec(Vec::with_capacity(capacity))
        }
    }

    /// Return a slice for the handle's view into the shared buffer
    #[inline]
    fn as_ref(&self) -> &[u8] {
        unsafe {
            if self.is_inline() {
                slice::from_raw_parts(self.inline_ptr(), self.inline_len())
            } else {
                slice::from_raw_parts(self.ptr, self.len)
            }
        }
    }

    /// Return a mutable slice for the handle's view into the shared buffer
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        debug_assert!(!self.is_static());

        unsafe {
            if self.is_inline() {
                slice::from_raw_parts_mut(self.inline_ptr(), self.inline_len())
            } else {
                slice::from_raw_parts_mut(self.ptr, self.len)
            }
        }
    }

    /// Return a mutable slice for the handle's view into the shared buffer
    /// including potentially uninitialized bytes.
    #[inline]
    unsafe fn as_raw(&mut self) -> &mut [u8] {
        debug_assert!(!self.is_static());

        if self.is_inline() {
            slice::from_raw_parts_mut(self.inline_ptr(), INLINE_CAP)
        } else {
            slice::from_raw_parts_mut(self.ptr, self.cap)
        }
    }

    /// Insert a byte into the next slot and advance the len by 1.
    #[inline]
    fn put_u8(&mut self, n: u8) {
        if self.is_inline() {
            let len = self.inline_len();
            assert!(len < INLINE_CAP);
            unsafe {
                *self.inline_ptr().offset(len as isize) = n;
            }
            self.set_inline_len(len + 1);
        } else {
            assert!(self.len < self.cap);
            unsafe {
                *self.ptr.offset(self.len as isize) = n;
            }
            self.len += 1;
        }
    }

    #[inline]
    fn len(&self) -> usize {
        if self.is_inline() {
            self.inline_len()
        } else {
            self.len
        }
    }

    /// Pointer to the start of the inline buffer
    #[inline]
    unsafe fn inline_ptr(&self) -> *mut u8 {
        (self as *const Self as *mut Self as *mut u8).offset(INLINE_DATA_OFFSET)
    }

    #[inline]
    fn inline_len(&self) -> usize {
        let p: &usize = unsafe { mem::transmute(&self.arc) };
        (p & INLINE_LEN_MASK) >> INLINE_LEN_OFFSET
    }

    /// Set the length of the inline buffer. This is done by writing to the
    /// least significant byte of the `arc` field.
    #[inline]
    fn set_inline_len(&mut self, len: usize) {
        debug_assert!(len <= INLINE_CAP);
        let p = self.arc.get_mut();
        *p = ((*p as usize & !INLINE_LEN_MASK) | (len << INLINE_LEN_OFFSET)) as _;
    }

    /// slice.
    #[inline]
    unsafe fn set_len(&mut self, len: usize) {
        if self.is_inline() {
            assert!(len <= INLINE_CAP);
            self.set_inline_len(len);
        } else {
            assert!(len <= self.cap);
            self.len = len;
        }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn capacity(&self) -> usize {
        if self.is_inline() {
            INLINE_CAP
        } else {
            self.cap
        }
    }

    fn split_off(&mut self, at: usize) -> Self {
        let mut other = unsafe { self.shallow_clone(true) };

        unsafe {
            other.set_start(at);
            self.set_end(at);
        }

        return other;
    }

    fn split_to(&mut self, at: usize) -> Self {
        let mut other = unsafe { self.shallow_clone(true) };

        unsafe {
            other.set_end(at);
            self.set_start(at);
        }

        return other;
    }

    fn truncate(&mut self, len: usize) {
        if len <= self.len() {
            unsafe {
                self.set_len(len);
            }
        }
    }

    fn try_unsplit(&mut self, other: Self) -> Result<(), Self> {
        let ptr;

        if other.is_empty() {
            return Ok(());
        }

        unsafe {
            ptr = self.ptr.offset(self.len as isize);
        }
        if ptr == other.ptr && self.kind() == KIND_ARC && other.kind() == KIND_ARC {
            debug_assert_eq!(self.arc.load(Acquire), other.arc.load(Acquire));
            // Contiguous blocks, just combine directly
            self.len += other.len;
            self.cap += other.cap;
            Ok(())
        } else {
            Err(other)
        }
    }

    fn resize(&mut self, new_len: usize, value: u8) {
        let len = self.len();
        if new_len > len {
            let additional = new_len - len;
            self.reserve(additional);
            unsafe {
                let dst = self.as_raw()[len..].as_mut_ptr();
                ptr::write_bytes(dst, value, additional);
                self.set_len(new_len);
            }
        } else {
            self.truncate(new_len);
        }
    }

    unsafe fn set_start(&mut self, start: usize) {
        // Setting the start to 0 is a no-op, so return early if this is the
        // case.
        if start == 0 {
            return;
        }

        let kind = self.kind();

        // Always check `inline` first, because if the handle is using inline
        // data storage, all of the `Inner` struct fields will be gibberish.
        if kind == KIND_INLINE {
            assert!(start <= INLINE_CAP);

            let len = self.inline_len();

            if len <= start {
                self.set_inline_len(0);
            } else {
                // `set_start` is essentially shifting data off the front of the
                // view. Inlined buffers only track the length of the slice.
                // So, to update the start, the data at the new starting point
                // is copied to the beginning of the buffer.
                let new_len = len - start;

                let dst = self.inline_ptr();
                let src = (dst as *const u8).offset(start as isize);

                ptr::copy(src, dst, new_len);

                self.set_inline_len(new_len);
            }
        } else {
            assert!(start <= self.cap);

            if kind == KIND_VEC {
                // Setting the start when in vec representation is a little more
                // complicated. First, we have to track how far ahead the
                // "start" of the byte buffer from the beginning of the vec. We
                // also have to ensure that we don't exceed the maximum shift.
                let (mut pos, prev) = self.uncoordinated_get_vec_pos();
                pos += start;

                if pos <= MAX_VEC_POS {
                    self.uncoordinated_set_vec_pos(pos, prev);
                } else {
                    // The repr must be upgraded to ARC. This will never happen
                    // on 64 bit systems and will only happen on 32 bit systems
                    // when shifting past 134,217,727 bytes. As such, we don't
                    // worry too much about performance here.
                    let _ = self.shallow_clone(true);
                }
            }

            // Updating the start of the view is setting `ptr` to point to the
            // new start and updating the `len` field to reflect the new length
            // of the view.
            self.ptr = self.ptr.offset(start as isize);

            if self.len >= start {
                self.len -= start;
            } else {
                self.len = 0;
            }

            self.cap -= start;
        }
    }

    unsafe fn set_end(&mut self, end: usize) {
        debug_assert!(self.is_shared());

        // Always check `inline` first, because if the handle is using inline
        // data storage, all of the `Inner` struct fields will be gibberish.
        if self.is_inline() {
            assert!(end <= INLINE_CAP);
            let new_len = cmp::min(self.inline_len(), end);
            self.set_inline_len(new_len);
        } else {
            assert!(end <= self.cap);

            self.cap = end;
            self.len = cmp::min(self.len, end);
        }
    }

    /// Checks if it is safe to mutate the memory
    fn is_mut_safe(&mut self) -> bool {
        let kind = self.kind();

        // Always check `inline` first, because if the handle is using inline
        // data storage, all of the `Inner` struct fields will be gibberish.
        if kind == KIND_INLINE {
            // Inlined buffers can always be mutated as the data is never shared
            // across handles.
            true
        } else if kind == KIND_VEC {
            true
        } else if kind == KIND_STATIC {
            false
        } else {
            // Otherwise, the underlying buffer is potentially shared with other
            // handles, so the ref_count needs to be checked.
            unsafe { (**self.arc.get_mut()).is_unique() }
        }
    }

    /// Increments the ref count. This should only be done if it is known that
    /// it can be done safely. As such, this fn is not public, instead other
    /// fns will use this one while maintaining the guarantees.
    /// Parameter `mut_self` should only be set to `true` if caller holds
    /// `&mut self` reference.
    ///
    /// "Safely" is defined as not exposing two `BytesMut` values that point to
    /// the same byte window.
    ///
    /// This function is thread safe.
    unsafe fn shallow_clone(&self, mut_self: bool) -> Self {
        // Always check `inline` first, because if the handle is using inline
        // data storage, all of the `Inner` struct fields will be gibberish.
        //
        // Additionally, if kind is STATIC, then Arc is *never* changed, making
        // it safe and faster to check for it now before an atomic acquire.

        if self.is_inline_or_static() {
            // In this case, a shallow_clone still involves copying the data.
            let mut inner: Self = mem::uninitialized();
            ptr::copy_nonoverlapping(self, &mut inner, 1);
            inner
        } else {
            self.shallow_clone_sync(mut_self)
        }
    }

    #[cold]
    unsafe fn shallow_clone_sync(&self, mut_self: bool) -> Self {
        // The function requires `&self`, this means that `shallow_clone`
        // could be called concurrently.
        //
        // The first step is to load the value of `arc`. This will determine
        // how to proceed. The `Acquire` ordering synchronizes with the
        // `compare_and_swap` that comes later in this function. The goal is
        // to ensure that if `arc` is currently set to point to a `Shared`,
        // that the current thread acquires the associated memory.
        let arc = self.arc.load(Acquire);
        let kind = arc as usize & KIND_MASK;

        if kind == KIND_ARC {
            self.shallow_clone_arc(arc)
        } else {
            assert_eq!(kind, KIND_VEC);
            self.shallow_clone_vec(arc as usize, mut_self)
        }
    }

    unsafe fn shallow_clone_arc(&self, arc: *mut Shared<P::RefCount>) -> Self {
        debug_assert!(arc as usize & KIND_MASK == KIND_ARC);

        let old_size = (*arc).ref_count.fetch_inc(Relaxed);

        if old_size == usize::MAX {
            abort();
        }

        Inner {
            arc: P::new(arc),
            ..*self
        }
    }

    #[cold]
    unsafe fn shallow_clone_vec(&self, arc: usize, mut_self: bool) -> Self {
        // If  the buffer is still tracked in a `Vec<u8>`. It is time to
        // promote the vec to an `Arc`. This could potentially be called
        // concurrently, so some care must be taken.

        debug_assert!(arc & KIND_MASK == KIND_VEC);

        let original_capacity_repr =
            (arc as usize & ORIGINAL_CAPACITY_MASK) >> ORIGINAL_CAPACITY_OFFSET;

        // The vec offset cannot be concurrently mutated, so there
        // should be no danger reading it.
        let off = (arc as usize) >> VEC_POS_OFFSET;

        // First, allocate a new `Shared` instance containing the
        // `Vec` fields. It's important to note that `ptr`, `len`,
        // and `cap` cannot be mutated without having `&mut self`.
        // This means that these fields will not be concurrently
        // updated and since the buffer hasn't been promoted to an
        // `Arc`, those three fields still are the components of the
        // vector.
        let shared = Box::new(Shared {
            vec: rebuild_vec(self.ptr, self.len, self.cap, off),
            original_capacity_repr,
            // Initialize ref_count to 2. One for this reference, and one
            // for the new clone that will be returned from
            // `shallow_clone`.
            ref_count: P::RefCount::new(2),
        });

        let shared = Box::into_raw(shared);

        // The pointer should be aligned, so this assert should
        // always succeed.
        debug_assert!(0 == (shared as usize & 0b11));

        // If there are no references to self in other threads,
        // expensive atomic operations can be avoided.
        if mut_self {
            self.arc.store(shared, Relaxed);
            return Inner {
                arc: P::new(shared),
                ..*self
            };
        }

        // Try compare & swapping the pointer into the `arc` field.
        // `Release` is used synchronize with other threads that
        // will load the `arc` field.
        //
        // If the `compare_and_swap` fails, then the thread lost the
        // race to promote the buffer to shared. The `Acquire`
        // ordering will synchronize with the `compare_and_swap`
        // that happened in the other thread and the `Shared`
        // pointed to by `actual` will be visible.
        let actual = self
            .arc
            .compare_and_swap(arc as *mut Shared<P::RefCount>, shared, AcqRel);

        if actual as usize == arc {
            // The upgrade was successful, the new handle can be
            // returned.
            return Inner {
                arc: P::new(shared),
                ..*self
            };
        }

        // The upgrade failed, a concurrent clone happened. Release
        // the allocation that was made in this thread, it will not
        // be needed.
        let shared = Box::from_raw(shared);
        mem::forget(*shared);

        // Buffer already promoted to shared storage, so increment ref
        // count.
        self.shallow_clone_arc(actual)
    }

    #[inline]
    fn reserve(&mut self, additional: usize) {
        let len = self.len();
        let rem = self.capacity() - len;

        if additional <= rem {
            // The handle can already store at least `additional` more bytes, so
            // there is no further work needed to be done.
            return;
        }

        let kind = self.kind();

        // Always check `inline` first, because if the handle is using inline
        // data storage, all of the `Inner` struct fields will be gibberish.
        if kind == KIND_INLINE {
            let new_cap = len + additional;

            // Promote to a vector
            let mut v = Vec::with_capacity(new_cap);
            v.extend_from_slice(self.as_ref());

            self.ptr = v.as_mut_ptr();
            self.len = v.len();
            self.cap = v.capacity();

            // Since the minimum capacity is `INLINE_CAP`, don't bother encoding
            // the original capacity as INLINE_CAP
            self.arc = P::new(KIND_VEC as *mut Shared<P::RefCount>);

            mem::forget(v);
            return;
        }

        if kind == KIND_VEC {
            // If there's enough free space before the start of the buffer, then
            // just copy the data backwards and reuse the already-allocated
            // space.
            //
            // Otherwise, since backed by a vector, use `Vec::reserve`
            unsafe {
                let (off, prev) = self.uncoordinated_get_vec_pos();

                // Only reuse space if we stand to gain at least capacity/2
                // bytes of space back
                if off >= additional && off >= (self.cap / 2) {
                    // There's space - reuse it
                    //
                    // Just move the pointer back to the start after copying
                    // data back.
                    let base_ptr = self.ptr.offset(-(off as isize));
                    ptr::copy(self.ptr, base_ptr, self.len);
                    self.ptr = base_ptr;
                    self.uncoordinated_set_vec_pos(0, prev);

                    // Length stays constant, but since we moved backwards we
                    // can gain capacity back.
                    self.cap += off;
                } else {
                    // No space - allocate more
                    let mut v = rebuild_vec(self.ptr, self.len, self.cap, off);
                    v.reserve(additional);

                    // Update the info
                    self.ptr = v.as_mut_ptr().offset(off as isize);
                    self.len = v.len() - off;
                    self.cap = v.capacity() - off;

                    // Drop the vec reference
                    mem::forget(v);
                }
                return;
            }
        }

        let arc = *self.arc.get_mut();

        debug_assert!(kind == KIND_ARC);

        // Reserving involves abandoning the currently shared buffer and
        // allocating a new vector with the requested capacity.
        //
        // Compute the new capacity
        let mut new_cap = len + additional;
        let original_capacity;
        let original_capacity_repr;

        unsafe {
            original_capacity_repr = (*arc).original_capacity_repr;
            original_capacity = original_capacity_from_repr(original_capacity_repr);

            // First, try to reclaim the buffer. This is possible if the current
            // handle is the only outstanding handle pointing to the buffer.
            if (*arc).is_unique() {
                // This is the only handle to the buffer. It can be reclaimed.
                // However, before doing the work of copying data, check to make
                // sure that the vector has enough capacity.
                let v = &mut (*arc).vec;

                if v.capacity() >= new_cap {
                    // The capacity is sufficient, reclaim the buffer
                    let ptr = v.as_mut_ptr();

                    ptr::copy(self.ptr, ptr, len);

                    self.ptr = ptr;
                    self.cap = v.capacity();

                    return;
                }

                // The vector capacity is not sufficient. The reserve request is
                // asking for more than the initial buffer capacity. Allocate more
                // than requested if `new_cap` is not much bigger than the current
                // capacity.
                //
                // There are some situations, using `reserve_exact` that the
                // buffer capacity could be below `original_capacity`, so do a
                // check.
                new_cap = cmp::max(cmp::max(v.capacity() << 1, new_cap), original_capacity);
            } else {
                new_cap = cmp::max(new_cap, original_capacity);
            }
        }

        // Create a new vector to store the data
        let mut v = Vec::with_capacity(new_cap);

        // Copy the bytes
        v.extend_from_slice(self.as_ref());

        // Release the shared handle. This must be done *after* the bytes are
        // copied.
        P::RefCount::release_shared(arc);

        // Update self
        self.ptr = v.as_mut_ptr();
        self.len = v.len();
        self.cap = v.capacity();

        let arc = (original_capacity_repr << ORIGINAL_CAPACITY_OFFSET) | KIND_VEC;

        self.arc = P::new(arc as *mut Shared<P::RefCount>);

        // Forget the vector handle
        mem::forget(v);
    }

    /// Returns true if the buffer is stored inline
    #[inline]
    fn is_inline(&self) -> bool {
        self.kind() == KIND_INLINE
    }

    #[inline]
    fn is_inline_or_static(&self) -> bool {
        // The value returned by `kind` isn't itself safe, but the value could
        // inform what operations to take, and unsafely do something without
        // synchronization.
        //
        // KIND_INLINE and KIND_STATIC will *never* change, so branches on that
        // information is safe.
        let kind = self.kind();
        kind == KIND_INLINE || kind == KIND_STATIC
    }

    /// Used for `debug_assert` statements. &mut is used to guarantee that it is
    /// safe to check VEC_KIND
    #[inline]
    fn is_shared(&mut self) -> bool {
        match self.kind() {
            KIND_VEC => false,
            _ => true,
        }
    }

    /// Used for `debug_assert` statements
    #[inline]
    fn is_static(&mut self) -> bool {
        match self.kind() {
            KIND_STATIC => true,
            _ => false,
        }
    }

    #[inline]
    fn kind(&self) -> usize {
        // This function is going to probably raise some eyebrows. The function
        // returns true if the buffer is stored inline. This is done by checking
        // the least significant bit in the `arc` field.
        //
        // Now, you may notice that `arc` is an `AtomicPtr` and this is
        // accessing it as a normal field without performing an atomic load...
        //
        // Again, the function only cares about the least significant bit, and
        // this bit is set when `Inner` is created and never changed after that.
        // All platforms have atomic "word" operations and won't randomly flip
        // bits, so even without any explicit atomic operations, reading the
        // flag will be correct.
        //
        // This function is very critical performance wise as it is called for
        // every operation. Performing an atomic load would mess with the
        // compiler's ability to optimize. Simple benchmarks show up to a 10%
        // slowdown using a `Relaxed` atomic load on x86.

        #[cfg(target_endian = "little")]
        #[inline]
        fn imp<P: SharedPtr>(arc: &P) -> usize {
            unsafe {
                let p: &u8 = mem::transmute(arc);
                (*p as usize) & KIND_MASK
            }
        }

        #[cfg(target_endian = "big")]
        #[inline]
        fn imp<P: SharedPtr>(arc: &P) -> usize {
            unsafe {
                let p: &usize = mem::transmute(arc);
                *p & KIND_MASK
            }
        }

        imp(&self.arc)
    }

    #[inline]
    fn uncoordinated_get_vec_pos(&mut self) -> (usize, usize) {
        // Similar to above, this is a pretty crazed function. This should only
        // be called when in the KIND_VEC mode. This + the &mut self argument
        // guarantees that there is no possibility of concurrent calls to this
        // function.
        let prev = unsafe {
            let p: &P = &self.arc;
            let p: &usize = mem::transmute(p);
            *p
        };

        (prev >> VEC_POS_OFFSET, prev)
    }

    #[inline]
    fn uncoordinated_set_vec_pos(&mut self, pos: usize, prev: usize) {
        // Once more... crazy
        debug_assert!(pos <= MAX_VEC_POS);

        unsafe {
            let p: &mut P = &mut self.arc;
            let p: &mut usize = mem::transmute(p);
            *p = (pos << VEC_POS_OFFSET) | (prev & NOT_VEC_POS_MASK);
        }
    }
}

fn rebuild_vec(ptr: *mut u8, mut len: usize, mut cap: usize, off: usize) -> Vec<u8> {
    unsafe {
        let ptr = ptr.offset(-(off as isize));
        len += off;
        cap += off;

        Vec::from_raw_parts(ptr, len, cap)
    }
}

impl<P: SharedPtr> Drop for Inner<P> {
    fn drop(&mut self) {
        let kind = self.kind();

        if kind == KIND_VEC {
            let (off, _) = self.uncoordinated_get_vec_pos();

            // Vector storage, free the vector
            drop(rebuild_vec(self.ptr, self.len, self.cap, off));
        } else if kind == KIND_ARC {
            P::RefCount::release_shared(*self.arc.get_mut());
        }
    }
}

impl<C: RefCount> Shared<C> {
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
        self.ref_count.load(Acquire) == 1
    }
}

fn original_capacity_to_repr(cap: usize) -> usize {
    let width = PTR_WIDTH - ((cap >> MIN_ORIGINAL_CAPACITY_WIDTH).leading_zeros() as usize);
    cmp::min(
        width,
        MAX_ORIGINAL_CAPACITY_WIDTH - MIN_ORIGINAL_CAPACITY_WIDTH,
    )
}

fn original_capacity_from_repr(repr: usize) -> usize {
    if repr == 0 {
        return 0;
    }

    1 << (repr + (MIN_ORIGINAL_CAPACITY_WIDTH - 1))
}

#[test]
fn test_original_capacity_to_repr() {
    for &cap in &[0, 1, 16, 1000] {
        assert_eq!(0, original_capacity_to_repr(cap));
    }

    for &cap in &[1024, 1025, 1100, 2000, 2047] {
        assert_eq!(1, original_capacity_to_repr(cap));
    }

    for &cap in &[2048, 2049] {
        assert_eq!(2, original_capacity_to_repr(cap));
    }

    // TODO: more

    for &cap in &[65536, 65537, 68000, 1 << 17, 1 << 18, 1 << 20, 1 << 30] {
        assert_eq!(7, original_capacity_to_repr(cap), "cap={}", cap);
    }
}

#[test]
fn test_original_capacity_from_repr() {
    assert_eq!(0, original_capacity_from_repr(0));
    assert_eq!(1024, original_capacity_from_repr(1));
    assert_eq!(1024 * 2, original_capacity_from_repr(2));
    assert_eq!(1024 * 4, original_capacity_from_repr(3));
    assert_eq!(1024 * 8, original_capacity_from_repr(4));
    assert_eq!(1024 * 16, original_capacity_from_repr(5));
    assert_eq!(1024 * 32, original_capacity_from_repr(6));
    assert_eq!(1024 * 64, original_capacity_from_repr(7));
}

unsafe impl Send for Inner<AtomicPtr<Shared<AtomicUsize>>> {}
unsafe impl Sync for Inner<AtomicPtr<Shared<AtomicUsize>>> {}

// While there is `std::process:abort`, it's only available in Rust 1.17, and
// our minimum supported version is currently 1.15. So, this acts as an abort
// by triggering a double panic, which always aborts in Rust.
struct Abort;

impl Drop for Abort {
    fn drop(&mut self) {
        panic!();
    }
}

#[inline(never)]
#[cold]
fn abort() {
    let _a = Abort;
    panic!();
}
