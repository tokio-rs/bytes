extern crate bytes;
extern crate byteorder;
#[cfg(feature = "iovec")]
extern crate iovec;

#[cfg(feature = "std")]
use bytes::Buf;
#[cfg(feature = "iovec")]
use iovec::IoVec;
#[cfg(feature = "std")]
use std::io::Cursor;

#[test]
#[cfg(feature = "std")]
fn test_fresh_cursor_vec() {
    let mut buf = Cursor::new(b"hello".to_vec());

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
#[cfg(feature = "std")]
fn test_get_u8() {
    let mut buf = Cursor::new(b"\x21zomg");
    assert_eq!(0x21, buf.get_u8());
}

#[test]
#[cfg(feature = "std")]
fn test_get_u16() {
    let buf = b"\x21\x54zomg";
    assert_eq!(0x2154, Cursor::new(buf).get_u16_be());
    assert_eq!(0x5421, Cursor::new(buf).get_u16_le());
}

#[test]
#[should_panic]
#[cfg(feature = "std")]
fn test_get_u16_buffer_underflow() {
    let mut buf = Cursor::new(b"\x21");
    buf.get_u16_be();
}

#[test]
#[cfg(all(feature = "iovec", feature = "std"))]
fn test_bufs_vec() {
    let buf = Cursor::new(b"hello world");

    let b1: &[u8] = &mut [0];
    let b2: &[u8] = &mut [0];

    let mut dst: [&IoVec; 2] =
        [b1.into(), b2.into()];

    assert_eq!(1, buf.bytes_vec(&mut dst[..]));
}
