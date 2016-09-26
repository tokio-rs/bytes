extern crate bytes;

use bytes::{Buf, IntoBuf, Bytes};

pub fn dump<T>(data: &T) where
    for<'a> &'a T: IntoBuf,
{
    let mut dst: Vec<u8> = vec![];
    data.into_buf().copy_to(&mut dst);
    println!("GOT: {:?}", dst);
}

pub fn main() {
    let b = Bytes::from_slice(b"hello world");
    dump(&b);
    dump(&b);
}
