use bytes::{ByteStr, Buf, MutBuf};
use bytes::buf::AppendBuf;

#[test]
pub fn test_initial_buf_empty() {
    // Run in a loop a bunch in hope that if there is a memory issue, it will
    // be exposed
    for _ in 0..1000 {
        let mut buf = AppendBuf::with_capacity(100);
        let mut dst: Vec<u8> = vec![];

        assert_eq!(buf.remaining(), 128);

        buf.write_slice(b"hello world");
        assert_eq!(buf.remaining(), 117);
        assert_eq!(buf.bytes(), b"hello world");

        let view1 = buf.slice(0, 11);
        view1.buf().copy_to(&mut dst).unwrap();

        assert_eq!(dst, b"hello world");
        assert_eq!(view1, buf.slice(0, 11));

        drop(buf);
        let mut buf = AppendBuf::with_capacity(100);
        buf.write_slice(b"zomg no no no no");

        assert_eq!(dst, b"hello world");
    }
}
