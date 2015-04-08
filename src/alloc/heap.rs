use alloc::{Allocator, Mem, MemRef};
use std::{mem, ptr, usize};
use std::ops::DerefMut;

const MAX_ALLOC_SIZE: usize = usize::MAX;

pub struct Heap;

impl Heap {
    pub fn allocate(&self, len: usize) -> MemRef {
        // Make sure that the allocation is within the permitted range
        if len > MAX_ALLOC_SIZE {
            return MemRef::none();
        }

        let alloc_len = len +
            mem::size_of::<Mem>() +
            mem::size_of::<Vec<u8>>();

        unsafe {
            let mut vec: Vec<u8> = Vec::with_capacity(alloc_len);
            vec.set_len(alloc_len);

            let ptr = vec.deref_mut().as_mut_ptr();

            ptr::write(ptr as *mut Vec<u8>, vec);

            let ptr = ptr.offset(mem::size_of::<Vec<u8>>() as isize);
            ptr::write(ptr as *mut Mem, Mem::new(len, mem::transmute(self as &Allocator)));

            // Return the info
            MemRef::new(ptr as *mut Mem)
        }
    }

    pub fn deallocate(&self, mem: *mut Mem) {
        unsafe {
            let ptr = mem as *mut u8;
            let _ = ptr::read(ptr.offset(-(mem::size_of::<Vec<u8>>() as isize)) as *const Vec<u8>);
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
