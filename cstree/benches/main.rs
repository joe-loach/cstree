use core::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use cstree::{RawSyntaxKind, Syntax, build::*, green::GreenNode};

#[derive(Debug)]
pub enum Element<'s> {
    Node(Vec<Element<'s>>),
    Token(&'s str),
    Plus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestKind {
    Element { n: u32 },
    Plus,
}

impl Syntax for TestKind {
    fn from_raw(raw: RawSyntaxKind) -> Self {
        if raw.0 == u32::MAX - 1 {
            TestKind::Plus
        } else {
            TestKind::Element { n: raw.0 }
        }
    }

    fn into_raw(self) -> RawSyntaxKind {
        match self {
            TestKind::Element { n } => RawSyntaxKind(n),
            TestKind::Plus => RawSyntaxKind(u32::MAX - 1),
        }
    }

    fn static_data(self) -> Option<&'static [u8]> {
        match self {
            TestKind::Plus => Some(b"+"),
            TestKind::Element { .. } => None,
        }
    }
}

pub fn build_tree(root: &Element<'_>, use_static_text: bool) -> GreenNode {
    let mut builder = GreenNodeBuilder::new();
    build_recursive(root, &mut builder, 0, use_static_text);
    builder.finish()
}

pub fn build_recursive(
    root: &Element<'_>,
    builder: &mut GreenNodeBuilder<TestKind>,
    mut from: u32,
    use_static_text: bool,
) -> u32 {
    match root {
        Element::Node(children) => {
            builder.start_node(TestKind::Element { n: from });
            for child in children {
                from = build_recursive(child, builder, from + 1, use_static_text);
            }
            builder.finish_node();
        }
        Element::Token(text) => {
            builder.token(TestKind::Element { n: from }, text);
        }
        Element::Plus if use_static_text => {
            builder.static_token(TestKind::Plus);
        }
        Element::Plus => {
            builder.token(TestKind::Plus, "+");
        }
    }
    from
}

fn two_level_tree() -> Element<'static> {
    use Element::*;
    Node(vec![
        Node(vec![Token("0.0"), Plus, Token("0.1")]),
        Node(vec![Token("1.0")]),
        Node(vec![Token("2.0"), Plus, Token("2.1"), Plus, Token("2.2")]),
    ])
}

pub fn create(c: &mut Criterion) {
    const GROUP_NAME: &str = "two-level tree (inline tokens)";

    let mut group = c.benchmark_group(GROUP_NAME);
    group.throughput(Throughput::Elements(1));

    let tree = two_level_tree();

    group.bench_function("with static text", |b| {
        b.iter(|| {
            let tree = build_tree(&tree, true);
            black_box(tree);
        })
    });

    group.bench_function("without static text", |b| {
        b.iter(|| {
            let tree = build_tree(&tree, false);
            black_box(tree);
        })
    });

    group.finish();
}

criterion_group!(benches, create);
criterion_main!(benches);
