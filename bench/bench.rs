#![feature(test, core)]

use bytes::ByteBuf;
use bytes::traits::*;
use iobuf::{RWIobuf};
use test::Bencher;

extern crate bytes;
extern crate iobuf;
extern crate test;

const SIZE:usize = 4_096;

#[bench]
pub fn bench_byte_buf_fill_4kb(b: &mut Bencher) {
    b.iter(|| {
        let mut buf = ByteBuf::mut_with_capacity(SIZE);

        for _ in 0..SIZE {
            buf.write_slice(&[0]);
        }
    });
}

#[bench]
pub fn bench_rw_iobuf_fill_4kb(b: &mut Bencher) {
    b.iter(|| {
        let mut buf = RWIobuf::new(SIZE);

        for _ in 0..SIZE {
            let _ = buf.fill(&[0]);
        }
    });
}
