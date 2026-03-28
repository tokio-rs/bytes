use crate::BufMut;

/// # Safety
///
/// It must be safe to call Self::CursorMut::advance_mut. This should be true
/// if the type Self::CursorMut does not allow to access bytes writen to it.
/// For exemple, Vec<u8> is not ok, as the content written to it is accesible.
/// On the other hand &mut [u8] is ok, as the content written cant't be accessed.
pub unsafe trait CursorMut: BufMut {
    type CursorMut<'a>: BufMut
    where
        Self: 'a;

    fn cursor_mut(&mut self, index: usize) -> Self::CursorMut<'_>;
}

unsafe impl CursorMut for &mut [u8] {
    type CursorMut<'a>
        = &'a mut [u8]
    where
        Self: 'a;

    fn cursor_mut(&mut self, index: usize) -> Self::CursorMut<'_> {
        &mut self[index..]
    }
}
