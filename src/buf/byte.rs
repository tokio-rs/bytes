use {alloc, Bytes, SeqByteStr, MAX_CAPACITY};
use traits::{Buf, MutBuf, MutBufExt, ByteStr};
use std::{cmp, fmt, ptr};

/*
 *
 * ===== ByteBuf =====
 *
 */

/// A `Buf` backed by a contiguous region of memory.
///
/// This `Buf` is better suited for cases where there is a clear delineation
/// between reading and writing.
pub struct ByteBuf {
    mem: alloc::MemRef,
    cap: u32,
    pos: u32,
    lim: u32,
    mark: Option<u32>,
}

impl ByteBuf {
    /// Create a new `ByteBuf` by copying the contents of the given slice.
    pub fn from_slice(bytes: &[u8]) -> ByteBuf {
        let mut buf = ByteBuf::mut_with_capacity(bytes.len());
        buf.write(bytes).ok().expect("unexpected failure");
        buf.flip()
    }

    pub fn mut_with_capacity(capacity: usize) -> MutByteBuf {
        assert!(capacity <= MAX_CAPACITY);
        MutByteBuf { buf: ByteBuf::new(capacity as u32) }
    }

    pub fn none() -> ByteBuf {
        ByteBuf {
            mem: alloc::MemRef::none(),
            cap: 0,
            pos: 0,
            lim: 0,
            mark: None,
        }
    }

    pub unsafe fn from_mem_ref(mem: alloc::MemRef, cap: u32, pos: u32, lim: u32) -> ByteBuf {
        debug_assert!(pos <= lim && lim <= cap, "invalid arguments; cap={}; pos={}; lim={}", cap, pos, lim);

        ByteBuf {
            mem: mem,
            cap: cap,
            pos: pos,
            lim: lim,
            mark: None,
        }
    }

    fn new(mut capacity: u32) -> ByteBuf {
        // Handle 0 capacity case
        if capacity == 0 {
            return ByteBuf::none();
        }

        // Round the capacity to the closest power of 2
        capacity = capacity.next_power_of_two();

        // Allocate the memory
        let mem = alloc::heap(capacity as usize);

        // If the allocation failed, return a blank buf
        if mem.is_none() {
            return ByteBuf::none();
        }

        ByteBuf {
            mem: mem,
            cap: capacity,
            pos: 0,
            lim: capacity,
            mark: None,
        }
    }

    pub fn capacity(&self) -> usize {
        self.cap as usize
    }

    pub fn flip(self) -> MutByteBuf {
        let mut buf = MutByteBuf { buf: self };
        buf.clear();
        buf
    }

    /// Flips the buffer back to mutable, resetting the write position
    /// to the byte after the previous write.
    pub fn resume(mut self) -> MutByteBuf {
        self.pos = self.lim;
        self.lim = self.cap;
        MutByteBuf { buf: self }
    }

    pub fn read_slice(&mut self, dst: &mut [u8]) -> usize {
        let len = cmp::min(dst.len(), self.remaining());
        let cnt = len as u32;

        unsafe {
            ptr::copy_nonoverlapping(
                self.mem.ptr().offset(self.pos as isize),
                dst.as_mut_ptr(),
                len);
        }

        self.pos += cnt;
        len
    }

    pub fn to_seq_byte_str(self) -> SeqByteStr {
        unsafe {
            let ByteBuf { mem, pos, lim, .. } = self;
            SeqByteStr::from_mem_ref(
                mem, pos, lim - pos)
        }
    }

    #[inline]
    pub fn to_bytes(self) -> Bytes {
        Bytes::of(self.to_seq_byte_str())
    }

    /// Marks the current read location.
    ///
    /// Together with `reset`, this can be used to read from a section of the
    /// buffer multiple times. The marked location will be cleared when the
    /// buffer is flipped.
    pub fn mark(&mut self) {
        self.mark = Some(self.pos);
    }

    /// Resets the read position to the previously marked position.
    ///
    /// Together with `mark`, this can be used to read from a section of the
    /// buffer multiple times.
    ///
    /// # Panics
    ///
    /// This method will panic if no mark has been set.
    pub fn reset(&mut self) {
        self.pos = self.mark.take().expect("no mark set");
    }

    #[inline]
    fn pos(&self) -> usize {
        self.pos as usize
    }

    #[inline]
    fn lim(&self) -> usize {
        self.lim as usize
    }

    #[inline]
    fn remaining_u32(&self) -> u32 {
        self.lim - self.pos
    }
}

impl Buf for ByteBuf {

    #[inline]
    fn remaining(&self) -> usize {
        self.remaining_u32() as usize
    }

    #[inline]
    fn bytes<'a>(&'a self) -> &'a [u8] {
        &self.mem.bytes()[self.pos()..self.lim()]
    }

    #[inline]
    fn advance(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.remaining());
        self.pos += cnt as u32;
    }

    #[inline]
    fn read_slice(&mut self, dst: &mut [u8]) -> usize {
        ByteBuf::read_slice(self, dst)
    }
}

impl fmt::Debug for ByteBuf {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.bytes().fmt(fmt)
    }
}

/*
 *
 * ===== ROByteBuf =====
 *
 */

/// Same as `ByteBuf` but cannot be flipped to a `MutByteBuf`.
pub struct ROByteBuf {
    buf: ByteBuf,
}

impl ROByteBuf {
    pub unsafe fn from_mem_ref(mem: alloc::MemRef, cap: u32, pos: u32, lim: u32) -> ROByteBuf {
        ROByteBuf {
            buf: ByteBuf::from_mem_ref(mem, cap, pos, lim)
        }
    }

    pub fn to_seq_byte_str(self) -> SeqByteStr {
        self.buf.to_seq_byte_str()
    }

    pub fn to_bytes(self) -> Bytes {
        self.buf.to_bytes()
    }

    /// Marks the current read location.
    ///
    /// Together with `reset`, this can be used to read from a section of the
    /// buffer multiple times.
    pub fn mark(&mut self) {
        self.buf.mark = Some(self.buf.pos);
    }

    /// Resets the read position to the previously marked position.
    ///
    /// Together with `mark`, this can be used to read from a section of the
    /// buffer multiple times.
    ///
    /// # Panics
    ///
    /// This method will panic if no mark has been set.
    pub fn reset(&mut self) {
        self.buf.pos = self.buf.mark.take().expect("no mark set");
    }
}

impl Buf for ROByteBuf {

    fn remaining(&self) -> usize {
        self.buf.remaining()
    }

    fn bytes<'a>(&'a self) -> &'a [u8] {
        self.buf.bytes()
    }

    fn advance(&mut self, cnt: usize) {
        self.buf.advance(cnt)
    }

    fn read_slice(&mut self, dst: &mut [u8]) -> usize {
        self.buf.read_slice(dst)
    }
}

impl fmt::Debug for ROByteBuf {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.bytes().fmt(fmt)
    }
}

/*
 *
 * ===== MutByteBuf =====
 *
 */

pub struct MutByteBuf {
    buf: ByteBuf,
}

impl MutByteBuf {
    pub fn capacity(&self) -> usize {
        self.buf.capacity() as usize
    }

    pub fn flip(self) -> ByteBuf {
        let mut buf = self.buf;

        buf.lim = buf.pos;
        buf.pos = 0;
        buf
    }

    pub fn clear(&mut self) {
        self.buf.pos = 0;
        self.buf.lim = self.buf.cap;
    }

    #[inline]
    pub fn write_slice(&mut self, src: &[u8]) -> usize {
        let cnt = src.len() as u32;
        let rem = self.buf.remaining_u32();

        if rem < cnt {
            self.write_ptr(src.as_ptr(), rem)
        } else {
            self.write_ptr(src.as_ptr(), cnt)
        }
    }

    #[inline]
    fn write_ptr(&mut self, src: *const u8, len: u32) -> usize {
        unsafe {
            ptr::copy_nonoverlapping(
                src,
                self.buf.mem.ptr().offset(self.buf.pos as isize),
                len as usize);

            self.buf.pos += len;
            len as usize
        }
    }

    pub fn bytes<'a>(&'a self) -> &'a [u8] {
        &self.buf.mem.bytes()[..self.buf.pos()]
    }
}

impl MutBuf for MutByteBuf {
    fn remaining(&self) -> usize {
        self.buf.remaining()
    }

    unsafe fn advance(&mut self, cnt: usize) {
        self.buf.advance(cnt)
    }

    unsafe fn mut_bytes<'a>(&'a mut self) -> &'a mut [u8] {
        let pos = self.buf.pos();
        let lim = self.buf.lim();
        &mut self.buf.mem.bytes_mut()[pos..lim]
    }
}

impl fmt::Debug for MutByteBuf {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.bytes().fmt(fmt)
    }
}
