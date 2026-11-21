use super::*;
use crate::{PositionError, Selection};
use std::{collections::HashMap, ops::Range};

impl Document {
    /// Copies the selected content and its enclosing formatting into a fresh document.
    /// Rule and raw HTML blocks are atomic for extraction, with offsets 0 and 1.
    /// Empty headings, code blocks, and containers may likewise be selected as a whole.
    ///
    /// # Examples
    ///
    /// Selecting backwards from after 重 to before 😀 keeps the bold formatting on 重.
    ///
    /// ```
    /// use stratalith_core::builder::{document, paragraph, strong, text};
    /// use stratalith_core::{Affinity, BlockPosition, Node, Selection};
    ///
    /// let doc = document([paragraph([text("A😀"), strong([text("重要")])])]).unwrap();
    /// let anchor = BlockPosition {
    ///     block: doc.blocks()[0], offset: 3, affinity: Affinity::Downstream,
    /// };
    /// let focus = BlockPosition { offset: 1, ..anchor };
    ///
    /// let selected = doc.extract(Selection { anchor, focus }).unwrap();
    ///
    /// let expected = document([paragraph([text("😀"), strong([text("重")])])]).unwrap();
    /// assert!(selected.nodes().map(Node::kind).eq(expected.nodes().map(Node::kind)));
    /// ```
    pub fn extract(&self, selection: Selection) -> Result<Document, PositionError> {
        let range = selection.ordered_range(self, |position| {
            let node = self
                .node(position.block)
                .ok_or(PositionError::NodeNotFound(position.block))?;
            if atomic_block(node) {
                return (position.offset <= 1).then_some(()).ok_or(
                    PositionError::OffsetOutOfBounds {
                        offset: position.offset,
                        len: 1,
                    },
                );
            }
            position.to_node_position(self).map(|_| ())
        })?;
        let (start, end) = (range.start(), range.end());
        let mut result = Document::new();
        if start.block == end.block && start.offset == end.offset {
            return Ok(result);
        }
        let mut parents = HashMap::new();
        let mut active = false;
        for node in self.nodes() {
            active |= node.id == start.block;
            if !active || !matches!(node.kind, NodeKind::Block(_)) {
                continue;
            }
            let from = if node.id == start.block {
                start.offset
            } else {
                0
            };
            let to = if node.id == end.block {
                end.offset
            } else {
                usize::MAX
            };
            let inside = node.id != start.block && node.id != end.block;
            match &node.kind {
                _ if atomic_block(node) && from == 0 && to > 0 => {
                    self.copy_parent(node.id, &mut result, &mut parents);
                }
                NodeKind::Block(BlockKind::CodeBlock { language, code }) => {
                    let code = substring(code, from..to);
                    if !code.is_empty() || inside {
                        let parent = node
                            .parent
                            .map(|id| self.copy_parent(id, &mut result, &mut parents));
                        result.append_copy(
                            parent,
                            NodeKind::Block(BlockKind::CodeBlock {
                                language: language.clone(),
                                code,
                            }),
                        );
                    }
                }
                NodeKind::Block(BlockKind::Paragraph | BlockKind::Heading { .. }) => {
                    if inside
                        || (start.block != end.block
                            && matches!(node.kind, NodeKind::Block(BlockKind::Paragraph)))
                    {
                        self.copy_parent(node.id, &mut result, &mut parents);
                    }
                    let mut offset = 0;
                    for &id in &node.children {
                        self.copy_inline(id, &(from..to), &mut offset, &mut result, &mut parents);
                    }
                }
                _ => {}
            }
            if node.id == end.block {
                break;
            }
        }
        Ok(result)
    }

    fn copy_inline(
        &self,
        id: NodeId,
        range: &Range<usize>,
        offset: &mut usize,
        result: &mut Document,
        parents: &mut HashMap<NodeId, NodeId>,
    ) {
        let node = self.node(id).unwrap();
        let NodeKind::Inline(kind) = &node.kind else {
            unreachable!()
        };
        let Some(len) = kind.direct_offset_len() else {
            if node.children.is_empty() && range.contains(offset) {
                self.copy_parent(id, result, parents);
            }
            for &child in &node.children {
                self.copy_inline(child, range, offset, result, parents);
            }
            return;
        };
        let start = *offset;
        *offset += len;
        if *offset <= range.start || start >= range.end {
            return;
        }
        let part = range.start.saturating_sub(start)..range.end.saturating_sub(start);
        let kind = match kind {
            InlineKind::Text(value) => InlineKind::Text(substring(value, part)),
            InlineKind::Code(value) => InlineKind::Code(substring(value, part)),
            _ => {
                // Atomic inlines include their whole subtree (the image description).
                self.copy_subtree(id, result, parents);
                return;
            }
        };
        let parent = self.copy_parent(node.parent.unwrap(), result, parents);
        result.append_copy(Some(parent), NodeKind::Inline(kind));
    }

    fn copy_subtree(
        &self,
        id: NodeId,
        result: &mut Document,
        parents: &mut HashMap<NodeId, NodeId>,
    ) {
        self.copy_parent(id, result, parents);
        for &child in &self.node(id).unwrap().children {
            self.copy_subtree(child, result, parents);
        }
    }

    fn copy_parent(
        &self,
        id: NodeId,
        result: &mut Document,
        parents: &mut HashMap<NodeId, NodeId>,
    ) -> NodeId {
        if let Some(&copied) = parents.get(&id) {
            return copied;
        }
        let node = self.node(id).unwrap();
        let parent = node.parent.map(|id| self.copy_parent(id, result, parents));
        let copied = result.append_copy(parent, node.kind.clone());
        parents.insert(id, copied);
        copied
    }
}

fn atomic_block(node: &Node) -> bool {
    node.children.is_empty()
        && (matches!(
            node.kind,
            NodeKind::Block(
                BlockKind::Html(_)
                    | BlockKind::ThematicBreak
                    | BlockKind::Heading { .. }
                    | BlockKind::BlockQuote
                    | BlockKind::ListItem
                    | BlockKind::List { .. }
            )
        ) || matches!(&node.kind, NodeKind::Block(BlockKind::CodeBlock { code, .. }) if code.is_empty()))
}

fn substring(value: &str, range: Range<usize>) -> String {
    value.chars().skip(range.start).take(range.len()).collect()
}

#[cfg(test)]
mod tests {
    use crate::builder::{document, paragraph, text};
    use crate::{Affinity, BlockPosition, Selection};

    #[test]
    fn extracting_through_the_next_paragraph_start_keeps_the_selected_break() {
        let doc = document([paragraph([text("left")]), paragraph([text("right")])]).unwrap();
        let selection = Selection {
            anchor: BlockPosition {
                block: doc.blocks()[0],
                offset: 2,
                affinity: Affinity::Downstream,
            },
            focus: BlockPosition {
                block: doc.blocks()[1],
                offset: 0,
                affinity: Affinity::Downstream,
            },
        };
        let expected = document([paragraph([text("ft")]), paragraph([])]).unwrap();

        let fragment = doc.extract(selection).unwrap();

        assert_eq!(fragment, expected);
    }
}
