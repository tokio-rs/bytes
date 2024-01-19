use crate::Buf;
use crate::buf::SeekBuf;

use core::iter::FusedIterator;
use core::cell::Cell;
use core::num::NonZeroUsize;
use core::ops::{Bound, RangeBounds};

/// Provides a non-mutating view of the bytes in a buffer.
///
/// [BufCursor] implements [Iterator] for `&u8` items, and provides
/// compatibility for the full suite of compatible iterator helpers for any
/// buffers that support arbitrary position reads via [SeekBuf].
///
/// A cursor closely resembles a buffer itself; in fact, it also implements
/// [Buf] and [SeekBuf] like its parent. This also makes recursive cursor
/// instantiation possible, so sub-cursors can be consumed without mutating
/// the original cursor or original buffer.
///
/// [BufCursor] aims for optimal performance even for buffers which may
/// introduce latency for retrieving chunks (such as `dyn SeekBuf` buffers). It
/// stores the most recently retrieved chunk for subsequent access up to the
/// length of that chunk, amortizing the cost of any buffer retrieval calls
/// across the size of all returned chunks.
#[derive(Debug)]
pub struct BufCursor<'b, B: SeekBuf + ?Sized> {
    buf: &'b B,
    front_chunk_offset: Cell<usize>,
    back_chunk_offset: Cell<usize>,
    front_chunk: Cell<Option<&'b [u8]>>,
    back_chunk: Cell<Option<&'b [u8]>>,
}

impl<'b, B: SeekBuf + ?Sized> BufCursor<'b, B> {
    /// Creates a new [BufCursor] starting at the beginning of the provided
    /// buffer and ending at the end of the buffer, as determined by the length
    /// provided by [Buf::remaining].
    pub fn new(buf: &'b B) -> Self {
        Self {
            buf,
            front_chunk_offset: Cell::new(0),
            back_chunk_offset: Cell::new(buf.remaining()),
            front_chunk: Cell::new(None),
            back_chunk: Cell::new(None),
        }
    }

    /// Returns the offset of the original buffer that the cursor's front is
    /// currently set to read.
    ///
    /// This offset is zero-indexed from the beginning of the buffer.
    ///
    /// # Notes
    ///
    /// This method may allow callers to understand the layout of the underlying
    /// buffer while only provided with a sub-cursor.
    #[inline]
    pub fn front_offset(&self) -> usize {
        self.front_chunk_offset.get() - self.front_chunk_len()
    }


    /// Returns the offset of the original buffer that the cursor's back is
    /// currently set to read.
    ///
    /// This offset is zero-indexed from the beginning of the buffer.
    ///
    /// # Notes
    ///
    /// This method may allow callers to understand the layout of the underlying
    /// buffer while only provided with a sub-cursor.
    #[inline]
    pub fn back_offset(&self) -> usize {
        self.back_chunk_offset.get() + self.back_chunk_len()
    }

    /// Moves the cursor to the range specified and returns a new cursor at the
    /// respective front and back offsets, consuming itself in the process.
    ///
    /// If the range provided moves the cursor out of its supported range,
    /// then [None] is returned and the cursor (`self`) is destroyed.
    ///
    /// Should the current cursor need to remain valid after the call to
    /// [Self::seek], call [Self::cursor] first to create a new sub-cursor at
    /// the current cursor position, then call [Self::seek] on the new
    /// sub-cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::SeekBuf;
    ///
    /// let buf = b"<<< TEXT >>>".as_slice();
    ///
    /// let cursor = buf.cursor().seek(4..8).unwrap();
    ///
    /// let bytes = cursor.copied().collect::<Vec<u8>>();
    ///
    /// assert_eq!(bytes.as_slice(), b"TEXT".as_slice())
    /// ```
    pub fn seek<R: RangeBounds<usize>>(self, range: R) -> Option<Self> {
        let remaining = self.remaining();

        let relative_front_offset = match range.start_bound() {
            Bound::Included(start_inclusive) => *start_inclusive,
            // Exclusive range start bounds are unimplemented in Rust 2023,
            // but my be implemented in the future. This line may be uncovered.
            Bound::Excluded(start_exclusive) => *start_exclusive + 1,
            Bound::Unbounded => 0,
        };

        let relative_back_offset = match range.end_bound() {
            Bound::Included(end_inclusive) => *end_inclusive + 1,
            Bound::Excluded(end_exclusive) => *end_exclusive,
            Bound::Unbounded => remaining,
        };

        if relative_front_offset > relative_back_offset || relative_back_offset > remaining {
            return None;
        }

        let absolute_offset = self.front_offset();

        let front_chunk_offset = absolute_offset + relative_front_offset;
        let back_chunk_offset = absolute_offset + relative_back_offset;

        Some(Self {
            buf: self.buf,
            front_chunk_offset: Cell::new(front_chunk_offset),
            back_chunk_offset: Cell::new(back_chunk_offset),
            front_chunk: Cell::new(None),
            back_chunk: Cell::new(None),
        })
    }
}

impl<'b, B: SeekBuf + ?Sized> BufCursor<'b, B> {
    #[inline]
    fn front_chunk_len(&self) -> usize {
        self.front_chunk.get().map_or(0, |chunk| chunk.len())
    }

    #[inline]
    fn back_chunk_len(&self) -> usize {
        self.back_chunk.get().map_or(0, |chunk| chunk.len())
    }

    fn next_front_chunk(&self) -> Option<&'b [u8]> {
        match self.front_chunk.get() {
            Some(chunk) if !chunk.is_empty() => Some(chunk),
            _ => {
                let chunk = self.buf.chunk_from(self.front_chunk_offset.get())?;
                self.front_chunk.set(Some(chunk));
                self.front_chunk_offset
                    .set(self.front_chunk_offset.get() + chunk.len());
                Some(chunk)
            }
        }
    }

    fn next_back_chunk(&self) -> Option<&'b [u8]> {
        match self.back_chunk.get() {
            Some(chunk) if !chunk.is_empty() => Some(chunk),
            _ => {
                let chunk = self.buf.chunk_to(self.back_chunk_offset.get())?;
                self.back_chunk.set(Some(chunk));
                self.back_chunk_offset
                    .set(self.back_chunk_offset.get() - chunk.len());
                Some(chunk)
            }
        }
    }

    #[allow(unused)]
    fn advance_front_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
        let chunk_len = if let Some(chunk) = self.front_chunk.get() {
            let chunk_len = chunk.len();

            if n < chunk_len {
                self.front_chunk.set(Some(&chunk[n..]));
                return Ok(());
            }

            chunk_len
        } else {
            0
        };

        let remaining = self.remaining();

        if n < remaining {
            self.front_chunk_offset
                .set(self.front_chunk_offset.get() + n - chunk_len);
            self.front_chunk
                .set(self.buf.chunk_from(self.front_chunk_offset.get()));
            Ok(())
        } else {
            self.front_chunk_offset
                .set(self.front_chunk_offset.get() + remaining);
            self.front_chunk.set(None);

            match NonZeroUsize::new(n - remaining) {
                None => Ok(()),
                Some(n_remaining) => Err(n_remaining),
            }
        }
    }

    #[allow(unused)]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
        let chunk_len = if let Some(chunk) = self.back_chunk.get() {
            let chunk_len = chunk.len();

            if n < chunk_len {
                self.back_chunk.set(Some(&chunk[..chunk_len - n]));
                return Ok(());
            }

            chunk_len
        } else {
            0
        };

        let remaining = self.remaining();

        if n < remaining {
            self.back_chunk_offset
                .set(self.back_chunk_offset.get() - n - chunk_len);
            self.back_chunk
                .set(self.buf.chunk_from(self.back_chunk_offset.get()));
            Ok(())
        } else {
            self.back_chunk_offset
                .set(self.back_chunk_offset.get() - remaining);
            self.back_chunk.set(None);

            match NonZeroUsize::new(n - remaining) {
                None => Ok(()),
                Some(n_remaining) => Err(n_remaining),
            }
        }
    }
}

impl<'b, B: SeekBuf + ?Sized> Buf for BufCursor<'b, B> {
    #[inline]
    fn remaining(&self) -> usize {
        self.back_offset() - self.front_offset()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        self.next_front_chunk().unwrap_or(&[])
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        self.advance_front_by(cnt).unwrap()
    }
}

impl<'b, B: SeekBuf + ?Sized> SeekBuf for BufCursor<'b, B> {
    fn chunk_from(&self, start: usize) -> Option<&[u8]> {
        let remaining = self.remaining();
        if start >= remaining {
            return None;
        }

        let chunk = self.buf.chunk_from(self.front_offset() + start)?;

        Some(&chunk[..chunk.len().min(remaining - start)])
    }

    fn chunk_to(&self, end: usize) -> Option<&[u8]> {
        let remaining = self.remaining();
        if end > remaining {
            return None;
        }

        let chunk = self.buf.chunk_to(self.back_offset() - end)?;

        Some(&chunk[(remaining - end).min(chunk.len())..])
    }
}

impl<'b, B: SeekBuf + ?Sized> Iterator for BufCursor<'b, B> {
    type Item = &'b u8;

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = self.remaining();

        if remaining == 0 {
            return None;
        }

        let chunk = self.next_front_chunk()?;

        let (next, front_chunk) = match chunk.len() {
            // Most SeekBuf implementations will not need this line, but should
            // an implementation return an empty slice chunk instead of None,
            // this match case (chunk length of zero) may be hit.
            0 => (None, None),
            1 => (Some(&chunk[0]), None),
            _ => (Some(&chunk[0]), Some(&chunk[1..])),
        };

        self.front_chunk.set(front_chunk);

        next
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining(), Some(self.remaining()))
    }

    #[cfg(feature = "iter_advance_by")]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
        self.advance_front_by(n)
    }
}

impl<'b, B: SeekBuf + ?Sized> DoubleEndedIterator for BufCursor<'b, B> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining() == 0 {
            return None;
        }

        let chunk = self.next_back_chunk()?;

        let (next, back_chunk) = match chunk.len() {
            // Most SeekBuf implementations will not need this line, but should
            // an implementation return an empty slice chunk instead of None,
            // this match case (chunk length of zero) may be hit.
            0 => (None, None),
            1 => (Some(&chunk[0]), None),
            len => (Some(&chunk[len - 1]), Some(&chunk[..len - 1])),
        };

        self.back_chunk.set(back_chunk);

        next
    }

    #[cfg(feature = "iter_advance_by")]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
        self.advance_back_by(n)
    }
}

impl<'b, B: SeekBuf + ?Sized> FusedIterator for BufCursor<'b, B> {}

impl<'b, B: SeekBuf + ?Sized> ExactSizeIterator for BufCursor<'b, B> {}
