extern crate bytes;

#[cfg(feature = "std")]
use bytes::{Buf, Bytes, BytesMut};
#[cfg(feature = "std")]
use std::io::Cursor;

#[cfg(feature = "std")]
const LONG: &'static [u8] = b"mary had a little lamb, little lamb, little lamb";
#[cfg(feature = "std")]
const SHORT: &'static [u8] = b"hello world";

#[test]
#[cfg(feature = "std")]
fn collect_to_vec() {
    let buf: Vec<u8> = Cursor::new(SHORT).collect();
    assert_eq!(buf, SHORT);

    let buf: Vec<u8> = Cursor::new(LONG).collect();
    assert_eq!(buf, LONG);
}

#[test]
#[cfg(feature = "std")]
fn collect_to_bytes() {
    let buf: Bytes = Cursor::new(SHORT).collect();
    assert_eq!(buf, SHORT);

    let buf: Bytes = Cursor::new(LONG).collect();
    assert_eq!(buf, LONG);
}

#[test]
#[cfg(feature = "std")]
fn collect_to_bytes_mut() {
    let buf: BytesMut = Cursor::new(SHORT).collect();
    assert_eq!(buf, SHORT);

    let buf: BytesMut = Cursor::new(LONG).collect();
    assert_eq!(buf, LONG);
}
