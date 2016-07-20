use super::{Mem, MemRef};
use buf::{ByteBuf, MutByteBuf};
use stable_heap as heap;
use std::{mem, ptr, isize, usize};
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicPtr, AtomicUsize, Ordering};

// TODO: ensure that not Sync
pub struct Pool {
    inner: Arc<PoolInner>,
    marker: PhantomData<Cell<()>>,
}

struct PoolInner {
    ptr: *mut u8, // Pointer to the raw memory
    next: AtomicPtr<Entry>,
    cap: usize, // Total number of entries
    buf_len: usize, // Byte size of each byte buf
    entry_len: usize, // Byte size of each entry
}

struct Entry {
    inner: UnsafeCell<Inner>,
}

struct Inner {
    pool: Option<Pool>,
    refs: AtomicUsize,
    next: *mut Entry,
}

const MAX_REFCOUNT: usize = (isize::MAX) as usize;

impl Pool {
    /// Constructs a new `Pool` with with specified capacity such that each
    /// buffer in the pool has a length of `buf_len`.
    pub fn with_capacity(cap: usize, mut buf_len: usize) -> Pool {
        // Ensure that all buffers have a power of 2 size. This enables
        // optimizations in Buf implementations.
        buf_len = buf_len.next_power_of_two();

        let inner = Arc::new(PoolInner::with_capacity(cap, buf_len));

        // Iterate each entry and initialize the memory
        let mut next = ptr::null_mut();

        for i in 0..cap {
            unsafe {
                let off = i * inner.entry_len;
                let ptr = inner.ptr.offset(off as isize);
                let e = &mut *(ptr as *mut Entry);

                ptr::write(&mut e.inner as *mut UnsafeCell<Inner>, UnsafeCell::new(Inner {
                    pool: None,
                    refs: AtomicUsize::new(0),
                    next: next,
                }));

                next = ptr as *mut Entry;

                let ptr = ptr.offset(mem::size_of::<Entry>() as isize);
                ptr::write(ptr as *mut &Mem, e as &Mem);

                let ptr = ptr.offset(mem::size_of::<&Mem>() as isize);
                ptr::write(ptr as *mut usize, buf_len);
            }
        }

        // Set the next ptr to the head of the Entry linked list
        inner.next.store(next, Ordering::Relaxed);

        Pool {
            inner: inner,
            marker: PhantomData,
        }
    }

    /// Returns the number of buffers that the `Pool` holds.
    pub fn capacity(&self) -> usize {
        self.inner.cap
    }

    /// Returns a new `ByteBuf` backed by a buffer from the pool. If the pool
    /// is depleted, `None` is returned.
    pub fn new_byte_buf(&self) -> Option<MutByteBuf> {
        let len = self.inner.buf_len as u32;
        self.checkout().map(|mem| {
            let buf = unsafe { ByteBuf::from_mem_ref(mem, len, 0, len) };
            buf.flip()
        })
    }

    fn checkout(&self) -> Option<MemRef> {
        unsafe {
            let mut ptr = self.inner.next.load(Ordering::Acquire);

            loop {
                if ptr.is_null() {
                    // The pool is depleted
                    return None;
                }

                let inner = &*(*ptr).inner.get();

                let next = inner.next;

                let res = self.inner.next.compare_and_swap(ptr, next, Ordering::AcqRel);

                if res == ptr {
                    break;
                }

                ptr = res;
            }

            let inner = &mut *(*ptr).inner.get();

            // Unset next pointer & set the pool
            inner.next = ptr::null_mut();
            inner.refs.store(1, Ordering::Relaxed);
            inner.pool = Some(self.clone());

            let ptr = ptr as *mut u8;
            let ptr = ptr.offset(mem::size_of::<Entry>() as isize);

            Some(MemRef::new(ptr))
        }
    }

    fn clone(&self) -> Pool {
        Pool {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

impl PoolInner {
    fn with_capacity(cap: usize, buf_len: usize) -> PoolInner {
        let ptr = unsafe { heap::allocate(alloc_len(cap, buf_len), align()) };

        PoolInner {
            ptr: ptr,
            next: AtomicPtr::new(ptr::null_mut()),
            cap: cap,
            buf_len: buf_len,
            entry_len: entry_len(buf_len),
        }
    }
}

impl Drop for PoolInner {
    fn drop(&mut self) {
        unsafe { heap::deallocate(self.ptr, alloc_len(self.cap, self.buf_len), align()) }
    }
}

impl Entry {
    fn release(&self) {
        unsafe {
            let inner = &mut *self.inner.get();
            let pool = inner.pool.take()
                .expect("entry not associated with a pool");

            let mut next = pool.inner.next.load(Ordering::Acquire);

            loop {
                inner.next = next;

                let actual = pool.inner.next
                    .compare_and_swap(next, self as *const Entry as *mut Entry, Ordering::AcqRel);

                if actual == next {
                    break;
                }

                next = actual;
            }
        }
    }
}

impl Mem for Entry {
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
        let old_size = unsafe {
            (*self.inner.get()).refs.fetch_add(1, Ordering::Relaxed)
        };

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
        unsafe {
            let prev = (*self.inner.get()).refs.fetch_sub(1, Ordering::Release);

            if prev != 1 {
                return;
            }
        }

        atomic::fence(Ordering::Acquire);
        self.release();
    }
}

// TODO: is there a better way to do this?
unsafe impl Send for Entry {}
unsafe impl Sync for Entry {}

fn alloc_len(cap: usize, buf_len: usize) -> usize {
    cap * entry_len(buf_len)
}

fn entry_len(bytes_len: usize) -> usize {
    let len = bytes_len +
        mem::size_of::<Entry>() +
        mem::size_of::<&Mem>() +
        mem::size_of::<usize>();

    if len & (align() - 1) == 0 {
        len
    } else {
        (len & !align()) + align()
    }
}

fn align() -> usize {
    mem::size_of::<usize>()
}
