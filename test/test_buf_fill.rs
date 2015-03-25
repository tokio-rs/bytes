use bytes::*;
use std::io;

#[test]
pub fn test_filling_buf_from_reader() {
    let mut reader = chunks(vec![b"foo", b"bar", b"baz"]);
    let mut buf = ByteBuf::mut_with_capacity(1024);

    assert_eq!(9, buf.write(&mut reader).unwrap());
    assert_eq!(b"foobarbaz".to_bytes(), buf.flip().to_bytes());
}

fn chunks(chunks: Vec<&'static [u8]>) -> Chunked {
    Chunked { chunks: chunks }
}

struct Chunked {
    chunks: Vec<&'static [u8]>,
}

impl io::Read for Chunked {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        use std::cmp;
        use std::slice::bytes;

        if self.chunks.is_empty() {
            return Ok(0);
        }

        let src = self.chunks[0];
        let len = cmp::min(src.len(), dst.len());

        bytes::copy_memory(&mut dst[..len], &src[..len]);

        if len < src.len() {
            self.chunks[0] = &src[len..];
        } else {
            self.chunks.remove(0);
        }

        Ok(len)
    }
}
