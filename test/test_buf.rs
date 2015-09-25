use bytes::{Buf, MutBuf, MutBufExt};
use std::usize;
use std::io::{Cursor};

#[test]
pub fn test_fresh_cursor_vec() {
    let mut buf = Cursor::new(b"hello".to_vec());

    assert_eq!(buf.remaining(), 5);
    assert_eq!(buf.bytes(), b"hello");

    buf.advance(2);

    assert_eq!(buf.remaining(), 3);
    assert_eq!(buf.bytes(), b"llo");

    buf.advance(3);

    assert_eq!(buf.remaining(), 0);
    assert_eq!(buf.bytes(), b"");

    buf.advance(1);

    assert_eq!(buf.remaining(), 0);
    assert_eq!(buf.bytes(), b"");
}

#[test]
pub fn test_vec_as_mut_buf() {
    let mut buf = Vec::with_capacity(64);

    assert_eq!(buf.remaining(), usize::MAX);

    unsafe {
        assert!(buf.mut_bytes().len() >= 64);
    }

    buf.write(&b"zomg"[..]).unwrap();

    assert_eq!(&buf, b"zomg");

    assert_eq!(buf.remaining(), usize::MAX - 4);
    assert_eq!(buf.capacity(), 64);

    for _ in 0..16 {
        buf.write(&b"zomg"[..]).unwrap();
    }

    assert_eq!(buf.len(), 68);
}
