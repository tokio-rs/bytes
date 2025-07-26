use alloc::vec::Vec;
use core::{
    convert::Infallible,
    fmt::{self, Display},
};
#[cfg(feature = "std")]
use std::error::Error;

/// The error type for try_reserve methods.
#[derive(Debug)]
pub struct TryReserveError(TryReserveErrorInner);

#[derive(Debug)]
pub(crate) enum TryReserveErrorInner {
    Std(alloc::collections::TryReserveError),
    Overflow,
}

impl Display for TryReserveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            TryReserveErrorInner::Std(err) => Display::fmt(err, f),
            TryReserveErrorInner::Overflow => f.write_str("memory allocation failed because the computed capacity exceeded the collection's maximum"),
        }
    }
}

#[cfg(feature = "std")]
impl Error for TryReserveError {}

/// The allocation strategy
///
/// # Safety
///
/// `fallible_reserve` must behave the same as `Vec::reserve` or
/// `Vec::try_reserve`.
pub(crate) unsafe trait AllocStrategy {
    type Err;

    fn fallible_reserve<T>(&self, vec: &mut Vec<T>, additional: usize) -> Result<(), Self::Err>;

    #[track_caller]
    fn capacity_overflow(&self) -> Self::Err;
}

pub(crate) struct PanickingAllocStrategy;

unsafe impl AllocStrategy for PanickingAllocStrategy {
    type Err = Infallible;

    fn fallible_reserve<T>(&self, vec: &mut Vec<T>, additional: usize) -> Result<(), Self::Err> {
        vec.reserve(additional);
        Ok(())
    }

    #[track_caller]
    fn capacity_overflow(&self) -> Self::Err {
        panic!("overflow")
    }
}

pub(crate) struct FallibleAllocStrategy;

unsafe impl AllocStrategy for FallibleAllocStrategy {
    type Err = TryReserveError;

    fn fallible_reserve<T>(&self, vec: &mut Vec<T>, additional: usize) -> Result<(), Self::Err> {
        vec.try_reserve(additional)
            .map_err(|err| TryReserveError(TryReserveErrorInner::Std(err)))
    }

    #[track_caller]
    fn capacity_overflow(&self) -> Self::Err {
        TryReserveError(TryReserveErrorInner::Overflow)
    }
}
