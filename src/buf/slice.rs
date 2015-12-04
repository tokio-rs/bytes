use std::cmp;
use {Buf, MutBuf};

// TODO: Rename -> Cursor. Use as buf for various byte strings

pub struct SliceBuf<'a> {
    bytes: &'a [u8],
    pos: usize
}

impl<'a> SliceBuf<'a> {
    pub fn wrap(bytes: &'a [u8]) -> SliceBuf<'a> {
        SliceBuf { bytes: bytes, pos: 0 }
    }
}

impl<'a> Buf for SliceBuf<'a> {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    fn bytes<'b>(&'b self) -> &'b [u8] {
        &self.bytes[self.pos..]
    }

    fn advance(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.remaining());
        self.pos += cnt;
    }
}

pub struct MutSliceBuf<'a> {
    bytes: &'a mut [u8],
    pos: usize
}

impl<'a> MutSliceBuf<'a> {
    pub fn wrap(bytes: &'a mut [u8]) -> MutSliceBuf<'a> {
        MutSliceBuf {
            bytes: bytes,
            pos: 0
        }
    }
}

impl<'a> MutBuf for MutSliceBuf<'a> {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    unsafe fn advance(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.remaining());
        self.pos += cnt;
    }

    unsafe fn mut_bytes<'b>(&'b mut self) -> &'b mut [u8] {
        &mut self.bytes[self.pos..]
    }
}
