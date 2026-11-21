mod shortcuts;
mod structure;
use super::{Document, Node, NodeId};
use crate::{BlockKind, Change, Edit, EditError, InlineKind, NodeKind, PositionError, Selection};
use std::collections::{HashMap, HashSet};

fn byte_offset(text: &str, scalar: usize) -> usize {
    text.char_indices()
        .nth(scalar)
        .map_or(text.len(), |(byte, _)| byte)
}

impl Document {
    pub(crate) fn import_document(&self, source: &Self, selection: Selection) -> (Self, Selection) {
        let mut document = Self::new();
        document.arena.resume(&self.arena);
        let mut ids = HashMap::new();
        for node in source.nodes() {
            let parent = node.parent().map(|id| ids[&id]);
            let id = document.append_copy(parent, node.kind.clone());
            ids.insert(node.id, id);
        }
        let remap = |position: crate::BlockPosition| crate::BlockPosition {
            block: ids[&position.block],
            ..position
        };
        (
            document,
            Selection {
                anchor: remap(selection.anchor),
                focus: remap(selection.focus),
            },
        )
    }

    pub(crate) fn edit_snapshot(&self) -> Self {
        Self {
            blocks: self.blocks.clone(),
            arena: self.arena.clone(),
        }
    }

    /// Returns the document before restoration. Restored nodes keep their IDs,
    /// and subsequent edits allocate fresh IDs.
    pub(crate) fn restore_snapshot_document(&mut self, mut snapshot_document: Self) -> Self {
        snapshot_document.arena.resume(&self.arena);
        std::mem::replace(self, snapshot_document)
    }

    fn live(&self, id: NodeId) -> &Node {
        self.node(id).expect("node ID refers to a live node")
    }

    fn live_mut(&mut self, id: NodeId) -> &mut Node {
        self.arena
            .get_mut(id)
            .expect("node ID refers to a live node")
    }

    fn siblings(&self, parent: Option<NodeId>) -> &[NodeId] {
        parent.map_or(self.blocks(), |id| self.live(id).children())
    }

    fn index_of(&self, id: NodeId) -> usize {
        self.siblings(self.live(id).parent)
            .iter()
            .position(|&child| child == id)
            .unwrap()
    }

    pub(crate) fn surrounding_siblings(&self, id: NodeId) -> (&[NodeId], &[NodeId]) {
        let (before, rest) = self
            .siblings(self.live(id).parent)
            .split_at(self.index_of(id));
        (before, &rest[1..])
    }

    fn text_mut(&mut self, id: NodeId) -> Option<&mut String> {
        match &mut self.live_mut(id).kind {
            NodeKind::Inline(InlineKind::Text(text) | InlineKind::Code(text))
            | NodeKind::Block(BlockKind::CodeBlock { code: text, .. }) => Some(text),
            _ => None,
        }
    }

    /// Appends the records used to remap cursor positions to a buffer shared by
    /// all edits in the transaction, avoiding a temporary vector for each edit.
    pub(crate) fn apply_edit(
        &mut self,
        edit: Edit,
        cursor_position_changes: &mut Vec<Change>,
    ) -> Result<(), EditError> {
        match edit {
            Edit::InsertText { at, text } => {
                let point = at.to_node_position(self)?;
                if text.is_empty() {
                    return Ok(());
                }
                let len = text.chars().count();
                let id = point.node();
                if let Some(mark) = self.trailing_mark(at, &point) {
                    let parent = self.live(mark).parent.unwrap();
                    self.insert_inline(parent, self.index_of(mark) + 1, InlineKind::Text(text))
                        .unwrap();
                } else if let Some(target) = self.text_mut(id) {
                    target.insert_str(byte_offset(target, point.node_offset()), &text);
                } else {
                    let (parent, index) = match self.live(id).kind {
                        NodeKind::Block(_) => (id, 0),
                        NodeKind::Inline(_) => (
                            self.live(id).parent.unwrap(),
                            self.index_of(id) + point.node_offset(),
                        ),
                    };
                    self.insert_inline(parent, index, InlineKind::Text(text))
                        .expect("resolved inline insertion site");
                }
                self.normalize(at.block);
                cursor_position_changes.push(Change::Insert {
                    block: at.block,
                    offset: at.offset,
                    len,
                });
            }
            Edit::InsertHardBreak(at) => {
                let point = at.to_node_position(self)?;
                if matches!(
                    self.live(at.block).kind,
                    NodeKind::Block(BlockKind::CodeBlock { .. })
                ) {
                    let text = self.text_mut(at.block).unwrap();
                    text.insert(byte_offset(text, at.offset), '\n');
                } else {
                    let (parent, index) = self.split_inline_leaf(at.block, point);
                    self.insert_inline(parent, index, InlineKind::HardBreak)
                        .expect("resolved inline insertion site");
                }
                cursor_position_changes.push(Change::Insert {
                    block: at.block,
                    offset: at.offset,
                    len: 1,
                });
            }
            Edit::IndentListItem(item) => self.indent_item(item)?,
            Edit::OutdentListItem(item) => self.outdent_item(item)?,
            Edit::Delete(selection) => self.delete_selection(selection, cursor_position_changes)?,
            Edit::SplitParagraph(at) => {
                self.require_paragraph(at.block)?;
                let point = at.to_node_position(self)?;
                let right = self.split_paragraph(at.block, point);
                self.normalize(at.block);
                self.normalize(right);
                cursor_position_changes.push(Change::Split {
                    left: at.block,
                    right,
                    offset: at.offset,
                });
            }
            Edit::JoinParagraphs { left, right } => {
                self.require_paragraph(left)?;
                self.require_paragraph(right)?;
                if self.live(left).parent != self.live(right).parent
                    || self.index_of(right) != self.index_of(left) + 1
                {
                    return Err(EditError::NotAdjacentParagraphs);
                }
                let offset = self.block_len(left);
                self.join_contents(left, right);
                self.normalize(left);
                cursor_position_changes.push(Change::Join {
                    left,
                    right,
                    offset,
                });
            }
        }
        Ok(())
    }

    fn item_list(&self, item: NodeId) -> Result<NodeId, EditError> {
        let node = self.node(item).ok_or(PositionError::NodeNotFound(item))?;
        if !matches!(node.kind, NodeKind::Block(BlockKind::ListItem)) {
            return Err(EditError::CannotMoveListItem(item));
        }
        Ok(node.parent.expect("list items have a list parent"))
    }

    fn nested_list(&mut self, item: NodeId, list: NodeId) -> NodeId {
        use crate::ListKind;
        let NodeKind::Block(BlockKind::List { kind, tight }) = self.live(list).kind else {
            unreachable!("list items have a list parent")
        };
        if let Some(&last) = self.live(item).children.last()
            && let NodeKind::Block(BlockKind::List { kind: existing, .. }) = self.live(last).kind
            && std::mem::discriminant(&existing) == std::mem::discriminant(&kind)
        {
            return last;
        }
        let kind = match kind {
            ListKind::Bullet => ListKind::Bullet,
            ListKind::Ordered { .. } => ListKind::Ordered { start: 1 },
        };
        self.insert_child_block(
            item,
            self.live(item).children.len(),
            BlockKind::List { kind, tight },
        )
        .expect("a list item accepts a list")
    }

    fn move_item(&mut self, item: NodeId, from: NodeId, to: NodeId, index: usize) {
        let old_index = self.index_of(item);
        self.live_mut(from).children.remove(old_index);
        self.live_mut(item).parent = Some(to);
        self.live_mut(to).children.insert(index, item);
    }

    fn indent_item(&mut self, item: NodeId) -> Result<(), EditError> {
        let list = self.item_list(item)?;
        let previous = self
            .surrounding_siblings(item)
            .0
            .last()
            .copied()
            .ok_or(EditError::CannotMoveListItem(item))?;
        let nested = self.nested_list(previous, list);
        self.move_item(item, list, nested, self.live(nested).children.len());
        Ok(())
    }

    fn outdent_item(&mut self, item: NodeId) -> Result<(), EditError> {
        let list = self.item_list(item)?;
        let parent_item = self
            .live(list)
            .parent
            .ok_or(EditError::CannotMoveListItem(item))?;
        let outer = self.item_list(parent_item)?;
        // Keep content after the lifted item after it in document order.
        let tail = self.index_of(item) + 1;
        if tail < self.live(list).children.len() {
            let nested = self.nested_list(item, list);
            self.move_suffix(list, tail, nested);
        }
        self.move_suffix(parent_item, self.index_of(list) + 1, item);
        self.move_item(item, list, outer, self.index_of(parent_item) + 1);
        if self.live(list).children.is_empty() {
            self.remove_node(list).expect("the empty list is live");
        }
        Ok(())
    }

    fn require_paragraph(&self, id: NodeId) -> Result<(), EditError> {
        match self.node(id).ok_or(PositionError::NodeNotFound(id))?.kind() {
            NodeKind::Block(BlockKind::Paragraph) => Ok(()),
            _ => Err(EditError::ExpectedParagraph(id)),
        }
    }

    /// Moves a suffix intact, retaining all descendant IDs.
    fn move_suffix(&mut self, from: NodeId, index: usize, to: NodeId) {
        let suffix = self.live_mut(from).children.split_off(index);
        for &id in &suffix {
            self.live_mut(id).parent = Some(to);
        }
        self.live_mut(to).children.extend(suffix);
    }

    fn split_inline_leaf(&mut self, block: NodeId, point: crate::NodePosition) -> (NodeId, usize) {
        let leaf = point.node();
        if leaf == block {
            return (block, 0);
        }
        let parent = self.live(leaf).parent.unwrap();
        let mut cut = self.index_of(leaf);
        let offset = point.node_offset();
        if offset > 0 {
            cut += 1;
            if let Some(text) = self.text_mut(leaf) {
                let byte = byte_offset(text, offset);
                if byte < text.len() {
                    let suffix = text.split_off(byte);
                    let kind = match self.live(leaf).kind {
                        NodeKind::Inline(InlineKind::Text(_)) => InlineKind::Text(suffix),
                        NodeKind::Inline(InlineKind::Code(_)) => InlineKind::Code(suffix),
                        _ => unreachable!(),
                    };
                    self.insert_inline(parent, cut, kind).unwrap();
                }
            }
        }

        (parent, cut)
    }

    fn insert_paragraph_after(&mut self, block: NodeId) -> NodeId {
        self.sibling_block(block, true, BlockKind::Paragraph)
    }

    pub(crate) fn exit_code_block(&mut self, range: &crate::Range) -> NodeId {
        let start = range.start();
        let code = self.text_mut(start.block).expect("validated code block");
        let suffix = code.split_off(byte_offset(code, range.end().offset));
        code.truncate(byte_offset(code, start.offset));
        let paragraph = self.insert_paragraph_after(start.block);
        for (index, line) in suffix.split('\n').enumerate() {
            if index > 0 {
                self.append_copy(Some(paragraph), NodeKind::Inline(InlineKind::HardBreak));
            }
            if !line.is_empty() {
                self.append_copy(
                    Some(paragraph),
                    NodeKind::Inline(InlineKind::Text(line.into())),
                );
            }
        }
        paragraph
    }

    fn split_inlines_at(&mut self, block: NodeId, point: crate::NodePosition) -> usize {
        let (mut parent, mut cut) = self.split_inline_leaf(block, point);
        // Lift the cut to the paragraph, duplicating only containers that
        // actually span it. Whole containers move with their existing IDs.
        while parent != block {
            let outer = self.live(parent).parent.unwrap();
            let index = self.index_of(parent);
            if cut > 0 && cut < self.live(parent).children.len() {
                let kind = self.live(parent).kind.clone();
                let copy = self.insert_child(outer, index + 1, kind).unwrap();
                self.move_suffix(parent, cut, copy);
            }
            cut = index + usize::from(cut > 0);
            parent = outer;
        }
        cut
    }

    fn split_paragraph(&mut self, block: NodeId, point: crate::NodePosition) -> NodeId {
        let right = self.insert_paragraph_after(block);
        let cut = self.split_inlines_at(block, point);
        self.move_suffix(block, cut, right);
        right
    }

    pub(crate) fn insert_fragment(
        &mut self,
        target: Selection,
        fragment: &Document,
    ) -> Result<crate::BlockPosition, EditError> {
        let start = target.range(self)?.start();
        let left = start.block;
        if !matches!(
            self.live(left).kind,
            NodeKind::Block(BlockKind::Paragraph | BlockKind::Heading { .. })
        ) {
            return Err(EditError::IncompatibleBlockContents);
        }
        self.delete_selection(target, &mut Vec::new())?;
        let right = self.split_paragraph(left, start.to_node_position(self)?);
        let mut ids = HashMap::new();
        let mut last_block = left;
        for node in fragment.nodes() {
            let id = if let Some(parent) = node.parent() {
                self.append_copy(Some(ids[&parent]), node.kind().clone())
            } else if let NodeKind::Block(kind) = node.kind() {
                self.sibling_block(right, false, kind.clone())
            } else {
                unreachable!("document roots are blocks")
            };
            ids.insert(node.id(), id);
            if matches!(node.kind(), NodeKind::Block(_)) {
                last_block = id;
            }
        }
        let first = ids[&fragment.blocks()[0]];
        let last = ids[fragment.blocks().last().unwrap()];
        let paragraph =
            |doc: &Self, id| matches!(doc.live(id).kind, NodeKind::Block(BlockKind::Paragraph));
        let merge_suffix = paragraph(self, last);
        if paragraph(self, first) {
            self.join_contents(left, first);
            if last_block == first {
                last_block = left;
            }
        } else if start.offset == 0 {
            self.remove_node(left).unwrap();
        }
        let editable = matches!(
            self.live(last_block).kind,
            NodeKind::Block(
                BlockKind::Paragraph | BlockKind::Heading { .. } | BlockKind::CodeBlock { .. }
            )
        );
        let offset = if editable {
            self.block_len(last_block)
        } else {
            0
        };
        if merge_suffix {
            self.join_contents(last_block, right);
            self.normalize(last_block);
        } else if editable && self.live(right).children.is_empty() {
            self.remove_node(right).unwrap();
        } else if !editable {
            last_block = right;
        }
        Ok(crate::BlockPosition {
            block: last_block,
            offset,
            affinity: crate::Affinity::Upstream,
        })
    }

    pub(crate) fn linked_selection(&self, range: &crate::Range, url: &str) -> Option<Document> {
        let (start, end) = (range.start(), range.end());
        let block = start.block;
        if block != end.block
            || start.offset == end.offset
            || !matches!(
                self.live(block).kind,
                NodeKind::Block(BlockKind::Paragraph | BlockKind::Heading { .. })
            )
        {
            return None;
        }
        let mut doc = self.edit_snapshot();
        let first = doc.split_inlines_at(block, start.to_node_position(&doc).unwrap());
        let last = doc.split_inlines_at(block, end.to_node_position(&doc).unwrap());
        let mut pending = doc.live(block).children[first..last].to_vec();
        while let Some(id) = pending.pop() {
            let node = doc.live(id);
            match node.kind {
                NodeKind::Inline(InlineKind::Link { .. }) => return None,
                NodeKind::Inline(InlineKind::Strong | InlineKind::Emphasis) => {
                    pending.extend(node.children())
                }
                _ => {}
            }
        }
        let children: Vec<_> = doc.live_mut(block).children.drain(first..last).collect();
        let link = doc
            .insert_inline(
                block,
                first,
                InlineKind::Link {
                    url: url.into(),
                    title: None,
                },
            )
            .unwrap();
        for &child in &children {
            doc.live_mut(child).parent = Some(link);
        }
        doc.live_mut(link).children = children;
        Some(doc)
    }

    // At block end, the position resolver falls back to the last inline leaf.
    // A downstream caret there types outside its enclosing marks.
    fn trailing_mark(
        &self,
        at: crate::BlockPosition,
        point: &crate::NodePosition,
    ) -> Option<NodeId> {
        if at.affinity != crate::Affinity::Downstream {
            return None;
        }
        let leaf = self.live(point.node());
        let NodeKind::Inline(ref kind) = leaf.kind else {
            return None;
        };
        if point.node_offset() != kind.direct_offset_len()? {
            return None;
        }
        let mut outer = None;
        let mut current = leaf;
        loop {
            if matches!(
                current.kind,
                NodeKind::Inline(
                    InlineKind::Link { .. }
                        | InlineKind::Emphasis
                        | InlineKind::Strong
                        | InlineKind::Code(_)
                )
            ) {
                outer = Some(current.id);
            }
            let Some(parent) = current.parent else { break };
            if self.live(parent).children.last() != Some(&current.id) {
                break;
            }
            current = self.live(parent);
            if !matches!(current.kind, NodeKind::Inline(_)) {
                break;
            }
        }
        outer
    }

    pub(crate) fn in_mark_before(&self, at: crate::BlockPosition) -> bool {
        let position = crate::BlockPosition {
            affinity: crate::Affinity::Upstream,
            ..at
        };
        let Ok(point) = position.to_node_position(self) else {
            return false;
        };
        std::iter::successors(self.node(point.node()), |n| {
            n.parent.and_then(|id| self.node(id))
        })
        .any(|n| {
            matches!(
                n.kind,
                NodeKind::Inline(InlineKind::Code(_) | InlineKind::Strong | InlineKind::Emphasis)
            )
        })
    }

    fn leaves(&self, block: NodeId) -> Vec<(NodeId, usize)> {
        let mut leaves = Vec::new();
        let mut stack: Vec<_> = self.live(block).children.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            let node = self.live(id);
            let NodeKind::Inline(kind) = &node.kind else {
                unreachable!()
            };
            match kind.direct_offset_len() {
                Some(len) => leaves.push((id, len)),
                None => stack.extend(node.children.iter().rev().copied()),
            }
        }
        leaves
    }

    pub(crate) fn block_len(&self, block: NodeId) -> usize {
        match &self.live(block).kind {
            NodeKind::Block(BlockKind::CodeBlock { code, .. }) => code.chars().count(),
            _ => self.leaves(block).iter().map(|(_, len)| len).sum(),
        }
    }

    fn delete_units(
        &mut self,
        block: NodeId,
        start: usize,
        end: usize,
        cursor_position_changes: &mut Vec<Change>,
    ) {
        if start == end {
            return;
        }
        if let Some(text) = self.text_mut(block) {
            text.replace_range(byte_offset(text, start)..byte_offset(text, end), "");
        } else {
            let mut offset = 0;
            for (id, len) in self.leaves(block) {
                let from = start.saturating_sub(offset).min(len);
                let to = end.saturating_sub(offset).min(len);
                if from < to {
                    if from == 0 && to == len {
                        self.remove_node(id).unwrap();
                    } else {
                        let text = self.text_mut(id).expect("only text spans multiple units");
                        text.replace_range(byte_offset(text, from)..byte_offset(text, to), "");
                    }
                }
                offset += len;
            }
        }
        cursor_position_changes.push(Change::Delete { block, start, end });
    }

    fn join_contents(&mut self, left: NodeId, right: NodeId) {
        if let Some(text) = self.text_mut(right) {
            let suffix = std::mem::take(text);
            self.text_mut(left).unwrap().push_str(&suffix);
        } else {
            self.move_suffix(right, 0, left);
        }
        self.remove_node(right).unwrap();
    }

    fn delete_selection(
        &mut self,
        selection: Selection,
        cursor_position_changes: &mut Vec<Change>,
    ) -> Result<(), EditError> {
        let range = selection.range(self)?;
        let (start, end) = (range.start(), range.end());
        if start.block == end.block {
            if start.offset != end.offset {
                self.delete_units(
                    start.block,
                    start.offset,
                    end.offset,
                    cursor_position_changes,
                );
                self.normalize(start.block);
            }
            return Ok(());
        }
        let is_code_block = |id| {
            matches!(
                self.live(id).kind,
                NodeKind::Block(BlockKind::CodeBlock { .. })
            )
        };
        if is_code_block(start.block) != is_code_block(end.block) {
            return Err(EditError::IncompatibleBlockContents);
        }

        // Keep the ancestor path to the end block until its suffix has moved.
        let mut ancestors = Vec::new();
        let mut parent = self.live(end.block).parent;
        while let Some(id) = parent {
            ancestors.push(id);
            parent = self.live(id).parent;
        }
        let kept: HashSet<_> = ancestors.iter().copied().collect();
        let between: Vec<_> = self
            .nodes()
            .skip_while(|n| n.id != start.block)
            .skip(1)
            .take_while(|n| n.id != end.block)
            .filter(|n| matches!(n.kind, NodeKind::Block(_)) && !kept.contains(&n.id))
            .map(|n| n.id)
            .collect();

        self.delete_units(
            start.block,
            start.offset,
            self.block_len(start.block),
            cursor_position_changes,
        );
        self.delete_units(end.block, 0, end.offset, cursor_position_changes);
        for block in between {
            // A removed container may already have removed this descendant.
            if self.node(block).is_some() {
                self.remove_node(block).unwrap();
            }
            cursor_position_changes.push(Change::RemoveBlock { block, into: start });
        }
        self.join_contents(start.block, end.block);
        cursor_position_changes.push(Change::Join {
            left: start.block,
            right: end.block,
            offset: start.offset,
        });
        for id in ancestors {
            let children = &self.live(id).children;
            if children.is_empty() {
                self.remove_node(id).unwrap();
            } else if let [item] = children.as_slice()
                && self.live(*item).children.len() <= 1
                && let NodeKind::Block(BlockKind::List { tight, .. }) = &mut self.live_mut(id).kind
            {
                // A single item with at most one block has no boundary to render loose.
                *tight = true;
            }
        }
        self.normalize(start.block);
        Ok(())
    }

    /// Normalize the edited inline tree bottom-up, leaving opaque image
    /// descriptions untouched. The left text keeps its ID when merging.
    fn normalize(&mut self, block: NodeId) {
        let mut parents = vec![block];
        let mut index = 0;
        while index < parents.len() {
            for &id in &self.live(parents[index]).children {
                if matches!(
                    self.live(id).kind,
                    NodeKind::Inline(
                        InlineKind::Strong | InlineKind::Emphasis | InlineKind::Link { .. }
                    )
                ) {
                    parents.push(id);
                }
            }
            index += 1;
        }
        for parent in parents.into_iter().rev() {
            let mut previous = None;
            for id in self.live(parent).children.clone() {
                match &self.live(id).kind {
                    NodeKind::Inline(InlineKind::Text(s) | InlineKind::Code(s)) if s.is_empty() => {
                        self.remove_node(id).unwrap();
                    }
                    NodeKind::Inline(
                        InlineKind::Strong | InlineKind::Emphasis | InlineKind::Link { .. },
                    ) if self.live(id).children.is_empty() => {
                        self.remove_node(id).unwrap();
                    }
                    NodeKind::Inline(InlineKind::Text(_)) => {
                        if let Some(left) = previous {
                            let text = std::mem::take(self.text_mut(id).unwrap());
                            self.text_mut(left).unwrap().push_str(&text);
                            self.remove_node(id).unwrap();
                        } else {
                            previous = Some(id);
                        }
                    }
                    _ => previous = None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{document, paragraph, text};
    use crate::{Affinity, BlockPosition};

    #[test]
    fn edit_snapshot_preserves_node_ids_and_isolates_mutations() {
        let doc = document([paragraph([text("a")])]).unwrap();
        let block = doc.blocks()[0];
        let original_text = doc.node(block).unwrap().children()[0];

        let mut snapshot = doc.edit_snapshot();
        let inserted = snapshot
            .insert_inline(block, 1, InlineKind::Text("b".into()))
            .unwrap();

        assert_eq!(snapshot.blocks(), &[block]);
        assert_eq!(
            snapshot.node(block).unwrap().children(),
            &[original_text, inserted],
        );
        assert_eq!(snapshot.node(original_text).unwrap().parent(), Some(block));
        assert_eq!(doc.node(block).unwrap().children(), &[original_text]);
        assert!(doc.node(inserted).is_none());
    }

    #[test]
    fn apply_edit_records_split_with_resulting_block_ids() {
        let mut doc = document([paragraph([text("€😀")])]).unwrap();
        let block = doc.blocks()[0];
        let at = BlockPosition {
            block,
            offset: 1,
            affinity: Affinity::Downstream,
        };
        let mut cursor_position_changes = Vec::new();

        doc.apply_edit(Edit::SplitParagraph(at), &mut cursor_position_changes)
            .unwrap();

        let [left, right] = doc.blocks() else {
            panic!("split should leave two blocks");
        };
        assert_eq!(*left, block);
        assert!(matches!(
            cursor_position_changes.as_slice(),
            [Change::Split {
                left: recorded_left,
                right: recorded_right,
                offset: 1,
            }] if recorded_left == left && recorded_right == right
        ));
    }
}
