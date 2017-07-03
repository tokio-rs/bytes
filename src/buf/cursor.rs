// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// at http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cmp;
use std::io::{self, BufRead, Read};

/// A `Cursor` wraps another type and provides it with a
/// [`Seek`] implementation.
///
/// `Cursor`s are typically used with in-memory buffers to allow them to
/// implement [`Read`] and/or [`Write`], allowing these buffers to be used
/// anywhere you might use a reader or writer that does actual I/O.
///
/// The standard library implements some I/O traits on various types which
/// are commonly used as a buffer, like `Cursor<`[`Vec`]`<u8>>` and
/// `Cursor<`[`&[u8]`][bytes]`>`.
#[derive(Clone, Debug)]
pub struct Cursor<T> {
    inner: T,
    pos: u32,
}

impl<T> Cursor<T> {
    /// Creates a new cursor wrapping the provided underlying I/O object.
    ///
    /// Cursor initial position is `0` even if underlying object (e.
    /// g. `Vec`) is not empty. So writing to cursor starts with
    /// overwriting `Vec` content, not with appending to it.
    pub fn new(inner: T) -> Cursor<T> {
        Cursor { pos: 0, inner: inner }
    }

    /// Gets a reference to the underlying value in this cursor.
    pub fn get_ref(&self) -> &T { &self.inner }

    /// Gets a mutable reference to the underlying value in this cursor.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying value as it may corrupt this cursor's position.
    pub fn get_mut(&mut self) -> &mut T { &mut self.inner }

    /// Returns the current position of this cursor.
    pub fn position(&self) -> u32 { self.pos }

    /// Sets the position of this cursor.
    pub fn set_position(&mut self, pos: u32) { self.pos = pos; }
}

impl<T> Read for Cursor<T> where T: AsRef<[u8]> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = Read::read(&mut self.fill_buf()?, buf)?;
        self.pos += n as u32;
        Ok(n)
    }
}

impl<T> BufRead for Cursor<T> where T: AsRef<[u8]> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let amt = cmp::min(self.pos, self.inner.as_ref().len() as u32);
        Ok(&self.inner.as_ref()[(amt as usize)..])
    }
    fn consume(&mut self, amt: usize) { self.pos += amt as u32; }
}
