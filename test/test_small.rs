use bytes::{Buf, Bytes};
use super::gen_bytes;

#[test]
pub fn test_slice_round_trip() {
    let mut dst = vec![];
    let src = gen_bytes(3);

    let s = Bytes::from(src.clone());
    assert_eq!(3, s.len());

    s.buf().copy_to(&mut dst);
    assert_eq!(dst, src);
}

#[test]
pub fn test_index() {
    let src = gen_bytes(3);

    let s = Bytes::from(src.clone());

    for i in 0..3 {
        assert_eq!(src[i], s[i]);
    }
}

#[test]
#[should_panic]
pub fn test_index_out_of_range() {
    let s = Bytes::from(gen_bytes(3));
    let _ = s[2001];
}
