#[allow(unused)]
use crate::loom::sync::atomic::AtomicPtr;
use crate::shared_buf::{BufferParts, SharedBuf};
use alloc::vec::Vec;
use core::{ptr, slice, usize};
// ===== impl StaticVtable =====

pub(crate) struct StaticImpl(&'static [u8]);

unsafe impl SharedBuf for StaticImpl {
    fn into_parts(this: Self) -> BufferParts {
        (
            AtomicPtr::new(ptr::null_mut()),
            this.0.as_ptr(),
            this.0.len(),
        )
    }

    unsafe fn from_parts(_data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Self {
        StaticImpl(slice::from_raw_parts(ptr, len))
    }

    unsafe fn clone(_: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BufferParts {
        let slice = slice::from_raw_parts(ptr, len);

        (AtomicPtr::new(ptr::null_mut()), slice.as_ptr(), slice.len())
    }

    unsafe fn into_vec(_: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
        let slice = slice::from_raw_parts(ptr, len);
        slice.to_vec()
    }

    unsafe fn drop(_: &mut AtomicPtr<()>, _: *const u8, _: usize) {
        // nothing to drop for &'static [u8]
    }
}
