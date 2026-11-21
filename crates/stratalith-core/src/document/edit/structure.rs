use super::*;
use crate::{Affinity, BlockPosition};

impl Document {
    pub(super) fn sibling_block(&mut self, anchor: NodeId, after: bool, kind: BlockKind) -> NodeId {
        let index = self.index_of(anchor) + usize::from(after);
        match self.live(anchor).parent {
            Some(parent) => self.insert_child_block(parent, index, kind),
            None => self.insert_block(index, kind),
        }
        .expect("valid sibling block")
    }

    fn siblings_mut(&mut self, parent: Option<NodeId>) -> &mut Vec<NodeId> {
        match parent {
            None => &mut self.blocks,
            Some(id) => &mut self.live_mut(id).children,
        }
    }

    pub(super) fn relocate(&mut self, id: NodeId, parent: Option<NodeId>, index: usize) {
        let old = self.live(id).parent;
        let from = self.index_of(id);
        self.siblings_mut(old).remove(from);
        self.live_mut(id).parent = parent;
        self.siblings_mut(parent).insert(index, id);
    }

    pub(super) fn item_of(&self, block: NodeId) -> Option<NodeId> {
        self.live(block)
            .parent
            .filter(|&id| matches!(self.live(id).kind, NodeKind::Block(BlockKind::ListItem)))
    }

    // Split the container around a child and promote it, retaining both
    // surrounding subtrees and their document order.
    fn lift(&mut self, child: NodeId) {
        let parent = self.live(child).parent.unwrap();
        let end = self.index_of(child) + 1;
        if end < self.live(parent).children.len() {
            let NodeKind::Block(kind) = self.live(parent).kind.clone() else {
                unreachable!()
            };
            let right = self.sibling_block(parent, true, kind);
            self.move_suffix(parent, end, right);
        }
        self.relocate(child, self.live(parent).parent, self.index_of(parent) + 1);
        if self.live(parent).children.is_empty() {
            self.remove_node(parent).unwrap();
        }
    }

    pub(super) fn exit_item(&mut self, item: NodeId) {
        self.lift(item);
        // The item is now between the two halves of its former list. Unwrap
        // its blocks in order, including any nested list following the caret.
        let parent = self.live(item).parent;
        let index = self.index_of(item);
        for child in self.live(item).children.clone().into_iter().rev() {
            self.relocate(child, parent, index);
        }
        self.remove_node(item).unwrap();
    }

    pub(crate) fn enter_at(&mut self, at: BlockPosition) -> Result<BlockPosition, EditError> {
        let block = at.block;
        let point = at.to_node_position(self)?;
        let empty = self.block_len(block) == 0;
        let parent = self.live(block).parent;
        let right = if let Some(item) = self.item_of(block) {
            let list = self.live(item).parent.unwrap();
            if empty && self.live(item).children == [block] {
                if self.item_of(list).is_some() {
                    self.outdent_item(item)?;
                } else {
                    self.exit_item(item);
                }
                block
            } else {
                let right = self.split_paragraph(block, point);
                let next = self.sibling_block(item, true, BlockKind::ListItem);
                self.move_suffix(item, self.index_of(right), next);
                right
            }
        } else if parent
            .is_some_and(|id| matches!(self.live(id).kind, NodeKind::Block(BlockKind::BlockQuote)))
        {
            if empty {
                self.lift(block);
                block
            } else if at.offset == 0 {
                let before = self.sibling_block(block, false, BlockKind::Paragraph);
                self.lift(before);
                block
            } else {
                let right = self.split_paragraph(block, point);
                self.lift(right);
                right
            }
        } else if at.offset == 0
            && matches!(
                self.live(block).kind,
                NodeKind::Block(BlockKind::Heading { .. })
            )
        {
            if empty {
                self.live_mut(block).kind = NodeKind::Block(BlockKind::Paragraph);
            } else {
                self.sibling_block(block, false, BlockKind::Paragraph);
            }
            block
        } else {
            self.split_paragraph(block, point)
        };
        Ok(BlockPosition {
            block: right,
            offset: 0,
            affinity: Affinity::Downstream,
        })
    }

    pub(crate) fn boundary_join(&self, at: BlockPosition, backward: bool) -> Option<Selection> {
        let inline_block = |id| {
            matches!(
                self.live(id).kind,
                NodeKind::Block(BlockKind::Paragraph | BlockKind::Heading { .. })
            )
        };
        if !inline_block(at.block) {
            return None;
        }
        let mut outer = at.block;
        let neighbor = loop {
            let (before, after) = self.surrounding_siblings(outer);
            if let Some(&id) = if backward {
                before.last()
            } else {
                after.first()
            } {
                break id;
            }
            outer = self.live(outer).parent?;
        };
        let mut target = neighbor;
        while !inline_block(target) {
            let children = &self.live(target).children;
            target = *if backward {
                children.last()
            } else {
                children.first()
            }?;
        }
        let other = BlockPosition {
            block: target,
            offset: if backward { self.block_len(target) } else { 0 },
            ..at
        };
        Some(Selection {
            anchor: at,
            focus: other,
        })
    }

    pub(crate) fn exit_block_on_backspace(
        &self,
        at: BlockPosition,
    ) -> Option<(Self, BlockPosition)> {
        if at.offset != 0 {
            return None;
        }
        let block = at.block;
        if self.blocks() == [block]
            && matches!(
                self.live(block).kind,
                NodeKind::Block(BlockKind::Heading { .. })
            )
            && self.block_len(block) == 0
        {
            let mut document = self.edit_snapshot();
            let paragraph = document.sibling_block(block, false, BlockKind::Paragraph);
            document.remove_node(block).unwrap();
            return Some((
                document,
                BlockPosition {
                    block: paragraph,
                    offset: 0,
                    affinity: Affinity::Downstream,
                },
            ));
        }
        let item = self.item_of(block)?;
        let list = self.live(item).parent?;
        if !matches!(self.live(block).kind, NodeKind::Block(BlockKind::Paragraph))
            || self.live(item).children.first() != Some(&block)
        {
            return None;
        }
        // An empty leading item with a following sibling joins back to
        // the parent paragraph. Other items become a separate paragraph.
        let joins_parent = self.item_of(list).is_some()
            && self.live(list).children.len() > 1
            && self.index_of(item) == 0
            && self.block_len(block) == 0;
        if joins_parent {
            return None;
        }
        let mut document = self.edit_snapshot();
        document.exit_item(item);
        Some((document, at))
    }
}
