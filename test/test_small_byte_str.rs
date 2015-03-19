use bytes::SmallByteStr;
use bytes::traits::*;
use super::gen_bytes;

#[test]
pub fn test_slice_round_trip() {
    let mut dst = vec![];
    let src = gen_bytes(3);

    let s = SmallByteStr::from_slice(src.as_slice()).unwrap();
    assert_eq!(3, s.len());

    s.buf().read(&mut dst).unwrap();
    assert_eq!(dst, src);
}

#[test]
pub fn test_index() {
    let src = gen_bytes(3);

    let s = SmallByteStr::from_slice(src.as_slice()).unwrap();

    for i in 0..3 {
        assert_eq!(src[i], s[i]);
    }
}

#[test]
#[should_panic]
pub fn test_index_out_of_range() {
    let s = SmallByteStr::from_slice(gen_bytes(3).as_slice()).unwrap();
    let _ = s[2001];
}
