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

            let old_n = n;
            n += buf.chunks_vectored(&mut dst[n..]);

            let total_length: usize = dst[old_n..n].iter().map(|s| s.len()).sum();
            if total_length < buf.remaining() {
                // * we don't gather all the remaining data of the current buffer,
                // must stop here to preserve the correct data ordering.
                break;
            }
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
