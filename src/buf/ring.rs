use {alloc, Buf, MutBuf};
use std::{cmp, fmt, io, ptr};

enum Mark {
    NoMark,
    At { pos: usize, len: usize },
}

/// Buf backed by a continous chunk of memory. Maintains a read cursor and a
/// write cursor. When reads and writes reach the end of the allocated buffer,
/// wraps around to the start.
///
/// This type is suited for use cases where reads and writes are intermixed.
pub struct RingBuf {
    ptr: alloc::MemRef,  // Pointer to the memory
    cap: usize,          // Capacity of the buffer
    pos: usize,          // Offset of read cursor
    len: usize,          // Number of bytes to read
    mark: Mark,          // Marked read position
}

// TODO: There are most likely many optimizations that can be made
impl RingBuf {
    /// Allocates a new `RingBuf` with the specified capacity.
    pub fn new(mut capacity: usize) -> RingBuf {
        // Handle the 0 length buffer case
        if capacity == 0 {
            return RingBuf {
                ptr: alloc::MemRef::none(),
                cap: 0,
                pos: 0,
                len: 0,
                mark: Mark::NoMark,
            }
        }

        // Round to the next power of 2 for better alignment
        capacity = capacity.next_power_of_two();

        let mem = alloc::heap(capacity as usize);

        RingBuf {
            ptr: mem,
            cap: capacity,
            pos: 0,
            len: 0,
            mark: Mark::NoMark,
        }
    }

    /// Returns `true` if the buf cannot accept any further writes.
    pub fn is_full(&self) -> bool {
        self.cap == self.len
    }

    /// Returns `true` if the buf cannot accept any further reads.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of bytes that the buf can hold.
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Marks the current read location.
    ///
    /// Together with `reset`, this can be used to read from a section of the
    /// buffer multiple times. The mark will be cleared if it is overwritten
    /// during a write.
    pub fn mark(&mut self) {
        self.mark = Mark::At { pos: self.pos, len: self.len };
    }

    /// Resets the read position to the previously marked position.
    ///
    /// Together with `mark`, this can be used to read from a section of the
    /// buffer multiple times.
    ///
    /// # Panics
    ///
    /// This method will panic if no mark has been set,
    pub fn reset(&mut self){
        match self.mark {
            Mark::NoMark => panic!("no mark set"),
            Mark::At {pos, len} => {
                self.pos = pos;
                self.len = len;
                self.mark = Mark::NoMark;
            }
        }
    }

    /// Resets all internal state to the initial state.
    pub fn clear(&mut self) {
        self.pos = 0;
        self.len = 0;
        self.mark = Mark::NoMark;
    }

    /// Returns the number of bytes remaining to read.
    fn read_remaining(&self) -> usize {
        self.len
    }

    /// Returns the remaining write capacity until which the buf becomes full.
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

        // Adjust the mark to account for bytes written.
        if let Mark::At { ref mut len, .. } = self.mark {
            *len += cnt;
        }

        // Clear the mark if we've written past it.
        if let Mark::At { len, .. } = self.mark {
            if len > self.cap {
                self.mark = Mark::NoMark;
            }
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

    unsafe fn advance(&mut self, cnt: usize) {
        self.advance_writer(cnt)
    }

    unsafe fn mut_bytes(&mut self) -> &mut [u8] {
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
        if !Buf::has_remaining(self) {
            return Ok(0);
        }

        Ok(self.read_slice(buf))
    }
}

impl io::Write for RingBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !MutBuf::has_remaining(self) {
            return Ok(0);
        }

        Ok(self.write_slice(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

unsafe impl Send for RingBuf { }
