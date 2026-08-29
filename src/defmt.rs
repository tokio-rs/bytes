use crate::{Bytes, BytesMut};

macro_rules! defmt_impl {
    ($ty:ident) => {
        impl defmt::Format for $ty {
            fn format(&self, fmt: defmt::Formatter<'_>) {
                defmt::write!(fmt, "{=[u8]}", self.as_ref())
            }
        }
    };
}

defmt_impl!(Bytes);
defmt_impl!(BytesMut);
