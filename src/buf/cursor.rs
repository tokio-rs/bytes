use crate::{buf::Chain, Buf};
use std::collections::VecDeque;

pub trait Cursor: Buf {
    type Cursor<'a>: Buf
    where
        Self: 'a;

    fn cursor(&self, index: usize) -> Self::Cursor<'_>;
}

impl Cursor for &[u8] {
    type Cursor<'a>
        = &'a [u8]
    where
        Self: 'a;

    fn cursor(&self, index: usize) -> Self::Cursor<'_> {
        &self[index..]
    }
}

impl Cursor for VecDeque<u8> {
    type Cursor<'a>
        = Chain<&'a [u8], &'a [u8]>
    where
        Self: 'a;

    fn cursor(&self, index: usize) -> Self::Cursor<'_> {
        let (s1, s2) = self.as_slices();
        let mut chain = s1.chain(s2);
        chain.advance(index);
        chain
    }
}
