use alloc::collections::VecDeque;
#[cfg(feature = "std")]
use std::io;

use super::Buf;

impl Buf for VecDeque<u8> {
    fn remaining(&self) -> usize {
        self.len()
    }

    fn chunk(&self) -> &[u8] {
        let (s1, s2) = self.as_slices();
        if s1.is_empty() {
            s2
        } else {
            s1
        }
    }

    #[cfg(feature = "std")]
    fn chunks_vectored<'a>(&'a self, dst: &mut [io::IoSlice<'a>]) -> usize {
        if self.is_empty() || dst.is_empty() {
            return 0;
        }

        let (s1, s2) = self.as_slices();
        dst[0] = io::IoSlice::new(s1);
        if s2.is_empty() || dst.len() == 1 {
            return 1;
        }

        dst[1] = io::IoSlice::new(s2);
        2
    }

    fn advance(&mut self, cnt: usize) {
        self.drain(..cnt);
    }
}

impl<T: Buf> Buf for VecDeque<T> {
    fn remaining(&self) -> usize {
        self.iter().map(|b| b.remaining()).sum()
    }

    fn chunk(&self) -> &[u8] {
        self.iter()
            .find(|b| b.has_remaining())
            .map(|b| b.chunk())
            .unwrap_or_default()
    }

    #[cfg(feature = "std")]
    fn chunks_vectored<'a>(&'a self, dst: &mut [io::IoSlice<'a>]) -> usize {
        let mut n = 0;
        for buf in self {
            if n >= dst.len() {
                break;
            }
            n += buf.chunks_vectored(&mut dst[n..]);
        }
        n
    }

    fn advance(&mut self, mut cnt: usize) {
        while cnt > 0 {
            let b = self
                .front_mut()
                .expect("advance called with cnt > remaining");
            let rem = b.remaining();
            if cnt < rem {
                b.advance(cnt);
                return;
            } else {
                cnt -= rem;
                self.pop_front();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bytes;
    use std::io::IoSlice;

    #[test]
    fn test_vec_deque_buf_sequential_integrity() {
        /// A mock Buf that simulates a "fragmented" memory layout.
        /// It claims to have 10 bytes remaining, but its `chunks_vectored`
        /// implementation only exposes the first 4 bytes.
        struct MockBuf(Bytes);
        impl Buf for MockBuf {
            fn remaining(&self) -> usize {
                self.0.remaining()
            }
            fn chunk(&self) -> &[u8] {
                &self.0.chunk()[..1]
            }
            fn advance(&mut self, cnt: usize) {
                self.0.advance(cnt);
            }
            /// Purposefully return fewer bytes than `remaining()` to test
            /// if the caller correctly stops at the first gap.
            fn chunks_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
                if dst.is_empty() || self.0.is_empty() {
                    return 0;
                }
                let limit = std::cmp::min(self.0.len(), 4);
                dst[0] = IoSlice::new(&self.0.chunk()[..limit]);
                1
            }
        }

        let buf1 = MockBuf(Bytes::from("0123456789")); // 10 bytes
        let buf2 = MockBuf(Bytes::from("ABCDEFGHIJ")); // 10 bytes

        let mut deque = VecDeque::new();
        deque.push_back(buf1);
        deque.push_back(buf2);

        let mut slices = [IoSlice::new(&[]); 16];
        let n = deque.chunks_vectored(&mut slices);

        let total_len: usize = slices[..n].iter().map(|s| s.len()).sum();

        // Verification Logic:
        // A correct implementation must stop gathering if a buffer cannot
        // expose its entire remaining content in a continuous vector of slices.
        // If total_len > 4, it means the implementation skipped the tail of
        // buf1 ("456789") and jumped straight to buf2, which breaks data ordering.
        assert!(
            total_len <= 4,
            "Error: Implementation gathered data from subsequent buffers before 
             exhausting the current buffer's remaining data! Total len: {}",
            total_len
        );
    }
}
