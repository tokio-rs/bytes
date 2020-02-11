#![deny(warnings, rust_2018_idioms)]

use bytes::buf::BufQueue;
use bytes::Bytes;
use bytes::Buf;
use std::collections::VecDeque;
use std::cmp;
use std::io::IoSlice;


#[test]
fn simple() {
    let mut queue = BufQueue::new();
    queue.push(Bytes::copy_from_slice(b"abc"));
    queue.push(Bytes::copy_from_slice(b"de"));
    assert_eq!(5, queue.remaining());
    assert_eq!(b"abc", queue.bytes());
    queue.advance(1);
    assert_eq!(4, queue.remaining());
    assert_eq!(b"bc", queue.bytes());
    queue.advance(2);
    assert_eq!(2, queue.remaining());
    assert_eq!(b"de", queue.bytes());
    queue.push(Bytes::copy_from_slice(b"fgh"));
    assert_eq!(5, queue.remaining());
    assert_eq!(b"de", queue.bytes());
    // advance past front bytes
    queue.advance(4);
    assert_eq!(1, queue.remaining());
    assert_eq!(b"h", queue.bytes());
    queue.advance(1);
    assert_eq!(0, queue.remaining());
    assert_eq!(b"", queue.bytes());
}

struct Rng {
    state: u32,
}

impl Rng {
    // copy-paste from https://en.wikipedia.org/wiki/Xorshift
    fn next(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

#[test]
fn random() {
    let mut rng = Rng { state: 1 };

    // Update these two synchronously
    let mut correct: VecDeque<u8> = Default::default();
    let mut testing: BufQueue<BufQueue<Bytes>> = Default::default();

    for _ in 0..10000 {
        // uncomment to have a look at what is tested
        //println!("{:?}", testing);

        assert_eq!(correct.remaining(), testing.remaining());

        let bytes = testing.bytes();
        assert!(correct.len() == 0 || bytes.len() != 0);
        assert_eq!(bytes, &correct.iter().cloned().take(bytes.len()).collect::<Vec<_>>()[..]);

        if correct.len() >= 1000 || rng.next() % 2 == 0 {
            let take = cmp::min(rng.next() as usize % 10, correct.len());
            testing.advance(take);
            correct.advance(take);
        } else {
            let mut inner = BufQueue::new();

            let inner_len = rng.next() % 3;
            for _ in 0..inner_len {
                let bytes_len = rng.next() % 5;
                let v: Vec<u8> = (0..bytes_len).map(|_| rng.next() as u8).collect();
                correct.extend(&v);
                inner.push(Bytes::from(v));
            }

            testing.push(inner);

            assert_eq!(correct.len(), testing.remaining());
        }
    }
}

#[test]
fn vectored() {
    let mut v: BufQueue<BufQueue<Bytes>> = Default::default();
    v.push({
        let mut i = BufQueue::new();
        i.push(Bytes::copy_from_slice(b"ab"));
        i.push(Bytes::copy_from_slice(b"cde"));
        i
    });
    v.push({
        let mut i = BufQueue::new();
        i.push(Bytes::copy_from_slice(b"fg"));
        i
    });

    let zero = &mut [];
    assert_eq!(0, v.bytes_vectored(zero));

    let mut one = [IoSlice::new(&[])];
    assert_eq!(1, v.bytes_vectored(&mut one));
    assert_eq!(b"ab", &*one[0]);

    let mut two = [IoSlice::new(&[]), IoSlice::new(&[])];
    assert_eq!(2, v.bytes_vectored(&mut two));
    assert_eq!(b"ab", &*two[0]);
    assert_eq!(b"cde", &*two[1]);

    let mut three = [IoSlice::new(&[]), IoSlice::new(&[]), IoSlice::new(&[])];
    assert_eq!(3, v.bytes_vectored(&mut three));
    assert_eq!(b"ab", &*three[0]);
    assert_eq!(b"cde", &*three[1]);
    assert_eq!(b"fg", &*three[2]);

    let mut four = [IoSlice::new(&[]), IoSlice::new(&[]), IoSlice::new(&[]), IoSlice::new(&[])];
    assert_eq!(3, v.bytes_vectored(&mut four));
    assert_eq!(b"ab", &*four[0]);
    assert_eq!(b"cde", &*four[1]);
    assert_eq!(b"fg", &*four[2]);
    assert_eq!(b"", &*four[3]);
}
