mod basic;
#[cfg(feature = "derive")]
mod regressions;
mod rollback;
mod sendsync;
#[cfg(feature = "serialize")]
mod serde;

use cstree::{RawSyntaxKind, Syntax, build::GreenNodeBuilder, green::GreenNode, util::NodeOrToken};

pub type SyntaxNode<D = ()> = cstree::syntax::SyntaxNode<SyntaxKind, D>;
pub type SyntaxToken<D = ()> = cstree::syntax::SyntaxToken<SyntaxKind, D>;
pub type SyntaxElement<D = ()> = cstree::syntax::SyntaxElement<SyntaxKind, D>;
pub type SyntaxElementRef<'a, D = ()> = cstree::syntax::SyntaxElementRef<'a, SyntaxKind, D>;

#[derive(Debug)]
pub enum Element<'s> {
    Node(Vec<Element<'s>>),
    Token(&'s str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct SyntaxKind(u32);

impl Syntax for SyntaxKind {
    type Data = str;

    fn from_raw(raw: RawSyntaxKind) -> Self {
        Self(raw.0)
    }

    fn into_raw(self) -> RawSyntaxKind {
        RawSyntaxKind(self.0)
    }

    fn data_to_bytes(data: &Self::Data) -> &[u8] {
        data.as_bytes()
    }

    fn data_from_bytes(data: &[u8]) -> Option<&Self::Data> {
        core::str::from_utf8(data).ok()
    }

    fn static_data(self) -> Option<&'static Self::Data> {
        None
    }
}

pub fn build_tree(root: &Element<'_>) -> GreenNode {
    let mut builder = GreenNodeBuilder::new();
    build_recursive(root, &mut builder, 0);
    builder.finish()
}

pub fn build_recursive(root: &Element<'_>, builder: &mut GreenNodeBuilder<SyntaxKind>, mut from: u32) -> u32 {
    match root {
        Element::Node(children) => {
            builder.start_node(SyntaxKind(from));
            for child in children {
                from = build_recursive(child, builder, from + 1);
            }
            builder.finish_node();
        }
        Element::Token(text) => {
            builder.token(SyntaxKind(from), text);
        }
    }
    from
}

#[track_caller]
pub fn assert_tree_eq(left: &SyntaxNode, right: &SyntaxNode) {
    if left.green() == right.green() {
        return;
    }

    if left.kind() != right.kind() || left.children_with_tokens().len() != right.children_with_tokens().len() {
        panic!("{} !=\n{}", left.debug(true), right.debug(true))
    }

    for elem in left.children_with_tokens().zip(right.children_with_tokens()) {
        match elem {
            (NodeOrToken::Node(ln), NodeOrToken::Node(rn)) => assert_tree_eq(&ln, &rn),
            (NodeOrToken::Node(n), NodeOrToken::Token(t)) => {
                panic!("{} != {}", n.debug(true), t.debug())
            }
            (NodeOrToken::Token(t), NodeOrToken::Node(n)) => {
                panic!("{} != {}", t.debug(), n.debug(true))
            }
            (NodeOrToken::Token(lt), NodeOrToken::Token(rt)) => {
                if lt.syntax_kind() != rt.syntax_kind() || lt.data() != rt.data() {
                    panic!("{} != {}", lt.debug(), rt.debug())
                }
            }
        }
    }
}
