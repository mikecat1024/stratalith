mod arena;
mod edit;
mod extract;

use crate::{BlockKind, InlineKind, NodeKind};
use arena::NodeArena;
pub use arena::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentError {
    InvalidIndex { index: usize, len: usize },
    NodeNotFound(NodeId),
    CannotHaveChildren(NodeId),
    InvalidChild { parent: NodeId },
    ListItemNeedsList,
}

impl std::fmt::Display for DocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIndex { index, len } => write!(f, "index {index} exceeds length {len}"),
            Self::NodeNotFound(id) => write!(f, "node {id:?} was not found"),
            Self::CannotHaveChildren(id) => write!(f, "node {id:?} cannot have children"),
            Self::InvalidChild { parent } => write!(f, "incompatible child for node {parent:?}"),
            Self::ListItemNeedsList => write!(f, "a list item must be a child of a list"),
        }
    }
}

impl std::error::Error for DocumentError {}

/// Owns nodes in an arena; roots and parent-child relationships use node IDs.
///
/// ```
/// use stratalith_core::builder::{document, paragraph, strong, text};
///
/// let (mut bold, mut content) = (None, None);
///
/// let doc = document([paragraph([
///     strong([text("Hello").capture(&mut content)]).capture(&mut bold),
/// ])]).unwrap();
///
/// assert_eq!(doc.node(bold.unwrap()).unwrap().children(), &[content.unwrap()]);
/// assert_eq!(doc.nodes().count(), 3);
/// ```
///
/// Parent-child relationships cannot be changed through a borrowed node:
///
/// ```compile_fail
/// use stratalith_core::builder::{document, paragraph};
/// let doc = document([paragraph([])]).unwrap();
/// let id = doc.blocks()[0];
/// doc.node(id).unwrap().children.push(id);
/// ```
pub struct Document {
    blocks: Vec<NodeId>,
    arena: NodeArena,
}

#[cfg(test)]
impl Document {
    // Preorder and child counts describe the tree independently of allocation.
    fn tree_nodes(&self) -> impl Iterator<Item = (&NodeKind, usize)> {
        self.nodes()
            .map(|node| (node.kind(), node.children().len()))
    }
}

#[cfg(test)]
impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.tree_nodes().eq(other.tree_nodes())
    }
}

#[cfg(test)]
impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.tree_nodes()).finish()
    }
}

/// A stored node. Only Document can create nodes or change their relationships.
#[derive(Debug, Clone)]
pub struct Node {
    id: NodeId,
    parent: Option<NodeId>,
    kind: NodeKind,
    children: Vec<NodeId>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DocumentNodes<'a> {
    document: &'a Document,
    stack: Vec<NodeId>,
}

impl<'a> Iterator for DocumentNodes<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let node = self
            .document
            .node(id)
            .expect("linked nodes exist in the arena");
        self.stack.extend(node.children.iter().rev().copied());
        Some(node)
    }
}

impl Document {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            arena: NodeArena::default(),
        }
    }

    /// IDs of top-level blocks in document order.
    pub fn blocks(&self) -> &[NodeId] {
        &self.blocks
    }

    /// Looks up a live node in this document. Deleted IDs return None.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.arena.get(id)
    }

    /// Inserts a new top-level block with a fresh ID.
    pub fn insert_block(&mut self, index: usize, kind: BlockKind) -> Result<NodeId, DocumentError> {
        check_index(index, self.blocks.len())?;
        if matches!(kind, BlockKind::ListItem) {
            return Err(DocumentError::ListItemNeedsList);
        }
        let id = self.allocate(None, NodeKind::Block(kind));
        self.blocks.insert(index, id);
        Ok(id)
    }

    /// Inserts a block into a quote or list item, or a list item into a list.
    /// On error, neither the document nor its ID allocation state is changed.
    ///
    /// ```
    /// use stratalith_core::builder::{block, document};
    /// use stratalith_core::{BlockKind, ListKind};
    ///
    /// let kind = BlockKind::List { kind: ListKind::Ordered { start: 3 }, tight: true };
    /// let mut doc = document([block(kind, [])]).unwrap();
    /// let list = doc.blocks()[0];
    ///
    /// let item = doc.insert_child_block(list, 0, BlockKind::ListItem).unwrap();
    /// let paragraph = doc.insert_child_block(item, 0, BlockKind::Paragraph).unwrap();
    ///
    /// assert_eq!(doc.node(item).unwrap().parent(), Some(list));
    /// assert_eq!(doc.node(paragraph).unwrap().parent(), Some(item));
    /// ```
    pub fn insert_child_block(
        &mut self,
        parent: NodeId,
        index: usize,
        kind: BlockKind,
    ) -> Result<NodeId, DocumentError> {
        self.insert_child(parent, index, NodeKind::Block(kind))
    }

    /// Inserts a new inline child into a paragraph, heading, or inline container.
    /// Links cannot contain other links, except within image descriptions.
    /// On error, neither the document nor its ID allocation state is changed.
    pub fn insert_inline(
        &mut self,
        parent: NodeId,
        index: usize,
        kind: InlineKind,
    ) -> Result<NodeId, DocumentError> {
        self.insert_child(parent, index, NodeKind::Inline(kind))
    }

    fn insert_child(
        &mut self,
        parent: NodeId,
        index: usize,
        kind: NodeKind,
    ) -> Result<NodeId, DocumentError> {
        let node = self
            .node(parent)
            .ok_or(DocumentError::NodeNotFound(parent))?;
        if !node.kind.can_have_children() {
            return Err(DocumentError::CannotHaveChildren(parent));
        }
        if !node.kind.accepts(&kind) {
            return Err(DocumentError::InvalidChild { parent });
        }
        if matches!(kind, NodeKind::Inline(InlineKind::Link { .. })) {
            self.ensure_no_enclosing_link(parent)?;
        }
        check_index(index, node.children.len())?;
        let id = self.allocate(Some(parent), kind);
        self.arena
            .get_mut(parent)
            .expect("parent exists")
            .children
            .insert(index, id);
        Ok(id)
    }

    fn ensure_no_enclosing_link(&self, parent: NodeId) -> Result<(), DocumentError> {
        let mut ancestor = Some(parent);
        while let Some(id) = ancestor {
            let node = self.node(id).expect("linked ancestors exist in the arena");
            match node.kind() {
                NodeKind::Inline(InlineKind::Link { .. }) => {
                    return Err(DocumentError::InvalidChild { parent });
                }
                // Image descriptions allow links even when the image is inside a link.
                NodeKind::Inline(InlineKind::Image { .. }) => break,
                NodeKind::Block(_) => break,
                NodeKind::Inline(
                    InlineKind::Text(_)
                    | InlineKind::Code(_)
                    | InlineKind::SoftBreak
                    | InlineKind::HardBreak
                    | InlineKind::Html(_)
                    | InlineKind::Strong
                    | InlineKind::Emphasis,
                ) => ancestor = node.parent(),
            }
        }
        Ok(())
    }

    /// Removes a node and all its descendants, without reusing their IDs.
    pub fn remove_node(&mut self, id: NodeId) -> Result<(), DocumentError> {
        let node = self.node(id).ok_or(DocumentError::NodeNotFound(id))?;
        match node.parent {
            Some(parent) => self
                .arena
                .get_mut(parent)
                .expect("parent exists")
                .children
                .retain(|child| *child != id),
            None => self.blocks.retain(|block| *block != id),
        }
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            let node = self
                .arena
                .remove(id)
                .expect("linked nodes exist in the arena");
            stack.extend(node.children);
        }
        Ok(())
    }

    /// Visits all nodes in document order using a depth-first preorder walk.
    pub fn nodes(&self) -> DocumentNodes<'_> {
        DocumentNodes {
            document: self,
            stack: self.blocks.iter().rev().copied().collect(),
        }
    }

    // Copies a node from an already valid tree into a matching ancestry.
    fn append_copy(&mut self, parent: Option<NodeId>, kind: NodeKind) -> NodeId {
        let id = self.allocate(parent, kind);
        match parent {
            Some(parent) => self.arena.get_mut(parent).unwrap().children.push(id),
            None => self.blocks.push(id),
        }
        id
    }

    fn allocate(&mut self, parent: Option<NodeId>, kind: NodeKind) -> NodeId {
        self.arena.insert_with_key(|id| Node {
            id,
            parent,
            kind,
            children: Vec::new(),
        })
    }
}

fn check_index(index: usize, len: usize) -> Result<(), DocumentError> {
    if index > len {
        return Err(DocumentError::InvalidIndex { index, len });
    }
    Ok(())
}

impl Node {
    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    pub fn kind(&self) -> &NodeKind {
        &self.kind
    }

    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{
        block, document as build_document, emphasis, inline, paragraph, strong, text, text_block,
    };
    use crate::{HeadingLevel, ListKind};
    use std::collections::HashSet;

    fn nested_document() -> Document {
        build_document([
            paragraph([strong([text("Hello "), emphasis([text("world")])])]),
            paragraph([text("Next paragraph")]),
        ])
        .unwrap()
    }

    fn assert_tree_integrity(document: &Document) {
        let nodes: Vec<_> = document.nodes().collect();
        let ids: HashSet<_> = nodes.iter().map(|node| node.id()).collect();
        assert_eq!(nodes.len(), ids.len());
        assert_eq!(nodes.len(), document.arena.len());
        for node in nodes {
            assert_eq!(document.node(node.id()).unwrap().id(), node.id());
            for child in node.children() {
                assert_eq!(document.node(*child).unwrap().parent(), Some(node.id()));
            }
        }
        for root in document.blocks() {
            let node = document.node(*root).unwrap();
            assert_eq!(node.parent(), None);
            assert!(matches!(node.kind(), NodeKind::Block(_)));
        }
    }

    #[test]
    fn arena_resolves_nodes_and_parent_child_links() {
        let mut document = build_document([paragraph([])]).unwrap();
        let paragraph = document.blocks()[0];

        let text = document
            .insert_inline(paragraph, 0, InlineKind::Text("Hello".into()))
            .unwrap();

        assert_eq!(document.blocks(), &[paragraph]);
        assert_eq!(document.node(paragraph).unwrap().children(), &[text]);
        assert_eq!(document.node(text).unwrap().parent(), Some(paragraph));
        assert_eq!(
            document.node(text).unwrap().kind(),
            &NodeKind::Inline(InlineKind::Text("Hello".into()))
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn walk_visits_nested_nodes_in_document_order() {
        let document = nested_document();

        let kinds: Vec<_> = document.nodes().map(|node| node.kind().clone()).collect();

        assert_eq!(
            kinds,
            [
                NodeKind::Block(BlockKind::Paragraph),
                NodeKind::Inline(InlineKind::Strong),
                NodeKind::Inline(InlineKind::Text("Hello ".into())),
                NodeKind::Inline(InlineKind::Emphasis),
                NodeKind::Inline(InlineKind::Text("world".into())),
                NodeKind::Block(BlockKind::Paragraph),
                NodeKind::Inline(InlineKind::Text("Next paragraph".into())),
            ]
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn walk_follows_document_order_after_insertion() {
        let mut document = build_document([paragraph([])]).unwrap();
        let later = document.blocks()[0];
        let first = document.insert_block(0, BlockKind::Paragraph).unwrap();
        let last_text = document
            .insert_inline(first, 0, InlineKind::Text("last".into()))
            .unwrap();

        let first_text = document
            .insert_inline(first, 0, InlineKind::Text("first".into()))
            .unwrap();

        assert_eq!(
            document.nodes().map(Node::id).collect::<Vec<_>>(),
            [first, first_text, last_text, later]
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn removing_inline_deletes_descendants_and_unlinks_parent() {
        let mut document = nested_document();
        let paragraph = document.blocks()[0];
        let removed: Vec<_> = document.nodes().skip(1).take(4).map(Node::id).collect();

        document.remove_node(removed[0]).unwrap();

        assert!(removed.iter().all(|id| document.node(*id).is_none()));
        assert!(document.node(paragraph).unwrap().children().is_empty());
        assert_eq!(document.nodes().count(), 3);
        assert_tree_integrity(&document);
    }

    #[test]
    fn removing_block_deletes_descendants_without_reusing_ids() {
        let mut document = nested_document();
        let removed: Vec<_> = document.nodes().take(5).map(Node::id).collect();
        let survivor = document.blocks()[1];

        document.remove_node(removed[0]).unwrap();
        let new = document.insert_block(0, BlockKind::Paragraph).unwrap();

        assert!(removed.iter().all(|id| document.node(*id).is_none()));
        assert!(!removed.contains(&new));
        assert_eq!(document.blocks(), &[new, survivor]);
        assert_tree_integrity(&document);
    }

    #[test]
    fn repeated_replacement_reuses_capacity_and_invalidates_old_ids() {
        let mut document = build_document([paragraph([])]).unwrap();
        let mut current = document.blocks()[0];
        let capacity = document.arena.capacity();
        let mut deleted = Vec::new();

        for _ in 0..512 {
            document.remove_node(current).unwrap();
            deleted.push(current);
            current = document.insert_block(0, BlockKind::Paragraph).unwrap();
        }

        assert_eq!(document.arena.capacity(), capacity);
        assert_eq!(document.arena.len(), 1);
        assert!(deleted.iter().all(|id| document.node(*id).is_none()));
        assert!(!deleted.contains(&current));
        assert_eq!(document.blocks(), &[current]);
        assert_tree_integrity(&document);
    }

    #[test]
    fn stale_id_cannot_remove_replacement_node() {
        let mut document = build_document([paragraph([])]).unwrap();
        let deleted = document.blocks()[0];
        document.remove_node(deleted).unwrap();
        let replacement = document.insert_block(0, BlockKind::Paragraph).unwrap();

        let result = document.remove_node(deleted);

        assert_eq!(result, Err(DocumentError::NodeNotFound(deleted)));
        assert_eq!(document.blocks(), &[replacement]);
        assert!(document.node(replacement).is_some());
        assert_tree_integrity(&document);
    }

    #[test]
    fn deleted_parent_cannot_receive_children_after_replacement() {
        let mut document = build_document([paragraph([])]).unwrap();
        let deleted = document.blocks()[0];
        document.remove_node(deleted).unwrap();
        let replacement = document.insert_block(0, BlockKind::Paragraph).unwrap();

        let result = document.insert_inline(deleted, 0, InlineKind::Strong);

        assert_eq!(result, Err(DocumentError::NodeNotFound(deleted)));
        assert!(document.node(replacement).unwrap().children().is_empty());
        assert_tree_integrity(&document);
    }

    #[test]
    fn code_block_rejects_children_without_allocating() {
        let kind = BlockKind::CodeBlock {
            language: None,
            code: "fn main() {}".into(),
        };
        let mut document = build_document([block(kind, [])]).unwrap();
        let code = document.blocks()[0];
        let node_count = document.arena.len();

        let result = document.insert_inline(code, 0, InlineKind::Text("invalid".into()));

        assert_eq!(result, Err(DocumentError::CannotHaveChildren(code)));
        assert_eq!(document.arena.len(), node_count);
        assert_tree_integrity(&document);
    }

    #[test]
    fn text_rejects_children_without_allocating() {
        let mut leaf = None;
        let mut document = build_document([paragraph([text("leaf").capture(&mut leaf)])]).unwrap();
        let leaf = leaf.unwrap();
        let node_count = document.arena.len();

        let result = document.insert_inline(leaf, 0, InlineKind::Strong);

        assert_eq!(result, Err(DocumentError::CannotHaveChildren(leaf)));
        assert_eq!(document.arena.len(), node_count);
        assert_tree_integrity(&document);
    }

    #[test]
    fn deleted_parent_rejects_insertion_without_allocating() {
        let mut document = build_document([paragraph([])]).unwrap();
        let parent = document.blocks()[0];
        document.remove_node(parent).unwrap();
        let node_count = document.arena.len();

        let result = document.insert_inline(parent, 0, InlineKind::Strong);

        assert_eq!(result, Err(DocumentError::NodeNotFound(parent)));
        assert_eq!(document.arena.len(), node_count);
        assert_tree_integrity(&document);
    }

    #[test]
    fn invalid_block_index_does_not_allocate() {
        let mut document = Document::new();

        let result = document.insert_block(1, BlockKind::Paragraph);

        assert_eq!(
            result,
            Err(DocumentError::InvalidIndex { index: 1, len: 0 })
        );
        assert!(document.arena.is_empty());
        assert_tree_integrity(&document);
    }

    #[test]
    fn invalid_inline_index_does_not_allocate() {
        let mut document = build_document([paragraph([])]).unwrap();
        let parent = document.blocks()[0];

        let result = document.insert_inline(parent, 1, InlineKind::Strong);

        assert_eq!(
            result,
            Err(DocumentError::InvalidIndex { index: 1, len: 0 })
        );
        assert_eq!(document.arena.len(), 1);
        assert_tree_integrity(&document);
    }

    #[test]
    fn removing_deleted_node_leaves_document_unchanged() {
        let mut document = nested_document();
        let root = document.blocks()[0];
        document.remove_node(root).unwrap();
        let remaining: Vec<_> = document.nodes().map(Node::id).collect();

        let result = document.remove_node(root);

        assert_eq!(result, Err(DocumentError::NodeNotFound(root)));
        assert_eq!(
            document.nodes().map(Node::id).collect::<Vec<_>>(),
            remaining
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn heading_link_and_unicode_text_are_preserved() {
        let (mut link, mut content) = (None, None);
        let heading = BlockKind::Heading {
            level: HeadingLevel::H2,
        };
        let link_kind = InlineKind::Link {
            url: "https://example.com".into(),
            title: Some("Example".into()),
        };

        let document = build_document([text_block(
            heading.clone(),
            [
                inline(link_kind.clone(), [text("Hello🦀").capture(&mut content)])
                    .capture(&mut link),
            ],
        )])
        .unwrap();

        assert_eq!(
            document.node(document.blocks()[0]).unwrap().kind(),
            &NodeKind::Block(heading)
        );
        assert_eq!(
            document.node(link.unwrap()).unwrap().kind(),
            &NodeKind::Inline(link_kind)
        );
        assert_eq!(
            document.node(content.unwrap()).unwrap().kind(),
            &NodeKind::Inline(InlineKind::Text("Hello🦀".into()))
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn code_block_does_not_end_walk_before_later_blocks() {
        let mut content = None;
        let document = build_document([
            block(
                BlockKind::CodeBlock {
                    language: None,
                    code: "code".into(),
                },
                [],
            ),
            paragraph([text("Next").capture(&mut content)]),
        ])
        .unwrap();
        let (code, paragraph) = (document.blocks()[0], document.blocks()[1]);

        let ids: Vec<_> = document.nodes().map(Node::id).collect();

        assert_eq!(ids, [code, paragraph, content.unwrap()]);
        assert_tree_integrity(&document);
    }

    #[test]
    fn walk_visits_quotes_and_nested_lists_in_document_order() {
        let (mut list, mut item, mut first, mut content, mut next) = (None, None, None, None, None);
        let (mut nested, mut nested_item, mut nested_quote, mut code) = (None, None, None, None);
        let list_kind = BlockKind::List {
            kind: ListKind::Ordered { start: 3 },
            tight: false,
        };
        let nested_kind = BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        };
        let quoted_code = block(
            BlockKind::BlockQuote,
            [block(
                BlockKind::CodeBlock {
                    language: Some("rust".into()),
                    code: "let x = 1;".into(),
                },
                [],
            )
            .capture(&mut code)],
        )
        .capture(&mut nested_quote);
        let nested_list = block(
            nested_kind.clone(),
            [block(BlockKind::ListItem, [quoted_code]).capture(&mut nested_item)],
        )
        .capture(&mut nested);
        let list_tree = block(
            list_kind.clone(),
            [
                block(
                    BlockKind::ListItem,
                    [
                        paragraph([text("first").capture(&mut content)]).capture(&mut first),
                        nested_list,
                    ],
                )
                .capture(&mut item),
                block(BlockKind::ListItem, []).capture(&mut next),
            ],
        )
        .capture(&mut list);
        let document = build_document([
            block(BlockKind::BlockQuote, [list_tree]),
            block(BlockKind::ThematicBreak, []),
        ])
        .unwrap();
        let (quote, last) = (document.blocks()[0], document.blocks()[1]);

        let ids: Vec<_> = document.nodes().map(Node::id).collect();

        assert_eq!(
            ids,
            [
                quote,
                list.unwrap(),
                item.unwrap(),
                first.unwrap(),
                content.unwrap(),
                nested.unwrap(),
                nested_item.unwrap(),
                nested_quote.unwrap(),
                code.unwrap(),
                next.unwrap(),
                last
            ]
        );
        assert_eq!(document.blocks(), &[quote, last]);
        assert_eq!(
            document.node(list.unwrap()).unwrap().kind(),
            &NodeKind::Block(list_kind)
        );
        assert_eq!(
            document.node(nested.unwrap()).unwrap().kind(),
            &NodeKind::Block(nested_kind)
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn removing_nested_block_deletes_subtree_and_unlinks_container() {
        let (mut nested, mut removed_paragraph, mut content, mut survivor) =
            (None, None, None, None);
        let mut document = build_document([block(
            BlockKind::BlockQuote,
            [
                block(
                    BlockKind::BlockQuote,
                    [paragraph([text("nested").capture(&mut content)])
                        .capture(&mut removed_paragraph)],
                )
                .capture(&mut nested),
                paragraph([]).capture(&mut survivor),
            ],
        )])
        .unwrap();
        let quote = document.blocks()[0];
        let (nested, survivor) = (nested.unwrap(), survivor.unwrap());

        document.remove_node(nested).unwrap();

        assert_eq!(document.node(quote).unwrap().children(), &[survivor]);
        assert!(document.node(nested).is_none());
        assert!(document.node(removed_paragraph.unwrap()).is_none());
        assert!(document.node(content.unwrap()).is_none());
        assert_eq!(
            document.nodes().map(Node::id).collect::<Vec<_>>(),
            [quote, survivor]
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn incompatible_children_are_rejected_without_changing_document() {
        let cases = [
            (BlockKind::BlockQuote, NodeKind::Inline(InlineKind::Strong)),
            (BlockKind::BlockQuote, NodeKind::Block(BlockKind::ListItem)),
            (
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                NodeKind::Block(BlockKind::Paragraph),
            ),
            (
                BlockKind::List {
                    kind: ListKind::Ordered { start: 0 },
                    tight: false,
                },
                NodeKind::Inline(InlineKind::Strong),
            ),
            (BlockKind::Paragraph, NodeKind::Block(BlockKind::BlockQuote)),
            (
                BlockKind::Heading {
                    level: HeadingLevel::H6,
                },
                NodeKind::Block(BlockKind::Paragraph),
            ),
        ];
        for (parent_kind, child_kind) in cases {
            let mut parent = None;
            let mut document = build_document([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(
                    BlockKind::ListItem,
                    [block(parent_kind, []).capture(&mut parent)],
                )],
            )])
            .unwrap();
            let parent = parent.unwrap();
            let before: Vec<_> = document.nodes().map(Node::id).collect();
            let capacity = document.arena.capacity();

            let result = match child_kind {
                NodeKind::Block(kind) => document.insert_child_block(parent, 0, kind),
                NodeKind::Inline(kind) => document.insert_inline(parent, 0, kind),
            };

            assert_eq!(result, Err(DocumentError::InvalidChild { parent }));
            assert!(document.node(parent).unwrap().children().is_empty());
            assert_eq!(document.nodes().map(Node::id).collect::<Vec<_>>(), before);
            assert_eq!(document.arena.capacity(), capacity);
            assert_tree_integrity(&document);
        }
    }

    #[test]
    fn inline_containers_reject_block_children() {
        let containers = [
            InlineKind::Strong,
            InlineKind::Emphasis,
            InlineKind::Link {
                url: "/".into(),
                title: None,
            },
            InlineKind::Image {
                url: "/image.png".into(),
                title: None,
            },
        ];
        for kind in containers {
            let mut parent = None;
            let mut document =
                build_document([paragraph([inline(kind, []).capture(&mut parent)])]).unwrap();
            let parent = parent.unwrap();

            let result = document.insert_child_block(parent, 0, BlockKind::Paragraph);

            assert_eq!(result, Err(DocumentError::InvalidChild { parent }));
            assert!(document.node(parent).unwrap().children().is_empty());
            assert_tree_integrity(&document);
        }
    }

    #[test]
    fn list_item_rejects_inline_and_list_item_children() {
        let children = [
            NodeKind::Inline(InlineKind::Text("invalid".into())),
            NodeKind::Block(BlockKind::ListItem),
        ];
        for child in children {
            let mut parent = None;
            let mut document = build_document([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, []).capture(&mut parent)],
            )])
            .unwrap();
            let parent = parent.unwrap();

            let result = match child {
                NodeKind::Block(kind) => document.insert_child_block(parent, 0, kind),
                NodeKind::Inline(kind) => document.insert_inline(parent, 0, kind),
            };

            assert_eq!(result, Err(DocumentError::InvalidChild { parent }));
            assert!(document.node(parent).unwrap().children().is_empty());
            assert_eq!(document.arena.len(), 2);
            assert_tree_integrity(&document);
        }
    }

    #[test]
    fn list_item_cannot_be_a_root() {
        let mut document = Document::new();

        let result = document.insert_block(0, BlockKind::ListItem);

        assert_eq!(result, Err(DocumentError::ListItemNeedsList));
        assert!(document.blocks().is_empty());
        assert!(document.arena.is_empty());
    }

    #[test]
    fn child_block_insertion_respects_index_and_rejects_invalid_index() {
        let mut last = None;
        let mut document = build_document([block(
            BlockKind::BlockQuote,
            [paragraph([]).capture(&mut last)],
        )])
        .unwrap();
        let quote = document.blocks()[0];
        let first = document
            .insert_child_block(quote, 0, BlockKind::ThematicBreak)
            .unwrap();

        let result = document.insert_child_block(quote, 3, BlockKind::Paragraph);

        assert_eq!(
            result,
            Err(DocumentError::InvalidIndex { index: 3, len: 2 })
        );
        assert_eq!(
            document.node(quote).unwrap().children(),
            &[first, last.unwrap()]
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn deleted_block_container_rejects_child_after_slot_reuse() {
        let mut document = build_document([block(BlockKind::BlockQuote, [])]).unwrap();
        let deleted = document.blocks()[0];
        document.remove_node(deleted).unwrap();
        let replacement = document.insert_block(0, BlockKind::BlockQuote).unwrap();

        let result = document.insert_child_block(deleted, 0, BlockKind::Paragraph);

        assert_eq!(result, Err(DocumentError::NodeNotFound(deleted)));
        assert!(document.node(replacement).unwrap().children().is_empty());
        assert_tree_integrity(&document);
    }

    #[test]
    fn new_block_leaves_and_heading_levels_preserve_content() {
        let kinds = [
            BlockKind::Heading {
                level: HeadingLevel::H5,
            },
            BlockKind::Heading {
                level: HeadingLevel::H6,
            },
            BlockKind::ThematicBreak,
            BlockKind::Html("<section>raw</section>\n".into()),
        ];
        let document = build_document(kinds.iter().cloned().map(|kind| block(kind, []))).unwrap();

        assert_eq!(
            document
                .nodes()
                .map(|node| node.kind().clone())
                .collect::<Vec<_>>(),
            kinds.into_iter().map(NodeKind::Block).collect::<Vec<_>>()
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn block_leaves_reject_block_and_inline_children() {
        let kinds = [
            BlockKind::ThematicBreak,
            BlockKind::Html("<hr>".into()),
            BlockKind::CodeBlock {
                language: None,
                code: "code".into(),
            },
        ];
        for kind in kinds {
            let mut document = build_document([block(kind, [])]).unwrap();
            let parent = document.blocks()[0];

            let block = document.insert_child_block(parent, 0, BlockKind::Paragraph);
            let inline = document.insert_inline(parent, 0, InlineKind::Strong);

            assert_eq!(block, Err(DocumentError::CannotHaveChildren(parent)));
            assert_eq!(inline, Err(DocumentError::CannotHaveChildren(parent)));
            assert!(document.node(parent).unwrap().children().is_empty());
            assert_tree_integrity(&document);
        }
    }

    #[test]
    fn inline_leaves_preserve_content_and_reject_children() {
        let kinds = [
            InlineKind::Text("text".into()),
            InlineKind::Code("a < b".into()),
            InlineKind::SoftBreak,
            InlineKind::HardBreak,
            InlineKind::Html("<br>".into()),
        ];
        for kind in kinds {
            let mut parent = None;
            let mut document =
                build_document([paragraph([inline(kind.clone(), []).capture(&mut parent)])])
                    .unwrap();
            let parent = parent.unwrap();

            let inline = document.insert_inline(parent, 0, InlineKind::Emphasis);
            let block = document.insert_child_block(parent, 0, BlockKind::Paragraph);

            assert_eq!(inline, Err(DocumentError::CannotHaveChildren(parent)));
            assert_eq!(block, Err(DocumentError::CannotHaveChildren(parent)));
            assert_eq!(
                document.node(parent).unwrap().kind(),
                &NodeKind::Inline(kind)
            );
            assert!(document.node(parent).unwrap().children().is_empty());
            assert_tree_integrity(&document);
        }
    }

    #[test]
    fn image_preserves_title_and_formatted_description_inside_link() {
        let (mut image, mut formatted, mut description) = (None, None, None);
        let link_kind = InlineKind::Link {
            url: "/target".into(),
            title: None,
        };
        let image_kind = InlineKind::Image {
            url: "/image.png".into(),
            title: Some("Title".into()),
        };
        let formatted_description =
            emphasis([text("Description").capture(&mut description)]).capture(&mut formatted);

        let document = build_document([paragraph([inline(
            link_kind,
            [inline(image_kind.clone(), [formatted_description]).capture(&mut image)],
        )])])
        .unwrap();

        let (image, formatted) = (image.unwrap(), formatted.unwrap());
        assert_eq!(
            document.node(image).unwrap().kind(),
            &NodeKind::Inline(image_kind)
        );
        assert_eq!(document.node(image).unwrap().children(), &[formatted]);
        assert_eq!(
            document.node(formatted).unwrap().children(),
            &[description.unwrap()]
        );
        assert_tree_integrity(&document);
    }

    #[test]
    fn link_rejects_nested_link_through_formatting() {
        for nested_depth in 0..3 {
            let link_kind = InlineKind::Link {
                url: "/target".into(),
                title: None,
            };
            let mut link = None;
            let mut document =
                build_document([paragraph(
                    [inline(link_kind.clone(), []).capture(&mut link)],
                )])
                .unwrap();
            let mut parent = link.unwrap();
            for _ in 0..nested_depth {
                parent = document
                    .insert_inline(parent, 0, InlineKind::Strong)
                    .unwrap();
            }
            let before = document.arena.len();

            let result = document.insert_inline(parent, 0, link_kind);

            assert_eq!(result, Err(DocumentError::InvalidChild { parent }));
            assert_eq!(document.arena.len(), before);
            assert!(document.node(parent).unwrap().children().is_empty());
            assert_tree_integrity(&document);
        }
    }

    #[test]
    fn image_description_allows_link_inside_outer_link() {
        let mut formatted = None;
        let link_kind = InlineKind::Link {
            url: "/target".into(),
            title: None,
        };
        let image_kind = InlineKind::Image {
            url: "/image.png".into(),
            title: None,
        };
        let mut document = build_document([paragraph([inline(
            link_kind.clone(),
            [inline(image_kind, [emphasis([]).capture(&mut formatted)])],
        )])])
        .unwrap();
        let parent = formatted.unwrap();

        let inner = document.insert_inline(parent, 0, link_kind).unwrap();

        assert_eq!(document.node(inner).unwrap().parent(), Some(parent));
        assert_tree_integrity(&document);
    }
}
