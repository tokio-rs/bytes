use super::{Buf, MutBuf};
use std::{cmp, fmt, mem, ptr, slice};
use std::num::UnsignedInt;
use std::rt::heap;

/// Buf backed by a continous chunk of memory. Maintains a read cursor and a
/// write cursor. When reads and writes reach the end of the allocated buffer,
/// wraps around to the start.
pub struct RingBuf {
    ptr: *mut u8,  // Pointer to the memory
    cap: usize,     // Capacity of the buffer
    pos: usize,     // Offset of read cursor
    len: usize      // Number of bytes to read
}

// TODO: There are most likely many optimizations that can be made
impl RingBuf {
    pub fn new(mut capacity: usize) -> RingBuf {
        // Handle the 0 length buffer case
        if capacity == 0 {
            return RingBuf {
                ptr: ptr::null_mut(),
                cap: 0,
                pos: 0,
                len: 0
            }
        }

        // Round to the next power of 2 for better alignment
        capacity = UnsignedInt::next_power_of_two(capacity);

        // Allocate the memory
        let ptr = unsafe { heap::allocate(capacity, mem::min_align_of::<u8>()) };

        RingBuf {
            ptr: ptr as *mut u8,
            cap: capacity,
            pos: 0,
            len: 0
        }
    }

    pub fn is_full(&self) -> bool {
        self.cap == self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    fn read_remaining(&self) -> usize {
        self.len
    }

    fn write_remaining(&self) -> usize {
        self.cap - self.len
    }

    fn advance_reader(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.read_remaining());

        self.pos += cnt;
        self.pos %= self.cap;
        self.len -= cnt;
    }

    fn advance_writer(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.write_remaining());
        self.len += cnt;
    }

    fn as_slice(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self.ptr as *const u8, self.cap)
        }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(self.ptr, self.cap)
        }
    }
}

impl Clone for RingBuf {
    fn clone(&self) -> RingBuf {
        use std::cmp;

        let mut ret = RingBuf::new(self.cap);

        ret.pos = self.pos;
        ret.len = self.len;

        unsafe {
            let to = self.pos + self.len;

            if to > self.cap {
                ptr::copy(ret.ptr, self.ptr as *const u8, to % self.cap);
            }

            ptr::copy(
                ret.ptr.offset(self.pos as isize),
                self.ptr.offset(self.pos as isize) as *const u8,
                cmp::min(self.len, self.cap - self.pos));
        }

        ret
    }

    // TODO: an improved version of clone_from is possible that potentially
    // re-uses the buffer
}

impl fmt::Debug for RingBuf {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "RingBuf[.. {}]", self.len)
    }
}

impl Drop for RingBuf {
    fn drop(&mut self) {
        if self.cap > 0 {
            unsafe {
                heap::deallocate(self.ptr, self.cap, mem::min_align_of::<u8>())
            }
        }
    }
}

impl Buf for RingBuf {

    fn remaining(&self) -> usize {
        self.read_remaining()
    }

    fn bytes(&self) -> &[u8] {
        let mut to = self.pos + self.len;

        if to > self.cap {
            to = self.cap
        }

        &self.as_slice()[self.pos .. to]
    }

    fn advance(&mut self, cnt: usize) {
        self.advance_reader(cnt)
    }
}

impl MutBuf for RingBuf {

    fn remaining(&self) -> usize {
        self.write_remaining()
    }

    fn advance(&mut self, cnt: usize) {
        self.advance_writer(cnt)
    }

    fn mut_bytes(&mut self) -> &mut [u8] {
        let mut from;
        let mut to;

        from = self.pos + self.len;
        from %= self.cap;

        to = from + <Self as MutBuf>::remaining(&self);

        if to >= self.cap {
            to = self.cap;
        }

        &mut self.as_mut_slice()[from..to]
    }
}

unsafe impl Send for RingBuf { }
