// pretend to like `crate::`
extern crate alloc;
#[path = "../src/buf/mod.rs"]
#[allow(warnings)]
mod buf;
#[path = "../src/debug.rs"]
#[allow(warnings)]
mod debug;
#[path = "../src/bytes.rs"]
#[allow(warnings)]
mod bytes;
#[path = "../src/bytes_mut.rs"]
#[allow(warnings)]
mod bytes_mut;
use std::process::abort;

use self::buf::{Buf, BufMut};
use self::bytes::Bytes;
use self::bytes_mut::BytesMut;

use std::sync::Arc;
use loom;
use loom::thread;

#[test]
fn bytes_cloning_vec() {
    loom::model(|| {
        let a = Bytes::from(b"abcdefgh".to_vec());
        let addr = a.as_ptr() as usize;

        // test the Bytes::clone is Sync by putting it in an Arc
        let a1 = Arc::new(a);
        let a2 = a1.clone();

        let t1 = thread::spawn(move || {
            let b: Bytes = (*a1).clone();
            assert_eq!(b.as_ptr() as usize, addr);
        });

        let t2 = thread::spawn(move || {
            let b: Bytes = (*a2).clone();
            assert_eq!(b.as_ptr() as usize, addr);
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}

#[test]
fn bytes_mut_cloning_frozen() {
    loom::model(|| {
        let a = BytesMut::from(&b"abcdefgh"[..]).split().freeze();
        let addr = a.as_ptr() as usize;

        // test the Bytes::clone is Sync by putting it in an Arc
        let a1 = Arc::new(a);
        let a2 = a1.clone();

        let t1 = thread::spawn(move || {
            let b: Bytes = (*a1).clone();
            assert_eq!(b.as_ptr() as usize, addr);
        });

        let t2 = thread::spawn(move || {
            let b: Bytes = (*a2).clone();
            assert_eq!(b.as_ptr() as usize, addr);
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}
