//! A buffer backed by a contiguous region of memory.

use {Buf, BufMut};
use std::fmt;

/*
 *
 * ===== SliceBuf =====
 *
 */

/// A `Buf` backed by a contiguous region of memory.
///
/// This `Buf` is better suited for cases where there is a clear delineation
/// between reading and writing.
pub struct SliceBuf<T = Box<[u8]>> {
    // Contiguous memory
    mem: T,
    // Current read position
    rd: usize,
    // Current write position
    wr: usize,
}

impl<T: AsRef<[u8]>> SliceBuf<T> {
    /// Creates a new `SliceBuf` wrapping the provided slice
    pub fn new(mem: T) -> SliceBuf<T> {
        SliceBuf {
            mem: mem,
            rd: 0,
            wr: 0,
        }
    }

    /// Return the number of bytes the buffer can contain
    pub fn capacity(&self) -> usize {
        self.mem.as_ref().len()
    }

    /// Return the read cursor position
    pub fn position(&self) -> usize {
        self.rd
    }

    /// Set the read cursor position
    pub fn set_position(&mut self, position: usize) {
        assert!(position <= self.wr, "position out of bounds");
        self.rd = position
    }

    /// Return the number of buffered bytes
    pub fn len(&self) -> usize {
        self.wr
    }

    /// Returns `true` if the buffer contains no unread bytes
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears the buffer, removing any written data
    pub fn clear(&mut self) {
        self.rd = 0;
        self.wr = 0;
    }}

impl<T> Buf for SliceBuf<T>
    where T: AsRef<[u8]>,
{
    fn remaining(&self) -> usize {
        self.wr - self.rd
    }

    fn bytes(&self) -> &[u8] {
        &self.mem.as_ref()[self.rd..self.wr]
    }

    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.remaining(), "buffer overflow");
        self.rd += cnt;
    }

    fn copy_to_slice(&mut self, dst: &mut [u8]) {
        assert!(self.remaining() >= dst.len());

        let len = dst.len();
        dst.copy_from_slice(&self.mem.as_ref()[self.rd..self.rd+len]);
        self.rd += len;
    }
}

impl<T> BufMut for SliceBuf<T>
    where T: AsRef<[u8]> + AsMut<[u8]>,
{
    fn remaining_mut(&self) -> usize {
        self.capacity() - self.wr
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        assert!(cnt <= self.remaining_mut());
        self.wr += cnt;
    }

    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.mem.as_mut()[self.wr..]
    }

    fn copy_from_slice(&mut self, src: &[u8]) {
        assert!(self.remaining_mut() >= src.len());

        let wr = self.wr;

        self.mem.as_mut()[wr..wr+src.len()]
            .copy_from_slice(src);

        self.wr += src.len();
    }
}

impl<T> fmt::Debug for SliceBuf<T>
    where T: AsRef<[u8]>,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.bytes().fmt(fmt)
    }
}

impl<T> fmt::Write for SliceBuf<T>
    where T: AsRef<[u8]> + AsMut<[u8]>
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        BufMut::put_str(self, s);
        Ok(())
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self, args)
    }
}
