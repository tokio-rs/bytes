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
/// `fallible_with_capacity` must behave the same as
/// `Vec::with_capacity`, `Vec::try_with_capacity` or always fail.
///
/// `fallible_reserve` must behave the same as `Vec::reserve`,
/// `Vec::try_reserve` or always fail.
pub(crate) unsafe trait AllocStrategy {
    type Err;

    fn fallible_with_capacity<T>(&self, capacity: usize) -> Result<Vec<T>, Self::Err>;

    fn fallible_reserve<T>(&self, vec: &mut Vec<T>, additional: usize) -> Result<(), Self::Err>;

    fn capacity_overflow(&self) -> Self::Err;
}

pub(crate) struct PanickingAllocStrategy;

unsafe impl AllocStrategy for PanickingAllocStrategy {
    type Err = Infallible;

    fn fallible_with_capacity<T>(&self, capacity: usize) -> Result<Vec<T>, Self::Err> {
        Ok(Vec::with_capacity(capacity))
    }

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

    fn fallible_with_capacity<T>(&self, capacity: usize) -> Result<Vec<T>, Self::Err> {
        let mut vec = Vec::new();
        self.fallible_reserve(&mut vec, capacity)?;
        Ok(vec)
    }

    fn fallible_reserve<T>(&self, vec: &mut Vec<T>, additional: usize) -> Result<(), Self::Err> {
        vec.try_reserve(additional)
            .map_err(|err| TryReserveError(TryReserveErrorInner::Std(err)))
    }

    fn capacity_overflow(&self) -> Self::Err {
        TryReserveError(TryReserveErrorInner::Overflow)
    }
}

pub(crate) struct NoAllocStrategy;

unsafe impl AllocStrategy for NoAllocStrategy {
    type Err = ();

    fn fallible_with_capacity<T>(&self, capacity: usize) -> Result<Vec<T>, Self::Err> {
        let _ = capacity;
        Err(())
    }

    fn fallible_reserve<T>(&self, vec: &mut Vec<T>, additional: usize) -> Result<(), Self::Err> {
        let _ = vec;
        let _ = additional;
        Err(())
    }

    fn capacity_overflow(&self) -> Self::Err {}
}
