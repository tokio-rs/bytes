use alloc::collections::VecDeque;
use crate::Buf;
use core::cmp;
#[cfg(feature = "std")]
use std::io::IoSlice;

/// Ring buffer of buffers.
///
/// `push` operation appends a buffer to the tail of the buffer,
/// and read operations (`bytes`, `bytes_vectored`, `advance` etc)
/// pop elements from the head of the buffer.
///
/// This type can be used to implement an outgoing network buffer,
/// when the front of the queue is written to the network and the back
/// of the queue gets new messages.
///
/// # Note
///
/// This type caches the remaining size (sum of all remaining sizes of all buffers).
/// If buffers owned by this `BufQueue` get their remaining size modified
/// not through this type, the behavior is undefined:
/// operations may hang forever, panic or produce otherwise unexpected results
/// (but not violate memory access).
#[derive(Debug)]
pub struct BufQueue<B: Buf> {
    deque: VecDeque<B>,
    remaining: usize,
}

impl<B: Buf> BufQueue<B> {
    /// Create an empty queue.
    pub fn new() -> Self {
        BufQueue::default()
    }

    /// Push a buf to the back of the deque.
    ///
    /// This operation is no-op if the buf has no remaining.
    ///
    /// # Panics
    ///
    /// This struct tracks the total remaining, and panics if
    /// the total overflows `usize`.
    pub fn push(&mut self, buf: B) {
        let rem = buf.remaining();
        if rem != 0 {
            self.deque.push_back(buf);
            self.remaining = self.remaining.checked_add(rem).expect("remaining overflow");
        }
    }
}

impl<B: Buf> Default for BufQueue<B> {
    fn default() -> Self {
        BufQueue {
            deque: VecDeque::default(),
            remaining: 0,
        }
    }
}

impl<B: Buf> Buf for BufQueue<B> {
    fn remaining(&self) -> usize {
        self.remaining
    }

    fn bytes(&self) -> &[u8] {
        match self.deque.front() {
            Some(b) => b.bytes(),
            None => &[],
        }
    }

    #[cfg(feature = "std")]
    fn bytes_vectored<'a>(&'a self, mut dst: &mut [IoSlice<'a>]) -> usize {
        let mut n = 0;
        for b in &self.deque {
            if dst.is_empty() {
                break;
            }
            let next = b.bytes_vectored(dst);
            dst = &mut dst[next..];
            n += next;
        }
        n
    }

    fn advance(&mut self, mut cnt: usize) {
        while cnt != 0 {
            let front = self.deque.front_mut().expect("must not be empty");
            let rem = front.remaining();
            let advance = cmp::min(cnt, rem);
            front.advance(advance);
            if rem == advance {
                self.deque.pop_front().unwrap();
            }
            cnt -= advance;
            self.remaining -= advance;
        }
    }
}
