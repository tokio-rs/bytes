use alloc::{Mem, MemRef};
use stable_heap as heap;
use std::{mem, ptr, isize, usize};
use std::sync::atomic::{self, AtomicUsize, Ordering};

const MAX_ALLOC_SIZE: usize = usize::MAX;
const MAX_REFCOUNT: usize = (isize::MAX) as usize;

/// Tracks a heap allocation and stores the atomic ref counter
struct Allocation {
    refs: AtomicUsize,
}

pub fn allocate(len: usize) -> MemRef {
    // Make sure that the allocation is within the permitted range
    if len > MAX_ALLOC_SIZE {
        return MemRef::none();
    }

    unsafe {
        let mut ptr = heap::allocate(alloc_len(len), align());
        let mut off = 0;

        ptr::write(ptr as *mut Allocation, Allocation::new());

        off += mem::size_of::<Allocation>();
        ptr::write(ptr.offset(off as isize) as *mut &Mem, &*(ptr as *const Allocation));

        off += mem::size_of::<&Mem>();
        ptr::write(ptr.offset(off as isize) as *mut usize, len);

        ptr = ptr.offset(mem::size_of::<Allocation>() as isize);

        MemRef::new(ptr)
    }
}

fn deallocate(ptr: *mut u8) {
    unsafe {
        let off = mem::size_of::<Allocation>() + mem::size_of::<&Mem>();
        let len = ptr::read(ptr.offset(off as isize) as *const usize);

        heap::deallocate(ptr, alloc_len(len), align());
    }
}

impl Allocation {
    fn new() -> Allocation {
        Allocation {
            refs: AtomicUsize::new(1),
        }
    }
}

impl Mem for Allocation {
    fn ref_inc(&self) {
        // Using a relaxed ordering is alright here, as knowledge of the
        // original reference prevents other threads from erroneously deleting
        // the object.
        //
        // As explained in the [Boost documentation][1], Increasing the
        // reference counter can always be done with memory_order_relaxed: New
        // references to an object can only be formed from an existing
        // reference, and passing an existing reference from one thread to
        // another must already provide any required synchronization.
        //
        // [1]: (www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html)
        let old_size = self.refs.fetch_add(1, Ordering::Relaxed);

        // However we need to guard against massive refcounts in case someone
        // is `mem::forget`ing Arcs. If we don't do this the count can overflow
        // and users will use-after free. We racily saturate to `isize::MAX` on
        // the assumption that there aren't ~2 billion threads incrementing
        // the reference count at once. This branch will never be taken in
        // any realistic program.
        //
        // We abort because such a program is incredibly degenerate, and we
        // don't care to support it.
        if old_size > MAX_REFCOUNT {
            panic!("too many refs");
        }
    }

    fn ref_dec(&self) {
        if self.refs.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }

        atomic::fence(Ordering::Acquire);
        deallocate(self as *const Allocation as *const u8 as *mut u8);
    }
}

#[inline]
fn alloc_len(bytes_len: usize) -> usize {
    let len = bytes_len +
        mem::size_of::<Allocation>() +
        mem::size_of::<&Mem>() +
        mem::size_of::<usize>();

    if len & (align() - 1) == 0 {
        len
    } else {
        (len & !align()) + align()
    }
}

#[inline]
fn align() -> usize {
    mem::size_of::<usize>()
}
