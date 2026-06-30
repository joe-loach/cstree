use super::*;

type GreenNodeBuilder = cstree::build::GreenNodeBuilder<SyntaxKind>;

fn with_builder(f: impl FnOnce(&mut GreenNodeBuilder)) -> SyntaxNode {
    let mut builder = GreenNodeBuilder::new();
    f(&mut builder);
    SyntaxNode::new_root(builder.finish())
}

#[test]
#[should_panic = "`left == right` failed"]
fn comparison_works() {
    let first = with_builder(|_| {});
    let second = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));
        builder.token(SyntaxKind(1), "hi");
        builder.finish_node();
    });
    assert_tree_eq(&first, &second);
}

#[test]
fn no_rollback_token() {
    let first = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));
        builder.token(SyntaxKind(1), "hi");
        builder.finish_node();
    });
    let second = with_builder(|builder| {
        let checkpoint = builder.checkpoint();
        builder.token(SyntaxKind(1), "hi");
        builder.start_node_at(checkpoint, SyntaxKind(0));
        builder.finish_node();
    });
    assert_tree_eq(&first, &second);
}

#[test]
fn no_rollback_node() {
    let first = with_builder(|builder| {
        builder.start_node(SyntaxKind(2));
        builder.start_node(SyntaxKind(0));
        builder.token(SyntaxKind(1), "hi");
        builder.finish_node();
        builder.finish_node();
    });
    let second = with_builder(|builder| {
        let checkpoint = builder.checkpoint();
        builder.start_node(SyntaxKind(0));
        builder.token(SyntaxKind(1), "hi");
        builder.finish_node();
        builder.start_node_at(checkpoint, SyntaxKind(2));
        builder.finish_node();
    });
    assert_tree_eq(&first, &second);
}

#[test]
#[should_panic = "unfinished nodes"]
fn no_rollback_unfinished_node() {
    let second = with_builder(|builder| {
        let checkpoint = builder.checkpoint();
        builder.start_node(SyntaxKind(0));
        builder.token(SyntaxKind(1), "hi");
        builder.start_node_at(checkpoint, SyntaxKind(2));
        builder.finish_node();
        builder.finish_node();
    });
    println!("{}", second.debug(true));
}

#[test]
fn simple() {
    let first = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));
        builder.finish_node();
    });
    let second = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));

        // Add a token, then remove it.
        let initial = builder.checkpoint();
        builder.token(SyntaxKind(1), "hi");
        builder.revert_to(initial);

        builder.finish_node();
    });
    assert_tree_eq(&first, &second);
}

#[test]
fn nested() {
    let first = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));
        builder.finish_node();
    });

    let second = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));
        // Add two tokens, then remove both.
        let initial = builder.checkpoint();
        builder.token(SyntaxKind(1), "hi");
        builder.token(SyntaxKind(2), "hello");
        builder.revert_to(initial);

        builder.finish_node();
    });

    let third = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));

        // Add two tokens, then remove one after the other.
        let initial = builder.checkpoint();
        builder.token(SyntaxKind(1), "hi");
        let second = builder.checkpoint();
        builder.token(SyntaxKind(2), "hello");
        builder.revert_to(second);
        builder.revert_to(initial);

        builder.finish_node();
    });

    assert_tree_eq(&first, &second);
    assert_tree_eq(&first, &third);
}

#[test]
fn unfinished_node() {
    let first = with_builder(|builder| {
        builder.start_node(SyntaxKind(2));
        builder.finish_node();
    });
    let second = with_builder(|builder| {
        builder.start_node(SyntaxKind(2));
        let checkpoint = builder.checkpoint();
        builder.start_node(SyntaxKind(0));
        builder.token(SyntaxKind(1), "hi");
        builder.revert_to(checkpoint);
        builder.finish_node();
    });
    assert_tree_eq(&first, &second);
}

#[test]
#[should_panic = "checkpoint no longer valid after reverting to an earlier checkpoint"]
fn misuse() {
    let first = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));
        builder.finish_node();
    });
    let second = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));

        // Add two tokens, but remove them in the wrong order.
        let initial = builder.checkpoint();
        builder.token(SyntaxKind(1), "hi");
        let new = builder.checkpoint();
        builder.token(SyntaxKind(2), "hello");
        builder.revert_to(initial);
        builder.revert_to(new);

        builder.finish_node();
    });

    assert_tree_eq(&first, &second);
}

#[test]
#[should_panic = "did you already `revert_to`?"]
fn misuse2() {
    with_builder(|builder| {
        builder.start_node(SyntaxKind(0));

        // Take two snapshots across a node boundary, but revert them in the wrong order.
        let initial = builder.checkpoint();
        builder.start_node(SyntaxKind(3));
        builder.token(SyntaxKind(1), "hi");
        let new = builder.checkpoint();
        builder.token(SyntaxKind(2), "hello");
        builder.revert_to(initial);
        builder.revert_to(new);

        builder.finish_node();
    });
}

#[test]
fn misuse3() {
    let first = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));
        builder.token(SyntaxKind(3), "no");
        builder.finish_node();
    });

    let second = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));

        // Add two tokens, revert to the initial state, add three tokens, and try to revert to an earlier checkpoint.
        let initial = builder.checkpoint();
        builder.token(SyntaxKind(1), "hi");
        let new = builder.checkpoint();
        builder.token(SyntaxKind(2), "hello");
        builder.revert_to(initial);

        // This is wrong, but there's not a whole lot the library can do about it.
        builder.token(SyntaxKind(3), "no");
        builder.token(SyntaxKind(4), "bad");
        builder.token(SyntaxKind(4), "wrong");
        builder.revert_to(new);

        builder.finish_node();
    });

    assert_tree_eq(&first, &second);
}

#[test]
#[should_panic = "was `finish_node` called early or did you already `revert_to`"]
fn misuse_combined() {
    with_builder(|builder| {
        builder.start_node(SyntaxKind(0));

        // Take two snapshots across a node boundary, revert to the earlier one but then try to start a node at the
        // later one.
        let initial = builder.checkpoint();
        builder.start_node(SyntaxKind(3));
        builder.token(SyntaxKind(1), "hi");
        let new = builder.checkpoint();
        builder.token(SyntaxKind(2), "hello");
        builder.revert_to(initial);
        builder.start_node_at(new, SyntaxKind(4));

        builder.finish_node();
    });
}

#[test]
#[should_panic = "reverting to an earlier checkpoint"]
fn misuse_combined2() {
    with_builder(|builder| {
        builder.start_node(SyntaxKind(0));

        // Take two snapshots with only tokens between them, revert to the earlier one but then try to start a node at
        // the later one.
        let initial = builder.checkpoint();
        builder.token(SyntaxKind(1), "hi");
        let new = builder.checkpoint();
        builder.token(SyntaxKind(2), "hello");
        builder.revert_to(initial);
        builder.start_node_at(new, SyntaxKind(3));

        builder.finish_node();
    });
}

#[test]
fn revert_then_start() {
    let first = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));
        builder.start_node(SyntaxKind(3));
        builder.token(SyntaxKind(2), "hello");
        builder.finish_node();
        builder.finish_node();
    });
    let second = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));

        // Take two snapshots with only tokens between them, revert to the earlier one but then try to start a node at
        // the later one.
        let initial = builder.checkpoint();
        builder.token(SyntaxKind(1), "hi");
        builder.revert_to(initial);
        builder.start_node_at(initial, SyntaxKind(3));
        builder.token(SyntaxKind(2), "hello");
        builder.finish_node();

        builder.finish_node();
    });
    assert_tree_eq(&first, &second);
}

#[test]
fn start_then_revert() {
    let first = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));
        builder.token(SyntaxKind(2), "hello");
        builder.finish_node();
    });
    let second = with_builder(|builder| {
        builder.start_node(SyntaxKind(0));

        // Take two snapshots with only tokens between them, revert to the earlier one but then try to start a node at
        // the later one.
        let initial = builder.checkpoint();
        builder.token(SyntaxKind(1), "hi");
        builder.start_node_at(initial, SyntaxKind(3));
        builder.revert_to(initial);
        builder.token(SyntaxKind(2), "hello");

        builder.finish_node();
    });
    assert_tree_eq(&first, &second);
}
