use std::collections::VecDeque;
use bytes::{Buf, SeekBuf};

#[test]
fn test_iterator() {
    let buf = b"Hello World!".as_slice();

    let mut cursor = buf.cursor();

    assert_eq!(cursor.next(), Some(&b'H'));
    assert_eq!(cursor.next(), Some(&b'e'));
    assert_eq!(cursor.next(), Some(&b'l'));
    assert_eq!(cursor.next(), Some(&b'l'));
    assert_eq!(cursor.next(), Some(&b'o'));

    assert_eq!(cursor.next_back(), Some(&b'!'));
    assert_eq!(cursor.next_back(), Some(&b'd'));
    assert_eq!(cursor.next_back(), Some(&b'l'));
    assert_eq!(cursor.next_back(), Some(&b'r'));
    assert_eq!(cursor.next_back(), Some(&b'o'));
    assert_eq!(cursor.next_back(), Some(&b'W'));

    assert_eq!(cursor.next(), Some(&b' '));

    assert_eq!(cursor.next(), None);
    assert_eq!(cursor.next_back(), None);
}

#[test]
fn test_seek() {
    let buf = b"<<< TEXT >>>".as_slice();

    let cursor = buf.cursor().seek(..).unwrap();

    assert_eq!(cursor.cursor().copied().collect::<Vec<u8>>().as_slice(), b"<<< TEXT >>>".as_slice());

    let cursor = buf.cursor().seek(4..8).unwrap();

    assert_eq!(cursor.cursor().copied().collect::<Vec<u8>>().as_slice(), b"TEXT".as_slice());

    let cursor = cursor.seek(0..=1).unwrap();

    assert_eq!(cursor.cursor().copied().collect::<Vec<u8>>().as_slice(), b"TE".as_slice());
}

#[test]
fn test_invalid_seek() {
    let buf = b"123".as_slice();

    assert!(buf.cursor().seek(4..).is_none());
}

#[test]
fn test_size() {
    let buf = b"123456789".as_slice();

    let mut cursor = buf.cursor();

    assert_eq!(cursor.size_hint(), (9, Some(9)));

    cursor.next();

    assert_eq!(cursor.size_hint(), (8, Some(8)));
}

#[test]
fn test_advance_by() {
    let buf = b"123456789".as_slice();

    let mut cursor = buf.cursor();

    cursor.advance_by(4).unwrap();

    assert_eq!(cursor.cursor().copied().collect::<Vec<u8>>().as_slice(), b"56789".as_slice());

    cursor.advance_back_by(4).unwrap();

    assert_eq!(cursor.cursor().copied().collect::<Vec<u8>>().as_slice(), b"5".as_slice());
}

#[test]
fn test_vec_deque_cursor() {
    let mut buf = VecDeque::with_capacity(8);

    while buf.len() < buf.capacity() {
        buf.push_back(b'0')
    }

    for _  in 0..4 {
        buf.pop_front();
    }

    for _  in 0..4 {
        buf.push_back(b'1')
    }

    let mut cursor = buf.cursor();

    cursor.advance_by(1).unwrap();

    assert_eq!(
        cursor.cursor().seek(..3).unwrap().copied().collect::<Vec<u8>>().as_slice(),
        &[b'0', b'0', b'0'],
    );

    cursor.advance_back_by(1).unwrap();

    assert_eq!(
        cursor.cursor().seek(buf.len() - 8..).unwrap().copied().collect::<Vec<u8>>().as_slice(),
        &[b'0', b'0', b'0', b'1', b'1', b'1'],
    );

    cursor.advance_back_by(cursor.remaining()).unwrap();
    assert_eq!(cursor.remaining(), 0);
}
