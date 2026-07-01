extern crate alloc;

use alloc::string::String;
use core::{
    fmt,
    hash::{Hash, Hasher},
    iter,
};

use text_size::{TextRange, TextSize};

use super::*;
use crate::{
    RawSyntaxKind, Syntax,
    green::{GreenNode, GreenToken},
    traversal::Direction,
};

/// Syntax tree token.
pub struct SyntaxToken<S: Syntax, D: 'static = ()> {
    parent: SyntaxNode<S, D>,
    index: u32,
    offset: TextSize,
}

impl<S: Syntax, D> Clone for SyntaxToken<S, D> {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            index: self.index,
            offset: self.offset,
        }
    }
}

impl<S: Syntax, D> Hash for SyntaxToken<S, D> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.parent.hash(state);
        self.index.hash(state);
        self.offset.hash(state);
    }
}

impl<S: Syntax, D> PartialEq for SyntaxToken<S, D> {
    fn eq(&self, other: &SyntaxToken<S, D>) -> bool {
        self.parent == other.parent && self.index == other.index && self.offset == other.offset
    }
}

impl<S: Syntax, D> Eq for SyntaxToken<S, D> {}

impl<S: Syntax, D> SyntaxToken<S, D> {
    /// Writes this token's [`Debug`](fmt::Debug) representation into the given `target`.
    pub fn write_debug(&self, target: &mut impl fmt::Write) -> fmt::Result {
        write!(target, "{:?}@{:?}", self.kind(), self.text_range())?;
        write!(target, " ")?;
        fmt_data_debug(self.data_bytes(), target)
    }

    /// Returns this token's [`Debug`](fmt::Debug) representation as a string.
    ///
    /// To avoid allocating for every token, see [`write_debug`](SyntaxToken::write_debug).
    #[inline]
    pub fn debug(&self) -> String {
        // NOTE: `fmt::Write` methods on `String` never fail
        let mut res = String::new();
        self.write_debug(&mut res).unwrap();
        res
    }

    /// Writes this token's [`Display`](fmt::Display) representation into the given `target`.
    #[inline]
    pub fn write_display(&self, target: &mut impl fmt::Write) -> fmt::Result {
        fmt_data_display(self.data_bytes(), target)
    }

    /// Returns this token's [`Display`](fmt::Display) representation as a string.
    ///
    /// To avoid allocating for every token, see [`write_display`](SyntaxToken::write_display).
    #[inline]
    pub fn display(&self) -> String {
        let mut res = String::new();
        self.write_display(&mut res).unwrap();
        res
    }
}

impl<S: Syntax, D> fmt::Debug for SyntaxToken<S, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_debug(f)
    }
}

impl<S: Syntax, D> fmt::Display for SyntaxToken<S, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_display(f)
    }
}

impl<S: Syntax, D> SyntaxToken<S, D> {
    pub(super) fn new(parent: &SyntaxNode<S, D>, index: u32, offset: TextSize) -> SyntaxToken<S, D> {
        Self {
            parent: parent.clone_uncounted(),
            index,
            offset,
        }
    }

    /// Returns a green tree, equal to the green tree this token
    /// belongs two, except with this token substitute. The complexity
    /// of operation is proportional to the depth of the tree
    pub fn replace_with(&self, replacement: GreenToken) -> GreenNode {
        assert_eq!(self.syntax_kind(), replacement.kind());
        let mut replacement = Some(replacement);
        let parent = self.parent();
        let me = self.index;

        let children = parent.green().children().enumerate().map(|(i, child)| {
            if i as u32 == me {
                replacement.take().unwrap().into()
            } else {
                child.cloned()
            }
        });
        let new_parent = GreenNode::new(parent.syntax_kind(), children);
        parent.replace_with(new_parent)
    }

    /// The internal representation of the kind of this token.
    #[inline]
    pub fn syntax_kind(&self) -> RawSyntaxKind {
        self.green().kind()
    }

    /// The kind of this token in terms of your language.
    #[inline]
    pub fn kind(&self) -> S {
        S::from_raw(self.syntax_kind())
    }

    /// The range this token covers in the source text, in bytes.
    #[inline]
    pub fn text_range(&self) -> TextRange {
        TextRange::at(self.offset, self.green().text_len())
    }

    /// Returns the typed source data of this token.
    #[inline]
    pub fn data(&self) -> &S::Data {
        S::data_from_bytes(self.green().data()).expect("green token data violated the syntax data invariant")
    }

    /// Returns the source bytes of this token.
    #[inline]
    pub fn data_bytes(&self) -> &[u8] {
        S::data_to_bytes(self.data())
    }

    /// Returns the source text of this token, if it is valid UTF-8.
    #[inline]
    pub fn text(&self) -> Option<&str> {
        core::str::from_utf8(self.data_bytes()).ok()
    }

    /// If the [syntax kind](Syntax) of this token always represents the same data, returns that data.
    #[inline(always)]
    pub fn static_data(&self) -> Option<&'static S::Data> {
        S::static_data(self.kind())
    }

    /// If the [syntax kind](Syntax) of this token always represents the same data, returns its bytes.
    #[inline(always)]
    pub fn static_data_bytes(&self) -> Option<&'static [u8]> {
        self.static_data().map(S::data_to_bytes)
    }

    /// If the [syntax kind](Syntax) of this token always represents the same text, returns
    /// that text.
    ///
    /// # Examples
    /// If there is a syntax kind `Plus` that represents just the `+` operator and we implement
    /// [`Syntax::static_text`] for it, we can retrieve this text in the resulting syntax tree.
    ///
    /// ```
    /// # use cstree::testing::*;
    /// # use cstree::build::*;
    /// let mut builder: GreenNodeBuilder<MySyntax> = GreenNodeBuilder::new();
    /// # builder.start_node(Root);
    /// # builder.token(Identifier, b"x");
    /// # builder.token(Whitespace, b" ");
    /// # builder.token(Plus, b"+");
    /// # builder.token(Whitespace, b" ");
    /// # builder.token(Int, b"3");
    /// # builder.finish_node();
    /// let tree = parse(&mut builder, "x + 3");
    /// # let tree: SyntaxNode<MySyntax> = SyntaxNode::new_root(builder.finish());
    /// let plus = tree
    ///     .children_with_tokens()
    ///     .nth(2) // `x`, then a space, then `+`
    ///     .unwrap()
    ///     .into_token()
    ///     .unwrap();
    /// assert_eq!(plus.static_text(), Some("+"));
    /// ```
    #[inline(always)]
    pub fn static_text(&self) -> Option<&'static str> {
        self.kind().static_text()
    }

    /// Returns `true` if `self` and `other` represent equal source text.
    ///
    /// This method is different from the `PartialEq` and `Eq` implementations in that it compares
    /// only the token text and not its source position.
    /// It compares the token bytes directly and does not consider source position.
    ///
    /// # Examples
    /// ```
    /// # use cstree::testing::*;
    /// let mut builder: GreenNodeBuilder<MySyntax> = GreenNodeBuilder::new();
    /// # builder.start_node(Root);
    /// # builder.token(Identifier, b"x");
    /// # builder.token(Whitespace, b" ");
    /// # builder.token(Plus, b"+");
    /// # builder.token(Whitespace, b" ");
    /// # builder.token(Identifier, b"x");
    /// # builder.token(Whitespace, b" ");
    /// # builder.token(Plus, b"+");
    /// # builder.token(Int, b"3");
    /// # builder.finish_node();
    /// let tree = parse(&mut builder, "x + x + 3");
    /// # let tree: SyntaxNode<MySyntax> = SyntaxNode::new_root(builder.finish());
    /// let mut tokens = tree.children_with_tokens();
    /// let tokens = tokens.by_ref();
    /// let first_x = tokens.next().unwrap().into_token().unwrap();
    ///
    /// // For the other tokens, skip over the whitespace between them
    /// let first_plus = tokens.skip(1).next().unwrap().into_token().unwrap();
    /// let second_x = tokens.skip(1).next().unwrap().into_token().unwrap();
    /// let second_plus = tokens.skip(1).next().unwrap().into_token().unwrap();
    /// assert!(first_x.text_eq(&second_x));
    /// assert!(first_plus.text_eq(&second_plus));
    /// ```
    #[inline]
    pub fn data_eq(&self, other: &Self) -> bool {
        self.data_bytes() == other.data_bytes()
    }

    /// Returns `true` if `self` and `other` represent equal source text.
    #[inline]
    pub fn text_eq(&self, other: &Self) -> bool {
        self.data_eq(other)
    }

    /// Returns the unterlying green tree token of this token.
    #[inline]
    pub fn green(&self) -> &GreenToken {
        self.parent
            .green()
            .children()
            .nth(self.index as usize)
            .unwrap()
            .as_token()
            .unwrap()
    }

    /// The parent node of this token.
    #[inline]
    pub fn parent(&self) -> &SyntaxNode<S, D> {
        &self.parent
    }

    /// Returns an iterator along the chain of parents of this token.
    #[inline]
    pub fn ancestors(&self) -> impl Iterator<Item = &SyntaxNode<S, D>> {
        self.parent().ancestors()
    }

    /// The tree element to the right of this one, i.e. the next child of this token's parent after this token.
    #[inline]
    pub fn next_sibling_or_token(&self) -> Option<SyntaxElementRef<'_, S, D>> {
        self.parent()
            .next_child_or_token_after(self.index as usize, self.text_range().end())
    }

    /// The tree element to the left of this one, i.e. the previous child of this token's parent after this token.
    #[inline]
    pub fn prev_sibling_or_token(&self) -> Option<SyntaxElementRef<'_, S, D>> {
        self.parent()
            .prev_child_or_token_before(self.index as usize, self.text_range().start())
    }

    /// Returns an iterator over all siblings of this token in the given `direction`, i.e. all of this
    /// token's parent's children from this token on to the left or the right.
    /// The first item in the iterator will always be this token.
    #[inline]
    pub fn siblings_with_tokens(&self, direction: Direction) -> impl Iterator<Item = SyntaxElement<S, D>> + use<S, D> {
        let me: SyntaxElement<S, D> = self.clone().into();
        iter::successors(Some(me), move |el| match direction {
            Direction::Next => el.next_sibling_or_token().map(|it| it.cloned()),
            Direction::Prev => el.prev_sibling_or_token().map(|it| it.cloned()),
        })
    }

    /// Returns the next token in the tree.
    /// This is not necessary a direct sibling of this token, but will always be further right in the tree.
    #[inline]
    pub fn next_token(&self) -> Option<&SyntaxToken<S, D>> {
        match self.next_sibling_or_token() {
            Some(element) => element.first_token(),
            None => self
                .parent()
                .ancestors()
                .find_map(|it| it.next_sibling_or_token())
                .and_then(|element| element.first_token()),
        }
    }

    /// Returns the previous token in the tree.
    /// This is not necessary a direct sibling of this token, but will always be further left in the tree.
    #[inline]
    pub fn prev_token(&self) -> Option<&SyntaxToken<S, D>> {
        match self.prev_sibling_or_token() {
            Some(element) => element.last_token(),
            None => self
                .parent()
                .ancestors()
                .find_map(|it| it.prev_sibling_or_token())
                .and_then(|element| element.last_token()),
        }
    }
}

fn fmt_data_debug<W: fmt::Write + ?Sized>(bytes: &[u8], f: &mut W) -> fmt::Result {
    if let Ok(text) = core::str::from_utf8(bytes) {
        write!(f, "{text:?}")
    } else if bytes.len() < 25 {
        write!(f, "{bytes:?}")
    } else {
        write!(f, "{:?}", &&bytes[..24])?;
        f.write_str(" ...")
    }
}

fn fmt_data_display<W: fmt::Write + ?Sized>(bytes: &[u8], f: &mut W) -> fmt::Result {
    if let Ok(text) = core::str::from_utf8(bytes) {
        f.write_str(text)
    } else if bytes.len() < 25 {
        write!(f, "{bytes:?}")
    } else {
        write!(f, "{:?}", &&bytes[..24])?;
        f.write_str(" ...")
    }
}
