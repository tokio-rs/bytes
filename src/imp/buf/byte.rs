use {alloc, Buf, MutBuf, Bytes, MAX_CAPACITY};
use std::{cmp, fmt};

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
        let mut buf = MutByteBuf::with_capacity(bytes.len());
        buf.write_slice(bytes);
        buf.flip()
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
        // Round the capacity to the closest power of 2
        capacity = capacity.next_power_of_two();

        unsafe {
            // Allocate the memory
            let mem = alloc::heap(capacity as usize);

            ByteBuf {
                mem: mem,
                cap: capacity,
                pos: 0,
                lim: capacity,
                mark: None,
            }
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

    pub fn read_slice(&mut self, dst: &mut [u8]) {
        assert!(self.remaining() >= dst.len());
        let len = dst.len();
        let cnt = len as u32;
        let pos = self.pos as usize;

        unsafe {
            dst.copy_from_slice(&self.mem.bytes()[pos..pos+len]);
        }

        self.pos += cnt;
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
    fn bytes(&self) -> &[u8] {
        unsafe { &self.mem.bytes()[self.pos()..self.lim()] }
    }

    #[inline]
    fn advance(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.remaining());
        self.pos += cnt as u32;
    }

    #[inline]
    fn read_slice(&mut self, dst: &mut [u8]) {
        ByteBuf::read_slice(self, dst)
    }
}

impl From<ByteBuf> for Bytes {
    fn from(src: ByteBuf) -> Bytes {
        unsafe {
            let ByteBuf { mem, pos, lim, .. } = src;
            Bytes::from_mem_ref(mem, pos, lim - pos)
        }
    }
}

impl fmt::Debug for ByteBuf {
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
    pub fn with_capacity(capacity: usize) -> MutByteBuf {
        assert!(capacity <= MAX_CAPACITY);
        MutByteBuf { buf: ByteBuf::new(capacity as u32) }
    }

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
        let cnt = cmp::min(src.len(), self.buf.remaining());
        let pos = self.buf.pos as usize;

        unsafe {
            self.buf.mem.mut_bytes()[pos..pos+cnt]
                .copy_from_slice(&src[0..cnt]);
        }

        self.buf.pos += cnt as u32;

        cnt
    }

    pub fn bytes(&self) -> &[u8] {
        unsafe { &self.buf.mem.bytes()[..self.buf.pos()] }
    }
}

impl MutBuf for MutByteBuf {
    fn remaining(&self) -> usize {
        self.buf.remaining()
    }

    unsafe fn advance(&mut self, cnt: usize) {
        self.buf.advance(cnt)

    }
    unsafe fn mut_bytes(&mut self) -> &mut [u8] {
        let pos = self.buf.pos();
        let lim = self.buf.lim();
        &mut self.buf.mem.mut_bytes()[pos..lim]
    }
}

impl fmt::Debug for MutByteBuf {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.bytes().fmt(fmt)
    }
}
