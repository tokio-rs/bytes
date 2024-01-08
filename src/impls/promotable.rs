use crate::shared_buf::{BufferParts, SharedBuf};
use alloc::{
    alloc::{dealloc, Layout},
    boxed::Box,
    vec::Vec,
};
use core::{mem, ptr, usize};

use super::shared::{self, SharedImpl};
#[allow(unused)]
use crate::loom::sync::atomic::AtomicMut;
use crate::loom::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
const KIND_ARC: usize = 0b0;
const KIND_VEC: usize = 0b1;
const KIND_MASK: usize = 0b1;

// ===== impl PromotableVtable =====

pub(crate) struct PromotableEvenImpl(pub Promotable);

pub(crate) struct PromotableOddImpl(pub Promotable);

pub(crate) enum Promotable {
    Owned(Box<[u8]>),
    Shared(SharedImpl),
}

unsafe impl SharedBuf for PromotableEvenImpl {
    fn into_parts(this: Self) -> (AtomicPtr<()>, *const u8, usize) {
        let slice = match this.0 {
            Promotable::Owned(slice) => slice,
            Promotable::Shared(shared) => return SharedImpl::into_parts(shared),
        };

        let len = slice.len();
        let ptr = Box::into_raw(slice) as *mut u8;
        assert!(ptr as usize & 0x1 == 0);

        let data = ptr_map(ptr, |addr| addr | KIND_VEC);

        (AtomicPtr::new(data.cast()), ptr, len)
    }

    unsafe fn from_parts(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Self {
        PromotableEvenImpl(promotable_from_bytes_parts(data, ptr, len, |shared| {
            ptr_map(shared.cast(), |addr| addr & !KIND_MASK)
        }))
    }

    unsafe fn clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BufferParts {
        let shared = data.load(Ordering::Acquire);
        let kind = shared as usize & KIND_MASK;

        if kind == KIND_ARC {
            shared::shallow_clone_arc(shared.cast(), ptr, len)
        } else {
            debug_assert_eq!(kind, KIND_VEC);
            let buf = ptr_map(shared.cast(), |addr| addr & !KIND_MASK);
            shallow_clone_vec(data, shared, buf, ptr, len)
        }
    }

    unsafe fn try_resize(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) {
        // The Vec "promotable" vtables do not store the capacity,
        // so we cannot truncate while using this repr. We *have* to
        // promote using `clone` so the capacity can be stored.
        drop(PromotableEvenImpl::clone(&*data, ptr, len));
    }

    unsafe fn into_vec(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
        promotable_into_vec(data, ptr, len, |shared| {
            ptr_map(shared.cast(), |addr| addr & !KIND_MASK)
        })
    }

    unsafe fn drop(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) {
        data.with_mut(|shared| {
            let shared = *shared;
            let kind = shared as usize & KIND_MASK;

            if kind == KIND_ARC {
                shared::release_shared(shared.cast());
            } else {
                debug_assert_eq!(kind, KIND_VEC);
                let buf = ptr_map(shared.cast(), |addr| addr & !KIND_MASK);
                free_boxed_slice(buf, ptr, len);
            }
        });
    }
}

unsafe fn promotable_from_bytes_parts(
    data: &mut AtomicPtr<()>,
    ptr: *const u8,
    len: usize,
    f: fn(*mut ()) -> *mut u8,
) -> Promotable {
    let shared = data.with_mut(|p| *p);
    let kind = shared as usize & KIND_MASK;

    if kind == KIND_ARC {
        Promotable::Shared(SharedImpl::from_parts(data, ptr, len))
    } else {
        debug_assert_eq!(kind, KIND_VEC);

        let buf = f(shared);

        let cap = (ptr as usize - buf as usize) + len;

        let vec = Vec::from_raw_parts(buf, cap, cap);

        Promotable::Owned(vec.into_boxed_slice())
    }
}

unsafe fn promotable_into_vec(
    data: &mut AtomicPtr<()>,
    ptr: *const u8,
    len: usize,
    f: fn(*mut ()) -> *mut u8,
) -> Vec<u8> {
    let shared = data.with_mut(|p| *p);
    let kind = shared as usize & KIND_MASK;

    if kind == KIND_ARC {
        shared::shared_into_vec_impl(shared.cast(), ptr, len)
    } else {
        // If Bytes holds a Vec, then the offset must be 0.
        debug_assert_eq!(kind, KIND_VEC);

        let buf = f(shared);

        let cap = (ptr as usize - buf as usize) + len;

        // Copy back buffer
        ptr::copy(ptr, buf, len);

        Vec::from_raw_parts(buf, len, cap)
    }
}

unsafe impl SharedBuf for PromotableOddImpl {
    fn into_parts(this: Self) -> BufferParts {
        let slice = match this.0 {
            Promotable::Owned(slice) => slice,
            Promotable::Shared(shared) => return SharedImpl::into_parts(shared),
        };

        let len = slice.len();
        let ptr = Box::into_raw(slice) as *mut u8;
        assert!(ptr as usize & 0x1 == 1);

        (AtomicPtr::new(ptr.cast()), ptr, len)
    }

    unsafe fn from_parts(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Self {
        PromotableOddImpl(promotable_from_bytes_parts(data, ptr, len, |shared| {
            shared.cast()
        }))
    }

    unsafe fn clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BufferParts {
        let shared = data.load(Ordering::Acquire);
        let kind = shared as usize & KIND_MASK;

        if kind == KIND_ARC {
            shared::shallow_clone_arc(shared as _, ptr, len)
        } else {
            debug_assert_eq!(kind, KIND_VEC);
            shallow_clone_vec(data, shared, shared.cast(), ptr, len)
        }
    }

    unsafe fn try_resize(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) {
        // The Vec "promotable" vtables do not store the capacity,
        // so we cannot truncate while using this repr. We *have* to
        // promote using `clone` so the capacity can be stored.
        drop(PromotableOddImpl::clone(&*data, ptr, len));
    }

    unsafe fn into_vec(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
        promotable_into_vec(data, ptr, len, |shared| shared.cast())
    }

    unsafe fn drop(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) {
        data.with_mut(|shared| {
            let shared = *shared;
            let kind = shared as usize & KIND_MASK;

            if kind == KIND_ARC {
                shared::release_shared(shared.cast());
            } else {
                debug_assert_eq!(kind, KIND_VEC);

                free_boxed_slice(shared.cast(), ptr, len);
            }
        });
    }
}

unsafe fn free_boxed_slice(buf: *mut u8, offset: *const u8, len: usize) {
    let cap = (offset as usize - buf as usize) + len;
    dealloc(buf, Layout::from_size_align(cap, 1).unwrap())
}

// Ideally we would always use this version of `ptr_map` since it is strict
// provenance compatible, but it results in worse codegen. We will however still
// use it on miri because it gives better diagnostics for people who test bytes
// code with miri.
//
// See https://github.com/tokio-rs/bytes/pull/545 for more info.
#[cfg(miri)]
fn ptr_map<F>(ptr: *mut u8, f: F) -> *mut u8
where
    F: FnOnce(usize) -> usize,
{
    let old_addr = ptr as usize;
    let new_addr = f(old_addr);
    let diff = new_addr.wrapping_sub(old_addr);
    ptr.wrapping_add(diff)
}

#[cfg(not(miri))]
fn ptr_map<F>(ptr: *mut u8, f: F) -> *mut u8
where
    F: FnOnce(usize) -> usize,
{
    let old_addr = ptr as usize;
    let new_addr = f(old_addr);
    new_addr as *mut u8
}

#[cold]
unsafe fn shallow_clone_vec(
    atom: &AtomicPtr<()>,
    ptr: *const (),
    buf: *mut u8,
    offset: *const u8,
    len: usize,
) -> BufferParts {
    // If  the buffer is still tracked in a `Vec<u8>`. It is time to
    // promote the vec to an `Arc`. This could potentially be called
    // concurrently, so some care must be taken.

    // First, allocate a new `Shared` instance containing the
    // `Vec` fields. It's important to note that `ptr`, `len`,
    // and `cap` cannot be mutated without having `&mut self`.
    // This means that these fields will not be concurrently
    // updated and since the buffer hasn't been promoted to an
    // `Arc`, those three fields still are the components of the
    // vector.
    let shared = Box::new(shared::Shared {
        buf,
        cap: (offset as usize - buf as usize) + len,
        // Initialize refcount to 2. One for this reference, and one
        // for the new clone that will be returned from
        // `shallow_clone`.
        ref_cnt: AtomicUsize::new(2),
    });

    let shared = Box::into_raw(shared);

    // The pointer should be aligned, so this assert should
    // always succeed.
    debug_assert!(
        0 == (shared as usize & KIND_MASK),
        "internal: Box<shared::Shared> should have an aligned pointer",
    );

    // Try compare & swapping the pointer into the `arc` field.
    // `Release` is used synchronize with other threads that
    // will load the `arc` field.
    //
    // If the `compare_exchange` fails, then the thread lost the
    // race to promote the buffer to shared. The `Acquire`
    // ordering will synchronize with the `compare_exchange`
    // that happened in the other thread and the `Shared`
    // pointed to by `actual` will be visible.
    match atom.compare_exchange(ptr as _, shared as _, Ordering::AcqRel, Ordering::Acquire) {
        Ok(actual) => {
            debug_assert!(actual as usize == ptr as usize);
            // The upgrade was successful, the new handle can be
            // returned.
            (AtomicPtr::new(shared.cast()), offset, len)
        }
        Err(actual) => {
            // The upgrade failed, a concurrent clone happened. Release
            // the allocation that was made in this thread, it will not
            // be needed.
            let shared = Box::from_raw(shared);
            mem::forget(*shared);

            // Buffer already promoted to shared storage, so increment ref
            // count.
            shared::shallow_clone_arc(actual as _, offset, len)
        }
    }
}
