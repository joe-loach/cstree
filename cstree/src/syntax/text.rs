//! Efficient representation of source data and text covered by a [`SyntaxNode`].

extern crate alloc;

use alloc::string::{String, ToString};
use core::fmt;

use crate::{
    Syntax,
    syntax::{SyntaxNode, SyntaxToken},
    text::{TextRange, TextSize},
};

/// An efficient representation of the bytes covered by a [`SyntaxNode`].
pub struct SyntaxData<'n, S: Syntax, D: 'static = ()> {
    node: &'n SyntaxNode<S, D>,
    range: TextRange,
}

impl<S: Syntax, D> Clone for SyntaxData<'_, S, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: Syntax, D> Copy for SyntaxData<'_, S, D> {}

impl<'n, S: Syntax, D> SyntaxData<'n, S, D> {
    pub(crate) fn new(node: &'n SyntaxNode<S, D>) -> Self {
        let range = node.text_range();
        SyntaxData { node, range }
    }

    /// The combined length of this data, in bytes.
    pub fn len(&self) -> TextSize {
        self.range.len()
    }

    /// Returns `true` if [`self.len()`](SyntaxData::len) is zero.
    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    /// Indexes this data by the given byte range and returns the corresponding slice representation.
    pub fn slice<Ra: private::SyntaxTextRange>(&self, range: Ra) -> Self {
        let start = range.start().unwrap_or_default();
        let end = range.end().unwrap_or_else(|| self.len());
        assert!(start <= end);
        let len = end - start;
        let start = self.range.start() + start;
        let end = start + len;
        let range = TextRange::new(start, end);
        assert!(self.range.contains_range(range));
        SyntaxData { node: self.node, range }
    }

    /// Applies the given function to byte chunks from descendant tokens.
    pub fn try_fold_chunks<T, F, E>(&self, init: T, mut f: F) -> Result<T, E>
    where
        F: FnMut(T, &[u8]) -> Result<T, E>,
    {
        self.tokens_with_ranges().try_fold(init, move |acc, (token, range)| {
            let start = u32::from(range.start()) as usize;
            let end = u32::from(range.end()) as usize;
            f(acc, &token.data_bytes()[start..end])
        })
    }

    /// Applies the given function to byte chunks from descendant tokens.
    pub fn fold_chunks<T, F>(&self, init: T, mut f: F) -> T
    where
        F: FnMut(T, &[u8]) -> T,
    {
        enum Void {}
        match self.try_fold_chunks(init, |acc, chunk| Ok::<T, Void>(f(acc, chunk))) {
            Ok(t) => t,
            Err(void) => match void {},
        }
    }

    /// Applies the given function to byte chunks until it fails.
    pub fn try_for_each_chunk<F: FnMut(&[u8]) -> Result<(), E>, E>(&self, mut f: F) -> Result<(), E> {
        self.try_fold_chunks((), move |(), chunk| f(chunk))
    }

    /// Applies the given function to all byte chunks.
    pub fn for_each_chunk<F: FnMut(&[u8])>(&self, mut f: F) {
        self.fold_chunks((), |(), chunk| f(chunk))
    }

    fn tokens_with_ranges(&self) -> impl Iterator<Item = (SyntaxToken<S, D>, TextRange)> + use<'n, S, D> {
        let text_range = self.range;
        self.node
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter_map(move |token| {
                let token_range = token.text_range();
                let range = text_range.intersect(token_range)?;
                Some((token, range - token_range.start()))
            })
    }
}

impl<S: Syntax, D> fmt::Debug for SyntaxData<'_, S, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.tokens_with_ranges().map(|(token, range)| {
                let start = u32::from(range.start()) as usize;
                let end = u32::from(range.end()) as usize;
                token.data_bytes()[start..end].to_vec()
            }))
            .finish()
    }
}

/// An efficient representation of UTF-8 text covered by a [`SyntaxNode`].
pub struct SyntaxText<'n, S: Syntax, D: 'static = ()> {
    node: &'n SyntaxNode<S, D>,
    range: TextRange,
}

impl<S: Syntax, D> Clone for SyntaxText<'_, S, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: Syntax, D> Copy for SyntaxText<'_, S, D> {}

impl<'n, S: Syntax, D> SyntaxText<'n, S, D> {
    pub(crate) fn new(node: &'n SyntaxNode<S, D>) -> Option<Self> {
        let text = SyntaxText {
            node,
            range: node.text_range(),
        };
        text.tokens_with_ranges()
            .all(|(token, range)| core::str::from_utf8(&token.data_bytes()[byte_bounds(range)]).is_ok())
            .then_some(text)
    }

    /// The combined length of this text, in bytes.
    pub fn len(&self) -> TextSize {
        self.range.len()
    }

    /// Returns `true` if [`self.len()`](SyntaxText::len) is zero.
    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    /// Returns `true` if `c` appears anywhere in this text.
    pub fn contains_char(&self, c: char) -> bool {
        self.try_for_each_chunk(|chunk| if chunk.contains(c) { Err(()) } else { Ok(()) })
            .is_err()
    }

    /// If `self.contains_char(c)`, returns the byte position of the first appearance of `c`.
    pub fn find_char(&self, c: char) -> Option<TextSize> {
        let mut acc: TextSize = 0.into();
        let res = self.try_for_each_chunk(|chunk| {
            if let Some(pos) = chunk.find(c) {
                let pos: TextSize = (pos as u32).into();
                return Err(acc + pos);
            }
            acc += TextSize::of(chunk);
            Ok(())
        });
        found(res)
    }

    /// If `offset < self.len()`, returns the first `char` at or after `offset`.
    pub fn char_at(&self, offset: TextSize) -> Option<char> {
        let mut start: TextSize = 0.into();
        let res = self.try_for_each_chunk(|chunk| {
            let end = start + TextSize::of(chunk);
            if start <= offset && offset < end {
                let off: usize = u32::from(offset - start) as usize;
                return Err(chunk[off..].chars().next().unwrap());
            }
            start = end;
            Ok(())
        });
        found(res)
    }

    /// Indexes this text by the given byte range and returns a `SyntaxText` for that slice.
    pub fn slice<Ra: private::SyntaxTextRange>(&self, range: Ra) -> Self {
        let start = range.start().unwrap_or_default();
        let end = range.end().unwrap_or_else(|| self.len());
        assert!(start <= end);
        let len = end - start;
        let start = self.range.start() + start;
        let end = start + len;
        let range = TextRange::new(start, end);
        assert!(self.range.contains_range(range));
        SyntaxText { node: self.node, range }
    }

    /// Applies the given function to text chunks until it fails.
    pub fn try_fold_chunks<T, F, E>(&self, init: T, mut f: F) -> Result<T, E>
    where
        F: FnMut(T, &str) -> Result<T, E>,
    {
        self.tokens_with_ranges().try_fold(init, move |acc, (token, range)| {
            let chunk = core::str::from_utf8(&token.data_bytes()[byte_bounds(range)]).unwrap();
            f(acc, chunk)
        })
    }

    /// Applies the given function to all text chunks.
    pub fn fold_chunks<T, F>(&self, init: T, mut f: F) -> T
    where
        F: FnMut(T, &str) -> T,
    {
        enum Void {}
        match self.try_fold_chunks(init, |acc, chunk| Ok::<T, Void>(f(acc, chunk))) {
            Ok(t) => t,
            Err(void) => match void {},
        }
    }

    /// Applies the given function to all text chunks until it fails.
    pub fn try_for_each_chunk<F: FnMut(&str) -> Result<(), E>, E>(&self, mut f: F) -> Result<(), E> {
        self.try_fold_chunks((), move |(), chunk| f(chunk))
    }

    /// Applies the given function to all text chunks.
    pub fn for_each_chunk<F: FnMut(&str)>(&self, mut f: F) {
        self.fold_chunks((), |(), chunk| f(chunk))
    }

    fn tokens_with_ranges(&self) -> impl Iterator<Item = (SyntaxToken<S, D>, TextRange)> + use<'n, S, D> {
        let text_range = self.range;
        self.node
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter_map(move |token| {
                let token_range = token.text_range();
                let range = text_range.intersect(token_range)?;
                Some((token, range - token_range.start()))
            })
    }
}

#[inline]
fn byte_bounds(range: TextRange) -> core::ops::Range<usize> {
    u32::from(range.start()) as usize..u32::from(range.end()) as usize
}

#[inline]
fn found<T>(res: Result<(), T>) -> Option<T> {
    res.err()
}

impl<S: Syntax, D> fmt::Debug for SyntaxText<'_, S, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.to_string(), f)
    }
}

impl<S: Syntax, D> fmt::Display for SyntaxText<'_, S, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.try_for_each_chunk(|chunk| fmt::Display::fmt(chunk, f))
    }
}

impl<S: Syntax, D> From<SyntaxText<'_, S, D>> for String {
    fn from(text: SyntaxText<'_, S, D>) -> String {
        text.to_string()
    }
}

impl<S: Syntax, D> PartialEq<str> for SyntaxText<'_, S, D> {
    fn eq(&self, mut rhs: &str) -> bool {
        self.try_for_each_chunk(|chunk| {
            if !rhs.starts_with(chunk) {
                return Err(());
            }
            rhs = &rhs[chunk.len()..];
            Ok(())
        })
        .is_ok()
            && rhs.is_empty()
    }
}

impl<S: Syntax, D> PartialEq<SyntaxText<'_, S, D>> for str {
    fn eq(&self, rhs: &SyntaxText<'_, S, D>) -> bool {
        rhs == self
    }
}

impl<S: Syntax, D> PartialEq<&'_ str> for SyntaxText<'_, S, D> {
    fn eq(&self, rhs: &&str) -> bool {
        self == *rhs
    }
}

impl<S: Syntax, D> PartialEq<SyntaxText<'_, S, D>> for &'_ str {
    fn eq(&self, rhs: &SyntaxText<'_, S, D>) -> bool {
        rhs == self
    }
}

impl<S1, S2, D1, D2> PartialEq<SyntaxText<'_, S2, D2>> for SyntaxText<'_, S1, D1>
where
    S1: Syntax,
    S2: Syntax,
{
    fn eq(&self, other: &SyntaxText<'_, S2, D2>) -> bool {
        if self.range.len() != other.range.len() {
            return false;
        }
        let mut lhs = self.tokens_with_ranges();
        let mut rhs = other.tokens_with_ranges();
        zip_texts(&mut lhs, &mut rhs).is_none() && lhs.all(|it| it.1.is_empty()) && rhs.all(|it| it.1.is_empty())
    }
}

fn zip_texts<'it1, 'it2, It1, It2, S1, S2, D1, D2>(xs: &mut It1, ys: &mut It2) -> Option<()>
where
    It1: Iterator<Item = (SyntaxToken<S1, D1>, TextRange)>,
    It2: Iterator<Item = (SyntaxToken<S2, D2>, TextRange)>,
    D1: 'static,
    D2: 'static,
    S1: Syntax + 'it1,
    S2: Syntax + 'it2,
{
    let mut x = xs.next()?;
    let mut y = ys.next()?;
    loop {
        while x.1.is_empty() {
            x = xs.next()?;
        }
        while y.1.is_empty() {
            y = ys.next()?;
        }
        let x_text = core::str::from_utf8(&x.0.data_bytes()[byte_bounds(x.1)]).unwrap();
        let y_text = core::str::from_utf8(&y.0.data_bytes()[byte_bounds(y.1)]).unwrap();
        if !(x_text.starts_with(y_text) || y_text.starts_with(x_text)) {
            return Some(());
        }
        let advance = core::cmp::min(x.1.len(), y.1.len());
        x.1 = TextRange::new(x.1.start() + advance, x.1.end());
        y.1 = TextRange::new(y.1.start() + advance, y.1.end());
    }
}

impl<S: Syntax, D> Eq for SyntaxText<'_, S, D> {}

mod private {
    use core::ops;

    use crate::text::{TextRange, TextSize};

    pub trait SyntaxTextRange {
        fn start(&self) -> Option<TextSize>;
        fn end(&self) -> Option<TextSize>;
    }

    impl SyntaxTextRange for TextRange {
        fn start(&self) -> Option<TextSize> {
            Some(TextRange::start(*self))
        }

        fn end(&self) -> Option<TextSize> {
            Some(TextRange::end(*self))
        }
    }

    impl SyntaxTextRange for ops::Range<TextSize> {
        fn start(&self) -> Option<TextSize> {
            Some(self.start)
        }

        fn end(&self) -> Option<TextSize> {
            Some(self.end)
        }
    }

    impl SyntaxTextRange for ops::RangeFrom<TextSize> {
        fn start(&self) -> Option<TextSize> {
            Some(self.start)
        }

        fn end(&self) -> Option<TextSize> {
            None
        }
    }

    impl SyntaxTextRange for ops::RangeTo<TextSize> {
        fn start(&self) -> Option<TextSize> {
            None
        }

        fn end(&self) -> Option<TextSize> {
            Some(self.end)
        }
    }

    impl SyntaxTextRange for ops::RangeFull {
        fn start(&self) -> Option<TextSize> {
            None
        }

        fn end(&self) -> Option<TextSize> {
            None
        }
    }
}
