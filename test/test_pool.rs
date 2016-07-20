use bytes::alloc::Pool;
use bytes::{Buf, MutBuf};
use rand::{self, Rng};
use byteorder::{ByteOrder, BigEndian};

#[test]
fn test_pool_of_zero_capacity() {
    let pool = Pool::with_capacity(0, 0);
    assert!(pool.new_byte_buf().is_none());

    let pool = Pool::with_capacity(0, 1_024);
    assert!(pool.new_byte_buf().is_none());
}

#[test]
fn test_pool_with_one_capacity() {
    let pool = Pool::with_capacity(1, 1024);

    let mut buf = pool.new_byte_buf().unwrap();
    assert!(pool.new_byte_buf().is_none());

    assert_eq!(1024, buf.remaining());

    buf.write_slice(b"Hello World");
    let mut buf = buf.flip();

    let mut dst = vec![];

    buf.copy_to(&mut dst).unwrap();

    assert_eq!(&dst[..], b"Hello World");

    // return the buffer to the pool
    drop(buf);

    let _ = pool.new_byte_buf().unwrap();
}

#[test]
fn test_pool_stress() {
    let pool = Pool::with_capacity(100, 4);
    let mut bufs = Vec::with_capacity(100);
    let mut rng = rand::thread_rng();

    let mut s = [0; 4];

    for i in 0..50_000u32 {
        let action: usize = rng.gen();

        match action % 3 {
            0 if bufs.len() < 100 => {
                let mut buf = pool.new_byte_buf().unwrap();
                BigEndian::write_u32(&mut s, i);
                buf.write_slice(&s);
                bufs.push((i, buf.flip()));
            }
            1 if bufs.len() > 0 => {
                // drop
                let len = bufs.len();
                let _ = bufs.remove(rng.gen::<usize>() % len);
            }
            2 if bufs.len() > 0 => {
                // read
                let len = bufs.len();
                let (i, mut buf) = bufs.remove(rng.gen::<usize>() % len);
                buf.mark();
                buf.read_slice(&mut s);
                buf.reset();
                let v = BigEndian::read_u32(&s);
                assert_eq!(i, v);
                bufs.push((i, buf));
            }
            3 if bufs.len() > 0 => {
                // write data
                let len = bufs.len();
                let (i, buf) = bufs.remove(rng.gen::<usize>() % len);
                let mut buf = buf.flip();
                BigEndian::write_u32(&mut s, i);
                buf.write_slice(&s);
                bufs.push((i, buf.flip()));
            }
            _ => {}
        }
    }
}
