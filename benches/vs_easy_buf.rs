#![feature(test)]

extern crate tokio_core;
extern crate bytes;
extern crate test;

mod bench_easy_buf {
    use test::{self, Bencher};
    use tokio_core::io::EasyBuf;

    #[bench]
    fn alloc_small(b: &mut Bencher) {
        b.iter(|| {
            for _ in 0..1024 {
                test::black_box(EasyBuf::with_capacity(12));
            }
        })
    }

    #[bench]
    fn alloc_mid(b: &mut Bencher) {
        b.iter(|| {
            test::black_box(EasyBuf::with_capacity(128));
        })
    }

    #[bench]
    fn alloc_big(b: &mut Bencher) {
        b.iter(|| {
            test::black_box(EasyBuf::with_capacity(4096));
        })
    }

    #[bench]
    fn deref_front(b: &mut Bencher) {
        let mut buf = EasyBuf::with_capacity(4096);
        buf.get_mut().extend_from_slice(&[0; 1024][..]);

        b.iter(|| {
            for _ in 0..1024 {
                test::black_box(buf.as_slice());
            }
        })
    }

    #[bench]
    fn deref_mid(b: &mut Bencher) {
        let mut buf = EasyBuf::with_capacity(4096);
        buf.get_mut().extend_from_slice(&[0; 1024][..]);
        let _a = buf.drain_to(512);

        b.iter(|| {
            for _ in 0..1024 {
                test::black_box(buf.as_slice());
            }
        })
    }

    #[bench]
    fn alloc_write_drain_to_mid(b: &mut Bencher) {
        b.iter(|| {
            let mut buf = EasyBuf::with_capacity(128);
            buf.get_mut().extend_from_slice(&[0u8; 64]);
            test::black_box(buf.drain_to(64));
        })
    }

    #[bench]
    fn drain_write_drain(b: &mut Bencher) {
        let data = [0u8; 128];

        b.iter(|| {
            let mut buf = EasyBuf::with_capacity(1024);
            let mut parts = Vec::with_capacity(8);

            for _ in 0..8 {
                buf.get_mut().extend_from_slice(&data[..]);
                parts.push(buf.drain_to(128));
            }

            test::black_box(parts);
        })
    }
}

mod bench_bytes {
    use test::{self, Bencher};
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
    fn split_off_and_drop(b: &mut Bencher) {
        b.iter(|| {
            for _ in 0..1024 {
                let v = vec![10, 20, 30, 40];
                let mut b = Bytes::from(v);
                test::black_box(b.split_off(3));
                test::black_box(b);
            }
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
    fn alloc_write_drain_to_mid(b: &mut Bencher) {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(128);
            buf.put_slice(&[0u8; 64]);
            test::black_box(buf.drain_to(64));
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
                parts.push(buf.drain_to(128));
            }

            test::black_box(parts);
        })
    }
}
