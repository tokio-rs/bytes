use bytes::{Buf, MutBuf};
use bytes::buf::SliceBuf;

#[test]
pub fn test_initial_buf_empty() {
    let buf = SliceBuf::with_capacity(100);

    assert!(buf.capacity() == 128);
    assert!(buf.remaining_write() == 128);
    assert!(buf.remaining_read() == 0);
}

#[test]
pub fn test_slice_buf_bytes() {
    let mut buf = SliceBuf::with_capacity(32);

    buf.copy_from(&b"hello "[..]);
    assert_eq!(&b"hello "[..], buf.bytes());

    buf.copy_from(&b"world"[..]);
    assert_eq!(&b"hello world"[..], buf.bytes());
}

#[test]
pub fn test_byte_buf_read_write() {
    let mut buf = SliceBuf::with_capacity(32);

    buf.copy_from(&b"hello world"[..]);
    assert_eq!(21, buf.remaining_write());

    buf.copy_from(&b" goodbye"[..]);
    assert_eq!(13, buf.remaining_write());

    let mut dst = [0; 5];

    let pos = buf.position();
    assert_eq!(5, buf.copy_to(&mut dst[..]));
    assert_eq!(b"hello", &dst);

    buf.set_position(pos);
    assert_eq!(5, buf.copy_to(&mut dst[..]));
    assert_eq!(b"hello", &dst);

    assert_eq!(5, buf.copy_to(&mut dst[..]));
    assert_eq!(b" worl", &dst);

    let mut dst = [0; 2];
    assert_eq!(2, buf.copy_to(&mut dst[..]));
    assert_eq!(b"d ", &dst);

    let mut dst = [0; 7];
    assert_eq!(7, buf.copy_to(&mut dst[..]));
    assert_eq!(b"goodbye", &dst);

    assert_eq!(13, buf.remaining_write());

    buf.copy_from(&b" have fun"[..]);
    assert_eq!(4, buf.remaining_write());

    assert_eq!(buf.bytes(), b" have fun");

    buf.set_position(0);
    assert_eq!(buf.bytes(), b"hello world goodbye have fun");

    buf.clear();
    assert_eq!(buf.bytes(), b"");
}
