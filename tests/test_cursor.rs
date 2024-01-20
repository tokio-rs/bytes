use bytes::{Buf, SeekBufExt};
use std::collections::VecDeque;

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

    assert_eq!(
        cursor.cursor().copied().collect::<Vec<u8>>().as_slice(),
        b"<<< TEXT >>>".as_slice()
    );

    let cursor = buf.cursor().seek(4..8).unwrap();

    assert_eq!(
        cursor.cursor().copied().collect::<Vec<u8>>().as_slice(),
        b"TEXT".as_slice()
    );

    let cursor = cursor.seek(0..=1).unwrap();

    assert_eq!(
        cursor.cursor().copied().collect::<Vec<u8>>().as_slice(),
        b"TE".as_slice()
    );
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

    assert_eq!(
        cursor.cursor().copied().collect::<Vec<u8>>().as_slice(),
        b"56789".as_slice()
    );

    cursor.advance_back_by(4).unwrap();

    assert_eq!(
        cursor.cursor().copied().collect::<Vec<u8>>().as_slice(),
        b"5".as_slice()
    );
}

#[test]
fn test_vec_deque_cursor() {
    let mut buf = VecDeque::with_capacity(8);

    while buf.len() < buf.capacity() {
        buf.push_back(b'0')
    }

    for _ in 0..4 {
        buf.pop_front();
    }

    for _ in 0..4 {
        buf.push_back(b'1')
    }

    let mut cursor = buf.cursor();

    cursor.advance_by(1).unwrap();

    assert_eq!(
        cursor
            .cursor()
            .seek(..3)
            .unwrap()
            .copied()
            .collect::<Vec<u8>>()
            .as_slice(),
        &[b'0', b'0', b'0'],
    );

    cursor.advance_back_by(1).unwrap();

    assert_eq!(
        cursor
            .cursor()
            .seek(buf.len() - 8..)
            .unwrap()
            .copied()
            .collect::<Vec<u8>>()
            .as_slice(),
        &[b'0', b'0', b'0', b'1', b'1', b'1'],
    );

    {
        // Advance forward through all remaining bytes in the cursor.
        let mut cursor = cursor.cursor();
        cursor.advance_by(cursor.remaining()).unwrap();
        assert_eq!(cursor.remaining(), 0);
    }

    {
        // Advance backward through all remaining bytes in the cursor.
        let mut cursor = cursor.cursor();
        cursor.advance_back_by(cursor.remaining()).unwrap();
        assert_eq!(cursor.remaining(), 0);
    }
}

/// PRNG implements a basic LCG random number generator with sane defaults.
struct PRNG {
    state: u32,
    multiplier: u32,
    increment: u32,
}

impl PRNG {
    pub fn new(seed: u32) -> Self {
        Self {
            state: seed,
            multiplier: 32310901,
            increment: 12345,
        }
    }

    pub fn random(&mut self) -> u32 {
        self.state = self
            .multiplier
            .overflowing_mul(self.state)
            .0
            .overflowing_add(self.increment)
            .0;
        self.state
    }
}

#[test]
fn test_vec_deque_cursor_random_insert_and_drain() {
    let mut prng = PRNG::new(7877 /* not important */);

    let mut buf = VecDeque::with_capacity(4096);

    for _ in 0..1000 {
        let remaining_mut = buf.capacity() - buf.len();

        buf.resize(buf.len() + (prng.random() as usize % remaining_mut), 0);
        let _ = buf.drain(..prng.random() as usize & buf.len());

        let cursor = buf.cursor();

        assert_eq!(
            cursor
                .cursor()
                .seek(..4.min(buf.len()))
                .unwrap()
                .copied()
                .collect::<Vec<u8>>()
                .as_slice(),
            &[0, 0, 0, 0][..4.min(buf.len())],
        );

        if buf.len() > 16 {
            let mut cursor = cursor.cursor().seek(4..12).unwrap();

            cursor.advance_by(0).unwrap();
            cursor.advance_back_by(0).unwrap();

            assert_eq!(
                cursor
                    .seek(2..6)
                    .unwrap()
                    .copied()
                    .collect::<Vec<u8>>()
                    .as_slice(),
                &[0, 0, 0, 0],
            );
        }

        assert_eq!(
            cursor
                .cursor()
                .rev()
                .take(4)
                .copied()
                .collect::<Vec<u8>>()
                .as_slice(),
            &[0, 0, 0, 0][..4.min(buf.len())],
        );

        {
            // Advance forward through all remaining bytes in the cursor.
            let mut cursor = cursor.cursor();
            cursor.advance_by(cursor.remaining()).unwrap();
            assert_eq!(cursor.remaining(), 0);
        }

        {
            // Advance backward through all remaining bytes in the cursor.
            let mut cursor = cursor.cursor();
            cursor.advance_back_by(cursor.remaining()).unwrap();
            assert_eq!(cursor.remaining(), 0);
        }
    }
}
