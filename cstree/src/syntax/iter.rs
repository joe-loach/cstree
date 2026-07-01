//! Red tree iterators.

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::iter::FusedIterator;

use text_size::TextSize;

use crate::{
    Syntax,
    green::{GreenElementRef, GreenNodeChildren},
    syntax::{SyntaxElement, SyntaxNode},
};

#[derive(Clone, Debug)]
struct Iter<'n> {
    green: GreenNodeChildren<'n>,
    offset: TextSize,
    index: usize,
}

impl<'n> Iter<'n> {
    fn new<S: Syntax, D>(parent: &'n SyntaxNode<S, D>) -> Self {
        let offset = parent.text_range().start();
        let green: GreenNodeChildren<'_> = parent.green().children();
        Iter {
            green,
            offset,
            index: 0,
        }
    }
}

impl<'n> Iterator for Iter<'n> {
    type Item = (GreenElementRef<'n>, usize, TextSize);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.green.next().map(|element| {
            let offset = self.offset;
            let index = self.index;
            self.offset += element.text_len();
            self.index += 1;
            (element, index, offset)
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.green.size_hint()
    }

    #[inline(always)]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.green.count()
    }
}

impl ExactSizeIterator for Iter<'_> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.green.len()
    }
}
impl FusedIterator for Iter<'_> {}

/// An iterator over the child nodes of a [`SyntaxNode`].
#[derive(Debug)]
pub struct SyntaxNodeChildren<S: Syntax, D: 'static = ()> {
    inner: vec::IntoIter<SyntaxNode<S, D>>,
}

impl<S: Syntax, D> Clone for SyntaxNodeChildren<S, D> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S: Syntax, D> SyntaxNodeChildren<S, D> {
    #[inline]
    pub(super) fn new(parent: &SyntaxNode<S, D>) -> Self {
        let mut children = Vec::with_capacity(parent.arity());
        for (element, index, offset) in Iter::new(parent) {
            if let Some(&node) = element.as_node() {
                children.push((*parent.get_or_add_node(node, index, offset).as_node().unwrap()).clone());
            }
        }
        Self {
            inner: children.into_iter(),
        }
    }
}

impl<S: Syntax, D> Iterator for SyntaxNodeChildren<S, D> {
    type Item = SyntaxNode<S, D>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline(always)]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }
}

impl<S: Syntax, D> ExactSizeIterator for SyntaxNodeChildren<S, D> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<S: Syntax, D> FusedIterator for SyntaxNodeChildren<S, D> {}

/// An iterator over the children of a [`SyntaxNode`].
#[derive(Debug)]
pub struct SyntaxElementChildren<S: Syntax, D: 'static = ()> {
    inner: vec::IntoIter<SyntaxElement<S, D>>,
}

impl<S: Syntax, D> Clone for SyntaxElementChildren<S, D> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S: Syntax, D> SyntaxElementChildren<S, D> {
    #[inline]
    pub(super) fn new(parent: &SyntaxNode<S, D>) -> Self {
        let children = Iter::new(parent)
            .map(|(green, index, offset)| parent.get_or_add_element(green, index, offset).cloned())
            .collect::<Vec<_>>();
        Self {
            inner: children.into_iter(),
        }
    }
}

impl<S: Syntax, D> Iterator for SyntaxElementChildren<S, D> {
    type Item = SyntaxElement<S, D>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline(always)]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.inner.count()
    }
}

impl<S: Syntax, D> ExactSizeIterator for SyntaxElementChildren<S, D> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<S: Syntax, D> FusedIterator for SyntaxElementChildren<S, D> {}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct DummyKind;

    impl Syntax for DummyKind {
        type Data = [u8];

        fn from_raw(_: crate::RawSyntaxKind) -> Self {
            unreachable!()
        }

        fn into_raw(self) -> crate::RawSyntaxKind {
            unreachable!()
        }

        fn data_to_bytes(data: &Self::Data) -> &[u8] {
            data
        }

        fn data_from_bytes(data: &[u8]) -> Option<&Self::Data> {
            Some(data)
        }

        fn static_data(self) -> Option<&'static Self::Data> {
            unreachable!()
        }
    }

    struct NotClone;

    fn assert_clone<C: Clone>() {}

    fn test_impls_clone() {
        assert_clone::<SyntaxNodeChildren<DummyKind, NotClone>>();
        assert_clone::<SyntaxElementChildren<DummyKind, NotClone>>();
    }
}
