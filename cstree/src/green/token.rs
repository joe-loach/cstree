use core::{fmt, hash, mem::ManuallyDrop, ptr::NonNull};

use crate::{
    RawSyntaxKind,
    interning::{Resolver, TokenData, TokenKey},
    text::TextSize,
};
use triomphe::Arc;

#[repr(align(2))] // to use 1 bit for pointer tagging. NB: this is an at-least annotation
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub(super) struct GreenTokenData {
    pub(super) kind: RawSyntaxKind,
    pub(super) data: Option<TokenKey>,
    pub(super) data_len: TextSize,
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
    pub(super) fn new(data: GreenTokenData) -> GreenToken {
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
    pub fn data<'i, I, Data>(&self, resolver: &'i I) -> Option<&'i Data>
    where
        I: Resolver<TokenKey, Data> + ?Sized,
        Data: TokenData + ?Sized,
    {
        self.raw_data().data.map(|key| resolver.resolve(key))
    }

    /// The original source text of this Token.
    #[inline]
    pub fn text<'i, I>(&self, resolver: &'i I) -> Option<&'i str>
    where
        I: Resolver<TokenKey, str> + ?Sized,
    {
        self.raw_data().data.map(|key| resolver.resolve(key))
    }

    /// Returns the length of data covered by this token, in bytes.
    #[inline]
    pub fn data_len(&self) -> TextSize {
        self.raw_data().data_len
    }

    /// Returns the length of text covered by this token, in bytes.
    #[inline]
    pub fn text_len(&self) -> TextSize {
        self.data_len()
    }

    /// Returns the interned key of data covered by this token.
    /// This key may be used for comparisons with other keys interned by the same interner.
    ///
    /// See also [`data`](GreenToken::data).
    #[inline]
    pub fn data_key(&self) -> Option<TokenKey> {
        self.raw_data().data
    }

    /// Returns the interned key of text covered by this token.
    #[inline]
    pub fn text_key(&self) -> Option<TokenKey> {
        self.data_key()
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
