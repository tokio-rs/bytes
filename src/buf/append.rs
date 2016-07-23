use alloc;
use buf::{MutBuf};
use str::{ByteStr, Bytes, SeqByteStr, SmallByteStr};
use std::cell::Cell;
use std::cmp;

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
        // Handle 0 capacity case
        if capacity == 0 {
            return AppendBuf::none();
        }

        // Round the capacity to the closest power of 2
        capacity = capacity.next_power_of_two();

        // Allocate the memory
        let mem = alloc::heap(capacity as usize);

        // If the allocation failed, return a blank buf
        if mem.is_none() {
            return AppendBuf::none();
        }

        AppendBuf {
            mem: mem,
            rd: Cell::new(0),
            wr: 0,
            cap: capacity,
        }
    }

    /// Returns an AppendBuf with no capacity
    pub fn none() -> AppendBuf {
        AppendBuf {
            mem: alloc::MemRef::none(),
            rd: Cell::new(0),
            wr: 0,
            cap: 0,
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

    pub fn bytes(&self) -> &[u8] {
        let rd = self.rd.get() as usize;
        let wr = self.wr as usize;
        unsafe { &self.mem.bytes()[rd..wr] }
    }

    pub fn shift(&self, n: usize) -> Bytes {
        let ret = self.slice(0, n);
        self.rd.set(self.rd.get() + ret.len() as u32);
        ret
    }

    pub fn slice(&self, begin: usize, end: usize) -> Bytes {
        if end <= begin {
            return Bytes::of(SmallByteStr::zero());
        }

        if let Some(bytes) = SmallByteStr::from_slice(&self.bytes()[begin..end]) {
            return Bytes::of(bytes);
        }

        let begin = cmp::min(self.wr, begin as u32 + self.rd.get());
        let end = cmp::min(self.wr, end as u32 + self.rd.get());

        let bytes = unsafe { SeqByteStr::from_mem_ref(self.mem.clone(), begin, end - begin) };

        Bytes::of(bytes)
    }
}

impl MutBuf for AppendBuf {
    fn remaining(&self) -> usize {
        (self.cap - self.wr) as usize
    }

    unsafe fn advance(&mut self, cnt: usize) {
        self.wr += cnt as u32;

        if self.wr > self.cap {
            self.wr = self.cap;
        }
    }

    unsafe fn mut_bytes<'a>(&'a mut self) -> &'a mut [u8] {
        let wr = self.wr as usize;
        let cap = self.cap as usize;
        &mut self.mem.bytes_mut()[wr..cap]
    }
}
