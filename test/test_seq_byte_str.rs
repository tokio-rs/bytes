use bytes::SeqByteStr;
use bytes::traits::*;
use super::gen_bytes;

#[test]
pub fn test_slice_round_trip() {
    let mut dst = vec![];
    let src = gen_bytes(2000);

    let s = SeqByteStr::from_slice(src.as_slice());
    assert_eq!(2000, s.len());

    s.buf().read(&mut dst).unwrap();
    assert_eq!(dst, src);
}

#[test]
pub fn test_index() {
    let src = gen_bytes(2000);

    let s = SeqByteStr::from_slice(src.as_slice());

    for i in 0..2000 {
        assert_eq!(src[i], s[i]);
    }
}

#[test]
#[should_panic]
pub fn test_index_out_of_range() {
    let s = SeqByteStr::from_slice(gen_bytes(2000).as_slice());
    let _ = s[2001];
}
