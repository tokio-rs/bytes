extern crate bytes;

use bytes::{Buf, BufMut, SliceBuf};

#[test]
fn test_initial_buf_empty() {
    let mut mem = [0u8; 100];
    let buf = SliceBuf::new(&mut mem[..]);

    assert!(buf.capacity() == 100);
    assert!(buf.remaining_mut() == 100);
    assert!(buf.remaining() == 0);
}

#[test]
fn test_slice_buf_bytes() {
    let mut mem = [0u8; 32];
    let mut buf = SliceBuf::new(&mut mem[..]);

    buf.copy_from(&b"hello "[..]);
    assert_eq!(&b"hello "[..], buf.bytes());

    buf.copy_from(&b"world"[..]);
    assert_eq!(&b"hello world"[..], buf.bytes());
}

#[test]
fn test_byte_buf_read_write() {
    let mut mem = [0u8; 32];
    let mut buf = SliceBuf::new(&mut mem[..]);

    buf.copy_from(&b"hello world"[..]);
    assert_eq!(21, buf.remaining_mut());

    buf.copy_from(&b" goodbye"[..]);
    assert_eq!(13, buf.remaining_mut());

    let mut dst = [0; 5];

    let pos = buf.position();
    buf.copy_to(&mut dst[..]);
    assert_eq!(b"hello", &dst);

    buf.set_position(pos);
    buf.copy_to(&mut dst[..]);
    assert_eq!(b"hello", &dst);

    buf.copy_to(&mut dst[..]);
    assert_eq!(b" worl", &dst);

    let mut dst = [0; 2];
    buf.copy_to(&mut dst[..]);
    assert_eq!(b"d ", &dst);

    let mut dst = [0; 7];
    buf.copy_to(&mut dst[..]);
    assert_eq!(b"goodbye", &dst);

    assert_eq!(13, buf.remaining_mut());

    buf.copy_from(&b" have fun"[..]);
    assert_eq!(4, buf.remaining_mut());

    assert_eq!(buf.bytes(), b" have fun");

    buf.set_position(0);
    assert_eq!(buf.bytes(), b"hello world goodbye have fun");

    buf.clear();
    assert_eq!(buf.bytes(), b"");
}
