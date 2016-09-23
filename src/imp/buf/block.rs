#![allow(warnings)]

use {alloc, Buf, MutBuf, Bytes};
use buf::AppendBuf;
use std::{cmp, ptr, slice};
use std::io::Cursor;
use std::rc::Rc;
use std::collections::{vec_deque, VecDeque};

/// Append only buffer backed by a chain of `AppendBuf` buffers.
///
/// Each `AppendBuf` block is of a fixed size and allocated on demand. This
/// makes the total capacity of a `BlockBuf` potentially much larger than what
/// is currently allocated.
pub struct BlockBuf {
    len: usize,
    cap: usize,
    blocks: VecDeque<AppendBuf>,
    new_block: NewBlock,
}

enum NewBlock {
    Heap(usize),
    // Pool(Rc<Pool>),
}

pub struct BlockBufCursor<'a> {
    rem: usize,
    blocks: vec_deque::Iter<'a, AppendBuf>,
    curr: Option<Cursor<&'a [u8]>>,
}

// TODO:
//
// - Add `comapct` fn which moves all buffered data into one block.
// - Add `slice` fn which returns `Bytes` for arbitrary views into the Buf
//
impl BlockBuf {
    /// Create BlockBuf
    pub fn new(max_blocks: usize, block_size: usize) -> BlockBuf {
        assert!(max_blocks > 1, "at least 2 blocks required");

        let new_block = NewBlock::Heap(block_size);

        BlockBuf {
            len: 0,
            cap: max_blocks * new_block.block_size(),
            blocks: VecDeque::with_capacity(max_blocks),
            new_block: new_block,
        }
    }

    /// Returns the number of buffered bytes
    #[inline]
    pub fn len(&self) -> usize {
        debug_assert_eq!(self.len, self.blocks.iter().map(|b| b.len()).fold(0, |a, b| a+b));
        self.len
    }

    /// Returns true if there are no buffered bytes
    #[inline]
    pub fn is_empty(&self) -> bool {
        return self.len() == 0
    }

    /// Returns a `Buf` for the currently buffered bytes.
    #[inline]
    pub fn buf(&self) -> BlockBufCursor {
        let mut iter = self.blocks.iter();

        // Get the next leaf node buffer
        let block = iter.next()
            .map(|block| Cursor::new(block.bytes()));

        BlockBufCursor {
            rem: self.len(),
            blocks: iter,
            curr: block,
        }
    }

    /// Consumes `n` buffered bytes, returning them as an immutable `Bytes`
    /// value.
    ///
    /// # Panics
    ///
    /// Panics if `n` is greater than the number of buffered bytes.
    #[inline]
    pub fn shift(&mut self, n: usize) -> Bytes {
        trace!("BlockBuf::shift; n={}", n);

        // Fast path
        match self.blocks.len() {
            0 => {
                assert!(n == 0, "buffer overflow");
                Bytes::empty()
            }
            1 => {
                let (ret, pop) = {
                    let block = self.blocks.front().expect("unexpected state");

                    let ret = block.shift(n);
                    self.len -= n;

                    (ret, self.len == 0 && !MutBuf::has_remaining(block))
                };

                if pop {
                    let _ = self.blocks.pop_front();
                }

                ret
            }
            _ => {
                self.shift_multi(n)
            }
        }
    }

    fn shift_multi(&mut self, mut n: usize) -> Bytes {
        let mut ret: Option<Bytes> = None;

        while n > 0 {
            if !self.have_buffered_data() {
                panic!("shift len out of buffered range");
            }

            let (segment, pop) = {
                let block = self.blocks.front().expect("unexpected state");


                let block_len = block.len();
                let segment_n = cmp::min(n, block_len);
                n -= segment_n;
                self.len -= segment_n;

                let pop = block_len == segment_n && !MutBuf::has_remaining(block);

                (block.shift(segment_n), pop)
            };

            if pop {
                let _ = self.blocks.pop_front();
            }

            ret = Some(match ret.take() {
                Some(curr) => {
                    curr.concat(&segment)
                }
                None => segment,
            });

        }

        ret.unwrap_or_else(|| Bytes::empty())
    }

    /// Drop the first `n` buffered bytes
    ///
    /// # Panics
    ///
    /// Panics if `n` is greater than the number of buffered bytes.
    pub fn drop(&mut self, mut n: usize) {
        while n > 0 {
            if !self.have_buffered_data() {
                panic!("shift len out of buffered range");
            }

            let pop = {
                let block = self.blocks.front().expect("unexpected state");

                let segment_n = cmp::min(n, block.len());
                n -= segment_n;
                self.len -= segment_n;

                block.drop(segment_n);

                block.len() == 0
            };

            if pop {
                let _ = self.blocks.pop_front();
            }
        }
    }

    pub fn is_compact(&mut self) -> bool {
        self.blocks.len() <= 1
    }

    /// Moves all buffered bytes into a single block.
    ///
    /// # Panics
    ///
    /// Panics if the buffered bytes cannot fit in a single block.
    pub fn compact(&mut self) {
        trace!("BlockBuf::compact; attempting compaction");

        if self.can_compact() {
            trace!("BlockBuf::compact; data not aligned at start -- compacting");

            let mut compacted = self.new_block.new_block()
                .expect("unable to allocate block");

            for block in self.blocks.drain(..) {
                compacted.write_slice(block.bytes());
            }

            assert!(self.blocks.is_empty(), "blocks not removed");

            self.blocks.push_back(compacted);
        }
    }

    #[inline]
    fn can_compact(&self) -> bool {
        if self.blocks.len() > 1 {
            return true;
        }

        self.blocks.front()
            .map(|b| b.capacity() != self.new_block.block_size())
            .unwrap_or(false)
    }

    /// Return byte slice if bytes are in sequential memory
    #[inline]
    pub fn bytes(&self) -> Option<&[u8]> {
        match self.blocks.len() {
            0 => Some(unsafe { slice::from_raw_parts(ptr::null(), 0) }),
            1 => self.blocks.front().map(|b| b.bytes()),
            _ => None,
        }
    }

    #[inline]
    fn block_size(&self) -> usize {
        self.new_block.block_size()
    }

    #[inline]
    fn allocate_block(&mut self) {
        if let Some(block) = self.new_block.new_block() {
            // Store the block
            self.blocks.push_back(block);
        }
    }

    #[inline]
    fn have_buffered_data(&self) -> bool {
        self.len() > 0
    }

    #[inline]
    fn needs_alloc(&self) -> bool {
        if let Some(buf) = self.blocks.back() {
            // `unallocated_blocks` is checked here because if further blocks
            // cannot be allocated, an empty slice should be returned.
            if MutBuf::has_remaining(buf) {
                return false;
            }
        }

        true
    }
}

impl MutBuf for BlockBuf {
    #[inline]
    fn remaining(&self) -> usize {
        // TODO: Ensure that the allocator has enough capacity to provide the
        // remaining bytes
        self.cap - self.len
    }

    #[inline]
    fn has_remaining(&self) -> bool {
        // TODO: Ensure that the allocator has enough capacity to provide the
        // remaining bytes
        self.cap != self.len
    }

    unsafe fn advance(&mut self, cnt: usize) {
        trace!("BlockBuf::advance; cnt={:?}", cnt);

        // `mut_bytes` only returns bytes from the last block, thus it should
        // only be possible to advance the last block
        if let Some(buf) = self.blocks.back_mut() {
            self.len += cnt;
            buf.advance(cnt);
        }
    }

    #[inline]
    unsafe fn mut_bytes(&mut self) -> &mut [u8] {
        if self.needs_alloc() {
            if self.blocks.len() != self.blocks.capacity() {
                self.allocate_block()
            }
        }

        self.blocks.back_mut()
            .map(|buf| buf.mut_bytes())
            .unwrap_or(slice::from_raw_parts_mut(ptr::null_mut(), 0))
    }
}

impl Default for BlockBuf {
    fn default() -> BlockBuf {
        BlockBuf::new(16, 8_192)
    }
}

impl<'a> Buf for BlockBufCursor<'a> {
    fn remaining(&self) -> usize {
        self.rem
    }

    fn bytes(&self) -> &[u8] {
        self.curr.as_ref()
            .map(|buf| Buf::bytes(buf))
            .unwrap_or(unsafe { slice::from_raw_parts(ptr::null(), 0)})
    }

    fn advance(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.rem);

        // Advance the internal cursor
        self.rem -= cnt;

        // Advance the leaf buffer
        while cnt > 0 {
            {
                let curr = self.curr.as_mut()
                    .expect("expected a value");

                if curr.remaining() > cnt {
                    curr.advance(cnt);
                    break;
                }

                cnt -= curr.remaining();
            }

            self.curr = self.blocks.next()
                .map(|block| Cursor::new(block.bytes()));
        }
    }
}

impl NewBlock {
    #[inline]
    fn block_size(&self) -> usize {
        match *self {
            NewBlock::Heap(size) => size,
            // NewBlock::Pool(ref pool) => pool.buffer_len(),
        }
    }

    #[inline]
    fn new_block(&self) -> Option<AppendBuf> {
        match *self {
            NewBlock::Heap(size) => Some(AppendBuf::with_capacity(size as u32)),
            // NewBlock::Pool(ref pool) => pool.new_append_buf(),
        }
    }
}
