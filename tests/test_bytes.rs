extern crate bytes;

use bytes::{Bytes, BytesMut, BufMut};

const LONG: &'static [u8] = b"mary had a little lamb, little lamb, little lamb";
const SHORT: &'static [u8] = b"hello world";

fn inline_cap() -> usize {
    use std::mem;
    4 * mem::size_of::<usize>() - 1
}

fn is_sync<T: Sync>() {}
fn is_send<T: Send>() {}

#[test]
fn test_bounds() {
    is_sync::<Bytes>();
    is_sync::<BytesMut>();
    is_send::<Bytes>();
    is_send::<BytesMut>();
}

#[test]
fn from_slice() {
    let a = Bytes::from(&b"abcdefgh"[..]);
    assert_eq!(a, b"abcdefgh"[..]);
    assert_eq!(a, &b"abcdefgh"[..]);
    assert_eq!(a, Vec::from(&b"abcdefgh"[..]));
    assert_eq!(b"abcdefgh"[..], a);
    assert_eq!(&b"abcdefgh"[..], a);
    assert_eq!(Vec::from(&b"abcdefgh"[..]), a);

    let a = BytesMut::from(&b"abcdefgh"[..]);
    assert_eq!(a, b"abcdefgh"[..]);
    assert_eq!(a, &b"abcdefgh"[..]);
    assert_eq!(a, Vec::from(&b"abcdefgh"[..]));
    assert_eq!(b"abcdefgh"[..], a);
    assert_eq!(&b"abcdefgh"[..], a);
    assert_eq!(Vec::from(&b"abcdefgh"[..]), a);
}

#[test]
fn fmt() {
    let a = format!("{:?}", Bytes::from(&b"abcdefg"[..]));
    let b = "b\"abcdefg\"";

    assert_eq!(a, b);

    let a = format!("{:?}", BytesMut::from(&b"abcdefg"[..]));
    assert_eq!(a, b);
}

#[test]
fn len() {
    let a = Bytes::from(&b"abcdefg"[..]);
    assert_eq!(a.len(), 7);

    let a = BytesMut::from(&b"abcdefg"[..]);
    assert_eq!(a.len(), 7);

    let a = Bytes::from(&b""[..]);
    assert!(a.is_empty());

    let a = BytesMut::from(&b""[..]);
    assert!(a.is_empty());
}

#[test]
fn index() {
    let a = Bytes::from(&b"hello world"[..]);
    assert_eq!(a[0..5], *b"hello");
}

#[test]
fn slice() {
    let a = Bytes::from(&b"hello world"[..]);

    let b = a.slice(3, 5);
    assert_eq!(b, b"lo"[..]);

    let b = a.slice_to(5);
    assert_eq!(b, b"hello"[..]);

    let b = a.slice_from(3);
    assert_eq!(b, b"lo world"[..]);
}

#[test]
#[should_panic]
fn slice_oob_1() {
    let a = Bytes::from(&b"hello world"[..]);
    a.slice(5, inline_cap() + 1);
}

#[test]
#[should_panic]
fn slice_oob_2() {
    let a = Bytes::from(&b"hello world"[..]);
    a.slice(inline_cap() + 1, inline_cap() + 5);
}

#[test]
fn split_off() {
    let mut hello = Bytes::from(&b"helloworld"[..]);
    let world = hello.split_off(5);

    assert_eq!(hello, &b"hello"[..]);
    assert_eq!(world, &b"world"[..]);

    let mut hello = BytesMut::from(&b"helloworld"[..]);
    let world = hello.split_off(5);

    assert_eq!(hello, &b"hello"[..]);
    assert_eq!(world, &b"world"[..]);
}

#[test]
#[should_panic]
fn split_off_oob() {
    let mut hello = Bytes::from(&b"helloworld"[..]);
    hello.split_off(inline_cap() + 1);
}

#[test]
fn split_off_uninitialized() {
    let mut bytes = BytesMut::with_capacity(1024);
    let other = bytes.split_off(128);

    assert_eq!(bytes.len(), 0);
    assert_eq!(bytes.capacity(), 128);

    assert_eq!(other.len(), 0);
    assert_eq!(other.capacity(), 896);
}

#[test]
fn split_to_1() {
    // Inline
    let mut a = Bytes::from(SHORT);
    let b = a.split_to(4);

    assert_eq!(SHORT[4..], a);
    assert_eq!(SHORT[..4], b);

    // Allocated
    let mut a = Bytes::from(LONG);
    let b = a.split_to(4);

    assert_eq!(LONG[4..], a);
    assert_eq!(LONG[..4], b);

    let mut a = Bytes::from(LONG);
    let b = a.split_to(30);

    assert_eq!(LONG[30..], a);
    assert_eq!(LONG[..30], b);
}

#[test]
fn split_to_2() {
    let mut a = Bytes::from(LONG);
    assert_eq!(LONG, a);

    let b = a.split_to(1);

    assert_eq!(LONG[1..], a);
    drop(b);
}

#[test]
#[should_panic]
fn split_to_oob() {
    let mut hello = Bytes::from(&b"helloworld"[..]);
    hello.split_to(inline_cap() + 1);
}

#[test]
#[should_panic]
fn split_to_oob_mut() {
    let mut hello = BytesMut::from(&b"helloworld"[..]);
    hello.split_to(inline_cap() + 1);
}

#[test]
fn split_to_uninitialized() {
    let mut bytes = BytesMut::with_capacity(1024);
    let other = bytes.split_to(128);

    assert_eq!(bytes.len(), 0);
    assert_eq!(bytes.capacity(), 896);

    assert_eq!(other.len(), 0);
    assert_eq!(other.capacity(), 128);
}

#[test]
fn fns_defined_for_bytes_mut() {
    let mut bytes = BytesMut::from(&b"hello world"[..]);

    bytes.as_ptr();
    bytes.as_mut_ptr();

    // Iterator
    let v: Vec<u8> = bytes.iter().map(|b| *b).collect();
    assert_eq!(&v[..], bytes);
}

#[test]
fn reserve_convert() {
    // Inline -> Vec
    let mut bytes = BytesMut::with_capacity(8);
    bytes.put("hello");
    bytes.reserve(40);
    assert_eq!(bytes.capacity(), 45);
    assert_eq!(bytes, "hello");

    // Inline -> Inline
    let mut bytes = BytesMut::with_capacity(inline_cap());
    bytes.put("abcdefghijkl");

    let a = bytes.split_to(10);
    bytes.reserve(inline_cap() - 3);
    assert_eq!(inline_cap(), bytes.capacity());

    assert_eq!(bytes, "kl");
    assert_eq!(a, "abcdefghij");

    // Vec -> Vec
    let mut bytes = BytesMut::from(LONG);
    bytes.reserve(64);
    assert_eq!(bytes.capacity(), LONG.len() + 64);

    // Arc -> Vec
    let mut bytes = BytesMut::from(LONG);
    let a = bytes.split_to(30);

    bytes.reserve(128);
    assert_eq!(bytes.capacity(), (bytes.len() + 128).next_power_of_two());

    drop(a);
}

#[test]
fn reserve_growth() {
    let mut bytes = BytesMut::with_capacity(64);
    bytes.put("hello world");
    let _ = bytes.take();

    bytes.reserve(65);
    assert_eq!(bytes.capacity(), 128);
}

#[test]
fn reserve_allocates_at_least_original_capacity() {
    let mut bytes = BytesMut::with_capacity(128);

    for i in 0..120 {
        bytes.put(i as u8);
    }

    let _other = bytes.take();

    bytes.reserve(16);
    assert_eq!(bytes.capacity(), 128);
}

#[test]
fn reserve_max_original_capacity_value() {
    const SIZE: usize = 128 * 1024;

    let mut bytes = BytesMut::with_capacity(SIZE);

    for _ in 0..SIZE {
        bytes.put(0u8);
    }

    let _other = bytes.take();

    bytes.reserve(16);
    assert_eq!(bytes.capacity(), 64 * 1024);
}

#[test]
fn inline_storage() {
    let mut bytes = BytesMut::with_capacity(inline_cap());
    let zero = [0u8; 64];

    bytes.put(&zero[0..inline_cap()]);
    assert_eq!(*bytes, zero[0..inline_cap()]);
}

#[test]
fn extend() {
    let mut bytes = BytesMut::with_capacity(0);
    bytes.extend(LONG);
    assert_eq!(*bytes, LONG[..]);
}

#[test]
fn into_vec_kind_vec() {
    let vec = vec![10, 20, 30];
    let ptr = vec.as_slice().as_ptr();

    let bytes = Bytes::from(vec);

    let vec = bytes.into_vec();
    assert_eq!(vec![10, 20, 30], vec);
    assert_eq!(ptr, vec.as_slice().as_ptr())
}

#[test]
fn into_vec_kind_arc_unique() {
    let vec = vec![10, 20, 30, 40];
    let ptr = vec.as_slice().as_ptr();

    let mut bytes = Bytes::from(vec);
    bytes.split_off(3); // and immediately drop that bytes

    let vec = bytes.into_vec();
    assert_eq!(vec![10, 20, 30], vec);
    assert_eq!(ptr, vec.as_slice().as_ptr())
}

#[test]
fn into_vec_kind_arc_unique_move() {
    let vec = vec![10, 20, 30, 40];
    let ptr = vec.as_slice().as_ptr();

    let mut bytes = Bytes::from(vec);
    bytes.split_to(1); // and immediately drop that bytes

    let vec = bytes.into_vec();
    assert_eq!(vec![20, 30, 40], vec);
    assert_eq!(ptr, vec.as_slice().as_ptr())
}

#[test]
fn into_vec_kind_static() {
    let vec = Bytes::from_static(b"abcde").into_vec();
    assert_eq!(b"abcde", &vec[..]);
}

#[test]
fn into_vec_kind_inline() {
    let vec = Bytes::from(&b"abcde"[..]).into_vec();
    assert_eq!(b"abcde", &vec[..]);
}

#[test]
// Only run these tests on little endian systems. CI uses qemu for testing
// little endian... and qemu doesn't really support threading all that well.
#[cfg(target_endian = "little")]
fn stress() {
    // Tests promoting a buffer from a vec -> shared in a concurrent situation
    use std::sync::{Arc, Barrier};
    use std::thread;

    const THREADS: usize = 8;
    const ITERS: usize = 1_000;

    for i in 0..ITERS {
        let data = [i as u8; 256];
        let buf = Arc::new(Bytes::from(&data[..]));

        let barrier = Arc::new(Barrier::new(THREADS));
        let mut joins = Vec::with_capacity(THREADS);

        for _ in 0..THREADS {
            let c = barrier.clone();
            let buf = buf.clone();

            joins.push(thread::spawn(move || {
                c.wait();
                let buf: Bytes = (*buf).clone();
                drop(buf);
            }));
        }

        for th in joins {
            th.join().unwrap();
        }

        assert_eq!(*buf, data[..]);
    }
}
