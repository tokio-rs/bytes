/// impl_with_allocator implements a specified trait for a concrete type that
/// is expected to take some number of generic arguments and optionally a
/// trailing allocator argument of type [core::alloc::Allocator] only if the
/// unstable `allocator_api` feature is enabled.
#[macro_export]
macro_rules! impl_with_allocator {
    { impl $interface:ident for $concrete:ident<$( $generic_args:ty ),*> $implementation:tt } => {
        #[cfg(not(feature = "allocator_api"))]
        impl $interface for $concrete<$( $generic_args ),*> $implementation

        #[cfg(feature = "allocator_api")]
        impl<A: core::alloc::Allocator> $interface for $concrete<$( $generic_args ),*, A> $implementation
    };

    { unsafe impl $interface:ident for $concrete:ident<$( $generic_args:ty ),*> $implementation:tt } => {
        #[cfg(not(feature = "allocator_api"))]
        unsafe impl $interface for $concrete<$( $generic_args ),*> $implementation

        #[cfg(feature = "allocator_api")]
        unsafe impl<A: core::alloc::Allocator> $interface for $concrete<$( $generic_args ),*, A> $implementation
    };
}
