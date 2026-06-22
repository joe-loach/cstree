extern crate alloc;

use alloc::{sync::Arc as StdArc, vec::Vec};
use core::{borrow::Borrow, fmt, marker::PhantomData};

use hashbrown::HashMap;
use rustc_hash::FxBuildHasher;

use super::{InternKey, Interner, Resolver, TokenData, TokenDataError, TokenKey};

/// The default [`Interner`] used to deduplicate green token data.
#[derive(Debug)]
pub struct TokenInterner<Data: TokenData + ?Sized = str> {
    keys: HashMap<Vec<u8>, TokenKey, FxBuildHasher>,
    values: Vec<Data::Owned>,
    _data: PhantomData<fn() -> Data>,
}

impl<Data: TokenData + ?Sized> TokenInterner<Data> {
    pub(in crate::interning) fn new() -> Self {
        Self {
            keys: HashMap::default(),
            values: Vec::new(),
            _data: PhantomData,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternerError {
    KeySpaceExhausted,
    InvalidData(TokenDataError),
}

impl fmt::Display for InternerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InternerError::KeySpaceExhausted => write!(f, "key space exhausted"),
            InternerError::InvalidData(error) => write!(f, "{error}"),
        }
    }
}

impl core::error::Error for InternerError {}

impl<Data> Resolver<TokenKey, Data> for TokenInterner<Data>
where
    Data: TokenData + ?Sized,
    Data::Owned: Borrow<Data>,
{
    fn try_resolve(&self, key: TokenKey) -> Option<&Data> {
        let index = key.into_u32() as usize;
        self.values.get(index).map(Borrow::borrow)
    }
}

impl<Data> Resolver<TokenKey, Data> for StdArc<TokenInterner<Data>>
where
    Data: TokenData + ?Sized,
    Data::Owned: Borrow<Data>,
{
    fn try_resolve(&self, key: TokenKey) -> Option<&Data> {
        let index = key.into_u32() as usize;
        self.values.get(index).map(Borrow::borrow)
    }
}

// `TokenKey` can represent `1` to `u32::MAX` (due to the `NonNull` niche), so `u32::MAX` elements.
// Set indices start at 0, so everything shifts down by 1.
const N_INDICES: usize = u32::MAX as usize;

impl<Data> Interner<TokenKey, Data> for TokenInterner<Data>
where
    Data: TokenData + ?Sized,
    Data::Owned: Borrow<Data>,
{
    type Error = InternerError;

    fn try_get_or_intern(&mut self, data: &Data) -> Result<TokenKey, Self::Error> {
        self.try_get_or_intern_bytes(data.as_bytes())
    }

    fn try_get_or_intern_bytes(&mut self, bytes: &[u8]) -> Result<TokenKey, Self::Error> {
        if let Some(key) = self.keys.get(bytes) {
            return Ok(*key);
        } else if self.values.len() >= N_INDICES {
            return Err(InternerError::KeySpaceExhausted);
        }

        let owned = Data::from_bytes(bytes).map_err(InternerError::InvalidData)?;
        let index = self.values.len();
        let raw_key = u32::try_from(index).unwrap_or_else(|_| panic!("interned `{index}` despite keyspace exhaustion"));
        let key = TokenKey::try_from_u32(raw_key).ok_or(InternerError::KeySpaceExhausted)?;
        self.values.push(owned);
        self.keys.insert(bytes.to_vec(), key);
        Ok(key)
    }
}
