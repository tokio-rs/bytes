#![warn(rust_2018_idioms)]

use bytes::{Buf, BufMut, Bytes};
#[cfg(feature = "std")]
use std::io::IoSlice;

#[test]
fn collect_two_bufs() {
    let a = Bytes::from(&b"hello"[..]);
    let b = Bytes::from(&b"world"[..]);

    let res = a.chain(b).copy_to_bytes(10);
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

#[cfg(feature = "std")]
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

        assert_eq!(2, buf.chunks_vectored(&mut iovecs));
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

        assert_eq!(2, buf.chunks_vectored(&mut iovecs));
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

        assert_eq!(1, buf.chunks_vectored(&mut iovecs));
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

        assert_eq!(1, buf.chunks_vectored(&mut iovecs));
        assert_eq!(iovecs[0][..], b"ld"[..]);
        assert_eq!(iovecs[1][..], b""[..]);
        assert_eq!(iovecs[2][..], b""[..]);
        assert_eq!(iovecs[3][..], b""[..]);
    }
}

#[cfg(feature = "std")]
struct PartialBuf {
    data: &'static [u8],
    pos: usize,
}

#[cfg(feature = "std")]
impl Buf for PartialBuf {
    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn chunk(&self) -> &[u8] {
        &self.data[self.pos..self.pos.saturating_add(1).min(self.data.len())]
    }

    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.remaining());
        self.pos += cnt;
    }
}

#[cfg(feature = "std")]
#[test]
fn vectored_read_partial_first_buffer_preserves_order() {
    let first = PartialBuf {
        data: b"hello",
        pos: 0,
    };
    let mut chain = first.chain(&b"world"[..]);
    let empty: &[u8] = &[];
    let mut iovecs = [IoSlice::new(empty), IoSlice::new(empty)];
    let count = chain.chunks_vectored(&mut iovecs);
    let flattened: Vec<u8> = iovecs[..count]
        .iter()
        .flat_map(|slice| slice.iter().copied())
        .collect();
    assert!(b"helloworld".starts_with(&flattened));

    let mut output = Vec::new();
    let mut iterations = 0;
    while chain.has_remaining() {
        let empty: &[u8] = &[];
        let mut iovecs = [IoSlice::new(empty), IoSlice::new(empty)];
        let count = chain.chunks_vectored(&mut iovecs);
        let flattened: Vec<u8> = iovecs[..count]
            .iter()
            .flat_map(|slice| slice.iter().copied())
            .collect();
        assert!(!flattened.is_empty());
        assert!(b"helloworld"[output.len()..].starts_with(&flattened));
        output.extend_from_slice(&flattened);
        let len = flattened.len();
        chain.advance(len);
        iterations += 1;
        assert!(iterations <= b"helloworld".len());
    }
    assert_eq!(output, b"helloworld");
}

#[cfg(feature = "std")]
#[test]
fn vectored_read_complete_first_buffer_keeps_second_buffer_control() {
    let chain = (&b"hello"[..]).chain(&b"world"[..]);
    let empty: &[u8] = &[];
    let mut iovecs = [IoSlice::new(empty), IoSlice::new(empty)];

    let count = chain.chunks_vectored(&mut iovecs);
    let flattened: Vec<u8> = iovecs[..count]
        .iter()
        .flat_map(|slice| slice.iter().copied())
        .collect();
    assert_eq!(flattened, b"helloworld");
}

#[cfg(feature = "std")]
#[test]
fn vectored_read_chain_controls() {
    let empty: &[u8] = &[];

    let zero_destination = &mut (&b"hello"[..]).chain(&b"world"[..]);
    assert_eq!(zero_destination.chunks_vectored(&mut []), 0);
    assert_eq!(zero_destination.chunk(), b"hello");

    let first_empty = empty.chain(&b"world"[..]);
    let mut first_empty_slots = [IoSlice::new(empty), IoSlice::new(empty)];
    let count = first_empty.chunks_vectored(&mut first_empty_slots);
    assert_eq!(count, 1);
    assert_eq!(&first_empty_slots[0][..], b"world");

    let second_empty = (&b"hello"[..]).chain(empty);
    let mut second_empty_slots = [IoSlice::new(empty), IoSlice::new(empty)];
    let count = second_empty.chunks_vectored(&mut second_empty_slots);
    assert_eq!(count, 1);
    assert_eq!(&second_empty_slots[0][..], b"hello");

    let short_destination = (&b"hello"[..]).chain(&b"world"[..]);
    let mut short_slots = [IoSlice::new(empty)];
    let count = short_destination.chunks_vectored(&mut short_slots);
    assert_eq!(count, 1);
    assert_eq!(&short_slots[0][..], b"hello");

    let first = (&b"he"[..]).chain(&b"llo"[..]);
    let nested = first.chain(&b"world"[..]);
    let mut nested_slots = [
        IoSlice::new(empty),
        IoSlice::new(empty),
        IoSlice::new(empty),
    ];
    let count = nested.chunks_vectored(&mut nested_slots);
    assert_eq!(count, 3);
    assert_eq!(&nested_slots[0][..], b"he");
    assert_eq!(&nested_slots[1][..], b"llo");
    assert_eq!(&nested_slots[2][..], b"world");
}

#[test]
fn chain_growing_buffer() {
    let mut buff = [b' '; 10];
    let mut vec = b"wassup".to_vec();

    let mut chained = (&mut buff[..]).chain_mut(&mut vec).chain_mut(Vec::new()); // Required for potential overflow because remaining_mut for Vec is isize::MAX - vec.len(), but for chain_mut is usize::MAX

    chained.put_slice(b"hey there123123");

    assert_eq!(&buff, b"hey there1");
    assert_eq!(&vec, b"wassup23123");
}

#[test]
fn chain_overflow_remaining_mut() {
    let mut chained = Vec::<u8>::new().chain_mut(Vec::new()).chain_mut(Vec::new());

    assert_eq!(chained.remaining_mut(), usize::MAX);
    chained.put_slice(&[0; 256]);
    assert_eq!(chained.remaining_mut(), usize::MAX);
}

#[test]
fn chain_get_bytes() {
    let mut ab = Bytes::copy_from_slice(b"ab");
    let mut cd = Bytes::copy_from_slice(b"cd");
    let ab_ptr = ab.as_ptr();
    let cd_ptr = cd.as_ptr();
    let mut chain = (&mut ab).chain(&mut cd);
    let a = chain.copy_to_bytes(1);
    let bc = chain.copy_to_bytes(2);
    let d = chain.copy_to_bytes(1);

    assert_eq!(Bytes::copy_from_slice(b"a"), a);
    assert_eq!(Bytes::copy_from_slice(b"bc"), bc);
    assert_eq!(Bytes::copy_from_slice(b"d"), d);

    // assert `get_bytes` did not allocate
    assert_eq!(ab_ptr, a.as_ptr());
    // assert `get_bytes` did not allocate
    assert_eq!(cd_ptr.wrapping_offset(1), d.as_ptr());
}
