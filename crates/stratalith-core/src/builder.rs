//! Declarative document construction through the validated insertion APIs.
//!
//! Capture a list item's ID and the ID of an inline node inside it.
//!
//! ```
//! use stratalith_core::builder::{block, document, paragraph, strong, text};
//! use stratalith_core::{BlockKind, InlineKind, ListKind, NodeKind};
//!
//! let mut item = None;
//! let mut word = None;
//!
//! let doc = document([block(
//!     BlockKind::List { kind: ListKind::Bullet, tight: true },
//!     [block(BlockKind::ListItem, [paragraph([
//!         text("Buy "),
//!         strong([text("milk").capture(&mut word)]),
//!     ])]).capture(&mut item)],
//! )])?;
//!
//! let item = doc.node(item.unwrap()).unwrap();
//! let word = doc.node(word.unwrap()).unwrap();
//! assert_eq!(item.parent(), Some(doc.blocks()[0]));
//! assert_eq!(word.kind(), &NodeKind::Inline(InlineKind::Text("milk".into())));
//! # Ok::<(), stratalith_core::DocumentError>(())
//! ```

use crate::{BlockKind, Document, DocumentError, InlineKind, NodeId, NodeKind};

/// Describes a block and its children for [`document`].
///
/// See the [module example](crate::builder) for [`Self::capture`].
pub struct BlockBuilder<'a>(NodeBuilder<'a, BlockKind>);

/// Describes an inline node and its children for [`document`].
///
/// See the [module example](crate::builder) for [`Self::capture`].
pub struct InlineBuilder<'a>(NodeBuilder<'a, InlineKind>);

struct NodeBuilder<'a, Kind> {
    kind: Kind,
    children: Vec<NodeBuilder<'a, NodeKind>>,
    capture: Option<&'a mut Option<NodeId>>,
}

impl<'a, Kind> NodeBuilder<'a, Kind> {
    fn new(kind: Kind, children: impl Iterator<Item = NodeBuilder<'a, NodeKind>>) -> Self {
        Self {
            kind,
            children: children.collect(),
            capture: None,
        }
    }

    fn map_kind<Mapped>(self, map: impl FnOnce(Kind) -> Mapped) -> NodeBuilder<'a, Mapped> {
        NodeBuilder {
            kind: map(self.kind),
            children: self.children,
            capture: self.capture,
        }
    }
}

impl<'a> BlockBuilder<'a> {
    /// Writes this block's ID to `output` only if the whole document builds.
    pub fn capture(mut self, output: &'a mut Option<NodeId>) -> Self {
        self.0.capture = Some(output);
        self
    }
}

impl<'a> InlineBuilder<'a> {
    /// Writes this inline's ID to `output` only if the whole document builds.
    pub fn capture(mut self, output: &'a mut Option<NodeId>) -> Self {
        self.0.capture = Some(output);
        self
    }
}

/// Pass `[]` for a leaf block, such as a thematic break.
pub fn block<'a>(
    kind: BlockKind,
    children: impl IntoIterator<Item = BlockBuilder<'a>>,
) -> BlockBuilder<'a> {
    BlockBuilder(NodeBuilder::new(
        kind,
        children
            .into_iter()
            .map(|child| child.0.map_kind(NodeKind::Block)),
    ))
}

pub fn text_block<'a>(
    kind: BlockKind,
    children: impl IntoIterator<Item = InlineBuilder<'a>>,
) -> BlockBuilder<'a> {
    BlockBuilder(NodeBuilder::new(
        kind,
        children
            .into_iter()
            .map(|child| child.0.map_kind(NodeKind::Inline)),
    ))
}

/// Pass `[]` for a leaf inline, such as a hard break.
pub fn inline<'a>(
    kind: InlineKind,
    children: impl IntoIterator<Item = InlineBuilder<'a>>,
) -> InlineBuilder<'a> {
    InlineBuilder(NodeBuilder::new(
        kind,
        children
            .into_iter()
            .map(|child| child.0.map_kind(NodeKind::Inline)),
    ))
}

pub fn paragraph<'a>(children: impl IntoIterator<Item = InlineBuilder<'a>>) -> BlockBuilder<'a> {
    text_block(BlockKind::Paragraph, children)
}

pub fn strong<'a>(children: impl IntoIterator<Item = InlineBuilder<'a>>) -> InlineBuilder<'a> {
    inline(InlineKind::Strong, children)
}

pub fn emphasis<'a>(children: impl IntoIterator<Item = InlineBuilder<'a>>) -> InlineBuilder<'a> {
    inline(InlineKind::Emphasis, children)
}

/// Empty strings produce a node, and adjacent text nodes stay separate.
pub fn text<'a>(value: impl Into<String>) -> InlineBuilder<'a> {
    inline(InlineKind::Text(value.into()), [])
}

/// Builds a new document in the supplied order without normalizing its nodes.
///
/// Checks parent-child relationships and nested links during insertion.
/// On failure, captured ID outputs are unchanged.
pub fn document<'a>(
    blocks: impl IntoIterator<Item = BlockBuilder<'a>>,
) -> Result<Document, DocumentError> {
    let mut build = Build {
        document: Document::new(),
        captures: Vec::new(),
    };
    build.insert_nodes(
        blocks.into_iter().map(|block| block.0),
        Document::insert_block,
    )?;
    for (output, id) in build.captures {
        *output = Some(id);
    }
    Ok(build.document)
}

struct Build<'a> {
    document: Document,
    captures: Vec<(&'a mut Option<NodeId>, NodeId)>,
}

impl<'a> Build<'a> {
    fn insert_nodes<Kind>(
        &mut self,
        nodes: impl IntoIterator<Item = NodeBuilder<'a, Kind>>,
        insert: impl Fn(&mut Document, usize, Kind) -> Result<NodeId, DocumentError>,
    ) -> Result<(), DocumentError> {
        for (index, node) in nodes.into_iter().enumerate() {
            let id = insert(&mut self.document, index, node.kind)?;
            if let Some(output) = node.capture {
                self.captures.push((output, id));
            }
            self.insert_children(id, node.children)?;
        }
        Ok(())
    }

    fn insert_children(
        &mut self,
        parent: NodeId,
        children: Vec<NodeBuilder<'a, NodeKind>>,
    ) -> Result<(), DocumentError> {
        self.insert_nodes(children, |document, index, kind| match kind {
            NodeKind::Block(kind) => document.insert_child_block(parent, index, kind),
            NodeKind::Inline(kind) => document.insert_inline(parent, index, kind),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ListKind;

    #[test]
    fn builds_nested_blocks_and_preserves_inline_order_and_captured_ids() {
        let mut item = None;
        let mut empty = None;
        let mut word = None;
        let tree = [block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [block(
                BlockKind::ListItem,
                [paragraph([
                    text("").capture(&mut empty),
                    strong([emphasis([text("€😀").capture(&mut word)])]),
                ])],
            )
            .capture(&mut item)],
        )];

        let doc = document(tree).unwrap();

        let list = doc.blocks()[0];
        let item = item.unwrap();
        let empty = empty.unwrap();
        let word = word.unwrap();
        assert_eq!(doc.node(list).unwrap().children(), &[item]);
        assert_eq!(doc.node(item).unwrap().parent(), Some(list));
        assert_eq!(
            doc.node(empty).unwrap().kind(),
            &NodeKind::Inline(InlineKind::Text(String::new()))
        );
        assert_eq!(
            doc.node(word).unwrap().kind(),
            &NodeKind::Inline(InlineKind::Text("€😀".into()))
        );
        let ids: Vec<_> = doc.nodes().map(|node| node.id()).collect();
        assert_eq!(ids[3], empty);
        assert_eq!(ids[6], word);
        assert_eq!(ids.len(), 7);
    }

    #[test]
    fn invalid_descendant_leaves_all_capture_outputs_unchanged() {
        let original = document([paragraph([])]).unwrap();
        let existing = original.blocks()[0];
        let (mut root, mut child) = (None, Some(existing));
        let tree = [paragraph([
            inline(InlineKind::Text("leaf".into()), [text("invalid")]).capture(&mut child)
        ])
        .capture(&mut root)];

        let result = document(tree);

        assert!(matches!(result, Err(DocumentError::CannotHaveChildren(_))));
        assert_eq!(root, None);
        assert_eq!(child, Some(existing));
    }

    #[test]
    fn nested_links_are_rejected_through_formatting() {
        let link = || InlineKind::Link {
            url: "/".into(),
            title: None,
        };
        let tree = [paragraph([inline(
            link(),
            [strong([inline(link(), [text("nested")])])],
        )])];

        let result = document(tree);

        assert!(matches!(result, Err(DocumentError::InvalidChild { .. })));
    }
}
