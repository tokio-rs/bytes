use {alloc, MutBuf, Bytes};
use std::cell::Cell;

/// A `Buf` backed by a contiguous region of memory.
///
/// This buffer can only be written to once. Byte strings (immutable views) can
/// be created at any time, not just when the writing is complete.
pub struct AppendBuf {
    mem: alloc::MemRef,
    rd: Cell<u32>, // Read cursor
    wr: u32, // Write cursor
    cap: u32,
}

impl AppendBuf {
    pub fn with_capacity(mut capacity: u32) -> AppendBuf {
        // Round the capacity to the closest power of 2
        capacity = capacity.next_power_of_two();

        unsafe {
            // Allocate the memory
            let mem = alloc::heap(capacity as usize);

            AppendBuf::from_mem_ref(mem, capacity, 0)
        }
    }

    pub unsafe fn from_mem_ref(mem: alloc::MemRef, cap: u32, pos: u32) -> AppendBuf {
        AppendBuf {
            mem: mem,
            rd: Cell::new(pos),
            wr: pos,
            cap: cap,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        (self.wr - self.rd.get()) as usize
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        (self.cap - self.rd.get()) as usize
    }

    pub fn bytes(&self) -> &[u8] {
        let rd = self.rd.get() as usize;
        let wr = self.wr as usize;
        unsafe { &self.mem.bytes_slice(rd, wr) }
    }

    pub fn shift(&self, n: usize) -> Bytes {
        let ret = self.slice(0, n);
        self.rd.set(self.rd.get() + ret.len() as u32);
        assert!(self.rd.get() <= self.wr, "buffer overflow");
        ret
    }

    pub fn drop(&self, n: usize) {
        assert!(n <= self.len());
        self.rd.set(self.rd.get() + n as u32);
    }

    pub fn slice(&self, begin: usize, end: usize) -> Bytes {
        // TODO: Fix overflow potential

        let rd = self.rd.get();
        let wr = self.wr;

        let begin = begin as u32 + rd;
        let end = end as u32 + rd;

        assert!(begin <= end && end <= wr, "invalid range");

        unsafe { Bytes::from_mem_ref(self.mem.clone(), begin, end - begin) }
    }
}

impl MutBuf for AppendBuf {
    #[inline]
    fn remaining(&self) -> usize {
        (self.cap - self.wr) as usize
    }

    #[inline]
    fn has_remaining(&self) -> bool {
        // Implemented as an equality for the perfz
        self.cap != self.wr
    }

    #[inline]
    unsafe fn advance(&mut self, cnt: usize) {
        self.wr += cnt as u32;

        if self.wr > self.cap {
            panic!("buffer overflow");
        }
    }

    #[inline]
    unsafe fn mut_bytes<'a>(&'a mut self) -> &'a mut [u8] {
        let wr = self.wr as usize;
        let cap = self.cap as usize;
        self.mem.mut_bytes_slice(wr, cap)
    }
}

impl AsRef<[u8]> for AppendBuf {
    fn as_ref(&self) -> &[u8] {
        self.bytes()
    }
}
