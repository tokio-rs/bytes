use alloc::{MemRef};
use std::sync::Arc;

pub unsafe fn allocate(len: usize) -> MemRef {
    let mut v = Vec::with_capacity(len);
    v.set_len(len);

    MemRef::new(Arc::new(v.into_boxed_slice()))
}
