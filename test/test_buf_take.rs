use bytes::*;
use std::io::{Cursor, Read};

#[test]
pub fn test_take_from_buf() {
    let mut buf = Take::new(Cursor::new(b"hello world".to_vec()), 5);
    let mut res = vec![];

    buf.read_to_end(&mut res).unwrap();

    assert_eq!(&res, b"hello");
}
