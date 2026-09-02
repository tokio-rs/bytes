#![warn(rust_2018_idioms)]

use bytes::{buf::IntoIter, Buf, Bytes};

#[test]
fn iter_len() {
    let buf = Bytes::from_static(b"hello world");
    let iter = IntoIter::new(buf);

    assert_eq!(iter.size_hint(), (11, Some(11)));
    assert_eq!(iter.len(), 11);
}

#[test]
fn empty_iter_len() {
    let buf = Bytes::new();
    let iter = IntoIter::new(buf);

    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.len(), 0);
}

/// Regression test for issue #833: a custom `Buf` impl that reports
/// `remaining() > 0` but returns `&[]` from `chunk()` must surface a clear
/// `Buf contract` panic message when iterated via `IntoIter`,
/// rather than an opaque `index out of bounds` panic.
#[derive(Debug)]
struct InconsistentChunk {
    remaining: usize,
}

impl Buf for InconsistentChunk {
    fn remaining(&self) -> usize {
        self.remaining
    }

    fn chunk(&self) -> &[u8] {
        &[]
    }

    fn advance(&mut self, cnt: usize) {
        self.remaining = self.remaining.saturating_sub(cnt);
    }
}

#[test]
#[should_panic(expected = "Buf contract")]
fn issue_833_into_iter_panics_with_clear_message_on_bad_chunk() {
    let buf = InconsistentChunk { remaining: 1 };
    let mut iter = IntoIter::new(buf);
    let _ = iter.next();
}
