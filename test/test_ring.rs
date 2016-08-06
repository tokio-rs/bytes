use bytes::{RingBuf, Buf, MutBuf};

#[test]
pub fn test_initial_buf_empty() {
    let mut buf = RingBuf::new(16);
    assert_eq!(MutBuf::remaining(&buf), 16);
    assert_eq!(Buf::remaining(&buf), 0);

    let bytes_written = buf.copy_from(&[1, 2, 3][..]);
    assert_eq!(bytes_written, 3);

    let bytes_written = buf.copy_from(&[][..]);
    assert_eq!(bytes_written, 0);
    assert_eq!(MutBuf::remaining(&buf), 13);
    assert_eq!(Buf::remaining(&buf), 3);
    assert_eq!(buf.bytes(), [1, 2, 3]);

    let mut out = [0u8; 3];

    buf.mark();
    let bytes_read = buf.copy_to(&mut out[..]);
    assert_eq!(bytes_read, 3);
    assert_eq!(out, [1, 2, 3]);
    buf.reset();
    let bytes_read = buf.copy_to(&mut out[..]);
    assert_eq!(bytes_read, 3);
    assert_eq!(out, [1, 2, 3]);

    assert_eq!(MutBuf::remaining(&buf), 16);
    assert_eq!(Buf::remaining(&buf), 0);
}

#[test]
fn test_wrapping_write() {
    let mut buf = RingBuf::new(16);
    let mut out = [0;10];

    buf.copy_from(&[42;12][..]);
    let bytes_read = buf.copy_to(&mut out[..]);
    assert_eq!(bytes_read, 10);

    let bytes_written = buf.copy_from(&[23;8][..]);
    assert_eq!(bytes_written, 8);

    buf.mark();
    let bytes_read = buf.copy_to(&mut out[..]);
    assert_eq!(bytes_read, 10);
    assert_eq!(out, [42, 42, 23, 23, 23, 23, 23, 23, 23, 23]);
    buf.reset();
    let bytes_read = buf.copy_to(&mut out[..]);
    assert_eq!(bytes_read, 10);
    assert_eq!(out, [42, 42, 23, 23, 23, 23, 23, 23, 23, 23]);
}

#[test]
fn test_io_write_and_read() {
    let mut buf = RingBuf::new(16);
    let mut out = [0u8;8];

    let written = buf.copy_from(&[1;8][..]);
    assert_eq!(written, 8);

    buf.copy_to(&mut out[..]);
    assert_eq!(out, [1;8]);

    let written = buf.copy_from(&[2;8][..]);
    assert_eq!(written, 8);

    let bytes_read = buf.copy_to(&mut out[..]);
    assert_eq!(bytes_read, 8);
    assert_eq!(out, [2;8]);
}

#[test]
#[should_panic]
fn test_wrap_reset() {
    let mut buf = RingBuf::new(8);
    buf.copy_from(&[1, 2, 3, 4, 5, 6, 7][..]);
    buf.mark();
    buf.copy_to(&mut [0; 4][..]);
    buf.copy_from(&[1, 2, 3, 4][..]);
    buf.reset();
}

#[test]
// Test that writes across a mark/reset are preserved.
fn test_mark_write() {
    let mut buf = RingBuf::new(8);
    buf.copy_from(&[1, 2, 3, 4, 5, 6, 7][..]);
    buf.mark();
    buf.copy_from(&[8][..]);
    buf.reset();

    let mut buf2 = [0; 8];
    buf.copy_to(&mut buf2[..]);
    assert_eq!(buf2, [1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
// Test that "RingBuf::reset" does not reset the length of a
// full buffer to zero.
fn test_reset_full() {
    let mut buf = RingBuf::new(8);
    buf.copy_from(&[1, 2, 3, 4, 5, 6, 7, 8][..]);
    assert_eq!(MutBuf::remaining(&buf), 0);
    buf.mark();
    buf.reset();
    assert_eq!(MutBuf::remaining(&buf), 0);
}


#[test]
// Test that "RingBuf::clear" does the full reset
fn test_clear() {
    let mut buf = RingBuf::new(8);
    buf.copy_from(&[0; 8][..]);
    assert_eq!(MutBuf::remaining(&buf), 0);
    assert_eq!(Buf::remaining(&buf), 8);
    buf.clear();
    assert_eq!(MutBuf::remaining(&buf), 8);
    assert_eq!(Buf::remaining(&buf), 0);
}
