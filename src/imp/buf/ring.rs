use {Buf, MutBuf};
use imp::alloc;
use std::fmt;


/// `RingBuf` is backed by contiguous memory and writes may wrap.
///
/// When writing reaches the end of the memory, writing resume at the beginning
/// of the memory. Writes may never overwrite pending reads.
pub struct RingBuf<T = Box<[u8]>> {
    // Contiguous memory
    mem: T,
    // Current read position
    rd: u64,
    // Current write position
    wr: u64,
    // Mask used to convert the cursor to an offset
    mask: u64,
}

impl RingBuf {
    /// Allocates a new `RingBuf` with the specified capacity.
    pub fn with_capacity(capacity: usize) -> RingBuf {
        let mem = unsafe { alloc::with_capacity(capacity) };
        RingBuf::new(mem)
    }
}

impl<T: AsRef<[u8]>> RingBuf<T> {
    /// Creates a new `RingBuf` wrapping the provided slice
    pub fn new(mem: T) -> RingBuf<T> {
        // Ensure that the memory chunk provided has a length that is a power
        // of 2
        let len = mem.as_ref().len() as u64;
        let mask = len - 1;

        assert!(len & mask == 0, "mem length must be power of two");

        RingBuf {
            mem: mem,
            rd: 0,
            wr: 0,
            mask: mask,
        }
    }

    /// Returns the number of bytes that the buf can hold.
    pub fn capacity(&self) -> usize {
        self.mem.as_ref().len()
    }

    /// Return the read cursor position
    pub fn position(&self) -> u64 {
        self.rd
    }

    /// Set the read cursor position
    pub fn set_position(&mut self, position: u64) {
        assert!(position <= self.wr && position + self.capacity() as u64 >= self.wr,
                "position out of bounds");
        self.rd = position;
    }

    /// Return the number of buffered bytes
    pub fn len(&self) -> usize {
        if self.wr >= self.capacity() as u64 {
            (self.rd - (self.wr - self.capacity() as u64)) as usize
        } else {
            self.rd as usize
        }
    }

    /// Returns `true` if the buf cannot accept any further reads.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Resets all internal state to the initial state.
    pub fn clear(&mut self) {
        self.rd = 0;
        self.wr = 0;
    }

    /// Returns the number of bytes remaining to read.
    pub fn remaining_read(&self) -> usize {
        (self.wr - self.rd) as usize
    }

    /// Returns the remaining write capacity until which the buf becomes full.
    pub fn remaining_write(&self) -> usize {
        self.capacity() - self.remaining_read()
    }
}

impl<T: AsRef<[u8]>> fmt::Debug for RingBuf<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "RingBuf[.. {}]", self.len())
    }
}

impl<T: AsRef<[u8]>> Buf for RingBuf<T> {
    fn remaining(&self) -> usize {
        self.remaining_read()
    }

    fn bytes(&self) -> &[u8] {
        // This comparison must be performed in order to differentiate between
        // the at capacity case and the empty case.
        if self.wr > self.rd {
            let a = (self.rd & self.mask) as usize;
            let b = (self.wr & self.mask) as usize;

            if b > a {
                &self.mem.as_ref()[a..b]
            } else {
                &self.mem.as_ref()[a..]
            }
        } else {
            &[]
        }
    }

    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.remaining_read(), "buffer overflow");
        self.rd += cnt as u64
    }
}

impl<T> MutBuf for RingBuf<T>
    where T: AsRef<[u8]> + AsMut<[u8]>,
{
    fn remaining(&self) -> usize {
        self.remaining_write()
    }

    unsafe fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.remaining_write(), "buffer overflow");
        self.wr += cnt as u64;
    }

    unsafe fn mut_bytes(&mut self) -> &mut [u8] {
        let a = (self.wr & self.mask) as usize;

        if self.wr > self.rd {
            let b = (self.rd & self.mask) as usize;

            if a >= b {
                &mut self.mem.as_mut()[a..]
            } else {
                &mut self.mem.as_mut()[a..b]
            }
        } else {
            &mut self.mem.as_mut()[a..]
        }
    }
}
