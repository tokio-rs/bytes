use {Buf, BufMut, Bytes, BytesMut};

use std::{cmp, fmt};

/// A buffer backed by `BytesMut`
pub struct ByteBuf {
    mem: BytesMut,
    rd: usize,
}

impl ByteBuf {
    /// Create a new `ByteBuf` with 8kb capacity
    #[inline]
    pub fn new() -> ByteBuf {
        ByteBuf::with_capacity(8 * 1024)
    }

    /// Create a new `ByteBuf` with `cap` capacity
    #[inline]
    pub fn with_capacity(cap: usize) -> ByteBuf {
        ByteBuf {
            mem: BytesMut::with_capacity(cap),
            rd: 0,
        }
    }

    /// Create a new `ByteBuf` backed by `bytes`
    #[inline]
    pub fn from_bytes(bytes: BytesMut) -> ByteBuf {
        ByteBuf {
            mem: bytes,
            rd: 0,
        }
    }

    /// Create a new `ByteBuf` containing the given slice
    #[inline]
    pub fn from_slice<T: AsRef<[u8]>>(bytes: T) -> ByteBuf {
        let mut buf = ByteBuf::with_capacity(bytes.as_ref().len());
        buf.copy_from_slice(bytes.as_ref());
        buf
    }

    /// Return the number of bytes the buffer can contain
    pub fn capacity(&self) -> usize {
        self.mem.capacity()
    }

    /// Return the read cursor position
    pub fn position(&self) -> usize {
        self.rd
    }

    /// Set the read cursor position
    pub fn set_position(&mut self, position: usize) {
        assert!(position <= self.mem.len(), "position out of bounds");
        self.rd = position
    }

    /// Return the number of buffered bytes
    pub fn len(&self) -> usize {
        self.mem.len()
    }

    /// Returns `true` if the buffer contains no unread bytes
    pub fn is_empty(&self) -> bool {
        self.mem.is_empty()
    }

    /// Clears the buffer, removing any written data
    pub fn clear(&mut self) {
        self.rd = 0;
        unsafe { self.mem.set_len(0); }
    }

    /// Splits the buffer into two at the current read index.
    pub fn drain_read(&mut self) -> BytesMut {
        let drained = self.mem.drain_to_mut(self.rd);
        self.rd = 0;
        drained
    }

    /// Splits the buffer into two at the given index.
    pub fn drain_to(&mut self, at: usize) -> BytesMut {
        let drained = self.mem.drain_to_mut(at);

        if at >= self.rd {
            self.rd = 0;
        } else {
            self.rd -= at;
        }

        drained
    }

    /// Reserves capacity for at least additional more bytes to be written in
    /// the given `ByteBuf`. The `ByteBuf` may reserve more space to avoid
    /// frequent reallocations.
    pub fn reserve(&mut self, additional: usize) {
        if self.remaining_mut() < additional {
            let cap = cmp::max(self.capacity() * 2, self.len() + additional);
            let cap = cap.next_power_of_two();

            let mut new = ByteBuf::with_capacity(cap);

            new.copy_from_slice(self.mem.as_ref());
            new.rd = self.rd;

            *self = new;
        }
    }

    /// Reserves the minimum capacity for exactly additional more bytes to be
    /// written in the given `ByteBuf`. Does nothing if the capacity is already
    /// sufficient.
    ///
    /// Note that the allocator may give the collection more space than it
    /// requests. Therefore capacity can not be relied upon to be precisely
    /// minimal. Prefer reserve if future insertions are expected.
    pub fn reserve_exact(&mut self, additional: usize) {
        if self.remaining_mut() < additional {
            let cap = self.len() + additional;
            let mut new = ByteBuf::with_capacity(cap);

            new.copy_from_slice(self.mem.as_ref());
            new.rd = self.rd;

            *self = new;
        }
    }

    /// Gets a reference to the underlying `BytesMut`
    pub fn get_ref(&self) -> &BytesMut {
        &self.mem
    }

    /// Unwraps the `ByteBuf`, returning the underlying `BytesMut`
    pub fn into_inner(self) -> BytesMut {
        self.mem
    }
}

impl Buf for ByteBuf {
    #[inline]
    fn remaining(&self) -> usize {
        self.len() - self.rd
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        &self.mem[self.rd..]
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.remaining(), "buffer overflow");
        self.rd += cnt;
    }

    #[inline]
    fn copy_to_slice(&mut self, dst: &mut [u8]) {
        assert!(self.remaining() >= dst.len());

        let len = dst.len();
        dst.copy_from_slice(&self.bytes()[..len]);
        self.rd += len;
    }
}

impl BufMut for ByteBuf {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.capacity() - self.len()
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        let new_len = self.len() + cnt;
        self.mem.set_len(new_len);
    }

    #[inline]
    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        let len = self.len();
        &mut self.mem.as_raw()[len..]
    }

    #[inline]
    fn copy_from_slice(&mut self, src: &[u8]) {
        assert!(self.remaining_mut() >= src.len());

        let len = src.len();

        unsafe {
            self.bytes_mut()[..len].copy_from_slice(src);
            self.advance_mut(len);
        }
    }
}

impl From<ByteBuf> for Bytes {
    fn from(src: ByteBuf) -> Bytes {
        let bytes = BytesMut::from(src);
        bytes.freeze()
    }
}

impl From<ByteBuf> for BytesMut {
    fn from(src: ByteBuf) -> BytesMut {
        src.mem
    }
}

impl fmt::Debug for ByteBuf {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.bytes().fmt(fmt)
    }
}

impl fmt::Write for ByteBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        BufMut::put_str(self, s);
        Ok(())
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self, args)
    }
}

impl Clone for ByteBuf {
    fn clone(&self) -> Self {
        ByteBuf {
            mem: self.mem.clone(),
            rd: self.rd,
        }
    }
}
