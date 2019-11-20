use crate::BufMut;

use core::{cmp, mem::MaybeUninit};

/// A `BufMut` adapter which limits the amount of bytes that can be written
/// to an underlying buffer.
#[derive(Debug)]
pub struct Limit<T> {
    inner: T,
    limit: usize,
}

pub(super) fn new<T>(inner: T, limit: usize) -> Limit<T> {
    Limit {
        inner,
        limit,
    }
}

impl<T: BufMut> BufMut for Limit<T> {
    fn remaining_mut(&self) -> usize {
        cmp::min(self.inner.remaining_mut(), self.limit)
    }

    fn bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        let bytes = self.inner.bytes_mut();
        let end = cmp::min(bytes.len(), self.limit);
        &mut bytes[..end]
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        assert!(cnt <= self.limit);
        self.inner.advance_mut(cnt);
        self.limit -= cnt;
    }
}
