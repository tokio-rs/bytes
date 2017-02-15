extern crate bytes;

use bytes::{Bytes, BytesMut};

const LONG: &'static [u8] = b"mary had a little lamb, little lamb, little lamb";
const SHORT: &'static [u8] = b"hello world";

fn is_sync<T: Sync>() {}
fn is_send<T: Send>() {}

#[test]
fn test_bounds() {
    is_sync::<Bytes>();
    is_send::<Bytes>();
    is_send::<BytesMut>();
}

#[test]
fn from_slice() {
    let a = Bytes::from_slice(b"abcdefgh");
    assert_eq!(a, b"abcdefgh"[..]);
    assert_eq!(a, &b"abcdefgh"[..]);
    assert_eq!(a, Vec::from(&b"abcdefgh"[..]));
    assert_eq!(b"abcdefgh"[..], a);
    assert_eq!(&b"abcdefgh"[..], a);
    assert_eq!(Vec::from(&b"abcdefgh"[..]), a);

    let a = BytesMut::from_slice(b"abcdefgh");
    assert_eq!(a, b"abcdefgh"[..]);
    assert_eq!(a, &b"abcdefgh"[..]);
    assert_eq!(a, Vec::from(&b"abcdefgh"[..]));
    assert_eq!(b"abcdefgh"[..], a);
    assert_eq!(&b"abcdefgh"[..], a);
    assert_eq!(Vec::from(&b"abcdefgh"[..]), a);
}

#[test]
fn fmt() {
    let a = format!("{:?}", Bytes::from_slice(b"abcdefg"));
    let b = format!("{:?}", b"abcdefg");

    assert_eq!(a, b);

    let a = format!("{:?}", BytesMut::from_slice(b"abcdefg"));
    assert_eq!(a, b);
}

#[test]
fn len() {
    let a = Bytes::from_slice(b"abcdefg");
    assert_eq!(a.len(), 7);

    let a = BytesMut::from_slice(b"abcdefg");
    assert_eq!(a.len(), 7);

    let a = Bytes::from_slice(b"");
    assert!(a.is_empty());

    let a = BytesMut::from_slice(b"");
    assert!(a.is_empty());
}

#[test]
fn index() {
    let a = Bytes::from_slice(b"hello world");
    assert_eq!(a[0..5], *b"hello");
}

#[test]
fn slice() {
    let a = Bytes::from_slice(b"hello world");

    let b = a.slice(3, 5);
    assert_eq!(b, b"lo"[..]);

    let b = a.slice_to(5);
    assert_eq!(b, b"hello"[..]);

    let b = a.slice_from(3);
    assert_eq!(b, b"lo world"[..]);
}

#[test]
#[should_panic]
fn slice_oob_1() {
    let a = Bytes::from_slice(b"hello world");
    a.slice(5, 25);
}

#[test]
#[should_panic]
fn slice_oob_2() {
    let a = Bytes::from_slice(b"hello world");
    a.slice(25, 30);
}

#[test]
fn split_off() {
    let hello = Bytes::from_slice(b"helloworld");
    let world = hello.split_off(5);

    assert_eq!(hello, &b"hello"[..]);
    assert_eq!(world, &b"world"[..]);

    let mut hello = BytesMut::from_slice(b"helloworld");
    let world = hello.split_off(5);

    assert_eq!(hello, &b"hello"[..]);
    assert_eq!(world, &b"world"[..]);
}

#[test]
#[should_panic]
fn split_off_oob() {
    let hello = Bytes::from_slice(b"helloworld");
    hello.split_off(25);
}

#[test]
#[should_panic]
fn split_off_oob_mut() {
    let mut hello = BytesMut::from_slice(b"helloworld");
    hello.split_off(25);
}

#[test]
fn split_off_uninitialized() {
    let mut bytes = BytesMut::with_capacity(1024);
    let other = bytes.split_off(128);

    assert_eq!(bytes.len(), 0);
    assert_eq!(bytes.capacity(), 128);

    assert_eq!(other.len(), 0);
    assert_eq!(other.capacity(), 896);
}

#[test]
fn drain_to_1() {
    // Inline
    let a = Bytes::from_slice(SHORT);
    let b = a.drain_to(4);

    assert_eq!(SHORT[4..], a);
    assert_eq!(SHORT[..4], b);

    // Allocated
    let a = Bytes::from_slice(LONG);
    let b = a.drain_to(4);

    assert_eq!(LONG[4..], a);
    assert_eq!(LONG[..4], b);

    let a = Bytes::from_slice(LONG);
    let b = a.drain_to(30);

    assert_eq!(LONG[30..], a);
    assert_eq!(LONG[..30], b);
}

#[test]
#[should_panic]
fn drain_to_oob() {
    let hello = Bytes::from_slice(b"helloworld");
    hello.drain_to(30);
}

#[test]
#[should_panic]
fn drain_to_oob_mut() {
    let mut hello = BytesMut::from_slice(b"helloworld");
    hello.drain_to(30);
}

#[test]
fn drain_to_uninitialized() {
    let mut bytes = BytesMut::with_capacity(1024);
    let other = bytes.drain_to(128);

    assert_eq!(bytes.len(), 0);
    assert_eq!(bytes.capacity(), 896);

    assert_eq!(other.len(), 0);
    assert_eq!(other.capacity(), 128);
}
