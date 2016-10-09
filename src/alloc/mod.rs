//! Buffer allocation
//!
//! This module is currently not really in use

mod heap;

use std::sync::Arc;


pub struct MemRef {
    mem: Arc<Box<[u8]>>,
}

/// Allocate a segment of memory and return a `MemRef`.
pub unsafe fn heap(len: usize) -> MemRef {
    heap::allocate(len)
}

impl MemRef {
    #[inline]
    pub unsafe fn new(mem: Arc<Box<[u8]>>) -> MemRef {
        MemRef { mem: mem }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.mem.len()
    }

    #[inline]
    pub unsafe fn bytes(&self) -> &[u8] {
        &*self.mem
    }

    #[inline]
    pub unsafe fn bytes_slice(&self, start: usize, end: usize) -> &[u8] {
        use std::slice;
        let ptr = self.mem.as_ptr().offset(start as isize);
        slice::from_raw_parts(ptr, end - start)
    }

    #[inline]
    pub unsafe fn mut_bytes(&mut self) -> &mut [u8] {
        use std::slice;
        let len = self.mem.len();
        slice::from_raw_parts_mut(self.mem.as_ptr() as *mut u8, len)
    }

    /// Unsafe, unchecked access to the bytes
    #[inline]
    pub unsafe fn mut_bytes_slice(&mut self, start: usize, end: usize) -> &mut [u8] {
        use std::slice;
        let ptr = self.mem.as_ptr().offset(start as isize);
        slice::from_raw_parts_mut(ptr as *mut u8, end - start)
    }

    pub fn get_ref(&self) -> &Arc<Box<[u8]>> {
        &self.mem
    }
}

impl Clone for MemRef {
    #[inline]
    fn clone(&self) -> MemRef {
        MemRef { mem: self.mem.clone() }
    }
}
