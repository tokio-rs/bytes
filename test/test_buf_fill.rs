use bytes::*;
use bytes::buf::*;
use std::io;

#[test]
pub fn test_readijng_buf_from_reader() {
    let mut reader = chunks(vec![b"foo", b"bar", b"baz"]);
    let mut buf = MutByteBuf::with_capacity(1024);

    assert_eq!(3, reader.read_buf(&mut buf).unwrap());
    assert_eq!(Bytes::from(&b"foo"), Bytes::from(buf.flip()));
}

fn chunks(chunks: Vec<&'static [u8]>) -> Chunked {
    Chunked { chunks: chunks }
}

struct Chunked {
    chunks: Vec<&'static [u8]>,
}

impl io::Read for Chunked {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        use std::{cmp, ptr};

        if self.chunks.is_empty() {
            return Ok(0);
        }

        let src = self.chunks[0];
        let len = cmp::min(src.len(), dst.len());

        unsafe {
            ptr::copy_nonoverlapping(
                src[..len].as_ptr(),
                dst[..len].as_mut_ptr(),
                len);
        }

        if len < src.len() {
            self.chunks[0] = &src[len..];
        } else {
            self.chunks.remove(0);
        }

        Ok(len)
    }
}
