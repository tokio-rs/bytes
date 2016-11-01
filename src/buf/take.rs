use {Buf, BufMut};
use std::{cmp, fmt};

/// A buffer adapter which limits the bytes read from an underlying value.
#[derive(Debug)]
pub struct Take<T> {
    inner: T,
    limit: usize,
}

/// A buffer adapter which limits the bytes written from an underlying value.
#[derive(Debug)]
pub struct TakeMut<T> {
    inner: T,
    limit: usize,
}

pub fn new<T>(inner: T, limit: usize) -> Take<T> {
    Take {
        inner: inner,
        limit: limit,
    }
}

pub fn new_mut<T>(inner: T, limit: usize) -> TakeMut<T> {
    TakeMut {
        inner: inner,
        limit: limit,
    }
}

/*
 *
 * ===== impl Take =====
 *
 */

impl<T> Take<T> {
    /// Consumes this `Take`, returning the underlying value.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Gets a reference to the underlying value in this `Take`.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying value in this `Take`.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the maximum number of bytes that are made available from the
    /// underlying value.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Sets the maximum number of bytes that are made available from the
    /// underlying value.
    pub fn set_limit(&mut self, lim: usize) {
        self.limit = lim
    }
}

impl<T: Buf> Buf for Take<T> {
    fn remaining(&self) -> usize {
        cmp::min(self.inner.remaining(), self.limit)
    }

    fn bytes(&self) -> &[u8] {
        &self.inner.bytes()[..self.limit]
    }

    fn advance(&mut self, cnt: usize) {
        let cnt = cmp::min(cnt, self.limit);
        self.limit -= cnt;
        self.inner.advance(cnt);
    }
}

impl<T: BufMut> BufMut for Take<T> {
    fn remaining_mut(&self) -> usize {
        self.inner.remaining_mut()
    }

    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        self.inner.bytes_mut()
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        self.inner.advance_mut(cnt)
    }
}

impl<T: BufMut> fmt::Write for Take<T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        BufMut::put_str(self, s);
        Ok(())
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self, args)
    }
}

/*
 *
 * ===== impl TakeMut =====
 *
 */

impl<T> TakeMut<T> {
    /// Consumes this `TakeMut`, returning the underlying value.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Gets a reference to the underlying value in this `TakeMut`.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying value in this `TakeMut`.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the maximum number of bytes that are made available from the
    /// underlying value.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Sets the maximum number of bytes that are made available from the
    /// underlying value.
    pub fn set_limit(&mut self, lim: usize) {
        self.limit = lim
    }
}

impl<T: Buf> Buf for TakeMut<T> {
    fn remaining(&self) -> usize {
        self.inner.remaining()
    }

    fn bytes(&self) -> &[u8] {
        self.inner.bytes()
    }

    fn advance(&mut self, cnt: usize) {
        self.inner.advance(cnt)
    }
}

impl<T: BufMut> BufMut for TakeMut<T> {
    fn remaining_mut(&self) -> usize {
        cmp::min(self.inner.remaining_mut(), self.limit)
    }

    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.inner.bytes_mut()[..self.limit]
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        let cnt = cmp::min(cnt, self.limit);
        self.limit -= cnt;
        self.inner.advance_mut(cnt);
    }
}

impl<T: BufMut> fmt::Write for TakeMut<T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        BufMut::put_str(self, s);
        Ok(())
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self, args)
    }
}
