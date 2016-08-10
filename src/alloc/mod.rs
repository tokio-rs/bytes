mod heap;
mod pool;

pub use self::pool::Pool;

use std::{mem, ptr};

/// Ref-counted segment of memory
pub trait Mem: Send + Sync {
    /// Increment the ref count
    fn ref_inc(&self);

    /// Decrement the ref count
    fn ref_dec(&self);
}

pub struct MemRef {
    // Pointer to the memory
    // Layout:
    // - &Mem
    // - usize (len)
    // - u8... bytes
    ptr: *mut u8,
}

/// Allocate a segment of memory and return a `MemRef`.
pub fn heap(len: usize) -> MemRef {
    heap::allocate(len)
}

impl MemRef {
    #[inline]
    pub unsafe fn new(ptr: *mut u8) -> MemRef {
        MemRef { ptr: ptr }
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
    pub fn len(&self) -> usize {
        unsafe { *self.len_ptr() }
    }

    #[inline]
    pub unsafe fn bytes(&self) -> &[u8] {
        use std::slice;
        slice::from_raw_parts(self.bytes_ptr(), self.len())
    }

    #[inline]
    pub unsafe fn bytes_slice(&self, start: usize, end: usize) -> &[u8] {
        use std::slice;
        let ptr = self.bytes_ptr().offset(start as isize);
        slice::from_raw_parts(ptr, end - start)
    }

    #[inline]
    pub unsafe fn mut_bytes(&mut self) -> &mut [u8] {
        use std::slice;
        slice::from_raw_parts_mut(self.bytes_ptr(), self.len())
    }

    /// Unsafe, unchecked access to the bytes
    #[inline]
    pub unsafe fn mut_bytes_slice(&mut self, start: usize, end: usize) -> &mut [u8] {
        use std::slice;
        let ptr = self.bytes_ptr().offset(start as isize);
        slice::from_raw_parts_mut(ptr, end - start)
    }

    #[inline]
    fn mem(&self) -> &Mem {
        unsafe {
            *(self.ptr as *const &Mem)
        }
    }

    #[inline]
    unsafe fn len_ptr(&self) -> *mut usize {
        let off = mem::size_of::<&Mem>();
        self.ptr.offset(off as isize) as *mut usize
    }

    #[inline]
    unsafe fn bytes_ptr(&self) -> *mut u8 {
        let off = mem::size_of::<&Mem>() + mem::size_of::<usize>();
        self.ptr.offset(off as isize)
    }
}

impl Clone for MemRef {
    #[inline]
    fn clone(&self) -> MemRef {
        if self.is_none() {
            return MemRef::none();
        }

        self.mem().ref_inc();
        MemRef { ptr: self.ptr }
    }
}

impl Drop for MemRef {
    fn drop(&mut self) {
        if self.is_none() {
            return;
        }

        self.mem().ref_dec();
    }
}

unsafe impl Send for MemRef { }
unsafe impl Sync for MemRef { }
