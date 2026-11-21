use crate::{BlockPosition, Document, PositionError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    pub anchor: BlockPosition,
    pub focus: BlockPosition,
}

impl Selection {
    pub fn caret(at: BlockPosition) -> Self {
        Self {
            anchor: at,
            focus: at,
        }
    }

    /// Validates both endpoints and returns them in document order.
    ///
    /// # Examples
    ///
    /// In a backward selection, the focus is the start of the range.
    ///
    /// ```
    /// use stratalith_core::builder::{document, paragraph, text};
    /// use stratalith_core::{Affinity, BlockPosition, Selection};
    ///
    /// let doc = document([paragraph([text("one")]), paragraph([text("two")])]).unwrap();
    /// let (first, second) = (doc.blocks()[0], doc.blocks()[1]);
    /// let anchor = BlockPosition { block: second, offset: 2, affinity: Affinity::Downstream };
    /// let focus = BlockPosition { block: first, offset: 1, ..anchor };
    ///
    /// let selection = Selection { anchor, focus };
    /// let range = selection.range(&doc).unwrap();
    ///
    /// assert_eq!((range.start().block, range.start().offset), (first, 1));
    /// assert_eq!((range.end().block, range.end().offset), (second, 2));
    /// ```
    pub fn range(&self, document: &Document) -> Result<Range, PositionError> {
        self.ordered_range(document, |position| {
            position.to_node_position(document).map(|_| ())
        })
    }

    pub(crate) fn ordered_range(
        self,
        document: &Document,
        validate: impl Fn(BlockPosition) -> Result<(), PositionError>,
    ) -> Result<Range, PositionError> {
        let Selection { anchor, focus } = self;
        validate(anchor)?;
        validate(focus)?;

        let anchor_first = if anchor.block == focus.block {
            anchor.offset <= focus.offset
        } else {
            let first = document
                .nodes()
                .find(|node| node.id() == anchor.block || node.id() == focus.block)
                .expect("validated blocks exist in document order");
            first.id() == anchor.block
        };
        let (start, end) = if anchor_first {
            (anchor, focus)
        } else {
            (focus, anchor)
        };
        Ok(Range { start, end })
    }
}

/// Ordered, validated endpoints. A document mutation requires resolving again.
///
/// Use [`Selection::range`] to obtain a range from a selection.
/// The endpoints are private so callers cannot bypass validation and ordering.
///
/// ```compile_fail
/// use stratalith_core::{BlockPosition, Range};
///
/// fn unchecked_range(start: BlockPosition, end: BlockPosition) -> Range {
///     Range { start, end }
/// }
/// ```
pub struct Range {
    start: BlockPosition,
    end: BlockPosition,
}

impl Range {
    pub fn start(&self) -> BlockPosition {
        self.start
    }

    pub fn end(&self) -> BlockPosition {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{document, emphasis, paragraph, strong, text};
    use crate::{Affinity, BlockKind, NodeId};

    fn nested() -> (Document, NodeId, NodeId, NodeId) {
        let (mut left, mut right) = (None, None);
        let doc = document([paragraph([
            text("日😀").capture(&mut left),
            strong([emphasis([text("e\u{301}語").capture(&mut right)])]),
        ])])
        .unwrap();
        let block = doc.blocks()[0];
        (doc, block, left.unwrap(), right.unwrap())
    }

    #[test]
    fn reversed_selection_crosses_nested_inlines() {
        let (doc, block, left, right) = nested();
        let anchor = BlockPosition {
            block,
            offset: 4,
            affinity: Affinity::Downstream,
        };
        let focus = BlockPosition {
            offset: 1,
            ..anchor
        };

        let range = Selection { anchor, focus }.range(&doc).unwrap();

        let start = range.start().to_node_position(&doc).unwrap();
        let end = range.end().to_node_position(&doc).unwrap();

        assert_eq!((range.start(), range.end()), (focus, anchor));
        assert_eq!((start.node(), start.node_offset()), (left, 1));
        assert_eq!((end.node(), end.node_offset()), (right, 2));
    }

    #[test]
    fn range_uses_document_order_instead_of_id_allocation_order() {
        let (mut doc, later, _, _) = nested();
        let quote = doc.insert_block(0, BlockKind::BlockQuote).unwrap();
        let earlier = doc
            .insert_child_block(quote, 0, BlockKind::Paragraph)
            .unwrap();
        let anchor = BlockPosition {
            block: later,
            offset: 0,
            affinity: Affinity::Downstream,
        };
        let focus = BlockPosition {
            block: earlier,
            ..anchor
        };

        let range = Selection { anchor, focus }.range(&doc).unwrap();

        assert_eq!((range.start().block, range.end().block), (earlier, later));
    }

    #[test]
    fn equal_block_offsets_keep_anchor_before_focus() {
        let (doc, block, _, _) = nested();
        let anchor = BlockPosition {
            block,
            offset: 2,
            affinity: Affinity::Upstream,
        };
        let focus = BlockPosition {
            affinity: Affinity::Downstream,
            ..anchor
        };

        let range = Selection { anchor, focus }.range(&doc).unwrap();

        assert_eq!((range.start(), range.end()), (anchor, focus));
    }

    #[test]
    fn range_rejects_an_out_of_bounds_offset_at_either_endpoint() {
        let (doc, block, _, _) = nested();
        let valid = BlockPosition {
            block,
            offset: 0,
            affinity: Affinity::Downstream,
        };
        let invalid = BlockPosition { offset: 6, ..valid };

        for (anchor, focus) in [(invalid, valid), (valid, invalid)] {
            let result = Selection { anchor, focus }.range(&doc);

            assert_eq!(
                result.err(),
                Some(PositionError::OffsetOutOfBounds { offset: 6, len: 5 })
            );
        }
    }
}
