use {Buf, BufMut, Bytes};

use std::{cmp, ptr};

/// A value that writes bytes from itself into a `BufMut`.
///
/// Values that implement `Source` are used as an argument to
/// [`BufMut::put`](trait.BufMut.html#method.put).
///
/// # Examples
///
/// ```
/// use bytes::{BufMut, Source};
///
/// struct Repeat {
///     num: usize,
///     str: String,
/// }
///
/// impl Source for Repeat {
///     fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
///         for _ in 0..self.num {
///             buf.put(&self.str);
///         }
///     }
/// }
///
/// let mut dst = vec![];
/// dst.put(Repeat {
///     num: 3,
///     str: "hello".into(),
/// });
///
/// assert_eq!(*dst, b"hellohellohello"[..]);
/// ```
pub trait Source {
    /// Copy data from self into destination buffer
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{BufMut, Source};
    ///
    /// let mut dst = vec![];
    ///
    /// "hello".copy_to_buf(&mut dst);
    ///
    /// assert_eq!(*dst, b"hello"[..]);
    /// ```
    ///
    /// # Panics
    ///
    /// This function panis if `buf` does not have enough capacity for `self`.
    fn copy_to_buf<B: BufMut>(self, buf: &mut B);
}

impl Source for Vec<u8> {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(&self[..]);
    }
}

impl<'a> Source for &'a Vec<u8> {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(&self[..]);
    }
}

impl<'a> Source for &'a [u8] {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(self);
    }
}

impl Source for String {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(self.as_bytes());
    }
}

impl<'a> Source for &'a String {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(self.as_bytes());
    }
}

impl<'a> Source for &'a str {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(self.as_bytes());
    }
}

impl Source for u8 {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        let src = [self];
        buf.put_slice(&src);
    }
}

impl Source for i8 {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        buf.put_slice(&[self as u8])
    }
}

impl Source for Bytes {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        Source::copy_to_buf(self.as_ref(), buf);
    }
}

impl<'a> Source for &'a Bytes {
    fn copy_to_buf<B: BufMut>(self, buf: &mut B) {
        Source::copy_to_buf(self.as_ref(), buf);
    }
}

impl<T: Buf> Source for T {
    fn copy_to_buf<B: BufMut>(mut self, buf: &mut B) {
        assert!(buf.remaining_mut() >= self.remaining());

        while self.has_remaining() {
            let l;

            unsafe {
                let s = self.bytes();
                let d = buf.bytes_mut();
                l = cmp::min(s.len(), d.len());

                ptr::copy_nonoverlapping(
                    s.as_ptr(),
                    d.as_mut_ptr(),
                    l);
            }

            self.advance(l);
            unsafe { buf.advance_mut(l); }
        }
    }
}
