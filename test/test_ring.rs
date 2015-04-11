use bytes::RingBuf;


#[test]
pub fn test_initial_buf_empty() {
    use bytes::traits::{Buf, BufExt, MutBuf, MutBufExt};

    let mut buf = RingBuf::new(16);
    assert_eq!(MutBuf::remaining(&buf), 16);
    assert_eq!(Buf::remaining(&buf), 0);

    let bytes_written = buf.write(&[1, 2, 3][..]).unwrap();
    assert_eq!(bytes_written, 3);

    let bytes_written = buf.write(&[][..]).unwrap();
    assert_eq!(bytes_written, 0);
    assert_eq!(MutBuf::remaining(&buf), 13);
    assert_eq!(Buf::remaining(&buf), 3);
    assert_eq!(buf.bytes(), [1, 2, 3]);

    let mut out = [0u8; 3];

    buf.mark();
    let bytes_read = buf.read(&mut out[..]).unwrap();;
    assert_eq!(bytes_read, 3);
    assert_eq!(out, [1, 2, 3]);
    buf.reset();
    let bytes_read = buf.read(&mut out[..]).unwrap();;
    assert_eq!(bytes_read, 3);
    assert_eq!(out, [1, 2, 3]);

    assert_eq!(MutBuf::remaining(&buf), 16);
    assert_eq!(Buf::remaining(&buf), 0);
}

#[test]
fn test_wrapping_write() {
    use bytes::traits::{BufExt, MutBufExt};
    let mut buf = RingBuf::new(16);
    let mut out = [0;10];

    buf.write(&[42;12][..]).unwrap();
    let bytes_read = buf.read(&mut out[..]).unwrap();
    assert_eq!(bytes_read, 10);

    let bytes_written = buf.write(&[23;8][..]).unwrap();
    assert_eq!(bytes_written, 8);

    buf.mark();
    let bytes_read = buf.read(&mut out[..]).unwrap();
    assert_eq!(bytes_read, 10);
    assert_eq!(out, [42, 42, 23, 23, 23, 23, 23, 23, 23, 23]);
    buf.reset();
    let bytes_read = buf.read(&mut out[..]).unwrap();
    assert_eq!(bytes_read, 10);
    assert_eq!(out, [42, 42, 23, 23, 23, 23, 23, 23, 23, 23]);
}

#[test]
fn test_io_write_and_read() {
    use std::io::{Read, Write};

    let mut buf = RingBuf::new(16);
    let mut out = [0;8];

    let written = buf.write(&[1;8][..]).unwrap();
    assert_eq!(written, 8);

    buf.read(&mut out).unwrap();
    assert_eq!(out, [1;8]);

    let written = buf.write(&[2;8][..]).unwrap();
    assert_eq!(written, 8);

    let bytes_read = buf.read(&mut out).unwrap();
    assert_eq!(bytes_read, 8);
    assert_eq!(out, [2;8]);
}

#[test]
#[should_panic]
fn test_wrap_reset() {
    use std::io::{Read, Write};

    let mut buf = RingBuf::new(8);
    buf.write(&[1, 2, 3, 4, 5, 6, 7]).unwrap();
    buf.mark();
    buf.read(&mut [0; 4]).unwrap();
    buf.write(&[1, 2, 3, 4]).unwrap();
    buf.reset();
}
