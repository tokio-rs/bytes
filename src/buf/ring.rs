use {alloc, Buf, MutBuf};
use std::{cmp, fmt, io, ptr};

/// Buf backed by a continous chunk of memory. Maintains a read cursor and a
/// write cursor. When reads and writes reach the end of the allocated buffer,
/// wraps around to the start.
pub struct RingBuf {
    ptr: alloc::MemRef,  // Pointer to the memory
    cap: usize,          // Capacity of the buffer
    pos: usize,          // Offset of read cursor
    len: usize           // Number of bytes to read
}

// TODO: There are most likely many optimizations that can be made
impl RingBuf {
    pub fn new(mut capacity: usize) -> RingBuf {
        // Handle the 0 length buffer case
        if capacity == 0 {
            return RingBuf {
                ptr: alloc::MemRef::none(),
                cap: 0,
                pos: 0,
                len: 0
            }
        }

        // Round to the next power of 2 for better alignment
        capacity = capacity.next_power_of_two();

        let mem = alloc::heap(capacity as usize);

        RingBuf {
            ptr: mem,
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
        if self.cap == 0 {
            return;
        }
        cnt = cmp::min(cnt, self.read_remaining());

        self.pos += cnt;
        self.pos %= self.cap;
        self.len -= cnt;
    }

    fn advance_writer(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.write_remaining());
        self.len += cnt;
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
                ptr::copy(self.ptr.ptr() as *const u8, ret.ptr.ptr(), to % self.cap);
            }

            ptr::copy(
                self.ptr.ptr().offset(self.pos as isize) as *const u8,
                ret.ptr.ptr().offset(self.pos as isize),
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

impl Buf for RingBuf {

    fn remaining(&self) -> usize {
        self.read_remaining()
    }

    fn bytes(&self) -> &[u8] {
        let mut to = self.pos + self.len;

        if to > self.cap {
            to = self.cap
        }

        &self.ptr.bytes()[self.pos .. to]
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
        if self.cap == 0 {
            return self.ptr.bytes_mut();
        }
        let mut from;
        let mut to;

        from = self.pos + self.len;
        from %= self.cap;

        to = from + <Self as MutBuf>::remaining(&self);

        if to >= self.cap {
            to = self.cap;
        }

        &mut self.ptr.bytes_mut()[from..to]
    }
}

impl io::Read for RingBuf {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !MutBuf::has_remaining(self) {
            return Ok(0);
        }

        Ok(self.read_slice(buf))
    }
}

impl io::Write for RingBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !Buf::has_remaining(self) {
            return Ok(0);
        }

        Ok(self.write_slice(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

unsafe impl Send for RingBuf { }
