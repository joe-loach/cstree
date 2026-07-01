extern crate alloc;

use core::{ffi::c_void, fmt, hash, mem::ManuallyDrop, ptr::NonNull, slice};

use crate::{RawSyntaxKind, text::TextSize};
use triomphe::ThinArc;

#[repr(align(2))] // to use 1 bit for pointer tagging. NB: this is an at-least annotation
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub(super) struct GreenTokenHead {
    pub(super) kind: RawSyntaxKind,
}

/// Leaf node in the immutable "green" tree.
pub struct GreenToken {
    ptr: NonNull<c_void>,
}

unsafe impl Send for GreenToken {} // where GreenTokenHead: Send + Sync
unsafe impl Sync for GreenToken {} // where GreenTokenHead: Send + Sync

pub(super) const IS_TOKEN_TAG: usize = 0x1;
impl GreenToken {
    fn add_tag(ptr: NonNull<c_void>) -> NonNull<c_void> {
        unsafe {
            let ptr = ptr.as_ptr().map_addr(|addr| addr | IS_TOKEN_TAG);
            NonNull::new_unchecked(ptr)
        }
    }

    fn remove_tag(ptr: NonNull<c_void>) -> NonNull<c_void> {
        unsafe {
            let ptr = ptr.as_ptr().map_addr(|addr| addr & !IS_TOKEN_TAG);
            NonNull::new_unchecked(ptr)
        }
    }

    fn raw_arc(&self) -> ManuallyDrop<ThinArc<GreenTokenHead, u8>> {
        unsafe { ManuallyDrop::new(ThinArc::from_raw(Self::remove_tag(self.ptr).as_ptr())) }
    }

    fn data_parts(&self) -> (*const u8, usize) {
        let arc = self.raw_arc();
        ThinArc::with_arc(&arc, |arc| (arc.slice.as_ptr(), arc.slice.len()))
    }

    fn raw_kind(&self) -> RawSyntaxKind {
        let arc = self.raw_arc();
        ThinArc::with_arc(&arc, |arc| arc.header.header.kind)
    }

    /// Creates a new Token.
    #[inline]
    pub(super) fn new(kind: RawSyntaxKind, data: &[u8]) -> GreenToken {
        let arc = ThinArc::from_header_and_slice(GreenTokenHead { kind }, data);
        let ptr = NonNull::new(ThinArc::into_raw(arc) as *mut c_void).unwrap();
        GreenToken {
            ptr: Self::add_tag(ptr),
        }
    }

    /// [`RawSyntaxKind`] of this Token.
    #[inline]
    pub fn kind(&self) -> RawSyntaxKind {
        self.raw_kind()
    }

    /// The original source data of this Token.
    #[inline]
    pub fn data(&self) -> &[u8] {
        let (ptr, len) = self.data_parts();
        unsafe { slice::from_raw_parts(ptr, len) }
    }

    /// The original source text of this Token.
    #[inline]
    pub fn text(&self) -> Option<&str> {
        core::str::from_utf8(self.data()).ok()
    }

    /// Returns the length of data covered by this token, in bytes.
    #[inline]
    pub fn data_len(&self) -> TextSize {
        (self.data_parts().1 as u32).into()
    }

    /// Returns the length of text covered by this token, in bytes.
    #[inline]
    pub fn text_len(&self) -> TextSize {
        self.data_len()
    }
}

impl fmt::Debug for GreenToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GreenToken")
            .field("kind", &self.kind())
            .field("data", &self.data())
            .finish()
    }
}

impl Clone for GreenToken {
    fn clone(&self) -> Self {
        let arc = self.raw_arc();
        let ptr = ThinArc::into_raw(ThinArc::clone(&arc));
        let ptr = unsafe { NonNull::new_unchecked(ptr as *mut c_void) };
        GreenToken {
            ptr: Self::add_tag(ptr),
        }
    }
}

impl Eq for GreenToken {}
impl PartialEq for GreenToken {
    fn eq(&self, other: &Self) -> bool {
        self.kind() == other.kind() && self.data() == other.data()
    }
}

impl hash::Hash for GreenToken {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        self.kind().hash(state);
        self.data().hash(state);
    }
}

impl Drop for GreenToken {
    fn drop(&mut self) {
        unsafe {
            ThinArc::<GreenTokenHead, u8>::from_raw(Self::remove_tag(self.ptr).as_ptr());
        }
    }
}
