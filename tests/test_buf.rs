extern crate bytes;
extern crate byteorder;

use bytes::{Buf, Sink};
use std::io::Cursor;

#[test]
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

    buf.advance(1);

    assert_eq!(buf.remaining(), 0);
    assert_eq!(buf.bytes(), b"");
}

#[test]
fn test_get_u8() {
    let mut buf = Cursor::new(b"\x21zomg");
    assert_eq!(0x21, buf.get_u8());
}

#[test]
fn test_get_u16() {
    let buf = b"\x21\x54zomg";
    assert_eq!(0x2154, Cursor::new(buf).get_u16::<byteorder::BigEndian>());
    assert_eq!(0x5421, Cursor::new(buf).get_u16::<byteorder::LittleEndian>());
}

#[test]
#[should_panic]
fn test_get_u16_buffer_underflow() {
    let mut buf = Cursor::new(b"\x21");
    buf.get_u16::<byteorder::BigEndian>();
}

#[test]
fn test_vec_sink_capacity() {
    let mut sink: Vec<u8> = Vec::new();
    sink.reserve(16);
    assert!(sink.capacity() >= 16, "Capacity {} must be at least 16", sink.capacity());
    let mut source = Cursor::new(b"0123456789abcdef0123456789abcdef");
    sink.sink(&mut source);
    assert!(sink.len() <= sink.capacity(), "Length {} must be less than or equal to capacity {}", sink.len(), sink.capacity());
}
