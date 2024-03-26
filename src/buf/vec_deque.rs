use alloc::collections::VecDeque;

use super::{Buf, SeekBuf};

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

    fn advance(&mut self, cnt: usize) {
        self.drain(..cnt);
    }
}

impl SeekBuf for VecDeque<u8> {
    fn chunk_from(&self, start: usize) -> Option<&[u8]> {
        let slices = self.as_slices();

        if start < slices.0.len() {
            Some(&slices.0[start..])
        } else if start - slices.0.len() < slices.1.len() {
            Some(&slices.1[start - slices.0.len()..])
        } else {
            None
        }
    }

    fn chunk_to(&self, end: usize) -> Option<&[u8]> {
        let slices = self.as_slices();

        if end <= slices.0.len() {
            Some(&slices.0[..end])
        } else if end - slices.0.len() <= slices.1.len() {
            Some(&slices.1[..end - slices.0.len()])
        } else {
            None
        }
    }
}
