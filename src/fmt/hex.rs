use core::fmt::{Formatter, LowerHex, Result, UpperHex};

use super::BytesRef;
#[cfg(feature = "alloc")]
use crate::{Bytes, BytesMut};

impl LowerHex for BytesRef<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        for &b in self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

impl UpperHex for BytesRef<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        for &b in self.0 {
            write!(f, "{:02X}", b)?;
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
macro_rules! hex_impl {
    ($tr:ident, $ty:ty) => {
        impl $tr for $ty {
            fn fmt(&self, f: &mut Formatter<'_>) -> Result {
                $tr::fmt(&BytesRef(self.as_ref()), f)
            }
        }
    };
}

#[cfg(feature = "alloc")]
hex_impl!(LowerHex, Bytes);
#[cfg(feature = "alloc")]
hex_impl!(LowerHex, BytesMut);
#[cfg(feature = "alloc")]
hex_impl!(UpperHex, Bytes);
#[cfg(feature = "alloc")]
hex_impl!(UpperHex, BytesMut);
