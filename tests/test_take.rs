extern crate bytes;

use bytes::Buf;
use std::io::Cursor;

#[test]
fn long_prefix() {
    // Tests that get a prefix with a size greater than the buffer length will not
    // overrun the buffer. Regression test for #138.
    let buf = Cursor::new(b"hello world").prefix(100);
    assert_eq!(11, buf.remaining());
    assert_eq!(b"hello world", buf.bytes());
}
