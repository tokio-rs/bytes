use bytes::MutBuf;
use byteorder;
use std::usize;

#[test]
pub fn test_vec_as_mut_buf() {
    let mut buf = Vec::with_capacity(64);

    assert_eq!(buf.remaining(), usize::MAX);

    unsafe {
        assert!(buf.mut_bytes().len() >= 64);
    }

    buf.copy_from(&b"zomg"[..]);

    assert_eq!(&buf, b"zomg");

    assert_eq!(buf.remaining(), usize::MAX - 4);
    assert_eq!(buf.capacity(), 64);

    for _ in 0..16 {
        buf.copy_from(&b"zomg"[..]);
    }

    assert_eq!(buf.len(), 68);
}

#[test]
pub fn test_write_u8() {
    let mut buf = Vec::with_capacity(8);
    buf.write_u8(33);
    assert_eq!(b"\x21", &buf[..]);
}

#[test]
fn test_write_u16() {
    let mut buf = Vec::with_capacity(8);
    buf.write_u16::<byteorder::BigEndian>(8532);
    assert_eq!(b"\x21\x54", &buf[..]);

    buf.clear();
    buf.write_u16::<byteorder::LittleEndian>(8532);
    assert_eq!(b"\x54\x21", &buf[..]);
}
