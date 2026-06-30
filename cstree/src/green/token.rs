extern crate alloc;

use alloc::vec::Vec;
use core::{fmt, hash, mem::ManuallyDrop, ptr::NonNull};

use crate::{RawSyntaxKind, text::TextSize};
use triomphe::Arc;

#[repr(align(2))] // to use 1 bit for pointer tagging. NB: this is an at-least annotation
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub(super) struct GreenTokenData {
    pub(super) kind: RawSyntaxKind,
    pub(super) data: Vec<u8>,
}

/// Leaf node in the immutable "green" tree.
pub struct GreenToken {
    ptr: NonNull<GreenTokenData>,
}

unsafe impl Send for GreenToken {} // where GreenTokenData: Send + Sync
unsafe impl Sync for GreenToken {} // where GreenTokenData: Send + Sync

pub(super) const IS_TOKEN_TAG: usize = 0x1;
impl GreenToken {
    fn add_tag(ptr: NonNull<GreenTokenData>) -> NonNull<GreenTokenData> {
        unsafe {
            let ptr = ptr.as_ptr().map_addr(|addr| addr | IS_TOKEN_TAG);
            NonNull::new_unchecked(ptr)
        }
    }

    fn remove_tag(ptr: NonNull<GreenTokenData>) -> NonNull<GreenTokenData> {
        unsafe {
            let ptr = ptr.as_ptr().map_addr(|addr| addr & !IS_TOKEN_TAG);
            NonNull::new_unchecked(ptr)
        }
    }

    fn raw_data(&self) -> &GreenTokenData {
        unsafe { &*Self::remove_tag(self.ptr).as_ptr() }
    }

    /// Creates a new Token.
    #[inline]
    pub(super) fn new(kind: RawSyntaxKind, data: &[u8]) -> GreenToken {
        let data = GreenTokenData {
            kind,
            data: data.to_vec(),
        };
        let ptr = Arc::into_raw(Arc::new(data));
        let ptr = NonNull::new(ptr as *mut _).unwrap();
        GreenToken {
            ptr: Self::add_tag(ptr),
        }
    }

    /// [`RawSyntaxKind`] of this Token.
    #[inline]
    pub fn kind(&self) -> RawSyntaxKind {
        self.raw_data().kind
    }

    /// The original source data of this Token.
    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.raw_data().data
    }

    /// The original source text of this Token.
    #[inline]
    pub fn text(&self) -> Option<&str> {
        core::str::from_utf8(self.data()).ok()
    }

    /// Returns the length of data covered by this token, in bytes.
    #[inline]
    pub fn data_len(&self) -> TextSize {
        (self.raw_data().data.len() as u32).into()
    }

    /// Returns the length of text covered by this token, in bytes.
    #[inline]
    pub fn text_len(&self) -> TextSize {
        self.data_len()
    }
}

impl fmt::Debug for GreenToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data = self.raw_data();
        f.debug_struct("GreenToken")
            .field("kind", &data.kind)
            .field("data", &data.data)
            .finish()
    }
}

impl Clone for GreenToken {
    fn clone(&self) -> Self {
        let ptr = Self::remove_tag(self.ptr);
        let ptr = unsafe {
            let arc = ManuallyDrop::new(Arc::from_raw(ptr.as_ptr()));
            Arc::into_raw(Arc::clone(&arc))
        };
        let ptr = unsafe { NonNull::new_unchecked(ptr as *mut _) };
        GreenToken {
            ptr: Self::add_tag(ptr),
        }
    }
}

impl Eq for GreenToken {}
impl PartialEq for GreenToken {
    fn eq(&self, other: &Self) -> bool {
        self.raw_data() == other.raw_data()
    }
}

impl hash::Hash for GreenToken {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        self.raw_data().hash(state)
    }
}

impl Drop for GreenToken {
    fn drop(&mut self) {
        unsafe {
            Arc::from_raw(Self::remove_tag(self.ptr).as_ptr());
        }
    }
}
