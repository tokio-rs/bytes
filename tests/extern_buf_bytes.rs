#![warn(rust_2018_idioms)]

use bytes::{BufferParts, Bytes, BytesMut, SharedBuf};

use std::alloc::{alloc, dealloc, Layout};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::usize;

struct ExternBuf {
    ptr: NonNull<u8>,
    cap: usize,
    ref_count: AtomicUsize,
}

impl ExternBuf {
    // We're pretending that this is some sort of exotic allocation/recycling scheme
    pub fn from_size(sz: usize) -> Self {
        let layout = Layout::from_size_align(sz, 4).unwrap();
        let ptr = NonNull::new(unsafe { alloc(layout) }).unwrap();
        let num = ptr.as_ptr() as usize;
        println!("Alloc'd {}", num);
        ExternBuf {
            ptr,
            cap: sz,
            ref_count: AtomicUsize::new(1),
        }
    }

    pub fn into_shared(self) -> ExternBufWrapper {
        let b = Box::new(self);
        let inner = Box::into_raw(b);
        ExternBufWrapper { inner }
    }
}

impl From<&[u8]> for ExternBuf {
    fn from(buf: &[u8]) -> Self {
        let sz = buf.len();
        let newbuf = ExternBuf::from_size(sz);
        unsafe { ptr::copy_nonoverlapping(buf.as_ptr(), newbuf.ptr.as_ptr(), sz) };
        newbuf
    }
}

impl Drop for ExternBuf {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.cap, 4).unwrap();
        unsafe {
            let num = self.ptr.as_ptr() as usize;
            println!("dealloc'ing {}", num);
            dealloc(self.ptr.as_mut(), layout);
        }
    }
}

struct ExternBufWrapper {
    inner: *mut ExternBuf,
}

unsafe impl SharedBuf for ExternBufWrapper {
    fn into_parts(this: Self) -> BufferParts {
        unsafe {
            (
                AtomicPtr::new(this.inner.cast()),
                (*this.inner).ptr.as_ptr(),
                (*this.inner).cap,
            )
        }
    }

    unsafe fn from_parts(data: &mut AtomicPtr<()>, _ptr: *const u8, _len: usize) -> Self {
        let inner = data.load(Ordering::Acquire).cast();
        ExternBufWrapper { inner }
    }

    unsafe fn clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BufferParts {
        let inner: *mut ExternBuf = data.load(Ordering::Acquire).cast();
        let old_size = (*inner).ref_count.fetch_add(1, Ordering::Release);
        if old_size > usize::MAX >> 1 {
            panic!("wat");
        }
        (AtomicPtr::new(inner.cast()), ptr, len)
    }

    unsafe fn into_vec(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
        let inner: *mut ExternBuf = (*data.get_mut()).cast();
        if (*inner)
            .ref_count
            .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            let buf = (*inner).ptr;
            let cap = (*inner).cap;

            drop(Box::from_raw(
                inner as *mut std::mem::ManuallyDrop<ExternBuf>,
            ));

            // Copy back buffer
            ptr::copy(ptr, buf.as_ptr(), len);

            Vec::from_raw_parts(buf.as_ptr(), len, cap)
        } else {
            let v = std::slice::from_raw_parts(ptr, len).to_vec();
            Self::drop(data, ptr, len);
            v
        }
    }

    unsafe fn drop(data: &mut AtomicPtr<()>, _ptr: *const u8, _len: usize) {
        let inner: *mut ExternBuf = (*data.get_mut()).cast();
        if (*inner).ref_count.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }
        (*inner).ref_count.load(Ordering::Acquire);
        println!(
            "invoking drop over box::from_raw on {}",
            (*inner).ptr.as_ptr() as usize
        );
        drop(Box::from_raw(inner));
    }
}

fn is_sync<T: Sync>() {}
fn is_send<T: Send>() {}

#[ignore]
#[test]
fn test_bounds() {
    is_sync::<Bytes>();
    is_sync::<BytesMut>();
    is_send::<Bytes>();
    is_send::<BytesMut>();
}

#[ignore]
#[test]
fn test_layout() {
    use std::mem;

    assert_eq!(
        mem::size_of::<Bytes>(),
        mem::size_of::<usize>() * 4,
        "Bytes size should be 4 words",
    );
    assert_eq!(
        mem::size_of::<BytesMut>(),
        mem::size_of::<usize>() * 4,
        "BytesMut should be 4 words",
    );

    assert_eq!(
        mem::size_of::<Bytes>(),
        mem::size_of::<Option<Bytes>>(),
        "Bytes should be same size as Option<Bytes>",
    );

    assert_eq!(
        mem::size_of::<BytesMut>(),
        mem::size_of::<Option<BytesMut>>(),
        "BytesMut should be same size as Option<BytesMut>",
    );
}

#[ignore]
#[test]
fn roundtrip() {
    let eb = ExternBuf::from(&b"abcdefgh"[..]);
    let a = Bytes::from_shared_buf(eb.into_shared());
    let ebw = a.into_shared_buf::<ExternBufWrapper>().unwrap();
    let a = Bytes::from_shared_buf(ebw);
    let ebw2 = a.into_shared_buf::<ExternBufWrapper>().unwrap();
    let a2 = Bytes::from_shared_buf(ebw2);
    assert_eq!(a2, b"abcdefgh"[..]);
}

#[test]
fn from_slice() {
    let eb1 = ExternBuf::from(&b"abcdefgh"[..]);
    let a1 = Bytes::from_shared_buf(eb1.into_shared());
    assert_eq!(a1, b"abcdefgh"[..]);
    assert_eq!(a1, &b"abcdefgh"[..]);
    assert_eq!(a1, Vec::from(&b"abcdefgh"[..]));
    assert_eq!(b"abcdefgh"[..], a1);
    assert_eq!(&b"abcdefgh"[..], a1);
    assert_eq!(Vec::from(&b"abcdefgh"[..]), a1);

    let eb2 = ExternBuf::from(&b"abcdefgh"[..]);
    let a2 = Bytes::from_shared_buf(eb2.into_shared());
    assert_eq!(a2, b"abcdefgh"[..]);
    assert_eq!(a2, &b"abcdefgh"[..]);
    assert_eq!(a2, Vec::from(&b"abcdefgh"[..]));
    assert_eq!(b"abcdefgh"[..], a2);
    assert_eq!(&b"abcdefgh"[..], a2);
    assert_eq!(Vec::from(&b"abcdefgh"[..]), a2);
}

#[ignore]
#[test]
fn len() {
    let eb = ExternBuf::from(&b"abcdefg"[..]);
    let a = Bytes::from_shared_buf(eb.into_shared());
    assert_eq!(a.len(), 7);

    let eb = ExternBuf::from(&b""[..]);
    let a = Bytes::from_shared_buf(eb.into_shared());
    assert!(a.is_empty());
}

#[ignore]
#[test]
fn index() {
    let eb = ExternBuf::from(&b"hello world"[..]);
    let a = Bytes::from_shared_buf(eb.into_shared());
    assert_eq!(a[0..5], *b"hello");
}

#[ignore]
#[test]
fn slice() {
    let eb = ExternBuf::from(&b"hello world"[..]);
    let a = Bytes::from_shared_buf(eb.into_shared());

    let b = a.slice(3..5);
    assert_eq!(b, b"lo"[..]);

    let b = a.slice(0..0);
    assert_eq!(b, b""[..]);

    let b = a.slice(3..3);
    assert_eq!(b, b""[..]);

    let b = a.slice(a.len()..a.len());
    assert_eq!(b, b""[..]);

    let b = a.slice(..5);
    assert_eq!(b, b"hello"[..]);

    let b = a.slice(3..);
    assert_eq!(b, b"lo world"[..]);
}

#[ignore]
#[test]
#[should_panic]
fn slice_oob_1() {
    let eb = ExternBuf::from(&b"hello world"[..]);
    let a = Bytes::from_shared_buf(eb.into_shared());
    a.slice(5..44);
}

#[ignore]
#[test]
#[should_panic]
fn slice_oob_2() {
    let eb = ExternBuf::from(&b"hello world"[..]);
    let a = Bytes::from_shared_buf(eb.into_shared());
    a.slice(44..49);
}

#[ignore]
#[test]
fn split_off() {
    let eb = ExternBuf::from(&b"helloworld"[..]);
    let mut hello = Bytes::from_shared_buf(eb.into_shared());
    let world = hello.split_off(5);

    assert_eq!(hello, &b"hello"[..]);
    assert_eq!(world, &b"world"[..]);
}

#[ignore]
#[test]
#[should_panic]
fn split_off_oob() {
    let eb = ExternBuf::from(&b"helloworld"[..]);
    let mut hello = Bytes::from_shared_buf(eb.into_shared());
    let _ = hello.split_off(44);
}

#[ignore]
#[test]
fn split_off_to_loop() {
    let s = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

    for i in 0..(s.len() + 1) {
        {
            let eb = ExternBuf::from(&s[..]);
            let mut bytes = Bytes::from_shared_buf(eb.into_shared());
            let off = bytes.split_off(i);
            assert_eq!(i, bytes.len());
            let mut sum = Vec::new();
            sum.extend(bytes.iter());
            sum.extend(off.iter());
            assert_eq!(&s[..], &sum[..]);
        }
        {
            let eb = ExternBuf::from(&s[..]);
            let mut bytes = Bytes::from_shared_buf(eb.into_shared());
            let off = bytes.split_to(i);
            assert_eq!(i, off.len());
            let mut sum = Vec::new();
            sum.extend(off.iter());
            sum.extend(bytes.iter());
            assert_eq!(&s[..], &sum[..]);
        }
    }
}

#[ignore]
#[test]
fn truncate() {
    let s = &b"helloworld"[..];
    let eb = ExternBuf::from(&s[..]);
    let mut hello = Bytes::from_shared_buf(eb.into_shared());
    hello.truncate(15);
    assert_eq!(hello, s);
    hello.truncate(10);
    assert_eq!(hello, s);
    hello.truncate(5);
    assert_eq!(hello, "hello");
}

#[ignore]
#[test]
// Only run these tests on little endian systems. CI uses qemu for testing
// big endian... and qemu doesn't really support threading all that well.
#[cfg(any(miri, target_endian = "little"))]
fn stress() {
    // Tests promoting a buffer from a vec -> shared in a concurrent situation
    use std::sync::{Arc, Barrier};
    use std::thread;

    const THREADS: usize = 8;
    const ITERS: usize = if cfg!(miri) { 100 } else { 1_000 };

    for i in 0..ITERS {
        let data = [i as u8; 256];
        let eb = ExternBuf::from(&data[..]);
        let buf = Arc::new(Bytes::from_shared_buf(eb.into_shared()));

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
