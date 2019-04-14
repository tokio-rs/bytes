//! Crate-local import prelude, primarily intended as a facade for accessing
//! heap-allocated data structures.

#[cfg(all(feature = "alloc", not(feature = "std")))]
pub use alloc::boxed::Box;
#[cfg(all(feature = "alloc", not(feature = "std")))]
pub use alloc::string::String;
#[cfg(all(feature = "alloc", not(feature = "std")))]
pub use alloc::vec::Vec;

#[cfg(feature = "std")]
pub use std::prelude::v1::*;
