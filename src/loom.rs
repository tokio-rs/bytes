pub(crate) mod sync {
    pub(crate) mod atomic {
        pub(crate) use core::sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering};
    }
}
