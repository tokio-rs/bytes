extern crate bytes;

use bytes::Buf;
use std::io::Cursor;

#[test]
fn long_limit() {
    // Tests that get a limit with a size greater than the buffer length will not
    // overrun the buffer. Regression test for #138.
    let buf = Cursor::new(b"hello world").limit(100);
    assert_eq!(11, buf.remaining());
    assert_eq!(b"hello world", buf.bytes());
}
