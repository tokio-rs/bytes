//! Crate-local import prelude, primarily intended as a facade for accessing
//! heap-allocated data structures.

#[cfg(feature = "std")]
pub use std::prelude::v1::*;
