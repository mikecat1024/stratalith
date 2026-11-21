use crate::{BlockKind, Document, NodeId, NodeKind};

/// Chooses which side of an inline boundary a position belongs to.
///
/// # Examples
///
/// At the boundary in `Hello **world**`, offset 6 can resolve to the end of
/// `"Hello "` or the start of the bold `"world"`.
///
/// ```
/// use stratalith_core::builder::{document, paragraph, strong, text};
/// use stratalith_core::{Affinity, BlockPosition};
///
/// let (mut plain, mut world) = (None, None);
/// let doc = document([paragraph([
///     text("Hello ").capture(&mut plain),
///     strong([text("world").capture(&mut world)]),
/// ])]).unwrap();
///
/// let boundary = BlockPosition { block: doc.blocks()[0], offset: 6, affinity: Affinity::Upstream };
/// let upstream = boundary.to_node_position(&doc).unwrap();
/// let downstream = BlockPosition {
///     affinity: Affinity::Downstream,
///     ..boundary
/// }.to_node_position(&doc).unwrap();
///
/// assert_eq!((upstream.node(), upstream.node_offset()), (plain.unwrap(), 6));
/// assert_eq!((downstream.node(), downstream.node_offset()), (world.unwrap(), 0));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Affinity {
    Upstream,
    Downstream,
}

/// A position measured from the start of a text-bearing block.
///
/// Cursor-position-based lengths count one unit per Unicode scalar value in text.
/// Breaks and images occupy one unit each. Image descriptions do not add units.
/// Inline HTML is an opaque one-unit object. Combining sequences and emoji
/// sequences may occupy multiple units. The host handles grapheme navigation.
///
/// # Examples
///
/// The position after `😀` in `A😀B` is two Unicode scalar values from the
/// start of the block. The corresponding node offset is also two.
///
/// ```
/// use stratalith_core::builder::{document, paragraph, text};
/// use stratalith_core::{Affinity, BlockPosition};
///
/// let mut content = None;
/// let doc = document([paragraph([text("A😀B").capture(&mut content)])]).unwrap();
///
/// let position = BlockPosition { block: doc.blocks()[0], offset: 2, affinity: Affinity::Downstream };
/// let node_position = position.to_node_position(&doc).unwrap();
///
/// assert_eq!(node_position.node(), content.unwrap());
/// assert_eq!(node_position.node_offset(), 2);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockPosition {
    pub block: NodeId,
    pub offset: usize,
    pub affinity: Affinity,
}

/// A position resolved to a node within its block.
pub struct NodePosition {
    node: NodeId,
    node_offset: usize,
}

impl NodePosition {
    pub fn node(&self) -> NodeId {
        self.node
    }

    /// The offset from the start of the node, in the same units as [`BlockPosition`].
    pub fn node_offset(&self) -> usize {
        self.node_offset
    }
}

impl BlockPosition {
    /// Resolves this position to a node and node-relative offset in `document`.
    ///
    /// # Examples
    ///
    /// A hard break contributes one unit, as does each Unicode scalar value
    /// in a text node.
    ///
    /// ```
    /// use stratalith_core::builder::{document, inline, paragraph, text};
    /// use stratalith_core::{Affinity, BlockPosition, InlineKind};
    ///
    /// let mut hard_break = None;
    /// let doc = document([paragraph([
    ///     text("日😀"),
    ///     inline(InlineKind::HardBreak, []).capture(&mut hard_break),
    /// ])]).unwrap();
    ///
    /// let end = BlockPosition {
    ///     block: doc.blocks()[0],
    ///     offset: 3,
    ///     affinity: Affinity::Upstream,
    /// }.to_node_position(&doc).unwrap();
    ///
    /// assert_eq!((end.node(), end.node_offset()), (hard_break.unwrap(), 1));
    /// ```
    pub fn to_node_position(self, document: &Document) -> Result<NodePosition, PositionError> {
        let block = document
            .node(self.block)
            .ok_or(PositionError::NodeNotFound(self.block))?;
        let mut stack = match block.kind() {
            NodeKind::Block(BlockKind::Paragraph | BlockKind::Heading { .. }) => {
                block.children().iter().rev().copied().collect::<Vec<_>>()
            }
            NodeKind::Block(BlockKind::CodeBlock { code, .. }) => {
                let len = code.chars().count();
                if self.offset > len {
                    return Err(PositionError::OffsetOutOfBounds {
                        offset: self.offset,
                        len,
                    });
                }
                return Ok(NodePosition {
                    node: self.block,
                    node_offset: self.offset,
                });
            }
            _ => return Err(PositionError::NotTextBlock(self.block)),
        };

        let mut start = 0;
        let mut last = NodePosition {
            node: self.block,
            node_offset: 0,
        };
        while let Some(id) = stack.pop() {
            let node = document.node(id).expect("linked nodes exist");
            let NodeKind::Inline(kind) = node.kind() else {
                unreachable!("text blocks contain only inlines")
            };
            let Some(len) = kind.direct_offset_len() else {
                stack.extend(node.children().iter().rev().copied());
                continue;
            };
            let end = start + len;
            if self.offset < end
                || (self.offset == end && matches!(self.affinity, Affinity::Upstream))
            {
                return Ok(NodePosition {
                    node: id,
                    node_offset: self.offset - start,
                });
            }
            last = NodePosition {
                node: id,
                node_offset: len,
            };
            start = end;
        }

        if self.offset == start {
            Ok(last)
        } else {
            Err(PositionError::OffsetOutOfBounds {
                offset: self.offset,
                len: start,
            })
        }
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum PositionError {
    NodeNotFound(NodeId),
    NotTextBlock(NodeId),
    OffsetOutOfBounds { offset: usize, len: usize },
}

impl std::fmt::Display for PositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeNotFound(id) => write!(f, "node {id:?} was not found"),
            Self::NotTextBlock(id) => write!(f, "node {id:?} is not a text-bearing block"),
            Self::OffsetOutOfBounds { offset, len } => {
                write!(f, "offset {offset} exceeds block length {len}")
            }
        }
    }
}

impl std::error::Error for PositionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InlineKind;
    use crate::builder::{block, document, emphasis, inline, paragraph, strong, text};

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
    fn unicode_positions_count_scalar_values_within_each_node() {
        let (doc, block, left, right) = nested();
        let cases = [
            (0, left, 0),
            (1, left, 1),
            (2, right, 0),
            (3, right, 1),
            (4, right, 2),
            (5, right, 3),
        ];

        let results: Vec<_> = cases
            .iter()
            .map(|(offset, _, _)| {
                BlockPosition {
                    block,
                    offset: *offset,
                    affinity: Affinity::Downstream,
                }
                .to_node_position(&doc)
                .unwrap()
            })
            .collect();

        for (result, (_, node, node_offset)) in results.iter().zip(cases) {
            assert_eq!((result.node(), result.node_offset()), (node, node_offset));
        }
    }

    #[test]
    fn affinity_selects_opposite_sides_of_nested_boundary() {
        let (doc, block, left, right) = nested();
        let boundary = BlockPosition {
            block,
            offset: 2,
            affinity: Affinity::Upstream,
        };

        let upstream = boundary.to_node_position(&doc).unwrap();
        let downstream = BlockPosition {
            affinity: Affinity::Downstream,
            ..boundary
        }
        .to_node_position(&doc)
        .unwrap();

        assert_eq!((upstream.node(), upstream.node_offset()), (left, 2));
        assert_eq!((downstream.node(), downstream.node_offset()), (right, 0));
    }

    #[test]
    fn empty_block_resolves_to_itself() {
        let doc = document([paragraph([strong([])])]).unwrap();
        let block = doc.blocks()[0];

        let result = BlockPosition {
            block,
            offset: 0,
            affinity: Affinity::Downstream,
        }
        .to_node_position(&doc)
        .unwrap();

        assert_eq!(result.node(), block);
        assert_eq!(result.node_offset(), 0);
    }

    #[test]
    fn images_and_breaks_are_single_units_without_counting_image_description() {
        let mut image = None;
        let doc = document([paragraph([
            inline(
                InlineKind::Image {
                    url: "image.png".into(),
                    title: None,
                },
                [text("long description")],
            )
            .capture(&mut image),
            inline(InlineKind::HardBreak, []),
            inline(InlineKind::Html("<span>opaque</span>".into()), []),
        ])])
        .unwrap();
        let block = doc.blocks()[0];

        let after_image = BlockPosition {
            block,
            offset: 1,
            affinity: Affinity::Upstream,
        };

        let result = after_image.to_node_position(&doc).unwrap();
        let past_end = BlockPosition {
            offset: 4,
            ..after_image
        }
        .to_node_position(&doc);

        assert_eq!(
            past_end.err(),
            Some(PositionError::OffsetOutOfBounds { offset: 4, len: 3 })
        );
        assert_eq!(result.node(), image.unwrap());
        assert_eq!(result.node_offset(), 1);
    }

    #[test]
    fn code_blocks_use_scalar_offsets() {
        let doc = document([block(
            BlockKind::CodeBlock {
                language: None,
                code: "日😀".into(),
            },
            [],
        )])
        .unwrap();
        let block = doc.blocks()[0];

        let result = BlockPosition {
            block,
            offset: 2,
            affinity: Affinity::Downstream,
        }
        .to_node_position(&doc)
        .unwrap();

        assert_eq!(result.node_offset(), 2);
        assert_eq!(result.node(), block);
    }

    #[test]
    fn out_of_bounds_offsets_are_rejected() {
        let (doc, block, _, _) = nested();

        let result = BlockPosition {
            block,
            offset: 6,
            affinity: Affinity::Downstream,
        }
        .to_node_position(&doc);

        assert_eq!(
            result.err(),
            Some(PositionError::OffsetOutOfBounds { offset: 6, len: 5 })
        );
    }

    #[test]
    fn container_blocks_are_not_positions() {
        let doc = document([block(BlockKind::BlockQuote, [])]).unwrap();
        let block = doc.blocks()[0];

        let result = BlockPosition {
            block,
            offset: 0,
            affinity: Affinity::Downstream,
        }
        .to_node_position(&doc);

        assert_eq!(result.err(), Some(PositionError::NotTextBlock(block)));
    }

    #[test]
    fn deleted_block_ids_remain_invalid_after_reuse() {
        let (mut doc, block, _, _) = nested();
        doc.remove_node(block).unwrap();
        doc.insert_block(0, BlockKind::Paragraph).unwrap();

        let result = BlockPosition {
            block,
            offset: 0,
            affinity: Affinity::Downstream,
        }
        .to_node_position(&doc);

        assert_eq!(result.err(), Some(PositionError::NodeNotFound(block)));
    }

    #[test]
    fn downstream_skips_empty_text_at_a_boundary() {
        let mut next = None;
        let doc = document([paragraph([
            text("a"),
            text(""),
            text("日").capture(&mut next),
        ])])
        .unwrap();
        let block = doc.blocks()[0];

        let result = BlockPosition {
            block,
            offset: 1,
            affinity: Affinity::Downstream,
        }
        .to_node_position(&doc)
        .unwrap();

        assert_eq!(result.node(), next.unwrap());
        assert_eq!(result.node_offset(), 0);
    }

    #[test]
    fn empty_text_resolves_to_its_start() {
        let mut empty = None;
        let doc = document([paragraph([text("").capture(&mut empty)])]).unwrap();
        let block = doc.blocks()[0];

        let result = BlockPosition {
            block,
            offset: 0,
            affinity: Affinity::Downstream,
        }
        .to_node_position(&doc)
        .unwrap();

        assert_eq!(result.node(), empty.unwrap());
        assert_eq!(result.node_offset(), 0);
    }
}
