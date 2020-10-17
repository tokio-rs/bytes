use core::mem::MaybeUninit;
use core::ops::{Index, IndexMut, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive};

/// TODO
#[derive(Debug)]
#[repr(transparent)]
pub struct UninitSlice([MaybeUninit<u8>]);

impl UninitSlice {
    /// TODO
    pub unsafe fn from_raw_parts_mut<'a>(ptr: *mut u8, len: usize) -> &'a mut UninitSlice {
        let maybe_init: &mut [MaybeUninit<u8>] = std::slice::from_raw_parts_mut(ptr as *mut _, len);
        &mut *(maybe_init as *mut [MaybeUninit<u8>] as *mut UninitSlice)
    }

    /// TODO
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr() as *const _
    }

    /// TODO
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr() as *mut _
    }

    /// TODO
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

macro_rules! impl_index {
    ($($t:ty),*) => {
        $(
            impl Index<$t> for UninitSlice {
                type Output = UninitSlice;
            
                fn index(&self, index: $t) -> &UninitSlice {
                    let maybe_uninit = &self.0[index];
                    unsafe { &*(maybe_uninit as *const [MaybeUninit<u8>] as *const UninitSlice) }
                }
            }
            
            impl IndexMut<$t> for UninitSlice {
                fn index_mut(&mut self, index: $t) -> &mut UninitSlice {
                    let maybe_uninit = &mut self.0[index];
                    unsafe { &mut *(maybe_uninit as *mut [MaybeUninit<u8>] as *mut UninitSlice) }
                }
            }
        )*
    };
}

impl_index!(
    Range<usize>,
    RangeFrom<usize>,
    RangeFull,
    RangeInclusive<usize>,
    RangeTo<usize>,
    RangeToInclusive<usize>);
