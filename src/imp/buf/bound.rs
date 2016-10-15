use {Buf, IntoBuf};
use std::mem;

/// Takes a `T` that can be iterated as a buffer and provides buffer with a
/// 'static lifetime
pub struct BoundBuf<T>
    where T: 'static,
          &'static T: IntoBuf
{
    data: T, // This should never be mutated
    buf: <&'static T as IntoBuf>::Buf, // This buf should never leak out
}

impl<T> BoundBuf<T>
    where &'static T: IntoBuf,
{
    /// Creates a new `BoundBuf` wrapping the provided data
    pub fn new(data: T) -> BoundBuf<T> {
        let buf = unsafe {
            let r: &'static T = mem::transmute(&data);
            r.into_buf()
        };

        BoundBuf {
            data: data,
            buf: buf,
        }
    }

    /// Consumes this BoundBuf, returning the underlying value.
    pub fn into_inner(self) -> T {
        self.data
    }

    /// Gets a reference to the underlying value
    pub fn get_ref(&self) -> &T {
        &self.data
    }
}

impl<T> Buf for BoundBuf<T>
    where &'static T: IntoBuf
{
    fn remaining(&self) -> usize {
        self.buf.remaining()
    }

    fn bytes(&self) -> &[u8] {
        self.buf.bytes()
    }

    fn advance(&mut self, cnt: usize) {
        self.buf.advance(cnt)
    }
}
