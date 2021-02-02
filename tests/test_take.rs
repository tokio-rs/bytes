#![warn(rust_2018_idioms)]

#[cfg(feature = "std")]
use std::io::IoSlice;

use bytes::buf::Buf;

#[test]
fn long_take() {
    // Tests that get a take with a size greater than the buffer length will not
    // overrun the buffer. Regression test for #138.
    let buf = b"hello world".take(100);
    assert_eq!(11, buf.remaining());
    assert_eq!(b"hello world", buf.chunk());
}

// Provide a buf with two slices.
#[cfg(feature = "std")]
fn chained() -> impl Buf {
    let a: &[u8] = b"Hello ";
    let b: &[u8] = b"World";
    a.chain(b)
}

#[test]
#[cfg(feature = "std")]
fn take_vectored_doesnt_fit() {
    // When there are not enough io slices.
    let mut slices = [IoSlice::new(&[]); 1];
    let buf = chained().take(10);
    assert_eq!(1, buf.chunks_vectored(&mut slices));
    assert_eq!(b"Hello ", &slices[0] as &[u8]);
}

#[test]
#[cfg(feature = "std")]
fn take_vectored_long() {
    let mut slices = [IoSlice::new(&[]); 2];
    let buf = chained().take(20);
    assert_eq!(2, buf.chunks_vectored(&mut slices));
    assert_eq!(b"Hello ", &slices[0] as &[u8]);
    assert_eq!(b"World", &slices[1] as &[u8]);
}

#[test]
#[cfg(feature = "std")]
fn take_vectored_many_slices() {
    let mut slices = [IoSlice::new(&[]); 3];
    let buf = chained().take(10);
    assert_eq!(2, buf.chunks_vectored(&mut slices));
    assert_eq!(b"Hello ", &slices[0] as &[u8]);
    assert_eq!(b"Worl", &slices[1] as &[u8]);
}

#[test]
#[cfg(feature = "std")]
fn take_vectored_short() {
    let mut slices = [IoSlice::new(&[]); 3];
    let buf = chained().take(3);
    assert_eq!(1, buf.chunks_vectored(&mut slices));
    assert_eq!(b"Hel", &slices[0] as &[u8]);
}
