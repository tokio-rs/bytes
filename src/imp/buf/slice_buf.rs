//! A buffer backed by a contiguous region of memory.

use {Buf, MutBuf};
use imp::alloc;
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

impl SliceBuf {
    /// Constructs a new, empty `SliceBuf` with the specified capacity
    ///
    /// The `SliceBuf` will be backed by a `Box<[u8]>`.
    pub fn with_capacity(capacity: usize) -> SliceBuf {
        let mem = unsafe { alloc::with_capacity(capacity) };
        SliceBuf::new(mem)
    }

    /// Create a new `SliceBuf` and copy the contents of the given slice into
    /// it.
    pub fn from_slice<T: AsRef<[u8]>>(bytes: &T) -> SliceBuf {
        let mut buf = SliceBuf::with_capacity(bytes.as_ref().len());
        buf.write_slice(bytes.as_ref());
        buf
    }
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
    }

    /// Return the number of bytes left to read
    pub fn remaining_read(&self) -> usize {
        self.wr - self.rd
    }

    /// Return the remaining write capacity
    pub fn remaining_write(&self) -> usize {
        self.capacity() - self.wr
    }
}

impl<T> Buf for SliceBuf<T>
    where T: AsRef<[u8]>,
{
    fn remaining(&self) -> usize {
        self.remaining_read()
    }

    fn bytes(&self) -> &[u8] {
        &self.mem.as_ref()[self.rd..self.wr]
    }

    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.remaining(), "buffer overflow");
        self.rd += cnt;
    }

    fn read_slice(&mut self, dst: &mut [u8]) {
        assert!(self.remaining() >= dst.len());

        let len = dst.len();
        dst.copy_from_slice(&self.mem.as_ref()[self.rd..self.rd+len]);
        self.rd += len;
    }
}

impl<T> MutBuf for SliceBuf<T>
    where T: AsRef<[u8]> + AsMut<[u8]>,
{
    fn remaining(&self) -> usize {
        self.remaining_write()
    }

    unsafe fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.remaining_write());
        self.wr += cnt;
    }

    unsafe fn mut_bytes(&mut self) -> &mut [u8] {
        &mut self.mem.as_mut()[self.wr..]
    }

    fn write_slice(&mut self, src: &[u8]) {
        let wr = self.wr;

        self.mem.as_mut()[wr..wr+src.len()]
            .copy_from_slice(src);

        self.wr += src.len();
    }
}

impl Clone for SliceBuf {
    fn clone(&self) -> Self {
        SliceBuf {
            mem: self.mem.clone(),
            rd: self.rd,
            wr: self.wr,
        }
    }
}

impl<T> fmt::Debug for SliceBuf<T>
    where T: AsRef<[u8]>,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.bytes().fmt(fmt)
    }
}
