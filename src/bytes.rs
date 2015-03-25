use {ByteBuf, SmallByteStr};
use traits::{Buf, ByteStr, ToBytes};
use std::{fmt, mem, ops, ptr};
use std::any::{Any, TypeId};
use std::raw::TraitObject;
use core::nonzero::NonZero;

const INLINE: usize = 1;

/// A specialized `ByteStr` box.
#[unsafe_no_drop_flag]
pub struct Bytes {
    vtable: NonZero<usize>,
    data: *mut (),
}

impl Bytes {
    pub fn from_slice(bytes: &[u8]) -> Bytes {
        SmallByteStr::from_slice(bytes)
            .map(|small| Bytes::of(small))
            .unwrap_or_else(|| ByteBuf::from_slice(bytes).to_bytes())
    }

    pub fn of<B: ByteStr + 'static>(bytes: B) -> Bytes {
        unsafe {
            if inline::<B>() {
                let mut vtable;
                let mut data;

                {
                    let obj: &ByteStrPriv = &bytes;
                    let obj: TraitObject = mem::transmute(obj);
                    let ptr: *const *mut () = mem::transmute(obj.data);

                    data = *ptr;
                    vtable = obj.vtable;
                }

                // Prevent drop from being called
                mem::forget(bytes);

                Bytes {
                    vtable: NonZero::new(vtable as usize | INLINE),
                    data: data,
                }
            } else {
                let obj: Box<ByteStrPriv> = Box::new(bytes);
                let obj: TraitObject = mem::transmute(obj);

                Bytes {
                    vtable: NonZero::new(obj.vtable as usize),
                    data: obj.data,
                }
            }
        }
    }

    pub fn empty() -> Bytes {
        Bytes::of(SmallByteStr::zero())
    }

    /// If the underlying `ByteStr` is of type `B`, returns a reference to it
    /// otherwise None.
    pub fn downcast_ref<'a, B: ByteStr + 'static>(&'a self) -> Option<&'a B> {
        if TypeId::of::<B>() == self.obj().get_type_id() {
            unsafe {
                if inline::<B>() {
                    return Some(mem::transmute(&self.data));
                } else {
                    return Some(mem::transmute(self.data));
                }
            }
        }

        None
    }

    /// If the underlying `ByteStr` is of type `B`, returns the unwraped value,
    /// otherwise, returns the original `Bytes` as `Err`.
    pub fn try_unwrap<B: ByteStr + 'static>(self) -> Result<B, Bytes> {
        if TypeId::of::<B>() == self.obj().get_type_id() {
            unsafe {
                // Underlying ByteStr value is of the correct type. Unwrap it
                let mut ret;

                if inline::<B>() {
                    // The value is inline, read directly from the pointer
                    ret = ptr::read(mem::transmute(&self.data));
                } else {
                    ret = ptr::read(mem::transmute(self.data));
                }

                mem::forget(self);
                Ok(ret)
            }
        } else {
            Err(self)
        }
    }

    fn obj(&self) -> &ByteStrPriv {
        unsafe {
            let obj = if self.is_inline() {
                TraitObject {
                    data: mem::transmute(&self.data),
                    vtable: mem::transmute(*self.vtable - 1),
                }
            } else {
                TraitObject {
                    data: self.data,
                    vtable: mem::transmute(*self.vtable),
                }
            };

            mem::transmute(obj)
        }
    }

    fn obj_mut(&mut self) -> &mut ByteStrPriv {
        unsafe { mem::transmute(self.obj()) }
    }

    fn is_inline(&self) -> bool {
        (*self.vtable & INLINE) == INLINE
    }
}

fn inline<B: ByteStr>() -> bool {
    mem::size_of::<B>() <= mem::size_of::<usize>()
}

impl ByteStr for Bytes {

    type Buf = Box<Buf+'static>;

    fn buf(&self) -> Box<Buf+'static> {
        self.obj().buf()
    }

    fn concat<B: ByteStr+'static>(&self, other: &B) -> Bytes {
        self.obj().concat(&Bytes::of(other.clone()))
    }

    fn len(&self) -> usize {
        self.obj().len()
    }

    fn slice(&self, begin: usize, end: usize) -> Bytes {
        self.obj().slice(begin, end)
    }

    fn split_at(&self, mid: usize) -> (Bytes, Bytes) {
        self.obj().split_at(mid)
    }
}

impl ToBytes for Bytes {
    fn to_bytes(self) -> Bytes {
        self
    }
}

impl ops::Index<usize> for Bytes {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        self.obj().index(index)
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        super::debug(self, "Bytes", fmt)
    }
}

impl Clone for Bytes {
    fn clone(&self) -> Bytes {
        self.obj().clone()
    }
}

impl Drop for Bytes {
    fn drop(&mut self) {
        if *self.vtable == 0 {
            return;
        }

        unsafe {
            if self.is_inline() {
                self.obj_mut().drop();
            } else {
                let _: Box<ByteStrPriv> =
                    mem::transmute(self.obj());
            }
        }
    }
}

unsafe impl Send for Bytes { }
unsafe impl Sync for Bytes { }

trait ByteStrPriv {

    fn buf(&self) -> Box<Buf+'static>;

    fn clone(&self) -> Bytes;

    fn concat(&self, other: &Bytes) -> Bytes;

    fn drop(&mut self);

    fn get_type_id(&self) -> TypeId;

    fn index(&self, index: usize) -> &u8;

    fn len(&self) -> usize;

    fn slice(&self, begin: usize, end: usize) -> Bytes;

    fn split_at(&self, mid: usize) -> (Bytes, Bytes);
}

impl<B: ByteStr + 'static> ByteStrPriv for B {

    fn buf(&self) -> Box<Buf+'static> {
        Box::new(self.buf())
    }

    fn clone(&self) -> Bytes {
        Bytes::of(self.clone())
    }

    fn concat(&self, other: &Bytes) -> Bytes {
        self.concat(other)
    }

    fn drop(&mut self) {
        unsafe {
            ptr::read(mem::transmute(self))
        }
    }

    fn get_type_id(&self) -> TypeId {
        Any::get_type_id(self)
    }

    fn index(&self, index: usize) -> &u8 {
        ops::Index::index(self, index)
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn slice(&self, begin: usize, end: usize) -> Bytes {
        self.slice(begin, end)
    }

    fn split_at(&self, mid: usize) -> (Bytes, Bytes) {
        self.split_at(mid)
    }
}

#[test]
pub fn test_size_of() {
    let expect = mem::size_of::<usize>() * 2;

    assert_eq!(expect, mem::size_of::<Bytes>());
    assert_eq!(expect, mem::size_of::<Option<Bytes>>());
}
