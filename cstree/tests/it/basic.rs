use super::*;
use cstree::{RawSyntaxKind, build::GreenNodeBuilder, text::TextRange};

fn build_syntax_tree<D>(root: &Element<'_>) -> SyntaxNode<D> {
    SyntaxNode::new_root(build_tree(root))
}

fn two_level_tree() -> Element<'static> {
    use Element::*;
    Node(vec![
        Node(vec![Token("0.0"), Token("0.1")]),
        Node(vec![Token("1.0")]),
        Node(vec![Token("2.0"), Token("2.1"), Token("2.2")]),
    ])
}

fn tree_with_eq_tokens() -> Element<'static> {
    use Element::*;
    Node(vec![
        Node(vec![Token("a"), Token("b")]),
        Node(vec![Token("c")]),
        Node(vec![Token("a"), Token("b"), Token("c")]),
    ])
}

#[test]
fn create() {
    let tree = two_level_tree();
    let tree = build_syntax_tree::<()>(&tree);
    assert_eq!(tree.syntax_kind(), RawSyntaxKind(0));
    assert_eq!(tree.kind(), SyntaxKind(0));
    {
        let leaf1_0 = tree.children().nth(1).unwrap().children_with_tokens().next().unwrap();
        let leaf1_0 = leaf1_0.into_token().unwrap();
        assert_eq!(leaf1_0.syntax_kind(), RawSyntaxKind(5));
        assert_eq!(leaf1_0.kind(), SyntaxKind(5));
        assert_eq!(leaf1_0.text(), Some("1.0"));
        assert_eq!(leaf1_0.text_range(), TextRange::at(6.into(), 3.into()));
    }
    {
        let node2 = tree.children().nth(2).unwrap();
        assert_eq!(node2.syntax_kind(), RawSyntaxKind(6));
        assert_eq!(node2.kind(), SyntaxKind(6));
        assert_eq!(node2.children_with_tokens().count(), 3);
        assert_eq!(node2.text().unwrap(), "2.02.12.2");
    }
}

#[test]
fn token_text_eq() {
    let tree = tree_with_eq_tokens();
    let tree = build_syntax_tree::<()>(&tree);
    assert_eq!(tree.kind(), SyntaxKind(0));

    let leaf0_0 = tree.children().next().unwrap().children_with_tokens().next().unwrap();
    let leaf0_0 = leaf0_0.into_token().unwrap();
    let leaf0_1 = tree.children().next().unwrap().children_with_tokens().nth(1).unwrap();
    let leaf0_1 = leaf0_1.into_token().unwrap();

    let leaf1_0 = tree.children().nth(1).unwrap().children_with_tokens().next().unwrap();
    let leaf1_0 = leaf1_0.into_token().unwrap();

    let leaf2_0 = tree.children().nth(2).unwrap().children_with_tokens().next().unwrap();
    let leaf2_0 = leaf2_0.into_token().unwrap();
    let leaf2_1 = tree.children().nth(2).unwrap().children_with_tokens().nth(1).unwrap();
    let leaf2_1 = leaf2_1.into_token().unwrap();
    let leaf2_2 = tree.children().nth(2).unwrap().children_with_tokens().nth(2).unwrap();
    let leaf2_2 = leaf2_2.into_token().unwrap();

    assert!(leaf0_0.text_eq(leaf2_0));
    assert!(leaf0_1.text_eq(leaf2_1));
    assert!(leaf1_0.text_eq(leaf2_2));
    assert!(!leaf0_0.text_eq(leaf0_1));
    assert!(!leaf2_1.text_eq(leaf2_2));
    assert!(!leaf1_0.text_eq(leaf2_0));
}

#[test]
fn data() {
    let tree = two_level_tree();
    let tree = build_syntax_tree::<String>(&tree);
    {
        let node2 = tree.children().nth(2).unwrap();
        assert_eq!(*node2.try_set_data("data".into()).unwrap(), "data");
        let data = node2.get_data().unwrap();
        assert_eq!(data.as_str(), "data");
        node2.set_data("payload".into());
        let data = node2.get_data().unwrap();
        assert_eq!(data.as_str(), "payload");
    }
    {
        let node2 = tree.children().nth(2).unwrap();
        assert!(node2.try_set_data("already present".into()).is_err());
        let data = node2.get_data().unwrap();
        assert_eq!(data.as_str(), "payload");
        node2.set_data("new data".into());
    }
    {
        let node2 = tree.children().nth(2).unwrap();
        let data = node2.get_data().unwrap();
        assert_eq!(data.as_str(), "new data");
        node2.clear_data();
        // re-use `data` after node data was cleared
        assert_eq!(data.as_str(), "new data");
    }
    {
        let node2 = tree.children().nth(2).unwrap();
        assert_eq!(node2.get_data(), None);
    }
}

#[test]
fn inline_text() {
    let tree = two_level_tree();
    let tree: SyntaxNode = SyntaxNode::new_root(build_tree(&tree));
    {
        let leaf1_0 = tree.children().nth(1).unwrap().children_with_tokens().next().unwrap();
        let leaf1_0 = leaf1_0.into_token().unwrap();
        assert_eq!(leaf1_0.text(), Some("1.0"));
        assert_eq!(leaf1_0.text_range(), TextRange::at(6.into(), 3.into()));
        assert_eq!(format!("{leaf1_0}"), "1.0");
        assert_eq!(format!("{leaf1_0:?}"), "SyntaxKind(5)@6..9 \"1.0\"");
    }
    {
        let node2 = tree.children().nth(2).unwrap();
        assert_eq!(node2.text().unwrap(), "2.02.12.2");
        assert_eq!(format!("{node2}").as_str(), "2.02.12.2");
        assert_eq!(format!("{node2:?}"), "SyntaxKind(6)@9..18");
        assert_eq!(
            format!("{node2:#?}"),
            r#"SyntaxKind(6)@9..18
  SyntaxKind(7)@9..12 "2.0"
  SyntaxKind(8)@12..15 "2.1"
  SyntaxKind(9)@15..18 "2.2"
"#
        );
    }
}

#[test]
fn assert_debug_display() {
    use std::fmt;
    fn f<T: fmt::Debug + fmt::Display>() {}

    f::<SyntaxNode>();
    f::<SyntaxToken>();
    f::<SyntaxElement>();
    f::<SyntaxElementRef<'static>>();
    f::<cstree::util::NodeOrToken<String, u128>>();

    fn dbg<T: fmt::Debug>() {}
    dbg::<GreenNodeBuilder<SyntaxKind>>();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
enum ByteKind {
    Root,
    Bytes,
    Plus,
}

impl Syntax for ByteKind {
    fn from_raw(raw: RawSyntaxKind) -> Self {
        match raw.0 {
            0 => Self::Root,
            1 => Self::Bytes,
            2 => Self::Plus,
            _ => panic!("invalid byte kind"),
        }
    }

    fn into_raw(self) -> RawSyntaxKind {
        RawSyntaxKind(self as u32)
    }

    fn static_data(self) -> Option<&'static [u8]> {
        match self {
            Self::Plus => Some(b"+"),
            _ => None,
        }
    }
}

#[test]
fn byte_token_data() {
    let mut builder = GreenNodeBuilder::<ByteKind>::new();
    builder.start_node(ByteKind::Root);
    builder.token(ByteKind::Bytes, b"\xff\x00abc".as_slice());
    builder.static_token(ByteKind::Plus);
    builder.token(ByteKind::Bytes, b"\xff\x00abc".as_slice());
    builder.finish_node();
    let green = builder.finish();
    let root = cstree::syntax::SyntaxNode::<ByteKind>::new_root(green);

    let first = root.first_token().unwrap();
    let plus = first.next_token().unwrap();
    let second = plus.next_token().unwrap();

    assert_eq!(first.data(), b"\xff\x00abc");
    assert_eq!(plus.static_data(), Some(b"+".as_slice()));
    assert_eq!(root.data().len(), 11.into());
    assert_eq!(first.text(), None);
    assert!(first.data_eq(second));

    let chunks = root.data().fold_chunks(Vec::new(), |mut chunks, chunk| {
        chunks.push(chunk.to_vec());
        chunks
    });
    assert_eq!(
        chunks,
        vec![b"\xff\x00abc".to_vec(), b"+".to_vec(), b"\xff\x00abc".to_vec()]
    );
}
