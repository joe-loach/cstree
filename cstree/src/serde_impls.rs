//! Serialization and Deserialization for syntax trees.

use crate::{
    RawSyntaxKind, Syntax, build::GreenNodeBuilder, syntax::SyntaxNode, traversal::WalkEvent, util::NodeOrToken,
};
extern crate alloc;
use alloc::{borrow::Cow, collections::VecDeque, vec::Vec};
use core::{fmt, marker::PhantomData};
use serde::{
    Deserialize, Serialize,
    de::{Error, SeqAccess, Visitor},
    ser::SerializeTuple,
};

/// Expands to the first expression, if there's
/// no expression following, otherwise return the second expression.
///
/// Required for having two different values if the argument is `$(...)?`.
macro_rules! data_list {
    ($_:expr, $list:expr) => {
        $list
    };

    ($list:expr,) => {
        $list
    };
}

/// Generate the code that should be put inside the [`Serialize`] implementation
/// of a [`SyntaxNode`]-like type.
///
/// It serializes a [`SyntaxNode`] into a tuple with 2 elements.
/// The first element is the serialized event stream that was generated
/// by [`SyntaxNode::preorder_with_tokens()`].
/// The second element is a list of `D`s, where `D` is the data of the nodes.
/// The data may only be serialized if it's `Some(data)`. Each `EnterNode` event
/// contains a boolean which indicates if this node has a data. If it has one,
/// the deserializer should pop the first element from the data list and continue.
///
/// Takes the `Syntax` (`$l`), `SyntaxNode` (`$node`), `Serializer` (`$serializer`),
/// and an optional `data_list` which must be a `mut Vec<D>`.
macro_rules! gen_serialize {
    ($l:ident, $node:expr, $ser:ident, $($data_list:ident)?) => {{
        #[allow(unused_variables)]
        let events = $node.preorder_with_tokens().filter_map(|event| match event {
            WalkEvent::Enter(NodeOrToken::Node(node)) => {
                let has_data = false;
                $(let has_data = node
                    .get_data()
                    .map(|data| {
                        $data_list.push(data);
                        true
                    })
                    .unwrap_or(false);)?

                Some(Event::EnterNode($l::into_raw(node.kind()), has_data))
            }
            WalkEvent::Enter(NodeOrToken::Token(tok)) => Some(Event::Token($l::into_raw(tok.kind()), TokenPayload(Cow::Borrowed(tok.data_bytes())))),

            WalkEvent::Leave(NodeOrToken::Node(_)) => Some(Event::LeaveNode),
            WalkEvent::Leave(NodeOrToken::Token(_)) => None,
        });

        let mut tuple = $ser.serialize_tuple(2)?;

        // TODO(Stupremee): We can easily avoid this allocation but it would
        // require more weird and annoying-to-write code, so I'll skip it for now.
        tuple.serialize_element(&events.collect::<Vec<_>>())?;
        tuple.serialize_element(&data_list!(Vec::<()>::new(), $($data_list)?))?;

        tuple.end()
    }};
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "t", content = "c")]
#[serde(bound(deserialize = "'data: 'de"))]
enum Event<'data> {
    /// The second parameter indicates if this node needs data.
    /// If the boolean is true, the next element inside the data list
    /// must be attached to this node.
    EnterNode(RawSyntaxKind, bool),
    Token(RawSyntaxKind, #[serde(borrow)] TokenPayload<'data>),
    LeaveNode,
}

struct TokenPayload<'data>(Cow<'data, [u8]>);

impl Serialize for TokenPayload<'_> {
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::Serializer,
    {
        match core::str::from_utf8(self.0.as_ref()) {
            Ok(text) => serializer.serialize_str(text),
            Err(_) => serializer.serialize_bytes(self.0.as_ref()),
        }
    }
}

impl<'de> Deserialize<'de> for TokenPayload<'de> {
    fn deserialize<De>(deserializer: De) -> Result<Self, De::Error>
    where
        De: serde::Deserializer<'de>,
    {
        struct PayloadVisitor;

        impl<'de> Visitor<'de> for PayloadVisitor {
            type Value = TokenPayload<'de>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or byte sequence")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(TokenPayload(Cow::Owned(value.as_bytes().to_vec())))
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                self.visit_str(value)
            }

            fn visit_string<E>(self, value: alloc::string::String) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(TokenPayload(Cow::Owned(value.into_bytes())))
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(TokenPayload(Cow::Owned(value.to_vec())))
            }

            fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(TokenPayload(Cow::Owned(value)))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut bytes = Vec::new();
                while let Some(byte) = seq.next_element()? {
                    bytes.push(byte);
                }
                Ok(TokenPayload(Cow::Owned(bytes)))
            }
        }

        deserializer.deserialize_any(PayloadVisitor)
    }
}

/// Make a `SyntaxNode` serializable which will include the data for serialization.
pub(crate) struct SerializeWithData<'node, S: Syntax, D: 'static> {
    pub(crate) node: &'node SyntaxNode<S, D>,
}

impl<S, D> Serialize for SerializeWithData<'_, S, D>
where
    S: Syntax,
    D: Serialize,
{
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::Serializer,
    {
        let mut data_list = Vec::new();
        gen_serialize!(S, self.node, serializer, data_list)
    }
}

impl<S, D> Serialize for SyntaxNode<S, D>
where
    S: Syntax,
{
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::Serializer,
    {
        gen_serialize!(S, self, serializer,)
    }
}

impl<'de, S, D> Deserialize<'de> for SyntaxNode<S, D>
where
    S: Syntax,
    D: Deserialize<'de>,
{
    // Deserialization is done by walking down the deserialized event stream,
    // which is the first element inside the tuple. The events
    // are then passed to a `GreenNodeBuilder` which will do all
    // the hard work for use.
    //
    // While walking the event stream, we also store a list of booleans,
    // which indicate which node needs to set data. After creating the tree,
    // we walk down the nodes, check if the bool at `data_list[idx]` is true,
    // and if so, pop the first element of the data list and attach the data
    // to the current node.
    fn deserialize<De>(deserializer: De) -> Result<Self, De::Error>
    where
        De: serde::Deserializer<'de>,
    {
        struct EventVisitor<S: Syntax, D: 'static> {
            _marker: PhantomData<fn() -> SyntaxNode<S, D>>,
        }

        impl<'de, S, D> Visitor<'de> for EventVisitor<S, D>
        where
            S: Syntax,
            D: Deserialize<'de>,
        {
            type Value = (SyntaxNode<S, D>, VecDeque<bool>);

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a list of tree events")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut builder: GreenNodeBuilder<S> = GreenNodeBuilder::new();
                let mut data_indices = VecDeque::new();

                while let Some(next) = seq.next_element::<Event<'_>>()? {
                    match next {
                        Event::EnterNode(kind, has_data) => {
                            builder.start_node(S::from_raw(kind));
                            data_indices.push_back(has_data);
                        }
                        Event::Token(kind, data) => {
                            let kind = S::from_raw(kind);
                            let data = S::data_from_bytes(data.0.as_ref())
                                .ok_or_else(|| A::Error::custom("token payload does not match syntax data type"))?;
                            builder.token(kind, data);
                        }
                        Event::LeaveNode => builder.finish_node(),
                    }
                }

                Ok((SyntaxNode::new_root(builder.finish()), data_indices))
            }
        }

        struct ProcessedEvents<S: Syntax, D: 'static>(SyntaxNode<S, D>, VecDeque<bool>);
        impl<'de, S, D> Deserialize<'de> for ProcessedEvents<S, D>
        where
            S: Syntax,
            D: Deserialize<'de>,
        {
            fn deserialize<DE>(deserializer: DE) -> Result<Self, DE::Error>
            where
                DE: serde::Deserializer<'de>,
            {
                let (tree, ids) = deserializer.deserialize_seq(EventVisitor { _marker: PhantomData })?;
                Ok(Self(tree, ids))
            }
        }

        let (ProcessedEvents(tree, data_indices), mut data) =
            <(ProcessedEvents<S, D>, VecDeque<D>)>::deserialize(deserializer)?;

        tree.descendants().zip(data_indices).try_for_each(|(node, has_data)| {
            if has_data {
                let data = data
                    .pop_front()
                    .ok_or_else(|| De::Error::custom("invalid serialized tree"))?;
                node.set_data(data);
            }
            <Result<(), De::Error>>::Ok(())
        })?;

        if !data.is_empty() {
            Err(De::Error::custom(
                "serialized SyntaxNode contained too many data elements",
            ))
        } else {
            Ok(tree)
        }
    }
}

impl Serialize for RawSyntaxKind {
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for RawSyntaxKind {
    fn deserialize<De>(deserializer: De) -> Result<Self, De::Error>
    where
        De: serde::Deserializer<'de>,
    {
        Ok(Self(u32::deserialize(deserializer)?))
    }
}
