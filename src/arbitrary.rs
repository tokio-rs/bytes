use super::{Bytes, BytesMut};
use arbitrary::{Arbitrary, Result};

impl<'a> Arbitrary<'a> for Bytes {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> Result<Self> {
        let len = u.arbitrary_len::<u8>()?;

        u.bytes(len).map(Bytes::copy_from_slice)
    }
}

impl<'a> Arbitrary<'a> for BytesMut {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> Result<Self> {
        let len = u.arbitrary_len::<u8>()?;

        u.bytes(len).map(|slice| BytesMut::from_vec(slice.to_vec()))
    }
}
