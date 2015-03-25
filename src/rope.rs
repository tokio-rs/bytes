use {Bytes, ByteBuf, Source, BufError};
use traits::{Buf, ByteStr, MutBuf, MutBufExt, ToBytes};
use std::{cmp, mem, ops};
use std::sync::Arc;

// The implementation is mostly a port of the implementation found in the Java
// protobuf lib.

const CONCAT_BY_COPY_LEN: usize = 128;
const MAX_DEPTH: usize = 47;

// Used to decide when to rebalance the tree.
static MIN_LENGTH_BY_DEPTH: [usize; MAX_DEPTH] = [
              1,              2,              3,              5,              8,
             13,             21,             34,             55,             89,
            144,            233,            377,            610,            987,
          1_597,          2_584,          4_181,          6_765,         10_946,
         17_711,         28_657,         46_368,         75_025,        121_393,
        196_418,        317_811,        514_229,        832_040,      1_346_269,
      2_178_309,      3_524_578,      5_702_887,      9_227_465,     14_930_352,
     24_157_817,     39_088_169,     63_245_986,    102_334_155,    165_580_141,
    267_914_296,    433_494_437,    701_408_733,  1_134_903_170,  1_836_311_903,
  2_971_215_073,  4_294_967_295];

/// An immutable sequence of bytes formed by concatenation of other `ByteStr`
/// values, without copying the data in the pieces. The concatenation is
/// represented as a tree whose leaf nodes are each a `Bytes` value.
///
/// Most of the operation here is inspired by the now-famous paper [Ropes: an
/// Alternative to Strings. hans-j. boehm, russ atkinson and michael
/// plass](http://www.cs.rit.edu/usr/local/pub/jeh/courses/QUARTERS/FP/Labs/CedarRope/rope-paper.pdf).
///
/// Fundamentally the Rope algorithm represents the collection of pieces as a
/// binary tree. BAP95 uses a Fibonacci bound relating depth to a minimum
/// sequence length, sequences that are too short relative to their depth cause
/// a tree rebalance.  More precisely, a tree of depth d is "balanced" in the
/// terminology of BAP95 if its length is at least F(d+2), where F(n) is the
/// n-the Fibonacci number. Thus for depths 0, 1, 2, 3, 4, 5,... we have
/// minimum lengths 1, 2, 3, 5, 8, 13,...
pub struct Rope {
    inner: Arc<RopeInner>,
}

impl Rope {
    pub fn from_slice(bytes: &[u8]) -> Rope {
        Rope::new(Bytes::from_slice(bytes), Bytes::empty())
    }

    /// Returns a Rope consisting of the supplied Bytes as a single segment.
    pub fn of<B: ByteStr + 'static>(bytes: B) -> Rope {
        let bytes = Bytes::of(bytes);

        match bytes.try_unwrap() {
            Ok(rope) => rope,
            Err(bytes) => Rope::new(bytes, Bytes::empty()),
        }
    }

    fn new(left: Bytes, right: Bytes) -> Rope {
        Rope { inner: Arc::new(RopeInner::new(left, right)) }
    }

    pub fn len(&self) -> usize {
        self.inner.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /*
     *
     * ===== Priv fns =====
     *
     */

    fn depth(&self) -> u16 {
        self.inner.depth
    }

    fn left(&self) -> &Bytes {
        &self.inner.left
    }

    fn right(&self) -> &Bytes {
        &self.inner.right
    }

    fn pieces<'a>(&'a self) -> PieceIter<'a> {
        PieceIter::new(&self.inner)
    }
}

impl ByteStr for Rope {
    type Buf = RopeBuf;

    fn buf(&self) -> RopeBuf {
        RopeBuf::new(self.clone())
    }

    fn concat<B: ByteStr+'static>(&self, other: &B) -> Bytes {
        let left = Bytes::of(self.clone());
        let right = Bytes::of(other.clone());
        Bytes::of(concat(left, right))
    }

    fn len(&self) -> usize {
        Rope::len(self)
    }

    fn slice(&self, begin: usize, end: usize) -> Bytes {
        if begin >= end || begin >= self.len() {
            return Bytes::empty()
        }

        let end = cmp::min(end, self.len());
        let len = end - begin;

        // Empty slice
        if len == 0 {
            return Bytes::empty();
        }

        // Full rope
        if len == self.len() {
            return Bytes::of(self.clone());
        }

        // == Proper substring ==

        let left_len = self.inner.left.len();

        if end <= left_len {
            // Slice on the left
            return self.inner.left.slice(begin, end);
        }

        if begin >= left_len {
            // Slice on the right
            return self.inner.right.slice(begin - left_len, end - left_len);
        }

        // Split slice
        let left_slice = self.inner.left.slice_from(begin);
        let right_slice = self.inner.right.slice_to(end - left_len);

        Bytes::of(Rope::new(left_slice, right_slice))
    }
}

impl ToBytes for Rope {
    fn to_bytes(self) -> Bytes {
        Bytes::of(self)
    }
}

impl ops::Index<usize> for Rope {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        assert!(index < self.len());

        let left_len = self.inner.left.len();

        if index < left_len {
            self.inner.left.index(index)
        } else {
            self.inner.right.index(index - left_len)
        }
    }
}

impl Clone for Rope {
    fn clone(&self) -> Rope {
        Rope { inner: self.inner.clone() }
    }
}

impl<'a> Source for &'a Rope {
    type Error = BufError;

    fn fill<B: MutBuf>(self, _buf: &mut B) -> Result<usize, BufError> {
        unimplemented!();
    }
}

/*
 *
 * ===== Helper Fns =====
 *
 */

fn depth(bytes: &Bytes) -> u16 {
    match bytes.downcast_ref::<Rope>() {
        Some(rope) => rope.inner.depth,
        None => 0,
    }
}

fn is_balanced(bytes: &Bytes) -> bool {
    if let Some(rope) = bytes.downcast_ref::<Rope>() {
        return rope.len() >= MIN_LENGTH_BY_DEPTH[rope.depth() as usize];
    }

    true
}

fn concat(left: Bytes, right: Bytes) -> Rope {
    if right.is_empty() {
        return Rope::of(left);
    }

    if left.is_empty() {
        return Rope::of(right);
    }

    let len = left.len() + right.len();

    if len < CONCAT_BY_COPY_LEN {
        return concat_bytes(&left, &right, len);
    }

    if let Some(left) = left.downcast_ref::<Rope>() {
        let len = left.inner.right.len() + right.len();

        if len < CONCAT_BY_COPY_LEN {
            // Optimization from BAP95: As an optimization of the case
            // where the ByteString is constructed by repeated concatenate,
            // recognize the case where a short string is concatenated to a
            // left-hand node whose right-hand branch is short.  In the
            // paper this applies to leaves, but we just look at the length
            // here. This has the advantage of shedding references to
            // unneeded data when substrings have been taken.
            //
            // When we recognize this case, we do a copy of the data and
            // create a new parent node so that the depth of the result is
            // the same as the given left tree.
            let new_right = concat_bytes(&left.inner.right, &right, len);
            return Rope::new(left.inner.left.clone(), Bytes::of(new_right));
        }

        if depth(left.left()) > depth(left.right()) && left.depth() > depth(&right) {
            // Typically for concatenate-built strings the left-side is
            // deeper than the right.  This is our final attempt to
            // concatenate without increasing the tree depth.  We'll redo
            // the the node on the RHS.  This is yet another optimization
            // for building the string by repeatedly concatenating on the
            // right.
            let new_right = Rope::new(left.right().clone(), right);
            return Rope::new(left.left().clone(), Bytes::of(new_right));
        }
    }

    // Fine, we'll add a node and increase the tree depth -- unless we
    // rebalance ;^)
    let depth = cmp::max(depth(&left), depth(&right)) + 1;

    if len >= MIN_LENGTH_BY_DEPTH[depth as usize] {
        // No need to rebalance
        return Rope::new(left, right);
    }

    Balance::new().balance(left, right)
}

fn concat_bytes(left: &Bytes, right: &Bytes, len: usize) -> Rope {
    let mut buf = ByteBuf::mut_with_capacity(len);

    buf.write(left).ok().expect("unexpected error");
    buf.write(right).ok().expect("unexpected error");

    return Rope::of(buf.flip().to_bytes());
}

fn depth_for_len(len: usize) -> u16 {
    match MIN_LENGTH_BY_DEPTH.binary_search(&len) {
        Ok(idx) => idx as u16,
        Err(idx) => {
            // It wasn't an exact match, so convert to the index of the
            // containing fragment, which is one less even than the insertion
            // point.
            idx as u16 - 1
        }
    }
}

/*
 *
 * ===== RopeBuf =====
 *
 */

pub struct RopeBuf {
    rem: usize,

    // Only here for the ref count
    #[allow(dead_code)]
    rope: Rope,

    // This must be done with unsafe code to avoid having a lifetime bound on
    // RopeBuf but is safe due to Rope being held. As long as data doesn't
    // escape (which it shouldn't) it is safe. Doing this properly would
    // require HKT.
    pieces: PieceIter<'static>,
    leaf_buf: Option<Box<Buf+'static>>,
}

impl RopeBuf {
    fn new(rope: Rope) -> RopeBuf {
        // In order to get the lifetimes to work out, transmute to a 'static
        // lifetime. Never allow the iter to escape the internals of RopeBuf.
        let mut pieces: PieceIter<'static> =
            unsafe { mem::transmute(rope.pieces()) };

        // Get the next buf
        let leaf_buf = pieces.next()
            .map(|bytes| bytes.buf());

        let len = rope.len();

        RopeBuf {
            rope: rope,
            rem: len,
            pieces: pieces,
            leaf_buf: leaf_buf,
        }
    }
}

impl Buf for RopeBuf {
    fn remaining(&self) -> usize {
        self.rem
    }

    fn bytes(&self) -> &[u8] {
        self.leaf_buf.as_ref()
            .map(|b| b.bytes())
            .unwrap_or(&[])
    }

    fn advance(&mut self, mut cnt: usize) {
        cnt = cmp::min(cnt, self.rem);

        // Advance the internal cursor
        self.rem -= cnt;

        // Advance the leaf buffer
        while cnt > 0 {
            {
                let curr = self.leaf_buf.as_mut()
                    .expect("expected a value");

                if curr.remaining() > cnt {
                    curr.advance(cnt);
                    break;
                }

                cnt -= curr.remaining();
            }

            self.leaf_buf = self.pieces.next()
                .map(|bytes| bytes.buf());
        }
    }
}

/*
 *
 * ===== PieceIter =====
 *
 */

// TODO: store stack inline if possible
struct PieceIter<'a> {
    stack: Vec<&'a RopeInner>,
    next: Option<&'a Bytes>,
}

impl<'a> PieceIter<'a> {
    fn new(root: &'a RopeInner) -> PieceIter<'a> {
        let mut iter = PieceIter {
            stack: vec![],
            next: None,
        };

        iter.next = iter.get_leaf_by_left(root);
        iter
    }

    fn get_leaf_by_left(&mut self, mut root: &'a RopeInner) -> Option<&'a Bytes> {
        loop {
            self.stack.push(root);
            let left = &root.left;

            if left.is_empty() {
                return None;
            }

            if let Some(rope) = left.downcast_ref::<Rope>() {
                root = &*rope.inner;
                continue;
            }

            return Some(left);
        }
    }

    fn next_non_empty_leaf(&mut self) -> Option<&'a Bytes>{
        loop {
            if let Some(node) = self.stack.pop() {
                if let Some(rope) = node.right.downcast_ref::<Rope>() {
                    let res = self.get_leaf_by_left(&rope.inner);

                    if res.is_none() {
                        continue;
                    }

                    return res;
                }

                if node.right.is_empty() {
                    continue;
                }

                return Some(&node.right);
            }

            return None;
        }
    }
}

impl<'a> Iterator for PieceIter<'a> {
    type Item = &'a Bytes;

    fn next(&mut self) -> Option<&'a Bytes> {
        let ret = self.next.take();

        if ret.is_some() {
            self.next = self.next_non_empty_leaf();
        }

        ret
    }
}

/*
 *
 * ===== Balance =====
 *
 */

struct Balance {
    stack: Vec<Bytes>,
}

impl Balance {
    fn new() -> Balance {
        Balance { stack: vec![] }
    }

    fn balance(&mut self, left: Bytes, right: Bytes) -> Rope {
        self.do_balance(left);
        self.do_balance(right);

        let mut partial = self.stack.pop()
            .expect("expected a value");

        while !partial.is_empty() {
            let new_left = self.stack.pop()
                .expect("expected a value");

            partial = Bytes::of(Rope::new(new_left, partial));
        }

        Rope::of(partial)
    }

    fn do_balance(&mut self, root: Bytes) {
      // BAP95: Insert balanced subtrees whole. This means the result might not
      // be balanced, leading to repeated rebalancings on concatenate. However,
      // these rebalancings are shallow due to ignoring balanced subtrees, and
      // relatively few calls to insert() result.
      if is_balanced(&root) {
          self.insert(root);
      } else {
          let rope = root.try_unwrap::<Rope>()
              .ok().expect("expected a value");

          self.do_balance(rope.left().clone());
          self.do_balance(rope.right().clone());
      }
    }

    // Push a string on the balance stack (BAP95).  BAP95 uses an array and
    // calls the elements in the array 'bins'.  We instead use a stack, so the
    // 'bins' of lengths are represented by differences between the elements of
    // minLengthByDepth.
    //
    // If the length bin for our string, and all shorter length bins, are
    // empty, we just push it on the stack.  Otherwise, we need to start
    // concatenating, putting the given string in the "middle" and continuing
    // until we land in an empty length bin that matches the length of our
    // concatenation.
    fn insert(&mut self, bytes: Bytes) {
        let depth_bin = depth_for_len(bytes.len());
        let bin_end = MIN_LENGTH_BY_DEPTH[depth_bin as usize + 1];

        // BAP95: Concatenate all trees occupying bins representing the length
        // of our new piece or of shorter pieces, to the extent that is
        // possible.  The goal is to clear the bin which our piece belongs in,
        // but that may not be entirely possible if there aren't enough longer
        // bins occupied.
        if let Some(len) = self.peek().map(|r| r.len()) {
            if len >= bin_end {
                self.stack.push(bytes);
                return;
            }
        }

        let bin_start = MIN_LENGTH_BY_DEPTH[depth_bin as usize];

        // Concatenate the subtrees of shorter length
        let mut new_tree = self.stack.pop()
            .expect("expected a value");

        while let Some(len) = self.peek().map(|r| r.len()) {
            // If the head is big enough, break the loop
            if len >= bin_start { break; }

            let left = self.stack.pop()
                .expect("expected a value");

            new_tree = Bytes::of(Rope::new(left, new_tree));
        }

        // Concatenate the given string
        new_tree = Bytes::of(Rope::new(new_tree, bytes));

        // Continue concatenating until we land in an empty bin
        while let Some(len) = self.peek().map(|r| r.len()) {
            let depth_bin = depth_for_len(new_tree.len());
            let bin_end = MIN_LENGTH_BY_DEPTH[depth_bin as usize + 1];

            if len < bin_end {
                let left = self.stack.pop()
                    .expect("expected a value");

                new_tree = Bytes::of(Rope::new(left, new_tree));
            } else {
                break;
            }
        }

        self.stack.push(new_tree);
    }

    fn peek(&self) -> Option<&Bytes> {
        self.stack.last()
    }
}

struct RopeInner {
    left: Bytes,
    right: Bytes,
    depth: u16,
    len: u32,
}

impl RopeInner {
    fn new(left: Bytes, right: Bytes) -> RopeInner {
        // If left is 0 then right must be zero
        debug_assert!(!left.is_empty() || right.is_empty());

        let len = left.len() + right.len();
        let depth = cmp::max(depth(&left), depth(&right)) + 1;

        RopeInner {
            left: left,
            right: right,
            depth: depth,
            len: len as u32,
        }
    }
}
