use std::alloc::{GlobalAlloc, Layout, System};
use std::{mem};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use bytes::{Buf, Bytes};

#[global_allocator]
static LEDGER: Ledger = Ledger::new();

#[repr(C)]
struct Ledger {
    alloc_table: [(AtomicPtr<u8>, AtomicUsize); 512],
}

impl Ledger {
    const fn new() -> Self {
        // equivalent to size of (AtomicPtr<u8>, AtomicUsize), hopefully
        #[cfg(target_pointer_width = "64")]
            let tricky_bits = 0u128;

        #[cfg(target_pointer_width = "32")]
            let tricky_bits = 0u64;

        let magic_table = [tricky_bits; 512];

        // i know this looks bad but all the good ways to do this are unstable or not yet
        // supported in const contexts (even though they should be!)
        let alloc_table = unsafe { mem::transmute(magic_table) };

        Self { alloc_table }
    }

    /// Iterate over our table until we find an open entry, then insert into said entry
    fn insert(&self, ptr: *mut u8, size: usize) {
        for (entry_ptr, entry_size) in self.alloc_table.iter() {
            // SeqCst is good enough here, we don't care about perf, i just want to be correct!
            if entry_ptr.compare_exchange(
                null_mut(),
                ptr,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ).is_ok()
            {
                entry_size.store(size, Ordering::Relaxed);
                break;
            }
        }
    }

    fn lookup_size(&self, ptr: *mut u8) -> usize {
        for (entry_ptr, entry_size) in self.alloc_table.iter() {
            if entry_ptr.load(Ordering::Relaxed) == ptr {
                return entry_size.load(Ordering::Relaxed);
            }
        }

        panic!("Couldn't find a matching entry for {:x?}", ptr);
    }
}

unsafe impl GlobalAlloc for Ledger {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let ptr = System.alloc(layout);
        self.insert(ptr, size);
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let orig_size = self.lookup_size(ptr);

        if orig_size != layout.size() {
            panic!(
                "bad dealloc: alloc size was {}, dealloc size is {}",
                orig_size,
                layout.size()
            );
        } else {
            System.dealloc(ptr, layout);
        }
    }
}

#[test]
fn test_bytes_advance() {
    let mut bytes = Bytes::from(vec![10, 20, 30]);
    bytes.advance(1);
    drop(bytes);
}

#[test]
fn test_bytes_truncate() {
    let mut bytes = Bytes::from(vec![10, 20, 30]);
    bytes.truncate(2);
    drop(bytes);
}

#[test]
fn test_bytes_truncate_and_advance() {
    let mut bytes = Bytes::from(vec![10, 20, 30]);
    bytes.truncate(2);
    bytes.advance(1);
    drop(bytes);
}
