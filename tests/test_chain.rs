#![deny(warnings, rust_2018_idioms)]

use bytes::{Buf, BufMut, Bytes};
use bytes::buf::{BufExt, BufMutExt};
use std::io::IoSlice;

#[test]
fn collect_two_bufs() {
    let a = Bytes::from(&b"hello"[..]);
    let b = Bytes::from(&b"world"[..]);

    let res = a.chain(b).to_bytes();
    assert_eq!(res, &b"helloworld"[..]);
}

#[test]
fn writing_chained() {
    let mut a = [0u8; 64];
    let mut b = [0u8; 64];

    {
        let mut buf = (&mut a[..]).chain_mut(&mut b[..]);

        for i in 0u8..128 {
            buf.put_u8(i);
        }
    }

    for i in 0..64 {
        let expect = i as u8;
        assert_eq!(expect, a[i]);
        assert_eq!(expect + 64, b[i]);
    }
}

#[test]
fn iterating_two_bufs() {
    let a = Bytes::from(&b"hello"[..]);
    let b = Bytes::from(&b"world"[..]);

    let res: Vec<u8> = a.chain(b).into_iter().collect();
    assert_eq!(res, &b"helloworld"[..]);
}

#[test]
fn vectored_read() {
    let a = Bytes::from(&b"hello"[..]);
    let b = Bytes::from(&b"world"[..]);

    let mut buf = a.chain(b);

    {
        let b1: &[u8] = &mut [];
        let b2: &[u8] = &mut [];
        let b3: &[u8] = &mut [];
        let b4: &[u8] = &mut [];
        let mut iovecs = [
            IoSlice::new(b1),
            IoSlice::new(b2),
            IoSlice::new(b3),
            IoSlice::new(b4),
        ];

        assert_eq!(2, buf.bytes_vectored(&mut iovecs));
        assert_eq!(iovecs[0][..], b"hello"[..]);
        assert_eq!(iovecs[1][..], b"world"[..]);
        assert_eq!(iovecs[2][..], b""[..]);
        assert_eq!(iovecs[3][..], b""[..]);
    }

    buf.advance(2);

    {
        let b1: &[u8] = &mut [];
        let b2: &[u8] = &mut [];
        let b3: &[u8] = &mut [];
        let b4: &[u8] = &mut [];
        let mut iovecs = [
            IoSlice::new(b1),
            IoSlice::new(b2),
            IoSlice::new(b3),
            IoSlice::new(b4),
        ];

        assert_eq!(2, buf.bytes_vectored(&mut iovecs));
        assert_eq!(iovecs[0][..], b"llo"[..]);
        assert_eq!(iovecs[1][..], b"world"[..]);
        assert_eq!(iovecs[2][..], b""[..]);
        assert_eq!(iovecs[3][..], b""[..]);
    }

    buf.advance(3);

    {
        let b1: &[u8] = &mut [];
        let b2: &[u8] = &mut [];
        let b3: &[u8] = &mut [];
        let b4: &[u8] = &mut [];
        let mut iovecs = [
            IoSlice::new(b1),
            IoSlice::new(b2),
            IoSlice::new(b3),
            IoSlice::new(b4),
        ];

        assert_eq!(1, buf.bytes_vectored(&mut iovecs));
        assert_eq!(iovecs[0][..], b"world"[..]);
        assert_eq!(iovecs[1][..], b""[..]);
        assert_eq!(iovecs[2][..], b""[..]);
        assert_eq!(iovecs[3][..], b""[..]);
    }

    buf.advance(3);

    {
        let b1: &[u8] = &mut [];
        let b2: &[u8] = &mut [];
        let b3: &[u8] = &mut [];
        let b4: &[u8] = &mut [];
        let mut iovecs = [
            IoSlice::new(b1),
            IoSlice::new(b2),
            IoSlice::new(b3),
            IoSlice::new(b4),
        ];

        assert_eq!(1, buf.bytes_vectored(&mut iovecs));
        assert_eq!(iovecs[0][..], b"ld"[..]);
        assert_eq!(iovecs[1][..], b""[..]);
        assert_eq!(iovecs[2][..], b""[..]);
        assert_eq!(iovecs[3][..], b""[..]);
    }
}

#[test]
fn chain_to_bytes() {
    let mut ab = Bytes::copy_from_slice(b"ab");
    let mut cd = Bytes::copy_from_slice(b"cd");
    let mut chain = (&mut ab).chain(&mut cd);
    assert_eq!(Bytes::copy_from_slice(b"abcd"), chain.to_bytes());
    assert_eq!(Bytes::new(), ab);
    assert_eq!(Bytes::new(), cd);
}

#[test]
fn chain_to_bytes_first_empty() {
    let mut cd = Bytes::copy_from_slice(b"cd");
    let cd_ptr = cd.as_ptr();
    let mut chain = Bytes::new().chain(&mut cd);
    let cd_to_bytes = chain.to_bytes();
    assert_eq!(b"cd", cd_to_bytes.as_ref());
    // assert `to_bytes` did not allocate
    assert_eq!(cd_ptr, cd_to_bytes.as_ptr());
    assert_eq!(Bytes::new(), cd);
}

#[test]
fn chain_to_bytes_second_empty() {
    let mut ab = Bytes::copy_from_slice(b"ab");
    let ab_ptr = ab.as_ptr();
    let mut chain = (&mut ab).chain(Bytes::new());
    let ab_to_bytes = chain.to_bytes();
    assert_eq!(b"ab", ab_to_bytes.as_ref());
    // assert `to_bytes` did not allocate
    assert_eq!(ab_ptr, ab_to_bytes.as_ptr());
    assert_eq!(Bytes::new(), ab);
}

#[test]
fn chain_get_bytes() {
    let mut ab = Bytes::copy_from_slice(b"ab");
    let mut cd = Bytes::copy_from_slice(b"cd");
    let ab_ptr = ab.as_ptr();
    let cd_ptr = cd.as_ptr();
    let mut chain = (&mut ab).chain(&mut cd);
    let a = chain.get_bytes(1);
    let bc = chain.get_bytes(2);
    let d = chain.get_bytes(1);

    assert_eq!(Bytes::copy_from_slice(b"a"), a);
    assert_eq!(Bytes::copy_from_slice(b"bc"), bc);
    assert_eq!(Bytes::copy_from_slice(b"d"), d);

    // assert `get_bytes` did not allocate
    assert_eq!(ab_ptr, a.as_ptr());
    // assert `get_bytes` did not allocate
    assert_eq!(cd_ptr.wrapping_offset(1), d.as_ptr());
}
