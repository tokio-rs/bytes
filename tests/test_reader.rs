extern crate bytes;

#[cfg(feature = "std")]
use std::io::{BufRead, Cursor, Read};
#[cfg(feature = "std")]
use bytes::Buf;

#[test]
#[cfg(feature = "std")]
fn read() {
    let buf1 = Cursor::new(b"hello ");
    let buf2 = Cursor::new(b"world");
    let buf = Buf::chain(buf1, buf2); // Disambiguate with Read::chain
    let mut buffer = Vec::new();
    buf.reader().read_to_end(&mut buffer).unwrap();
    assert_eq!(b"hello world", &buffer[..]);
}

#[test]
#[cfg(feature = "std")]
fn buf_read() {
    let buf1 = Cursor::new(b"hell");
    let buf2 = Cursor::new(b"o\nworld");
    let mut reader = Buf::chain(buf1, buf2).reader();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!("hello\n", &line);
    line.clear();
    reader.read_line(&mut line).unwrap();
    assert_eq!("world", &line);
}
