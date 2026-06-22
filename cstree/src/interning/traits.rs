extern crate alloc;

use alloc::borrow::ToOwned;
use core::fmt;

use super::TokenKey;

/// Common interface for all intern keys via conversion to and from `u32`.
///
/// # Safety
/// Implementations must guarantee that keys can round-trip in both directions: going from `Self` to `u32` to `Self` and
/// going from `u32` to `Self` to `u32` must each yield the original value.
pub unsafe trait InternKey: Copy + Eq + fmt::Debug {
    /// Convert `self` into its raw representation.
    fn into_u32(self) -> u32;

    /// Try to reconstruct an intern key from its raw representation.
    /// Returns `None` if `key` is not a valid key.
    fn try_from_u32(key: u32) -> Option<Self>;
}

/// The read-only part of an interner.
/// Allows to perform lookups of intern keys to resolve them to their interned text.
/// Payload type that can be stored in a token.
///
/// The canonical representation used for interning, hashing, and length accounting is bytes. Implementations are also
/// responsible for rebuilding an aligned owned value from bytes so resolved tokens can return borrowed typed data.
pub trait TokenData: ToOwned + 'static {
    /// Returns the canonical byte representation of this payload.
    fn as_bytes(&self) -> &[u8];

    /// Rebuilds an owned, properly aligned value from canonical bytes.
    fn from_bytes(bytes: &[u8]) -> Result<Self::Owned, TokenDataError>;
}

/// Error returned when bytes cannot be decoded as a token payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenDataError {
    /// Bytes were not valid UTF-8 for `str` token data.
    InvalidUtf8,
    /// Bytes did not have the right size for a typed token payload.
    InvalidSize {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
}

impl fmt::Display for TokenDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenDataError::InvalidUtf8 => f.write_str("invalid UTF-8 token data"),
            TokenDataError::InvalidSize { expected, actual } => {
                write!(f, "invalid token data size: expected {expected} bytes, got {actual}")
            }
        }
    }
}

impl core::error::Error for TokenDataError {}

impl TokenData for str {
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        str::as_bytes(self)
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Result<Self::Owned, TokenDataError> {
        core::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| TokenDataError::InvalidUtf8)
    }
}

impl TokenData for [u8] {
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        self
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Result<Self::Owned, TokenDataError> {
        Ok(bytes.to_vec())
    }
}

#[cfg(feature = "bytemuck")]
impl<T> TokenData for T
where
    T: bytemuck::Pod + Copy + 'static,
{
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }

    #[inline]
    fn from_bytes(bytes: &[u8]) -> Result<Self::Owned, TokenDataError> {
        if bytes.len() != core::mem::size_of::<T>() {
            return Err(TokenDataError::InvalidSize {
                expected: core::mem::size_of::<T>(),
                actual: bytes.len(),
            });
        }
        Ok(*bytemuck::from_bytes(bytes))
    }
}

/// The read-only part of an interner.
/// Allows lookups of intern keys to resolve them to their interned data.
pub trait Resolver<Key: InternKey = TokenKey, Data: TokenData + ?Sized = str> {
    /// Tries to resolve the given `key` and return its interned data.
    ///
    /// If `self` does not contain any data for `key`, `None` is returned.
    fn try_resolve(&self, key: Key) -> Option<&Data>;

    /// Resolves `key` to its interned data.
    ///
    /// # Panics
    /// Panics if there is no data for `key`.
    ///
    /// Compatibility implementations for interners from other crates may also panic if `key` cannot be converted to the
    /// key type of the external interner. Please ensure you configure any external interners appropriately (for
    /// example by choosing an appropriately sized key type).
    fn resolve(&self, key: Key) -> &Data {
        self.try_resolve(key)
            .unwrap_or_else(|| panic!("failed to resolve `{key:?}`"))
    }
}

impl<R, Data> Resolver<TokenKey, Data> for &R
where
    R: Resolver<TokenKey, Data> + ?Sized,
    Data: TokenData + ?Sized,
{
    fn try_resolve(&self, key: TokenKey) -> Option<&Data> {
        (**self).try_resolve(key)
    }

    fn resolve(&self, key: TokenKey) -> &Data {
        (**self).resolve(key)
    }
}

impl<R, Data> Resolver<TokenKey, Data> for &mut R
where
    R: Resolver<TokenKey, Data> + ?Sized,
    Data: TokenData + ?Sized,
{
    fn try_resolve(&self, key: TokenKey) -> Option<&Data> {
        (**self).try_resolve(key)
    }

    fn resolve(&self, key: TokenKey) -> &Data {
        (**self).resolve(key)
    }
}

/// A full interner, which can intern new strings returning intern keys and also resolve intern keys to the interned
/// value.
///
/// **Note:** Because single-threaded interners may require mutable access, the methods on this trait take `&mut self`.
/// In order to use a multi- (or single)-threaded interner that allows access through a shared reference, it is
/// implemented for `&MultiThreadedTokenInterner` and `Arc<MultiThreadedTokenInterner>`, allowing it
/// to be used with a `&mut &MultiThreadedTokenInterner` and `&mut Arc<MultiThreadTokenInterner>`.
pub trait Interner<Key: InternKey = TokenKey, Data: TokenData + ?Sized = str>: Resolver<Key, Data> {
    /// Represents possible ways in which interning may fail.
    /// For example, this might be running out of fresh intern keys, or failure to allocate sufficient space for a new
    /// value.
    type Error;

    /// Interns `data` and returns a new intern key for it.
    /// If `data` was already previously interned, it will not be used and the existing intern key for its value will be
    /// returned.
    fn try_get_or_intern(&mut self, data: &Data) -> Result<Key, Self::Error>;

    /// Interns canonical bytes and returns a new intern key for them.
    fn try_get_or_intern_bytes(&mut self, bytes: &[u8]) -> Result<Key, Self::Error>;

    /// Interns `data` and returns a new intern key for it.
    ///
    /// # Panics
    /// Panics if the internment process raises an [`Error`](Interner::Error).
    fn get_or_intern(&mut self, data: &Data) -> Key {
        self.try_get_or_intern(data)
            .unwrap_or_else(|_| panic!("failed to intern token data"))
    }

    /// Interns canonical bytes and returns a new intern key for them.
    ///
    /// # Panics
    /// Panics if the bytes cannot be decoded or interned.
    fn get_or_intern_bytes(&mut self, bytes: &[u8]) -> Key {
        self.try_get_or_intern_bytes(bytes)
            .unwrap_or_else(|_| panic!("failed to intern token data bytes"))
    }
}

impl<I, Data> Interner<TokenKey, Data> for &mut I
where
    I: Interner<TokenKey, Data> + ?Sized,
    Data: TokenData + ?Sized,
{
    type Error = I::Error;

    fn try_get_or_intern(&mut self, data: &Data) -> Result<TokenKey, Self::Error> {
        (**self).try_get_or_intern(data)
    }

    fn try_get_or_intern_bytes(&mut self, bytes: &[u8]) -> Result<TokenKey, Self::Error> {
        (**self).try_get_or_intern_bytes(bytes)
    }

    fn get_or_intern(&mut self, data: &Data) -> TokenKey {
        (**self).get_or_intern(data)
    }

    fn get_or_intern_bytes(&mut self, bytes: &[u8]) -> TokenKey {
        (**self).get_or_intern_bytes(bytes)
    }
}
