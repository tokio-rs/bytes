#![warn(rust_2018_idioms)]

use std::ptr;

use bytes::buf::Buf;
use bytes::Bytes;

#[test]
fn long_take() {
    // Tests that get a take with a size greater than the buffer length will not
    // overrun the buffer. Regression test for #138.
    let buf = b"hello world".take(100);
    assert_eq!(11, buf.remaining());
    assert_eq!(b"hello world", buf.chunk());
}

#[test]
fn copy_to_bytes() {
    let mut buf = Bytes::from("Hello World").take(8);
    let buf_ptr = buf.chunk().as_ptr();
    let copied = buf.copy_to_bytes(4);
    assert!(ptr::eq(buf_ptr, copied.as_ptr()));
}
