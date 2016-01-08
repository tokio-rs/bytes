use bytes::MutByteBuf;
use bytes::traits::*;

#[test]
pub fn test_initial_buf_empty() {
    let buf = MutByteBuf::with_capacity(100);

    assert!(buf.capacity() == 128);
    assert!(buf.remaining() == 128);

    let buf = buf.flip();

    assert!(buf.remaining() == 0);

    let buf = buf.flip();

    assert!(buf.remaining() == 128);
}

#[test]
pub fn test_byte_buf_bytes() {
    let mut buf = MutByteBuf::with_capacity(32);
    buf.write(&b"hello "[..]).unwrap();
    assert_eq!(&b"hello "[..], buf.bytes());

    buf.write(&b"world"[..]).unwrap();
    assert_eq!(&b"hello world"[..], buf.bytes());
    let buf = buf.flip();
    assert_eq!(&b"hello world"[..], buf.bytes());
}

#[test]
pub fn test_byte_buf_read_write() {
    let mut buf = MutByteBuf::with_capacity(32);

    buf.write(&b"hello world"[..]).unwrap();
    assert_eq!(21, buf.remaining());

    buf.write(&b" goodbye"[..]).unwrap();
    assert_eq!(13, buf.remaining());

    let mut buf = buf.flip();
    let mut dst = [0; 5];

    buf.mark();
    assert_eq!(5, buf.read(&mut dst[..]).unwrap());
    assert_eq!(b"hello", &dst);
    buf.reset();
    assert_eq!(5, buf.read(&mut dst[..]).unwrap());
    assert_eq!(b"hello", &dst);

    assert_eq!(5, buf.read(&mut dst[..]).unwrap());
    assert_eq!(b" worl", &dst);

    let mut dst = [0; 2];
    assert_eq!(2, buf.read(&mut dst[..]).unwrap());
    assert_eq!(b"d ", &dst);

    let mut dst = [0; 7];
    assert_eq!(7, buf.read(&mut dst[..]).unwrap());
    assert_eq!(b"goodbye", &dst);

    let mut buf = buf.resume();
    assert_eq!(13, buf.remaining());

    buf.write(&b" have fun"[..]).unwrap();
    assert_eq!(4, buf.remaining());

    let buf = buf.flip();
    assert_eq!(buf.bytes(), b"hello world goodbye have fun");
}
