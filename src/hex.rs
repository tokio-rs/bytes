use std::fmt::{Formatter, LowerHex, Result, UpperHex};
use {Bytes, BytesMut};

struct BytesRef<'a>(&'a [u8]);

impl<'a> LowerHex for BytesRef<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        for b in self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

impl<'a> UpperHex for BytesRef<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        for b in self.0 {
            write!(f, "{:02X}", b)?;
        }
        Ok(())
    }
}

macro_rules! hex_impl {
    ($tr:ident, $ty:ty) => (
        impl $tr for $ty {
            fn fmt(&self, f: &mut Formatter) -> Result {
                $tr::fmt(&BytesRef(self.as_ref()), f)
            }
        }
    )
}

hex_impl!(LowerHex, Bytes);
hex_impl!(LowerHex, BytesMut);
hex_impl!(UpperHex, Bytes);
hex_impl!(UpperHex, BytesMut);
