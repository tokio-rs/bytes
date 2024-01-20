use bytes::{Buf, SeekBuf};
use std::collections::VecDeque;

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
    let mut buf = VecDeque::with_capacity(32);

    for i in 0..buf.capacity() {
        buf.push_back((i % 256) as u8);
    }

    assert_eq!(&buf.chunk()[..4], [0, 1, 2, 3].as_slice());
    assert_eq!(buf.chunk_to(2), Some([0, 1].as_slice()));
    assert_eq!(
        &buf.chunk_from(28).unwrap()[..4],
        [28, 29, 30, 31].as_slice()
    );

    for _ in 0..16 {
        buf.pop_front();
    }

    for _ in 0..15 {
        buf.push_back(0);
    }

    assert_eq!(&buf.chunk_from(0).unwrap()[..2], [16, 17].as_slice());

    buf.push_back(255);

    {
        let last_chunk = buf.chunk_to(buf.remaining()).unwrap();
        assert_eq!(&last_chunk[last_chunk.len() - 1..], [255].as_slice());
    }
}
