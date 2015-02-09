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

    // Access readable bytes as a Buf
    pub fn reader<'a>(&'a mut self) -> RingBufReader<'a> {
        RingBufReader { ring: self }
    }

    // Access writable bytes as a Buf
    pub fn writer<'a>(&'a mut self) -> RingBufWriter<'a> {
        RingBufWriter { ring: self }
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
                ptr::copy_memory(ret.ptr, self.ptr as *const u8, to % self.cap);
            }

            ptr::copy_memory(
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

pub struct RingBufReader<'a> {
    ring: &'a mut RingBuf
}

impl<'a> Buf for RingBufReader<'a> {

    fn remaining(&self) -> usize {
        self.ring.read_remaining()
    }

    fn bytes<'b>(&'b self) -> &'b [u8] {
        let mut to = self.ring.pos + self.ring.len;

        if to > self.ring.cap {
            to = self.ring.cap
        }

        &self.ring.as_slice()[self.ring.pos .. to]
    }

    fn advance(&mut self, cnt: usize) {
        self.ring.advance_reader(cnt)
    }
}

pub struct RingBufWriter<'a> {
    ring: &'a mut RingBuf
}

impl<'a> MutBuf for RingBufWriter<'a> {

    fn remaining(&self) -> usize {
        self.ring.write_remaining()
    }

    fn advance(&mut self, cnt: usize) {
        self.ring.advance_writer(cnt)
    }

    fn mut_bytes<'b>(&'b mut self) -> &'b mut [u8] {
        let mut from;
        let mut to;

        from = self.ring.pos + self.ring.len;
        from %= self.ring.cap;

        to = from + self.remaining();

        if to >= self.ring.cap {
            to = self.ring.cap;
        }

        &mut self.ring.as_mut_slice()[from..to]
    }
}

unsafe impl Send for RingBuf { }
