use {alloc, Bytes, SeqByteStr, BufResult, BufError, MAX_CAPACITY};
use traits::{Buf, MutBuf, ByteStr};
use std::{cmp, ptr};
use std::num::UnsignedInt;

/*
 *
 * ===== ByteBuf =====
 *
 */

pub struct ByteBuf {
    mem: alloc::MemRef,
    cap: u32,
    pos: u32,
    lim: u32
}

impl ByteBuf {
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
        }
    }

    pub unsafe fn from_mem_ref(mem: alloc::MemRef, cap: u32, pos: u32, lim: u32) -> ByteBuf {
        debug_assert!(pos <= lim && lim <= cap, "invalid arguments; cap={}; pos={}; lim={}", cap, pos, lim);

        ByteBuf {
            mem: mem,
            cap: cap,
            pos: pos,
            lim: lim,
        }
    }

    fn new(mut capacity: u32) -> ByteBuf {
        // Handle 0 capacity case
        if capacity == 0 {
            return ByteBuf::none();
        }

        // Round the capacity to the closest power of 2
        capacity = UnsignedInt::next_power_of_two(capacity);

        // Allocate the memory
        let mem = alloc::HEAP.allocate(capacity as usize);

        // If the allocation failed, return a blank buf
        if mem.is_none() {
            return ByteBuf::none();
        }

        ByteBuf {
            mem: mem,
            cap: capacity,
            pos: 0,
            lim: capacity
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

    pub fn read_slice(&mut self, dst: &mut [u8]) -> usize {
        let len = cmp::min(dst.len(), self.remaining());
        let cnt = len as u32;

        unsafe {
            ptr::copy_nonoverlapping_memory(
                dst.as_mut_ptr(),
                self.mem.ptr().offset(self.pos as isize), len);
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

    pub fn to_bytes(self) -> Bytes {
        Bytes::of(self.to_seq_byte_str())
    }

    fn pos(&self) -> usize {
        self.pos as usize
    }

    fn lim(&self) -> usize {
        self.lim as usize
    }

    fn remaining_u32(&self) -> u32 {
        self.lim - self.pos
    }

    fn ensure_remaining(&self, cnt: usize) -> BufResult<()> {
        if cnt > self.remaining() {
            return Err(BufError::Overflow);
        }

        Ok(())
    }
}

impl Buf for ByteBuf {
    fn remaining(&self) -> usize {
        self.remaining_u32() as usize
    }

    fn bytes<'a>(&'a self) -> &'a [u8] {
        &self.mem.bytes()[self.pos()..self.lim()]
    }

    fn advance(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.remaining());
        self.pos += cnt as u32;
    }

    fn read_slice(&mut self, dst: &mut [u8]) -> usize {
        ByteBuf::read_slice(self, dst)
    }
}

unsafe impl Send for ByteBuf { }

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

    pub fn write_slice(&mut self, src: &[u8]) -> BufResult<()> {
        try!(self.buf.ensure_remaining(src.len()));
        let cnt = src.len() as u32;

        unsafe {
            ptr::copy_nonoverlapping_memory(
                self.buf.mem.ptr().offset(self.buf.pos as isize),
                src.as_ptr(), src.len());
        }

        self.buf.pos += cnt;
        return Ok(());
    }
}

impl MutBuf for MutByteBuf {
    fn remaining(&self) -> usize {
        self.buf.remaining()
    }

    fn advance(&mut self, cnt: usize) {
        self.buf.advance(cnt)
    }

    fn mut_bytes<'a>(&'a mut self) -> &'a mut [u8] {
        let pos = self.buf.pos();
        let lim = self.buf.lim();
        &mut self.buf.mem.bytes_mut()[pos..lim]
    }
}
