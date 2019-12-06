#![deny(warnings, rust_2018_idioms)]

use bytes::Buf;
use std::io::IoSlice;

#[test]
fn test_fresh_cursor_vec() {
    let mut buf = &b"hello"[..];

    assert_eq!(buf.remaining(), 5);
    assert_eq!(buf.bytes(), b"hello");

    buf.advance(2);

    assert_eq!(buf.remaining(), 3);
    assert_eq!(buf.bytes(), b"llo");

    buf.advance(3);

    assert_eq!(buf.remaining(), 0);
    assert_eq!(buf.bytes(), b"");
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
#[should_panic]
fn test_get_u16_buffer_underflow() {
    let mut buf = &b"\x21"[..];
    buf.get_u16();
}

#[test]
fn test_bufs_vec() {
    let buf = &b"hello world"[..];

    let b1: &[u8] = &mut [];
    let b2: &[u8] = &mut [];

    let mut dst = [IoSlice::new(b1), IoSlice::new(b2)];

    assert_eq!(1, buf.bytes_vectored(&mut dst[..]));
}

#[test]
fn test_vec_deque() {
    use std::collections::VecDeque;

    let mut buffer: VecDeque<u8> = VecDeque::new();
    buffer.extend(b"hello world");
    assert_eq!(11, buffer.remaining());
    assert_eq!(b"hello world", buffer.bytes());
    buffer.advance(6);
    assert_eq!(b"world", buffer.bytes());
    buffer.extend(b" piece");
    let mut out = [0; 11];
    buffer.copy_to_slice(&mut out);
    assert_eq!(b"world piece", &out[..]);
}

#[test]
fn test_take() {
    // Pulling Read into the scope would result in a conflict between
    // Buf::bytes() from Read::bytes().
    let mut buf = std::io::Read::take(&b"hello world"[..], 5);
    assert_eq!(buf.bytes(), b"hello");
    assert_eq!(buf.remaining(), 5);

    buf.advance(3);
    assert_eq!(buf.bytes(), b"lo");
    assert_eq!(buf.remaining(), 2);

    buf.advance(2);
    assert_eq!(buf.bytes(), b"");
    assert_eq!(buf.remaining(), 0);
}

#[test]
#[should_panic]
fn test_take_advance_too_far() {
    let mut buf = std::io::Read::take(&b"hello world"[..], 5);
    buf.advance(10);
}

#[test]
fn test_take_limit_gt_length() {
    // The byte array has only 11 bytes, but we take 15 bytes.
    let mut buf = std::io::Read::take(&b"hello world"[..], 15);
    assert_eq!(buf.remaining(), 11);
    assert_eq!(buf.limit(), 15);

    buf.advance(5);
    assert_eq!(buf.remaining(), 6);
    // The limit is reduced my more than the number of bytes we advanced, to
    // the actual number of remaining bytes in the buffer.
    assert_eq!(buf.limit(), 6);
}
