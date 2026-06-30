extern crate alloc;

use alloc::vec::Vec;
use core::hash::{Hash, Hasher};

use rustc_hash::FxHasher;
use text_size::TextSize;

use crate::{
    Syntax,
    green::{GreenElement, GreenNode, GreenToken},
    util::NodeOrToken,
};

/// A checkpoint for maybe wrapping a node. See [`GreenNodeBuilder::checkpoint`] for details.
#[derive(Clone, Copy, Debug)]
pub struct Checkpoint {
    parent_idx: usize,
    child_idx: usize,
}

/// A builder for green trees.
/// Construct with [`new`](GreenNodeBuilder::new). To add tree nodes, start them with
/// [`start_node`](GreenNodeBuilder::start_node), add [`token`](GreenNodeBuilder::token)s and then
/// [`finish_node`](GreenNodeBuilder::finish_node). When the whole tree is constructed, call
/// [`finish`](GreenNodeBuilder::finish) to obtain the root.
#[derive(Debug)]
pub struct GreenNodeBuilder<S: Syntax> {
    parents: Vec<(S, usize)>,
    children: Vec<GreenElement>,
}

impl<S: Syntax> GreenNodeBuilder<S> {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            parents: Vec::with_capacity(8),
            children: Vec::with_capacity(8),
        }
    }

    fn node(&mut self, kind: S, offset: usize) -> GreenNode {
        let kind = S::into_raw(kind);
        let mut hasher = FxHasher::default();
        let mut text_len: TextSize = 0.into();
        for child in &self.children[offset..] {
            text_len += child.text_len();
            child.hash(&mut hasher);
        }
        let child_hash = hasher.finish() as u32;
        GreenNode::new_with_len_and_hash(kind, self.children.drain(offset..), text_len, child_hash)
    }

    fn make_token(kind: S, data: &[u8]) -> GreenToken {
        GreenToken::new(S::into_raw(kind), data)
    }

    /// Add a new token with the given data to the current node.
    ///
    /// ## Panics
    /// In debug mode, if `kind` has static data, this function verifies that `data` matches that data.
    #[inline]
    pub fn token(&mut self, kind: S, data: impl AsRef<[u8]>) {
        let data = data.as_ref();
        let token = match S::static_data(kind) {
            Some(static_data) => {
                debug_assert_eq!(
                    static_data, data,
                    "received `{kind:?}` token with data that does not match its static data"
                );
                Self::make_token(kind, static_data)
            }
            None => Self::make_token(kind, data),
        };
        self.children.push(token.into());
    }

    /// Add a new token from canonical bytes to the current node.
    #[inline]
    pub fn token_from_bytes(&mut self, kind: S, bytes: &[u8]) {
        self.token(kind, bytes);
    }

    /// Add a new token to the current node using its static data.
    ///
    /// ## Panics
    /// If `kind` does not have static data.
    #[inline]
    pub fn static_token(&mut self, kind: S) {
        let static_data = S::static_data(kind).unwrap_or_else(|| panic!("Missing static data for '{kind:?}'"));
        let token = Self::make_token(kind, static_data);
        self.children.push(token.into());
    }

    /// Start new node of the given `kind` and make it current.
    #[inline]
    pub fn start_node(&mut self, kind: S) {
        let len = self.children.len();
        self.parents.push((kind, len));
    }

    /// Finish the current branch and restore the previous branch as current.
    #[inline]
    pub fn finish_node(&mut self) {
        let (kind, first_child) = self.parents.pop().unwrap();
        let node = self.node(kind, first_child);
        self.children.push(node.into());
    }

    /// Take a snapshot of the builder's state, which can be used to retroactively insert surrounding nodes by calling
    /// [`start_node_at`](GreenNodeBuilder::start_node_at), or for backtracking by allowing to
    /// [`revert_to`](GreenNodeBuilder::revert_to) the returned checkpoint.
    #[inline]
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            parent_idx: self.parents.len(),
            child_idx: self.children.len(),
        }
    }

    /// Restore the builder's state to when the [`Checkpoint`] was taken.
    pub fn revert_to(&mut self, checkpoint: Checkpoint) {
        let Checkpoint { parent_idx, child_idx } = checkpoint;
        assert!(
            parent_idx <= self.parents.len(),
            "checkpoint no longer valid, was `finish_node` called early or did you already `revert_to`?"
        );
        assert!(
            child_idx <= self.children.len(),
            "checkpoint no longer valid after reverting to an earlier checkpoint"
        );
        if let Some(&(_, first_child)) = self.parents.last() {
            assert!(
                child_idx >= first_child,
                "checkpoint no longer valid, was an unmatched start_node_at called?"
            );
        }

        self.parents.truncate(parent_idx);
        self.children.truncate(child_idx);
    }

    /// Start a node at the given [`checkpoint`](GreenNodeBuilder::checkpoint), wrapping all nodes
    /// and tokens created since the checkpoint was taken.
    #[inline]
    pub fn start_node_at(&mut self, checkpoint: Checkpoint, kind: S) {
        let Checkpoint { parent_idx, child_idx } = checkpoint;
        assert!(
            parent_idx <= self.parents.len(),
            "checkpoint no longer valid, was `finish_node` called early or did you already `revert_to`?"
        );
        assert!(
            parent_idx >= self.parents.len(),
            "checkpoint contains one or more unfinished nodes"
        );
        assert!(
            child_idx <= self.children.len(),
            "checkpoint no longer valid after reverting to an earlier checkpoint"
        );
        if let Some(&(_, first_child)) = self.parents.last() {
            assert!(
                child_idx >= first_child,
                "checkpoint no longer valid, was an unmatched `start_node` or `start_node_at` called?"
            );
        }

        self.parents.push((kind, child_idx));
    }

    /// Complete building the tree.
    #[inline]
    pub fn finish(mut self) -> GreenNode {
        assert_eq!(self.children.len(), 1);
        match self.children.pop().unwrap() {
            NodeOrToken::Node(node) => node,
            NodeOrToken::Token(_) => panic!("called `finish` on a `GreenNodeBuilder` which only contained a token"),
        }
    }
}

impl<S: Syntax> Default for GreenNodeBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}
