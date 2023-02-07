/// Refcounted Immutable Buffer
#[allow(unused)]
use crate::loom::sync::atomic::AtomicMut;
use crate::loom::sync::atomic::AtomicPtr;
use alloc::vec::Vec;

/// A type alias for the tuple of:
/// 0. The data pointer referencing the container type used by the Bytes Instance
/// 1. The pointer offset into the buffer.
/// 2. The size of the buffer pointed to by [`ptr`]
pub type BufferParts = (AtomicPtr<()>, *const u8, usize);

/// A trait that describes the inner shared buffer for [`Bytes`] types.
///
/// The methods of the trait are all associated functions which are used as function
/// pointers in inner VTable implementation of the various modes of a [`Bytes`] instance.
///
/// An implementor of this trait must be cheaply clonable, and feature a singular buffer
/// which can be safely sliced in any fashion between the bounds of it's pointer and its `len`.
///
/// The remaining trait functions all take 3 parameters, which represent the state of the [`Bytes`]
/// instance that invoked the function.
/// The `data` param of each trait function equal the `AtomicPtr<()>` returned by into_parts.
/// The `ptr` param is the offset pointer into Self's buffer currently utilized in the calling [`Bytes`] instance.
/// The `len` param is the length of the slice from `ptr` currently utilized in the calling [`Bytes`] instance.
///
/// For implementors that leverage refcounting,  typically some sort of Wrapper struct
/// will need to act as a proxy between the [`Bytes`] instance and the inner type which does the
/// reference counting and manages its Buffer. This is similar to the implementation of [`Arc`].
///
/// # Example
///
/// [Here is an example implementation](https://github.com/tokio-rs/bytes/blob/master/tests/extern_buf_bytes.rs#L58)
///
/// # Safety
///
/// This trait deals exclusively with raw pointers.  These functions will cause UB if:
/// * The data pointer is NULL and the implemented functions expect a valid pointer.
/// * [`ptr`] is NULL or outside of the bounds of an allocated buffer.
/// * The len exceeds the capacity of the buffer pointed to by [`ptr`] and/or [`data`]
/// * The drop function deallocates the buffer in a different manner than it was allocated.
///
/// * [`Arc`]: std::sync::Arc
pub unsafe trait SharedBuf: 'static {
    /// Decompose `Self` into parts used by `Bytes`.
    fn into_parts(this: Self) -> BufferParts;

    /// Creates itself directly from the raw bytes parts decomposed with `into_bytes_parts`
    ///
    /// # Safety
    ///
    /// The implementation of this function must ensure that data and ptr and len are valid
    unsafe fn from_parts(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Self;

    /// (possibly) increases the reference count then
    /// returns the parts necessary to construct a new Bytes instance.
    ///
    /// # Safety
    ///
    /// The implementation of this function must ensure that data and ptr and len are valid
    unsafe fn clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BufferParts;

    /// Called before the `Bytes::truncate` is processed.  
    /// Useful if the implementation needs some preparation step for it.
    ///
    /// # Safety
    ///
    /// The implementation of this function must ensure that data and ptr and len are valid
    unsafe fn try_resize(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) {
        let (_, _, _) = (data, ptr, len);
    }

    /// Consumes underlying resources and return `Vec<u8>`, usually with allocation
    ///
    /// # Safety
    ///
    /// The implementation of this function must ensure that data and ptr and len are valid
    unsafe fn into_vec(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8>;

    /// Release underlying resources.
    /// Decrement a refcount.
    /// If refcount == 0 then drop or otherwise deallocate any resources allocated by T
    ///
    /// # Safety
    ///
    /// The implementation of this function must ensure that data and ptr and len are valid
    unsafe fn drop(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize);
}
