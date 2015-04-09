use rand::random;

extern crate bytes;
extern crate rand;

mod test_buf;
mod test_buf_fill;
mod test_byte_buf;
mod test_bytes;
mod test_ring;
mod test_rope;
mod test_seq_byte_str;
mod test_small_byte_str;

fn gen_bytes(n: usize) -> Vec<u8> {
    (0..n).map(|_| random()).collect()
}
