extern crate bytes;

use bytes::{Buf, Bytes, BytesMut};

const LONG: &'static [u8] = b"mary had a little lamb, little lamb, little lamb";
const SHORT: &'static [u8] = b"hello world";

#[test]
fn collect_to_vec() {
    let buf: Vec<u8> = SHORT.collect();
    assert_eq!(buf, SHORT);

    let buf: Vec<u8> = LONG.collect();
    assert_eq!(buf, LONG);
}

#[test]
fn collect_to_bytes() {
    let buf: Bytes = SHORT.collect();
    assert_eq!(buf, SHORT);

    let buf: Bytes = LONG.collect();
    assert_eq!(buf, LONG);
}

#[test]
fn collect_to_bytes_mut() {
    let buf: BytesMut = SHORT.collect();
    assert_eq!(buf, SHORT);

    let buf: BytesMut = LONG.collect();
    assert_eq!(buf, LONG);
}
