#![allow(warnings)]

use std::sync::Arc;

/// A sequential chunk of memory that is atomically reference counted.
pub struct Mem {
    mem: Arc<Box<[u8]>>,
}

pub unsafe fn with_capacity(mut capacity: usize) -> Box<[u8]> {
    // Round up to the next power of two
    capacity = capacity.next_power_of_two();

    let mut v: Vec<u8> = Vec::with_capacity(capacity);
    v.set_len(capacity);
    v.into_boxed_slice()
}

impl Mem {
    /// Return a new `Mem` with the given capacity
    pub unsafe fn with_capacity(capacity: usize) -> Mem {
        let mem = Arc::new(with_capacity(capacity));
        Mem { mem: mem }
    }

    pub unsafe fn from_boxed(src: Arc<Box<[u8]>>) -> Mem {
        Mem { mem: src }
    }

    /// Returns the length in bytes
    pub fn len(&self) -> usize {
        self.mem.len()
    }

    /// View of the underlying memory.
    ///
    /// The memory could be uninitialized.
    pub unsafe fn bytes(&self) -> &[u8] {
        &*self.mem
    }

    /// View of a range of the underlying memory.
    ///
    /// The offsets are not checked and the memory could be uninitialized.
    pub unsafe fn slice(&self, start: usize, end: usize) -> &[u8] {
        use std::slice;
        let ptr = self.mem.as_ptr().offset(start as isize);
        slice::from_raw_parts(ptr, end - start)
    }

    /// Mutable view of the underlying memory.
    ///
    /// The memory could be uninitialized.
    pub unsafe fn mut_bytes(&mut self) -> &mut [u8] {
        use std::slice;
        let len = self.mem.len();
        slice::from_raw_parts_mut(self.mem.as_ptr() as *mut u8, len)
    }

    /// Mutable view of a range of the underlying memory.
    ///
    /// The offsets are not checked and the memory could be uninitialized.
    pub unsafe fn mut_bytes_slice(&mut self, start: usize, end: usize) -> &mut [u8] {
        use std::slice;
        let ptr = self.mem.as_ptr().offset(start as isize);
        slice::from_raw_parts_mut(ptr as *mut u8, end - start)
    }
}
