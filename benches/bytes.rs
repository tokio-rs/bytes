#![feature(test)]

extern crate bytes;
extern crate test;

use test::Bencher;
use bytes::{Bytes, BytesMut, BufMut};

#[bench]
fn alloc_small(b: &mut Bencher) {
    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(BytesMut::with_capacity(12));
        }
    })
}

#[bench]
fn alloc_mid(b: &mut Bencher) {
    b.iter(|| {
        test::black_box(BytesMut::with_capacity(128));
    })
}

#[bench]
fn alloc_big(b: &mut Bencher) {
    b.iter(|| {
        test::black_box(BytesMut::with_capacity(4096));
    })
}

#[bench]
fn deref_unique(b: &mut Bencher) {
    let mut buf = BytesMut::with_capacity(4096);
    buf.put(&[0u8; 1024][..]);

    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(&buf[..]);
        }
    })
}

#[bench]
fn deref_unique_unroll(b: &mut Bencher) {
    let mut buf = BytesMut::with_capacity(4096);
    buf.put(&[0u8; 1024][..]);

    b.iter(|| {
        for _ in 0..128 {
            test::black_box(&buf[..]);
            test::black_box(&buf[..]);
            test::black_box(&buf[..]);
            test::black_box(&buf[..]);
            test::black_box(&buf[..]);
            test::black_box(&buf[..]);
            test::black_box(&buf[..]);
            test::black_box(&buf[..]);
        }
    })
}

#[bench]
fn deref_shared(b: &mut Bencher) {
    let mut buf = BytesMut::with_capacity(4096);
    buf.put(&[0u8; 1024][..]);
    let _b2 = buf.split_off(1024);

    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(&buf[..]);
        }
    })
}

#[bench]
fn deref_inline(b: &mut Bencher) {
    let mut buf = BytesMut::with_capacity(8);
    buf.put(&[0u8; 8][..]);

    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(&buf[..]);
        }
    })
}

#[bench]
fn deref_two(b: &mut Bencher) {
    let mut buf1 = BytesMut::with_capacity(8);
    buf1.put(&[0u8; 8][..]);

    let mut buf2 = BytesMut::with_capacity(4096);
    buf2.put(&[0u8; 1024][..]);

    b.iter(|| {
        for _ in 0..512 {
            test::black_box(&buf1[..]);
            test::black_box(&buf2[..]);
        }
    })
}

#[bench]
fn alloc_write_split_to_mid(b: &mut Bencher) {
    b.iter(|| {
        let mut buf = BytesMut::with_capacity(128);
        buf.put_slice(&[0u8; 64]);
        test::black_box(buf.split_to(64));
    })
}

#[bench]
fn drain_write_drain(b: &mut Bencher) {
    let data = [0u8; 128];

    b.iter(|| {
        let mut buf = BytesMut::with_capacity(1024);
        let mut parts = Vec::with_capacity(8);

        for _ in 0..8 {
            buf.put(&data[..]);
            parts.push(buf.split_to(128));
        }

        test::black_box(parts);
    })
}

#[bench]
fn slice_empty(b: &mut Bencher) {
    b.iter(|| {
        // Use empty vec to avoid measure of allocation/deallocation
        let bytes = Bytes::from(Vec::new());
        (bytes.slice(0, 0), bytes)
    })
}

#[bench]
fn slice_not_empty(b: &mut Bencher) {
    b.iter(|| {
        let b = Bytes::from(b"aabbccddeeffgghh".to_vec());
        for _ in 0..1024 {
            test::black_box(b.slice(3, 5));
            test::black_box(&b);
        }
    })
}
