#![feature(test)]

use bytes::ByteBuf;
use bytes::alloc::Pool;
use test::Bencher;
use std::sync::Arc;

extern crate bytes;
extern crate test;

const SIZE:usize = 4_096;

#[bench]
pub fn bench_allocate_arc_vec(b: &mut Bencher) {
    b.iter(|| {
        let mut v = Vec::with_capacity(200);

        for _ in 0..200 {
            let buf = Arc::new(Vec::<u8>::with_capacity(SIZE));
            v.push(buf);
        }
    });
}

#[bench]
pub fn bench_allocate_byte_buf(b: &mut Bencher) {
    b.iter(|| {
        let mut v = Vec::with_capacity(200);

        for _ in 0..200 {
            let buf = ByteBuf::mut_with_capacity(SIZE);
            v.push(buf);
        }
    });
}

#[bench]
pub fn bench_allocate_with_pool(b: &mut Bencher) {
    let mut pool = Pool::with_capacity(1_024, SIZE);

    b.iter(|| {
         let mut v = Vec::with_capacity(200);

         for _ in 0..200 {
             let buf = pool.new_byte_buf();
             v.push(buf);
         }
    })
}
