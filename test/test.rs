use rand::random;

extern crate bytes;
extern crate rand;
extern crate byteorder;

// == Buf
mod test_append;
mod test_block;
mod test_buf;
mod test_buf_fill;
mod test_byte_buf;
mod test_mut_buf;
mod test_ring;

// == Bytes
mod test_bytes;
mod test_rope;
mod test_seq;
mod test_small;

// == Pool
// mod test_pool;

fn gen_bytes(n: usize) -> Vec<u8> {
    (0..n).map(|_| random()).collect()
}
