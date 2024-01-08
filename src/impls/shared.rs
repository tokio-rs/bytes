#[allow(unused)]
use crate::loom::sync::atomic::AtomicMut;
use crate::loom::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use crate::shared_buf::{BufferParts, SharedBuf};
use alloc::{
    alloc::{dealloc, Layout},
    boxed::Box,
    vec::Vec,
};
use core::{mem, ptr, slice, usize};

// ===== impl SharedVtable =====

pub(crate) struct Shared {
    // Holds arguments to dealloc upon Drop, but otherwise doesn't use them
    pub(crate) buf: *mut u8,
    pub(crate) cap: usize,
    pub(crate) ref_cnt: AtomicUsize,
}

impl Drop for Shared {
    fn drop(&mut self) {
        unsafe { dealloc(self.buf, Layout::from_size_align(self.cap, 1).unwrap()) }
    }
}

// Assert that the alignment of `Shared` is divisible by 2.
// This is a necessary invariant since we depend on allocating `Shared` a
// shared object to implicitly carry the `KIND_ARC` flag in its pointer.
// This flag is set when the LSB is 0.
const _: [(); 0 - mem::align_of::<Shared>() % 2] = []; // Assert that the alignment of `Shared` is divisible by 2.

pub(crate) struct SharedImpl {
    shared: *mut Shared,
    offset: *const u8,
    len: usize,
}

impl SharedImpl {
    pub(crate) fn new(shared: *mut Shared, ptr: *const u8, len: usize) -> Self {
        SharedImpl {
            shared,
            offset: ptr,
            len,
        }
    }
}

unsafe impl SharedBuf for SharedImpl {
    fn into_parts(this: Self) -> (AtomicPtr<()>, *const u8, usize) {
        (AtomicPtr::new(this.shared.cast()), this.offset, this.len)
    }

    unsafe fn from_parts(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Self {
        SharedImpl {
            shared: (data.with_mut(|p| *p)).cast(),
            offset: ptr,
            len,
        }
    }

    unsafe fn clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BufferParts {
        let shared = data.load(Ordering::Relaxed);
        shallow_clone_arc(shared as _, ptr, len)
    }

    unsafe fn into_vec(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
        shared_into_vec_impl((data.with_mut(|p| *p)).cast(), ptr, len)
    }

    unsafe fn drop(data: &mut AtomicPtr<()>, _ptr: *const u8, _len: usize) {
        data.with_mut(|shared| {
            release_shared(shared.cast());
        });
    }
}

pub(crate) unsafe fn shared_into_vec_impl(
    shared: *mut Shared,
    ptr: *const u8,
    len: usize,
) -> Vec<u8> {
    // Check that the ref_cnt is 1 (unique).
    //
    // If it is unique, then it is set to 0 with AcqRel fence for the same
    // reason in release_shared.
    //
    // Otherwise, we take the other branch and call release_shared.
    if (*shared)
        .ref_cnt
        .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
    {
        let buf = (*shared).buf;
        let cap = (*shared).cap;

        // Deallocate Shared
        drop(Box::from_raw(shared as *mut mem::ManuallyDrop<Shared>));

        // Copy back buffer
        ptr::copy(ptr, buf, len);

        Vec::from_raw_parts(buf, len, cap)
    } else {
        let v = slice::from_raw_parts(ptr, len).to_vec();
        release_shared(shared);
        v
    }
}

pub(crate) unsafe fn shallow_clone_arc(
    shared: *mut Shared,
    ptr: *const u8,
    len: usize,
) -> BufferParts {
    let old_size = (*shared).ref_cnt.fetch_add(1, Ordering::Relaxed);

    if old_size > usize::MAX >> 1 {
        crate::abort();
    }

    let shared = AtomicPtr::new(shared.cast());
    (shared, ptr, len)
}

pub(crate) unsafe fn release_shared(ptr: *mut Shared) {
    // `Shared` storage... follow the drop steps from Arc.
    if (*ptr).ref_cnt.fetch_sub(1, Ordering::Release) != 1 {
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
    //
    // Thread sanitizer does not support atomic fences. Use an atomic load
    // instead.
    (*ptr).ref_cnt.load(Ordering::Acquire);

    // Drop the data
    drop(Box::from_raw(ptr));
}
