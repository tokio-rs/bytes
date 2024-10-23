#![warn(rust_2018_idioms)]

use std::collections::VecDeque;

use bytes::Buf;
#[cfg(feature = "std")]
use std::io::IoSlice;

#[test]
fn test_fresh_cursor_vec() {
    let mut buf = &b"hello"[..];

    assert_eq!(buf.remaining(), 5);
    assert_eq!(buf.chunk(), b"hello");

    buf.advance(2);

    assert_eq!(buf.remaining(), 3);
    assert_eq!(buf.chunk(), b"llo");

    buf.advance(3);

    assert_eq!(buf.remaining(), 0);
    assert_eq!(buf.chunk(), b"");
}

#[test]
fn test_get_u8() {
    let mut buf = &b"\x21zomg"[..];
    assert_eq!(0x21, buf.get_u8());
}

#[test]
fn test_get_u16() {
    let mut buf = &b"\x21\x54zomg"[..];
    assert_eq!(0x2154, buf.get_u16());
    let mut buf = &b"\x21\x54zomg"[..];
    assert_eq!(0x5421, buf.get_u16_le());
}

#[test]
fn test_get_int() {
    let mut buf = &b"\xd6zomg"[..];
    assert_eq!(-42, buf.get_int(1));
    let mut buf = &b"\xd6zomg"[..];
    assert_eq!(-42, buf.get_int_le(1));

    let mut buf = &b"\xfe\x1d\xc0zomg"[..];
    assert_eq!(0xffffffffffc01dfeu64 as i64, buf.get_int_le(3));
    let mut buf = &b"\xfe\x1d\xc0zomg"[..];
    assert_eq!(0xfffffffffffe1dc0u64 as i64, buf.get_int(3));
}

#[test]
#[should_panic]
fn test_get_u16_buffer_underflow() {
    let mut buf = &b"\x21"[..];
    buf.get_u16();
}

#[cfg(feature = "std")]
#[test]
fn test_bufs_vec() {
    let buf = &b"hello world"[..];

    let b1: &[u8] = &mut [];
    let b2: &[u8] = &mut [];

    let mut dst = [IoSlice::new(b1), IoSlice::new(b2)];

    assert_eq!(1, buf.chunks_vectored(&mut dst[..]));
}

#[test]
fn test_vec_deque() {
    let mut buffer = VecDeque::new();
    buffer.extend(b"hello world");
    assert_eq!(11, buffer.remaining());
    assert_eq!(b"hello world", buffer.chunk());
    buffer.advance(6);
    assert_eq!(b"world", buffer.chunk());
    buffer.extend(b" piece");
    let mut out = [0; 11];
    buffer.copy_to_slice(&mut out);
    assert_eq!(b"world piece", &out[..]);
}

#[cfg(feature = "std")]
#[test]
fn test_vec_deque_vectored() {
    let mut buffer = VecDeque::new();
    buffer.reserve_exact(128);
    assert_eq!(buffer.chunks_vectored(&mut [IoSlice::new(&[])]), 0);

    buffer.extend(0..64);
    buffer.drain(..32);
    buffer.extend(64..150);

    assert_eq!(buffer.chunks_vectored(&mut []), 0);

    let mut chunks = [IoSlice::new(&[]); 1];
    assert_eq!(buffer.chunks_vectored(&mut chunks), 1);
    assert!(!chunks[0].is_empty());
    let combined = chunks[0].iter().copied().collect::<Vec<u8>>();
    let expected = (32..150).take(chunks[0].len()).collect::<Vec<_>>();
    assert_eq!(combined, expected);

    let mut chunks = [IoSlice::new(&[]); 2];
    assert_eq!(buffer.chunks_vectored(&mut chunks), 2);
    assert!(!chunks[0].is_empty());
    assert!(!chunks[1].is_empty());
    let combined = chunks
        .iter()
        .flat_map(|chunk| chunk.iter())
        .copied()
        .collect::<Vec<u8>>();
    let expected = (32..150).collect::<Vec<u8>>();
    assert_eq!(combined, expected);

    assert_eq!(buffer.chunks_vectored(&mut [IoSlice::new(&[]); 8]), 2);
}

#[allow(unused_allocation)] // This is intentional.
#[test]
fn test_deref_buf_forwards() {
    struct Special;

    impl Buf for Special {
        fn remaining(&self) -> usize {
            unreachable!("remaining");
        }

        fn chunk(&self) -> &[u8] {
            unreachable!("chunk");
        }

        fn advance(&mut self, _: usize) {
            unreachable!("advance");
        }

        fn get_u8(&mut self) -> u8 {
            // specialized!
            b'x'
        }
    }

    // these should all use the specialized method
    assert_eq!(Special.get_u8(), b'x');
    assert_eq!((&mut Special as &mut dyn Buf).get_u8(), b'x');
    assert_eq!((Box::new(Special) as Box<dyn Buf>).get_u8(), b'x');
    assert_eq!(Box::new(Special).get_u8(), b'x');
}

#[test]
fn copy_to_bytes_less() {
    let mut buf = &b"hello world"[..];

    let bytes = buf.copy_to_bytes(5);
    assert_eq!(bytes, &b"hello"[..]);
    assert_eq!(buf, &b" world"[..])
}

#[test]
#[should_panic]
fn copy_to_bytes_overflow() {
    let mut buf = &b"hello world"[..];

    let _bytes = buf.copy_to_bytes(12);
}
