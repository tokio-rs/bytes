//! Support for generating random test cases with QuickCheck.

extern crate quickcheck;

use self::quickcheck::{Arbitrary, Gen};

use {Bytes, BytesMut};

// The implementations just use the pre-existing ones for `Vec<u8>`.

impl Arbitrary for Bytes {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        BytesMut::arbitrary(g).freeze()
    }

    fn shrink(&self) -> Box<Iterator<Item = Self>> {
        Box::new(self.as_ref().to_vec().shrink().map(|x| x.into()))
    }
}

impl Arbitrary for BytesMut {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        Vec::arbitrary(g).into()
    }

    fn shrink(&self) -> Box<Iterator<Item = Self>> {
        Box::new(self.as_ref().to_vec().shrink().map(|x| x.into()))
    }
}
