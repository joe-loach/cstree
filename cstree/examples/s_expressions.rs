//! In this tutorial, we will write parser and evaluator of arithmetic S-expressions, which look like
//! this:
//! ```
//! (+ (* 15 2) 62)
//! ```
//!
//! You may want to follow the conceptual overview of the design alongside this tutorial:
//! https://github.com/rust-analyzer/rust-analyzer/blob/master/docs/dev/syntax.md

use std::collections::VecDeque;

/// Let's start with defining all kinds of syntactical elements that we want to have in our
/// language's grammar.  These correspond to tokens and composite nodes in the syntax tree that we
/// want to parse the S-expressions into.
use cstree::Syntax;

/// Implementing the `Syntax` trait teaches `cstree` to convert our `SyntaxKind`s to its internal
/// types, allowing for a nicer `SyntaxNode` API where "kinds" are values from our `enum` instead of
/// plain `u32` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Syntax)]
#[repr(u32)]
pub enum SyntaxKind {
    #[static_text("(")]
    LParen, // '('
    #[static_text(")")]
    RParen, // ')'
    Word,       // '+', '15'
    Whitespace, // whitespaces is explicit
    Error,      // as well as errors

    // composite nodes
    List, // `(+ 2 3)`
    Atom, // `+`, `15`, wraps a WORD token
    Root, // top-level node: a list of s-expressions
}

/// When matching against the kind of the node, `SyntaxKind::Kind` is a fine name to use.
/// For specifying generic arguments like `Node<MySyntax>`, we'll use this alias to refer to the
/// syntax as a whole.
type SExprSyntax = SyntaxKind;
use SyntaxKind::*;

/// `GreenNode` is an immutable tree, which stores token bytes but doesn't contain offsets and parent pointers.
use cstree::green::GreenNode;

/// You can construct `GreenNode`s by hand, but a builder is helpful for top-down parsers: it maintains
/// a stack of currently in-progress nodes.
use cstree::build::GreenNodeBuilder;

/// The parse results are stored as a "green tree".
/// We'll discuss how to work with the results later.
struct Parse {
    green_node: GreenNode,
    #[allow(unused)]
    errors: Vec<String>,
}

/// Now, let's write a parser.
/// Note that `parse` does not return a `Result`:
/// By design, syntax trees can be built even for completely invalid source code.
fn parse(text: &str) -> Parse {
    struct Parser<'input> {
        /// input tokens, including whitespace.
        tokens: VecDeque<(SyntaxKind, &'input str)>,
        /// the in-progress green tree.
        builder: GreenNodeBuilder<SExprSyntax>,
        /// the list of syntax errors we've accumulated so far.
        errors: Vec<String>,
    }

    /// The outcome of parsing a single S-expression
    enum SExprResult {
        /// An S-expression (i.e. an atom, or a list) was successfully parsed
        Ok,
        /// Nothing was parsed, as no significant tokens remained
        Eof,
        /// An unexpected ')' was found
        RParen,
    }

    impl Parser<'_> {
        fn parse(mut self) -> Parse {
            // Make sure that the root node covers all source
            self.builder.start_node(Root);
            // Parse zero or more S-expressions
            loop {
                match self.sexp() {
                    SExprResult::Eof => break,
                    SExprResult::RParen => {
                        self.builder.start_node(Error);
                        self.errors.push("unmatched `)`".to_string());
                        self.bump(); // be sure to advance even in case of an error, so as to not get stuck
                        self.builder.finish_node();
                    }
                    SExprResult::Ok => {}
                }
            }
            // Don't forget to eat *trailing* whitespace
            self.skip_ws();
            // Close the root node.
            self.builder.finish_node();

            // Get the green tree from the builder.
            let tree = self.builder.finish();
            Parse {
                green_node: tree,
                errors: self.errors,
            }
        }

        fn list(&mut self) {
            assert_eq!(self.current(), Some(LParen));
            // Start the list node
            self.builder.start_node(List);
            self.bump(); // '('
            loop {
                match self.sexp() {
                    SExprResult::Eof => {
                        self.errors.push("expected `)`".to_string());
                        break;
                    }
                    SExprResult::RParen => {
                        self.bump();
                        break;
                    }
                    SExprResult::Ok => {}
                }
            }
            // close the list node
            self.builder.finish_node();
        }

        fn sexp(&mut self) -> SExprResult {
            // Eat leading whitespace
            self.skip_ws();
            // Either a list, an atom, a closing paren, or an eof.
            let t = match self.current() {
                None => return SExprResult::Eof,
                Some(RParen) => return SExprResult::RParen,
                Some(t) => t,
            };
            match t {
                LParen => self.list(),
                Word => {
                    self.builder.start_node(Atom);
                    self.bump();
                    self.builder.finish_node();
                }
                Error => self.bump(),
                _ => unreachable!(),
            }
            SExprResult::Ok
        }

        /// Advance one token, adding it to the current branch of the tree builder.
        fn bump(&mut self) {
            let (kind, text) = self.tokens.pop_front().unwrap();
            self.builder.token(kind, text.as_bytes());
        }

        /// Peek at the first unprocessed token
        fn current(&self) -> Option<SyntaxKind> {
            self.tokens.front().map(|(kind, _)| *kind)
        }

        fn skip_ws(&mut self) {
            while self.current() == Some(Whitespace) {
                self.bump()
            }
        }
    }

    Parser {
        tokens: lex(text),
        builder: GreenNodeBuilder::new(),
        errors: Vec::new(),
    }
    .parse()
}

/// To work with the parse results we need a view into the green tree - the syntax tree.
/// It is also immutable, like a `GreenNode`, but it contains parent pointers, offsets, and has
/// identity semantics.
type SyntaxNode = cstree::syntax::SyntaxNode<SExprSyntax>;
#[allow(unused)]
type SyntaxToken = cstree::syntax::SyntaxToken<SExprSyntax>;
#[allow(unused)]
type SyntaxElement = cstree::syntax::SyntaxElement<SExprSyntax>;

impl Parse {
    fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green_node.clone())
    }
}

/// Let's check that the parser works as expected
#[test]
fn test_parser() {
    let text = "(+ (* 15 2) 62)";
    let parse = parse(text);
    let node = parse.syntax();
    assert_eq!(
        node.debug(false),
        "Root@0..15", // root node, spanning 15 bytes
    );
    assert_eq!(node.children().count(), 1);
    let list = node.children().next().unwrap();
    let children = list
        .children_with_tokens()
        .map(|child| format!("{:?}@{:?}", child.kind(), child.text_range()))
        .collect::<Vec<_>>();

    assert_eq!(
        children,
        vec![
            "LParen@0..1".to_string(),
            "Atom@1..2".to_string(),
            "Whitespace@2..3".to_string(), // note, explicit whitespace!
            "List@3..11".to_string(),
            "Whitespace@11..12".to_string(),
            "Atom@12..14".to_string(),
            "RParen@14..15".to_string(),
        ]
    );
}

/// So far, we've been working with a homogeneous untyped tree.
/// That tree is nice to provide generic tree operations, like traversals, but it's a bad fit for
/// semantic analysis. cstree itself does not provide AST facilities directly, but it is possible to
/// layer AST on top of `SyntaxNode` API. Let's write a function to evaluate S-expressions.
///
/// For that, let's define AST nodes.
/// It'll be quite a bunch of repetitive code, so we'll use a macro.
/// For a real language, you may want to automatically generate the AST implementations with a task.
mod ast {
    use super::*;
    macro_rules! ast_node {
        ($ast:ident, $kind:ident) => {
            #[derive(PartialEq, Eq, Hash)]
            #[repr(transparent)]
            pub struct $ast(pub(crate) SyntaxNode);
            impl $ast {
                #[allow(unused)]
                pub fn cast(node: SyntaxNode) -> Option<Self> {
                    if node.kind() == SyntaxKind::$kind {
                        Some(Self(node))
                    } else {
                        None
                    }
                }
            }
        };
    }

    ast_node!(Root, Root);
    ast_node!(Atom, Atom);
    ast_node!(List, List);
}

// `SExpr` is slightly different because it can be both an atom and a list, so let's do it by hand.
#[derive(PartialEq, Eq, Hash)]
#[repr(transparent)]
struct SExpr(SyntaxNode);

enum SexpKind {
    Atom(ast::Atom),
    List(ast::List),
}

impl SExpr {
    fn cast(node: SyntaxNode) -> Option<Self> {
        use ast::*;
        if Atom::cast(node.clone()).is_some() || List::cast(node.clone()).is_some() {
            Some(SExpr(node))
        } else {
            None
        }
    }

    fn kind(&self) -> SexpKind {
        use ast::*;
        Atom::cast(self.0.clone())
            .map(SexpKind::Atom)
            .or_else(|| List::cast(self.0.clone()).map(SexpKind::List))
            .unwrap()
    }
}

// Let's enhance AST nodes with ancillary functions and eval.
impl ast::Root {
    fn sexps(&self) -> impl Iterator<Item = SExpr> {
        self.0.children().filter_map(SExpr::cast)
    }
}

enum Op {
    Add,
    Sub,
    Div,
    Mul,
}

impl ast::Atom {
    fn eval(&self) -> Option<i64> {
        self.text().parse().ok()
    }

    fn as_op(&self) -> Option<Op> {
        let op = match self.text() {
            "+" => Op::Add,
            "-" => Op::Sub,
            "*" => Op::Mul,
            "/" => Op::Div,
            _ => return None,
        };
        Some(op)
    }

    fn text(&self) -> &str {
        use cstree::util::NodeOrToken;

        match self.0.green().children().next() {
            Some(NodeOrToken::Token(token)) => SExprSyntax::static_text(SExprSyntax::from_raw(token.kind()))
                .or_else(|| token.text())
                .unwrap(),
            _ => unreachable!(),
        }
    }
}

impl ast::List {
    fn sexps(&self) -> impl Iterator<Item = SExpr> {
        self.0.children().filter_map(SExpr::cast)
    }

    fn eval(&self) -> Option<i64> {
        let op = match self.sexps().next()?.kind() {
            SexpKind::Atom(atom) => atom.as_op()?,
            _ => return None,
        };
        let arg1 = self.sexps().nth(1)?.eval()?;
        let arg2 = self.sexps().nth(2)?.eval()?;
        let res = match op {
            Op::Add => arg1 + arg2,
            Op::Sub => arg1 - arg2,
            Op::Mul => arg1 * arg2,
            Op::Div if arg2 == 0 => return None,
            Op::Div => arg1 / arg2,
        };
        Some(res)
    }
}

impl SExpr {
    fn eval(&self) -> Option<i64> {
        match self.kind() {
            SexpKind::Atom(atom) => atom.eval(),
            SexpKind::List(list) => list.eval(),
        }
    }
}

impl Parse {
    fn root(&self) -> ast::Root {
        ast::Root::cast(self.syntax()).unwrap()
    }
}

/// Let's test the eval!
fn main() {
    let sexps = "
92
(+ 62 30)
(/ 92 0)
nan
(+ (* 15 2) 62)
";
    let parse = parse(sexps);
    let root = parse.root();
    let res = root.sexps().map(|it| it.eval()).collect::<Vec<_>>();
    eprintln!("{res:?}");
    assert_eq!(res, vec![Some(92), Some(92), None, None, Some(92),])
}

/// Split the input string into a flat list of tokens (such as `LParen`, `Word`, and `Whitespace`)
fn lex(text: &str) -> VecDeque<(SyntaxKind, &str)> {
    fn tok(t: SyntaxKind) -> m_lexer::TokenKind {
        m_lexer::TokenKind(t.into_raw().0 as u16)
    }
    fn kind(t: m_lexer::TokenKind) -> SyntaxKind {
        match t.0 {
            0 => LParen,
            1 => RParen,
            2 => Word,
            3 => Whitespace,
            4 => Error,
            _ => unreachable!(),
        }
    }

    let lexer = m_lexer::LexerBuilder::new()
        .error_token(tok(Error))
        .tokens(&[
            (tok(LParen), r"\("),
            (tok(RParen), r"\)"),
            (tok(Word), r"[^\s()]+"),
            (tok(Whitespace), r"\s+"),
        ])
        .build();

    lexer
        .tokenize(text)
        .into_iter()
        .map(|t| (t.len, kind(t.kind)))
        .scan(0usize, |start_offset, (len, kind)| {
            // reconstruct the item's source text from offset and len
            let s = &text[*start_offset..*start_offset + len];
            *start_offset += len;
            Some((kind, s))
        })
        .collect()
}
