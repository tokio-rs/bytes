#![crate_name = "bytes"]

/// A trait for objects that provide random and sequential access to bytes.
pub trait Buf {

    fn remaining(&self) -> usize;

    fn bytes<'a>(&'a self) -> &'a [u8];

    fn advance(&mut self, cnt: usize);

    fn has_remaining(&self) -> bool {
        self.remaining() > 0
    }
}

pub trait MutBuf : Buf {
    fn mut_bytes<'a>(&'a mut self) -> &'a mut [u8];
}
