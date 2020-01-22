#![deny(warnings, rust_2018_idioms)]

use bytes::buf::{Buf, BufExt};
use bytes::Bytes;

#[test]
fn long_take() {
    // Tests that get a take with a size greater than the buffer length will not
    // overrun the buffer. Regression test for #138.
    let buf = b"hello world".take(100);
    assert_eq!(11, buf.remaining());
    assert_eq!(b"hello world", buf.bytes());
}

#[test]
fn take_to_bytes() {
    let mut abcd = Bytes::copy_from_slice(b"abcd");
    let abcd_ptr = abcd.as_ptr();
    let mut take = (&mut abcd).take(2);
    let ab = take.to_bytes();
    assert_eq!(Bytes::copy_from_slice(b"ab"), ab);
    // assert `to_bytes` did not allocate
    assert_eq!(abcd_ptr, ab.as_ptr());
    assert_eq!(Bytes::copy_from_slice(b"cd"), abcd);
}

#[test]
fn take_get_bytes() {
    let mut abcd = Bytes::copy_from_slice(b"abcd");
    let abcd_ptr = abcd.as_ptr();
    let mut take = (&mut abcd).take(2);
    let a = take.get_bytes(1);
    assert_eq!(Bytes::copy_from_slice(b"a"), a);
    // assert `to_bytes` did not allocate
    assert_eq!(abcd_ptr, a.as_ptr());
    assert_eq!(Bytes::copy_from_slice(b"bcd"), abcd);
}

#[test]
#[should_panic]
fn take_get_bytes_panics() {
    let abcd = Bytes::copy_from_slice(b"abcd");
    abcd.take(2).get_bytes(3);
}
