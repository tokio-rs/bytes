use crate::buf::SeekBuf;
use crate::Buf;

use core::cell::Cell;
use core::iter::FusedIterator;
use core::num::NonZeroUsize;
use core::ops::{Bound, Range, RangeBounds};

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
/// during iteration across the size of returned chunks.
#[derive(Debug)]
pub struct BufCursor<'b, B: SeekBuf + ?Sized> {
    buf: &'b B,

    // Offset from the start of the buffer to the end of the front chunk.
    // May overlap with the back chunk.
    front_chunk_offset: Cell<usize>,

    // Offset from the start of the buffer to the start of the back chunk.
    // May overlap with the front chunk.
    back_chunk_offset: Cell<usize>,

    front_chunk: Cell<Option<&'b [u8]>>,
    back_chunk: Cell<Option<&'b [u8]>>,
}

impl<'b, B: SeekBuf + ?Sized> BufCursor<'b, B> {
    /// Creates a new [BufCursor] starting at the beginning of the provided
    /// buffer and ending at the end of the buffer, as determined by the length
    /// provided by [Buf::remaining].
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::buf::BufCursor;
    ///
    /// let buf = b"Hello World!".as_slice();
    ///
    /// let mut cursor = BufCursor::new(&buf);
    ///
    /// assert_eq!(cursor.next(), Some(&b'H'));
    /// assert_eq!(cursor.next_back(), Some(&b'!'));
    /// ```
    pub fn new(buf: &'b B) -> Self {
        Self {
            buf,
            front_chunk_offset: Cell::new(0),
            back_chunk_offset: Cell::new(buf.remaining()),
            front_chunk: Cell::new(None),
            back_chunk: Cell::new(None),
        }
    }

    /// Returns the absolute cursor position within the original buffer as a
    /// range of bytes.
    ///
    /// # Notes
    ///
    /// This method may allow callers to understand the layout of the
    /// underlying buffer while only provided with a sub-cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::SeekBufExt;
    ///
    /// let buf = b"Hello World!".as_slice();
    ///
    /// let mut cursor = buf.cursor().seek(4..8).unwrap();
    ///
    /// assert_eq!(cursor.next(), Some(&b'o'));
    /// assert_eq!(cursor.next_back(), Some(&b'o'));
    ///
    /// assert_eq!(cursor.cursor_position(), 5..7);
    /// ```
    #[inline]
    pub fn cursor_position(&self) -> Range<usize> {
        self.front_offset()..self.back_offset()
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
    /// use bytes::SeekBufExt;
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

        let absolute_base_offset = self.front_offset();

        let mut front_chunk_offset = absolute_base_offset + relative_front_offset;
        let mut back_chunk_offset = absolute_base_offset + relative_back_offset;

        // Check if the existing front chunk is valid after the seek operation.
        // That is, if the new `front_chunk_offset` lies between the start of
        // the buffer and the existing `self.front_chunk_offset` bound.
        let front_chunk = match self.front_chunk.get() {
            Some(front_chunk) if front_chunk_offset < self.front_chunk_offset.get() => {
                // Reuse existing front chunk after seek operation.
                front_chunk_offset = self.front_chunk_offset.get();
                Some(&front_chunk[relative_front_offset..])
            }
            _ => None, // New front chunk buf lookup required.
        };

        // Check if the existing back chunk is valid  after the seek operation.
        // That is, if the new `back_chunk_offset` lies between the end of the
        // buffer and the existing `self.back_chunk_offset` bound.
        let back_chunk = match self.back_chunk.get() {
            Some(back_chunk) if back_chunk_offset > self.back_chunk_offset.get() => {
                // Reuse existing back chunk after seek operation.
                let back_chunk_remaining = back_chunk_offset - self.back_chunk_offset.get();
                back_chunk_offset = self.back_chunk_offset.get();
                Some(&back_chunk[..back_chunk_remaining])
            }
            _ => None, // New back chunk buf lookup required.
        };

        Some(Self {
            buf: self.buf,
            front_chunk_offset: Cell::new(front_chunk_offset),
            back_chunk_offset: Cell::new(back_chunk_offset),
            front_chunk: Cell::new(front_chunk),
            back_chunk: Cell::new(back_chunk),
        })
    }

    /// Advances the cursor forward in the buffer by a set number of bytes.
    ///
    /// `advance_by(n)` will always return `Ok(())` if the cursor successfully
    /// moves forward by `n` bytes. If the cursor front would exceed the cursor
    /// back (or the buffer end), then `Err(NonZeroUsize)` with value `k` is
    /// returned, where `k` is the number of bytes that were in excess of the
    /// limit and were not advanced.
    ///
    /// Calling `advance_by(0)` can be used to opportunistically preload the
    /// next front chunk from the buffer (if not already loaded) without moving
    /// the cursor forward.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::SeekBufExt;
    ///
    /// let buf = b"123456789".as_slice();
    ///
    /// let mut cursor = buf.cursor();
    ///
    /// cursor.advance_by(4).unwrap();
    ///
    /// assert_eq!(
    ///     cursor.copied().collect::<Vec<u8>>().as_slice(),
    ///     b"56789".as_slice(),
    /// );
    /// ```
    pub fn advance_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
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

        // Clear the current front chunk since the advance surpassed it.
        self.front_chunk.set(None);

        // Invariant: Front chunk offset is always greater than or equal to the
        // current chunk's length, since it is a sum of the lengths of all
        // preceding chunks (including the current chunk).
        debug_assert!(
            self.front_chunk_offset.get() >= chunk_len,
            "internal exception"
        );

        if n < remaining {
            self.front_chunk_offset
                .set(self.front_chunk_offset.get() + n - chunk_len);

            // Load next front chunk from buffer.
            let _ = self.next_front_chunk();

            Ok(())
        } else {
            self.front_chunk_offset
                .set(self.front_chunk_offset.get() + remaining - chunk_len);

            debug_assert!(self.remaining() == 0);

            match NonZeroUsize::new(n - remaining) {
                None => Ok(()),
                Some(n_remaining) => Err(n_remaining),
            }
        }
    }

    /// Advances the cursor backwards in the buffer by a set number of bytes.
    ///
    /// `advance_back_by(n)` will always return `Ok(())` if the cursor
    /// successfully moves backward by `n` bytes. If the cursor back would
    /// exceed the cursor front (or the buffer start), then `Err(NonZeroUsize)`
    /// with value `k` is returned, where `k` is the number of bytes that were
    /// in excess of the limit and were not advanced.
    ///
    /// Calling `advance_back_by(0)` can be used to opportunistically preload
    /// the next back chunk from the buffer (if not already loaded) without
    /// moving the cursor backward.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::SeekBufExt;
    ///
    /// let buf = b"123456789".as_slice();
    ///
    /// let mut cursor = buf.cursor();
    ///
    /// cursor.advance_back_by(4).unwrap();
    ///
    /// assert_eq!(
    ///     cursor.copied().collect::<Vec<u8>>().as_slice(),
    ///     b"12345".as_slice(),
    /// );
    /// ```
    pub fn advance_back_by(&mut self, n: usize) -> Result<(), NonZeroUsize> {
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

        // Clear the current back chunk since the advance surpassed it.
        self.back_chunk.set(None);

        // Invariant: The back chunk offset + chunk_len is always more than the
        // difference between the back offset and the front offset (remaining).
        debug_assert!(
            self.back_chunk_offset.get() + chunk_len >= remaining,
            "internal exception"
        );

        if n < remaining {
            self.back_chunk_offset
                .set(self.back_chunk_offset.get() + chunk_len - n);

            // Load next back chunk from buffer.
            let _ = self.next_back_chunk();

            Ok(())
        } else {
            self.back_chunk_offset
                .set(self.back_chunk_offset.get() + chunk_len - remaining);

            debug_assert!(self.remaining() == 0);

            match NonZeroUsize::new(n - remaining) {
                None => Ok(()),
                Some(n_remaining) => Err(n_remaining),
            }
        }
    }
}

impl<'b, B: SeekBuf + ?Sized> BufCursor<'b, B> {
    /// Returns the absolute offset from the beginning of the buffer
    /// to the cursor's current front position (inclusive).
    /// It can be thought of as `buf[front_offset..]`.
    #[inline]
    fn front_offset(&self) -> usize {
        // Invariant: The front chunk size is always less than or equal to the
        // front chunk offset, since the chunk offset is the total size of all
        // encountered front chunks.
        debug_assert!(self.front_chunk_len() <= self.front_chunk_offset.get());
        self.front_chunk_offset.get() - self.front_chunk_len()
    }

    /// Returns the absolute offset from the beginning of the buffer
    /// to the cursor's current back position (exclusive).
    /// It can be thought of as `buf[..back_offset]`.
    #[inline]
    fn back_offset(&self) -> usize {
        self.back_chunk_offset.get() + self.back_chunk_len()
    }

    /// Current length of the active front chunk. It is not guaranteed that the
    /// entire length of the chunk is valid for the current cursor, and must be
    /// compared to the current back offset as well.
    #[inline]
    fn front_chunk_len(&self) -> usize {
        self.front_chunk.get().map_or(0, |chunk| chunk.len())
    }

    /// Current length of the active back chunk. It is not guaranteed that the
    /// entire length of the chunk is valid for the current cursor, and must be
    /// compared to the current front offset as well.
    #[inline]
    fn back_chunk_len(&self) -> usize {
        self.back_chunk.get().map_or(0, |chunk| chunk.len())
    }

    /// Returns the currently active front chunk, or retrieves a new front
    /// chunk from the buffer if one is not active.
    #[inline]
    fn next_front_chunk(&self) -> Option<&'b [u8]> {
        match self.front_chunk.get() {
            // likely branch.
            Some(chunk) if !chunk.is_empty() => Some(chunk),
            _ => {
                // unlikely branch.
                self.load_next_front_chunk();
                self.front_chunk.get()
            }
        }
    }

    /// Returns the currently active back chunk, or retrieves a new back
    /// chunk from the buffer if one is not active.
    #[inline]
    fn next_back_chunk(&self) -> Option<&'b [u8]> {
        match self.back_chunk.get() {
            // likely branch.
            Some(chunk) if !chunk.is_empty() => Some(chunk),
            _ => {
                // unlikely branch.
                self.load_next_back_chunk();
                self.back_chunk.get()
            }
        }
    }

    fn load_next_front_chunk(&self) {
        let chunk = self.buf.chunk_from(self.front_chunk_offset.get());
        let chunk_len = chunk.map_or(0, |chunk| chunk.len());

        self.front_chunk_offset
            .set(self.front_chunk_offset.get() + chunk_len);

        self.front_chunk.set(chunk);
    }

    fn load_next_back_chunk(&self) {
        let chunk = self.buf.chunk_to(self.back_chunk_offset.get());
        let chunk_len = chunk.map_or(0, |chunk| chunk.len());

        // This assertion checks that the buf is implemented correctly.
        // A chunk should never exceed the back chunk offset, unless
        // the buf's `remaining` method mismatched the total number of
        // bytes available in the buffer when returned by `chunk` calls.
        assert!(
            self.back_chunk_offset.get() >= chunk_len,
            "chunk length overflow"
        );

        self.back_chunk_offset
            .set(self.back_chunk_offset.get() - chunk_len);

        self.back_chunk.set(chunk);
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
        self.advance_by(cnt).unwrap()
    }
}

impl<'b, B: SeekBuf + ?Sized> SeekBuf for BufCursor<'b, B> {
    #[inline]
    fn chunk_from(&self, start: usize) -> Option<&[u8]> {
        let start_offset = self.front_offset() + start;

        if start_offset >= self.back_offset() {
            return None;
        }

        let chunk = self.buf.chunk_from(start_offset)?;

        let included_len = chunk.len().min(self.back_offset() - start_offset);

        Some(&chunk[..included_len])
    }

    #[inline]
    fn chunk_to(&self, end: usize) -> Option<&[u8]> {
        let end_offset = self.front_offset() + end;

        if end_offset > self.back_offset() {
            return None;
        }

        let chunk = self.buf.chunk_to(end_offset)?;

        let excluded_len = chunk.len().saturating_sub(end);

        Some(&chunk[excluded_len..])
    }
}

impl<'b, B: SeekBuf + ?Sized> Iterator for BufCursor<'b, B> {
    type Item = &'b u8;

    #[inline]
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

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining(), Some(self.remaining()))
    }
}

impl<'b, B: SeekBuf + ?Sized> DoubleEndedIterator for BufCursor<'b, B> {
    #[inline]
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
}

impl<'b, B: SeekBuf + ?Sized> FusedIterator for BufCursor<'b, B> {}

impl<'b, B: SeekBuf + ?Sized> ExactSizeIterator for BufCursor<'b, B> {}
