use core::fmt;
use core::mem::MaybeUninit;
use core::ops::{
    Index, IndexMut, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive,
};

/// Uninititialized byte slice.
///
/// Returned by `BufMut::bytes_mut()`, the referenced byte slice may be
/// uninitialized. The wrapper provides safe access without introducing
/// undefined behavior.
#[repr(transparent)]
pub struct UninitSlice([MaybeUninit<u8>]);

impl UninitSlice {
    /// Create a `&mut UninitSlice` from a pointer and a length.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` references a valid memory region owned
    /// by the caller representing a byte slice for the duration of `'a`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::buf::UninitSlice;
    ///
    /// let bytes = b"hello world".to_vec();
    /// let ptr = bytes.as_ptr() as *mut _;
    /// let len = bytes.len();
    ///
    /// let slice = unsafe { UninitSlice::from_raw_parts_mut(ptr, len) };
    /// ```
    pub unsafe fn from_raw_parts_mut<'a>(ptr: *mut u8, len: usize) -> &'a mut UninitSlice {
        let maybe_init: &mut [MaybeUninit<u8>] =
            core::slice::from_raw_parts_mut(ptr as *mut _, len);
        &mut *(maybe_init as *mut [MaybeUninit<u8>] as *mut UninitSlice)
    }

    /// Return a raw pointer to the slice's buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut data = [0, 1, 2];
    /// let mut slice = &mut data[..];
    /// let ptr = BufMut::bytes_mut(&mut slice).as_ptr();
    /// ```
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr() as *const _
    }

    /// Return a raw pointer to the slice's buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut data = [0, 1, 2];
    /// let mut slice = &mut data[..];
    /// let ptr = BufMut::bytes_mut(&mut slice).as_mut_ptr();
    /// ```
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr() as *mut _
    }

    /// Returns the number of bytes in the slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::BufMut;
    ///
    /// let mut data = [0, 1, 2];
    /// let mut slice = &mut data[..];
    /// let len = BufMut::bytes_mut(&mut slice).len();
    ///
    /// assert_eq!(len, 3);
    /// ```
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for UninitSlice {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("UninitSlice[...]")
            .finish()
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
    RangeToInclusive<usize>
);
