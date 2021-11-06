use std::alloc::{GlobalAlloc, Layout, System};
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
        const ELEM: (AtomicPtr<u8>, AtomicUsize) =
            (AtomicPtr::new(null_mut()), AtomicUsize::new(0));
        let alloc_table = [ELEM; 512];

        Self { alloc_table }
    }

    /// Iterate over our table until we find an open entry, then insert into said entry
    fn insert(&self, ptr: *mut u8, size: usize) {
        for (entry_ptr, entry_size) in self.alloc_table.iter() {
            // SeqCst is good enough here, we don't care about perf, i just want to be correct!
            if entry_ptr
                .compare_exchange(null_mut(), ptr, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                entry_size.store(size, Ordering::SeqCst);
                break;
            }
        }
    }

    fn remove(&self, ptr: *mut u8) -> usize {
        for (entry_ptr, entry_size) in self.alloc_table.iter() {
            if entry_ptr
                .compare_exchange(ptr, null_mut(), Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return entry_size.swap(0, Ordering::SeqCst);
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
        let orig_size = self.remove(ptr);

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
