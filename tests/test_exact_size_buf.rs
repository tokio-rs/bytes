#![warn(rust_2018_idioms)]

use bytes::{buf::ExactSizeBuf, Buf, BufMut, Bytes, BytesMut};

const ABC: &'static [u8] = b"abcdefghijklmnopqrstuvwxyz";

const FOOBAR: &'static [u8] = b"foobar";
const BAZBLEE: &'static [u8] = b"bazblee";

#[test]
fn bytes_empty() {
    let buf = Bytes::copy_from_slice(&[]);

    assert_eq!(ExactSizeBuf::len(&buf), 0);
    assert_eq!(ExactSizeBuf::is_empty(&buf), true);
}

#[test]
fn bytes_non_empty() {
    let buf = Bytes::copy_from_slice(ABC);

    assert_eq!(ExactSizeBuf::len(&buf), 26);
    assert_eq!(ExactSizeBuf::is_empty(&buf), false);
}

#[test]
fn bytes_mut_empty() {
    let buf = BytesMut::new();

    assert_eq!(ExactSizeBuf::len(&buf), 0);
    assert_eq!(ExactSizeBuf::is_empty(&buf), true);
}

#[test]
fn bytes_mut_non_empty() {
    let mut buf = BytesMut::with_capacity(ABC.len());
    buf.put(&ABC[..]);

    assert_eq!(ExactSizeBuf::len(&buf), 26);
    assert_eq!(ExactSizeBuf::is_empty(&buf), false);
}

#[test]
fn slice_empty() {
    let buf: &[u8] = &[][..];

    assert_eq!(ExactSizeBuf::len(buf), 0);
    assert_eq!(ExactSizeBuf::is_empty(buf), true);
}

#[test]
fn slice_non_empty() {
    let buf: &[u8] = ABC;

    assert_eq!(ExactSizeBuf::len(buf), 26);
    assert_eq!(ExactSizeBuf::is_empty(buf), false);
}

#[test]
fn chain_empty_empty() {
    let first_buf = Bytes::copy_from_slice(&[]);
    let last_buf = Bytes::copy_from_slice(&[]);
    let buf = first_buf.chain(last_buf);

    assert_eq!(ExactSizeBuf::len(&buf), 0);
    assert_eq!(ExactSizeBuf::is_empty(&buf), true);
}

#[test]
fn chain_empty_non_empty() {
    let first_buf = Bytes::copy_from_slice(&[]);
    let last_buf = Bytes::copy_from_slice(BAZBLEE);
    let buf = first_buf.chain(last_buf);

    assert_eq!(ExactSizeBuf::len(&buf), BAZBLEE.len());
    assert_eq!(ExactSizeBuf::is_empty(&buf), false);
}

#[test]
fn chain_non_empty_non_empty() {
    let first_buf = Bytes::copy_from_slice(FOOBAR);
    let last_buf = Bytes::copy_from_slice(BAZBLEE);
    let buf = first_buf.chain(last_buf);

    assert_eq!(ExactSizeBuf::len(&buf), FOOBAR.len() + BAZBLEE.len());
    assert_eq!(ExactSizeBuf::is_empty(&buf), false);
}

#[test]
fn chain_non_empty_empty() {
    let first_buf = Bytes::copy_from_slice(FOOBAR);
    let last_buf = Bytes::copy_from_slice(&[]);
    let buf = first_buf.chain(last_buf);

    assert_eq!(ExactSizeBuf::len(&buf), FOOBAR.len());
    assert_eq!(ExactSizeBuf::is_empty(&buf), false);
}

#[test]
fn take_0_from_empty() {
    let buf = Bytes::copy_from_slice(&[]).take(0);
    assert_eq!(ExactSizeBuf::len(&buf), 0);
    assert_eq!(ExactSizeBuf::is_empty(&buf), true);
}

#[test]
fn take_3_from_empty() {
    let buf = Bytes::copy_from_slice(&[]).take(3);
    assert_eq!(ExactSizeBuf::len(&buf), 0);
    assert_eq!(ExactSizeBuf::is_empty(&buf), true);
}

#[test]
fn take_0_from_non_empty() {
    let buf = Bytes::copy_from_slice(ABC).take(0);
    assert_eq!(ExactSizeBuf::len(&buf), 0);
    assert_eq!(ExactSizeBuf::is_empty(&buf), true);
}

#[test]
fn take_100_from_non_empty() {
    let buf = Bytes::copy_from_slice(ABC).take(100);
    assert_eq!(ExactSizeBuf::len(&buf), ABC.len());
    assert_eq!(ExactSizeBuf::is_empty(&buf), false);
}
