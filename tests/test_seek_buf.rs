use std::collections::VecDeque;
use bytes::{Buf, SeekBuf};

#[test]
fn test_seek_buf_cursor() {
    let buf = b"try to find the T in the haystack".as_slice();

    let remaining = buf.remaining();

    assert!(buf.cursor().find(|&&b| b == b'Q').is_none());
    assert!(buf.cursor().find(|&&b| b == b'T').is_some());

    // No bytes in the buffer were consumed while using the cursor.
    assert_eq!(remaining, buf.remaining());
}

#[test]
fn test_chunk_from() {
    let buf = b"hello world".as_slice();

    assert_eq!(buf.chunk_from(6), Some(b"world".as_slice()));
    assert_eq!(buf.chunk_from(100), None);
}

#[test]
fn test_chunk_to() {
    let buf = b"hello world".as_slice();

    assert_eq!(buf.chunk_to(5), Some(b"hello".as_slice()));
    assert_eq!(buf.chunk_to(100), None);

    // It may not be intuitive, but an identity of `chunk_to` is that when
    // passed an `end` of zero, it will always return an empty slice instead
    // of `None`.
    assert_eq!([].as_slice().chunk_to(0), Some([].as_slice()));
}

#[test]
fn test_vec_deque() {
    let mut buf = VecDeque::with_capacity(4);

    while buf.len() < buf.capacity() {
        buf.push_back(b'0')
    }

    assert_eq!(&buf.chunk()[..4], [b'0', b'0', b'0', b'0'].as_slice());
    assert_eq!(buf.chunk_to(2), Some([b'0', b'0'].as_slice()));
    assert_eq!(buf.chunk_from(buf.len() - 2), Some([b'0', b'0'].as_slice()));

    buf.pop_front();
    buf.pop_front();

    buf.push_back(b'3');
    buf.push_back(b'4');

    assert_eq!(&buf.chunk_from(0).unwrap()[..2], [b'0', b'0'].as_slice());
    assert_eq!(buf.chunk_to(buf.len()), Some([b'3', b'4'].as_slice()));
}
