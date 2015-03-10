use std::{mem, ptr};
use std::rt::heap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::usize;

const MAX_ALLOC_SIZE: usize = usize::MAX;

/// Allocates memory to be used by Bufs or Bytes. Allows allocating memory
/// using alternate stratgies than the default Rust heap allocator. Also does
/// not require that allocations are continuous in memory.
///
/// For example, an alternate allocator could use a slab of 4kb chunks of
/// memory and return as many chunks as needed to satisfy the length
/// requirement.
pub trait Allocator: Sync + Send {

  /// Allocate memory. May or may not be contiguous.
  fn allocate(&self, len: usize) -> MemRef;

  /// Deallocate a chunk of memory
  fn deallocate(&self, mem: *mut Mem);
}

pub struct MemRef {
    ptr: *mut u8,
}

impl MemRef {
    pub fn new(mem: *mut Mem) -> MemRef {
        let ptr = mem as *mut u8;

        unsafe {
            MemRef {
                ptr: ptr.offset(mem::size_of::<Mem>() as isize),
            }
        }
    }

    #[inline]
    pub fn none() -> MemRef {
        MemRef { ptr: ptr::null_mut() }
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.ptr.is_null()
    }

    #[inline]
    pub fn ptr(&self) -> *mut u8 {
        self.ptr
    }

    pub fn bytes(&self) -> &[u8] {
        use std::raw::Slice;

        unsafe {
            mem::transmute(Slice {
                data: self.ptr(),
                len: self.mem().len,
            })
        }
    }

    #[inline]
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        unsafe { mem::transmute(self.bytes()) }
    }

    #[inline]
    fn mem_ptr(&self) -> *mut Mem {
        unsafe {
            self.ptr.offset(-(mem::size_of::<Mem>() as isize)) as *mut Mem
        }
    }

    #[inline]
    fn mem(&self) -> &Mem {
        unsafe {
            mem::transmute(self.mem_ptr())
        }
    }
}

impl Clone for MemRef {
    #[inline]
    fn clone(&self) -> MemRef {
        self.mem().refs.fetch_add(1, Ordering::Relaxed);
        MemRef { ptr: self.ptr }
    }
}

impl Drop for MemRef {
    fn drop(&mut self) {
        // Guard against the ref having already been dropped
        if self.ptr.is_null() { return; }

        // Decrement the ref count
        if 1 == self.mem().refs.fetch_sub(1, Ordering::Relaxed) {
            // Last ref dropped, free the memory
            unsafe {
                let alloc: &Allocator = mem::transmute(self.mem().allocator);
                alloc.deallocate(self.mem_ptr());
            }
        }
    }
}

unsafe impl Send for MemRef { }
unsafe impl Sync for MemRef { }

/// Memory allocated by an Allocator must be prefixed with Mem
pub struct Mem {
    // TODO: It should be possible to reduce the size of this struct
    allocator: *const Allocator,
    refs: AtomicUsize,
    len: usize,
}

impl Mem {
    fn new(len: usize, allocator: *const Allocator) -> Mem {
        Mem {
            allocator: allocator,
            refs: AtomicUsize::new(1),
            len: len,
        }
    }
}

pub static HEAP: Heap = Heap;

#[allow(missing_copy_implementations)]
pub struct Heap;

impl Heap {
    pub fn allocate(&self, len: usize) -> MemRef {
        // Make sure that the allocation is within the permitted range
        if len > MAX_ALLOC_SIZE {
            return MemRef::none();
        }

        let alloc_len = len + mem::size_of::<Mem>();

        unsafe {
            // Attempt to allocate the memory
            let ptr: *mut Mem = mem::transmute(
                heap::allocate(alloc_len, mem::min_align_of::<u8>()));

            // If failed, return None
            if ptr.is_null() {
                return MemRef::none();
            }

            // Write the mem header
            ptr::write(ptr, Mem::new(len, mem::transmute(self as &Allocator)));

            // Return the info
            MemRef::new(ptr)
        }
    }

    pub fn deallocate(&self, mem: *mut Mem) {
        unsafe {
            let m: &Mem = mem::transmute(mem);

            heap::deallocate(
                mem as *mut u8, m.len + mem::size_of::<Mem>(),
                mem::min_align_of::<u8>())
        }
    }
}

impl Allocator for Heap {
    fn allocate(&self, len: usize) -> MemRef {
        Heap::allocate(self, len)
    }

    fn deallocate(&self, mem: *mut Mem) {
        Heap::deallocate(self, mem)
    }
}
