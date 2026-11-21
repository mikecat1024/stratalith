use crate::{Affinity, BlockPosition, Document, NodeId, PositionError, Selection};

/// Edit coordinates refer to the document produced by preceding edits.
pub enum Edit {
    /// Insert into the text or inline context selected by affinity.
    InsertText {
        at: BlockPosition,
        text: String,
    },
    /// Insert an inline hard break, or a literal newline in a code block.
    InsertHardBreak(BlockPosition),
    IndentListItem(NodeId),
    OutdentListItem(NodeId),
    /// Delete between either ordering of endpoints. Across blocks, retain the
    /// start block and append the surviving suffix. Joining code block content
    /// with inline content returns an error.
    Delete(Selection),
    /// Split the paragraph at the given block offset, as when pressing Enter.
    /// The content before the offset stays in the original paragraph, which
    /// keeps its ID. The rest moves into a new paragraph immediately after it.
    /// For example, `abc|def` becomes two paragraphs, `abc` and `def`.
    SplitParagraph(BlockPosition),
    /// Append the right paragraph's content to the left and remove the right
    /// paragraph. Both must have the same parent (or both be top-level), and
    /// `right` must immediately follow `left`. The left paragraph keeps its ID.
    /// For example, `abc` and `def` become `abcdef`, as when pressing Backspace
    /// at the start of `def`.
    JoinParagraphs {
        left: NodeId,
        right: NodeId,
    },
}

/// Ordered block-coordinate changes.
///
/// Offsets use the units defined by [`BlockPosition`]. An insertion's `len` is
/// the cursor-position-based length of the inserted text.
pub enum Change {
    Insert {
        block: NodeId,
        offset: usize,
        len: usize,
    },
    Delete {
        block: NodeId,
        start: usize,
        end: usize,
    },
    Split {
        left: NodeId,
        right: NodeId,
        offset: usize,
    },
    Join {
        left: NodeId,
        right: NodeId,
        offset: usize,
    },
    RemoveBlock {
        block: NodeId,
        into: BlockPosition,
    },
}

/// A position in the document before applying a change set.
pub struct BeforeEditPosition(BlockPosition);

impl BeforeEditPosition {
    /// The position must be valid in the original document of the change set
    /// it will be passed to.
    pub fn new(position: BlockPosition) -> Self {
        Self(position)
    }
}

/// A position in the document after applying a change set.
pub struct AfterEditPosition(BlockPosition);

impl AfterEditPosition {
    pub fn into_position(self) -> BlockPosition {
        self.0
    }
}

pub struct ChangeSet(Vec<Change>);

impl ChangeSet {
    pub fn changes(&self) -> &[Change] {
        &self.0
    }

    /// Applies the changes in order to a position from the original document.
    /// At an insertion or split, downstream positions follow the new content.
    ///
    /// ```compile_fail,E0308
    /// use stratalith_core::{AfterEditPosition, ChangeSet};
    ///
    /// fn remap(changes: &ChangeSet, after: AfterEditPosition) {
    ///     changes.get_position_after_edits(after);
    /// }
    /// ```
    pub fn get_position_after_edits(&self, before: BeforeEditPosition) -> AfterEditPosition {
        let mut pos = before.0;
        for change in &self.0 {
            let follows = |offset| {
                pos.offset > offset
                    || (pos.offset == offset && matches!(pos.affinity, Affinity::Downstream))
            };
            match *change {
                Change::Insert { block, offset, len } if pos.block == block && follows(offset) => {
                    pos.offset += len
                }
                Change::Delete { block, start, end }
                    if pos.block == block && pos.offset > start =>
                {
                    pos.offset -= pos.offset.min(end) - start
                }
                Change::Split {
                    left,
                    right,
                    offset,
                } if pos.block == left && follows(offset) => {
                    pos.block = right;
                    pos.offset -= offset;
                }
                Change::Join {
                    left,
                    right,
                    offset,
                } if pos.block == right => {
                    pos.block = left;
                    pos.offset += offset;
                }
                Change::RemoveBlock { block, into } if pos.block == block => {
                    pos.block = into.block;
                    pos.offset = into.offset;
                }
                _ => {}
            }
        }
        AfterEditPosition(pos)
    }
}

#[derive(Debug)]
pub enum EditError {
    Position(PositionError),
    ExpectedParagraph(NodeId),
    NotAdjacentParagraphs,
    CannotMoveListItem(NodeId),
    IncompatibleBlockContents,
}

impl From<PositionError> for EditError {
    fn from(error: PositionError) -> Self {
        Self::Position(error)
    }
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Position(e) => e.fmt(f),
            Self::ExpectedParagraph(id) => write!(f, "expected paragraph at {id:?}"),
            Self::CannotMoveListItem(id) => write!(f, "list item {id:?} cannot move to that level"),
            Self::NotAdjacentParagraphs => {
                f.write_str("paragraphs must be consecutive siblings in document order")
            }
            Self::IncompatibleBlockContents => {
                f.write_str("cannot join code block content with inline content")
            }
        }
    }
}
impl std::error::Error for EditError {}

/// An atomic sequence of edits.
///
/// The selection is expressed in the original document and mapped on success.
///
/// ```
/// use stratalith_core::builder::{document, paragraph};
/// use stratalith_core::{
///     Affinity, BeforeEditPosition, BlockPosition, Edit, InlineKind, NodeKind, Selection, Transaction,
/// };
///
/// let mut doc = document([paragraph([])])?;
/// let at = BlockPosition { block: doc.blocks()[0], offset: 0, affinity: Affinity::Downstream };
/// let before = BeforeEditPosition::new(at);
/// let tx = Transaction::new([Edit::InsertText { at, text: "c😀".into() }]);
///
/// let (changes, selection) = tx.apply(&mut doc, Selection { anchor: at, focus: at })?;
///
/// let children = doc.node(at.block).unwrap().children();
/// assert_eq!(children.len(), 1);
/// assert_eq!(
///     doc.node(children[0]).unwrap().kind(),
///     &NodeKind::Inline(InlineKind::Text("c😀".into())),
/// );
/// assert_eq!(selection.focus.offset, 2);
/// assert_eq!(changes.get_position_after_edits(before).into_position().offset, 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Transaction {
    edits: Vec<Edit>,
}

impl Transaction {
    pub fn new(edits: impl IntoIterator<Item = Edit>) -> Self {
        Self {
            edits: edits.into_iter().collect(),
        }
    }

    pub fn apply(
        self,
        document: &mut Document,
        selection: Selection,
    ) -> Result<(ChangeSet, Selection), EditError> {
        selection.range(document)?;
        let (staged, changes) = self.stage(document)?;
        let selection = Selection {
            anchor: changes
                .get_position_after_edits(BeforeEditPosition::new(selection.anchor))
                .into_position(),
            focus: changes
                .get_position_after_edits(BeforeEditPosition::new(selection.focus))
                .into_position(),
        };
        *document = staged;
        Ok((changes, selection))
    }

    pub(crate) fn stage(self, document: &Document) -> Result<(Document, ChangeSet), EditError> {
        // A snapshot keeps failed transactions atomic, but deep-clones the
        // entire document even for a single keystroke. The editor also retains
        // full snapshots for undo. A paged copy-on-write arena could share
        // unchanged pages across snapshots and copy only pages being edited.
        let mut staged = document.edit_snapshot();
        let mut changes = Vec::new();
        for edit in self.edits {
            staged.apply_edit(edit, &mut changes)?;
        }
        Ok((staged, ChangeSet(changes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{
        BlockBuilder, block, document, emphasis, inline, paragraph, strong, text,
    };
    use crate::{BlockKind, InlineKind, NodeKind};

    fn pos(block: NodeId, offset: usize) -> BlockPosition {
        BlockPosition {
            block,
            offset,
            affinity: Affinity::Downstream,
        }
    }

    fn caret(at: BlockPosition) -> Selection {
        Selection {
            anchor: at,
            focus: at,
        }
    }

    fn greeting<'a>(
        hello: &'a mut Option<NodeId>,
        world: &'a mut Option<NodeId>,
    ) -> BlockBuilder<'a> {
        paragraph([strong([
            text("Hello ").capture(hello),
            emphasis([text("go😀").capture(world)]),
        ])])
    }

    fn nested() -> (Document, NodeId, NodeId, NodeId) {
        let (mut hello, mut world) = (None, None);
        let doc = document([greeting(&mut hello, &mut world)]).unwrap();
        let block = doc.blocks()[0];
        (doc, block, hello.unwrap(), world.unwrap())
    }

    fn assert_text(doc: &Document, id: NodeId, expected: &str) {
        assert_eq!(
            doc.node(id).unwrap().kind(),
            &NodeKind::Inline(InlineKind::Text(expected.into()))
        );
    }

    #[test]
    fn insertion_uses_affinity_at_nested_boundary() {
        for (affinity, hello_after, world_after) in [
            (Affinity::Upstream, "Hello !", "go😀"),
            (Affinity::Downstream, "Hello ", "!go😀"),
        ] {
            let (mut doc, block, hello, world) = nested();
            let at = BlockPosition {
                affinity,
                ..pos(block, 6)
            };
            let tx = Transaction::new([Edit::InsertText {
                at,
                text: "!".into(),
            }]);

            let (_, selection) = tx.apply(&mut doc, caret(pos(block, 6))).unwrap();

            assert_text(&doc, hello, hello_after);
            assert_text(&doc, world, world_after);
            assert_eq!(selection.focus, pos(block, 7));
        }
    }

    #[test]
    fn reversed_deletion_retains_both_partial_text_nodes() {
        let (mut doc, block, hello, world) = nested();
        let selection = || Selection {
            anchor: pos(block, 8),
            focus: pos(block, 4),
        };
        let tx = Transaction::new([Edit::Delete(selection())]);

        let (_, after) = tx.apply(&mut doc, selection()).unwrap();

        assert_text(&doc, hello, "Hell");
        assert_text(&doc, world, "😀");
        assert_eq!((after.anchor, after.focus), (pos(block, 4), pos(block, 4)));
    }

    #[test]
    fn split_inside_nested_text_recreates_the_open_ancestors_and_maps_both_sides() {
        let (mut doc, block, hello, world) = nested();
        let at = pos(block, 7);
        let upstream = BlockPosition {
            affinity: Affinity::Upstream,
            ..at
        };
        let tx = Transaction::new([Edit::SplitParagraph(at)]);
        let before = BeforeEditPosition::new(pos(block, 9));

        let (changes, after) = tx
            .apply(
                &mut doc,
                Selection {
                    anchor: upstream,
                    focus: at,
                },
            )
            .unwrap();
        let mapped = changes.get_position_after_edits(before);

        let right = doc.blocks()[1];
        let bold = doc.node(right).unwrap().children()[0];
        let emphasis = doc.node(bold).unwrap().children()[0];
        let suffix = doc.node(emphasis).unwrap().children()[0];
        assert_eq!(
            doc.node(bold).unwrap().kind(),
            &NodeKind::Inline(InlineKind::Strong)
        );
        assert_eq!(
            doc.node(emphasis).unwrap().kind(),
            &NodeKind::Inline(InlineKind::Emphasis)
        );
        assert_eq!(doc.node(suffix).unwrap().parent(), Some(emphasis));
        assert_text(&doc, hello, "Hello ");
        assert_text(&doc, world, "g");
        assert_text(&doc, suffix, "o😀");
        assert_eq!((after.anchor, after.focus), (upstream, pos(right, 0)));
        assert_eq!(mapped.into_position(), pos(right, 2));
    }

    #[test]
    fn joining_nested_sibling_paragraphs_merges_text_and_maps_a_later_insertion() {
        let (mut left, mut right, mut first, mut second) = (None, None, None, None);
        let mut doc = document([block(
            BlockKind::BlockQuote,
            [
                paragraph([text("c").capture(&mut first)]).capture(&mut left),
                paragraph([text("a").capture(&mut second)]).capture(&mut right),
            ],
        )])
        .unwrap();
        let quote = doc.blocks()[0];
        let (left, right) = (left.unwrap(), right.unwrap());
        let tx = Transaction::new([
            Edit::JoinParagraphs { left, right },
            Edit::InsertText {
                at: pos(left, 2),
                text: "t".into(),
            },
        ]);

        let (_, after) = tx.apply(&mut doc, caret(pos(right, 1))).unwrap();

        assert_text(&doc, first.unwrap(), "cat");
        assert!(doc.node(second.unwrap()).is_none());
        assert_eq!(doc.node(quote).unwrap().children(), &[left]);
        assert_eq!(after.focus, pos(left, 3));
    }

    #[test]
    fn deleting_across_blocks_removes_intervening_content_and_moves_intact_suffix() {
        let (mut hello, mut world, mut middle, mut last, mut suffix) =
            (None, None, None, None, None);
        let mut doc = document([
            greeting(&mut hello, &mut world),
            block(
                BlockKind::BlockQuote,
                [
                    paragraph([]).capture(&mut middle),
                    paragraph([
                        inline(InlineKind::HardBreak, []),
                        inline(InlineKind::Code("x".into()), []).capture(&mut suffix),
                    ])
                    .capture(&mut last),
                ],
            ),
        ])
        .unwrap();
        let (first, quote) = (doc.blocks()[0], doc.blocks()[1]);
        let (middle, last) = (middle.unwrap(), last.unwrap());
        let tx = Transaction::new([Edit::Delete(Selection {
            anchor: pos(last, 1),
            focus: pos(first, 5),
        })]);
        let before = BeforeEditPosition::new(pos(last, 0));

        let (changes, after) = tx
            .apply(
                &mut doc,
                Selection {
                    anchor: pos(last, 2),
                    focus: pos(middle, 0),
                },
            )
            .unwrap();
        let mapped = changes.get_position_after_edits(before);

        assert_eq!(after.anchor, pos(first, 6));
        assert_eq!(after.focus, pos(first, 5));
        assert_eq!(mapped.into_position(), pos(first, 5));
        assert_text(&doc, hello.unwrap(), "Hello");
        assert_eq!(doc.node(suffix.unwrap()).unwrap().parent(), Some(first));
        for removed in [world.unwrap(), quote, middle, last] {
            assert!(doc.node(removed).is_none());
        }
    }

    #[test]
    fn failure_after_split_preserves_document_and_id_allocation() {
        let (mut doc, block, _, world) = nested();
        let (mut control, _, _, _) = nested();
        let tx = Transaction::new([
            Edit::SplitParagraph(pos(block, 7)),
            Edit::SplitParagraph(pos(block, 8)),
        ]);

        let result = tx.apply(&mut doc, caret(pos(block, 0)));

        assert!(matches!(
            result,
            Err(EditError::Position(PositionError::OffsetOutOfBounds { .. }))
        ));
        assert_eq!(doc.blocks(), &[block]);
        assert_text(&doc, world, "go😀");
        assert_eq!(
            doc.insert_block(1, BlockKind::Paragraph),
            control.insert_block(1, BlockKind::Paragraph)
        );
    }

    #[test]
    fn deleting_image_keeps_the_left_id_when_neighboring_text_merges() {
        let (mut left, mut image, mut description, mut right) = (None, None, None, None);
        let mut doc = document([paragraph([
            text("a").capture(&mut left),
            inline(
                InlineKind::Image {
                    url: "a.png".into(),
                    title: None,
                },
                [text("Description").capture(&mut description)],
            )
            .capture(&mut image),
            text("b").capture(&mut right),
        ])])
        .unwrap();
        let block = doc.blocks()[0];
        let tx = Transaction::new([Edit::Delete(Selection {
            anchor: pos(block, 1),
            focus: pos(block, 2),
        })]);

        let (_, after) = tx.apply(&mut doc, caret(pos(block, 3))).unwrap();

        assert_text(&doc, left.unwrap(), "ab");
        assert_eq!(after.focus, pos(block, 2));
        for removed in [image.unwrap(), description.unwrap(), right.unwrap()] {
            assert!(doc.node(removed).is_none());
        }
    }

    #[test]
    fn split_at_start_moves_the_whole_subtree_with_its_ids() {
        let (mut doc, block, hello, world) = nested();
        let bold = doc.node(hello).unwrap().parent().unwrap();
        let tx = Transaction::new([Edit::SplitParagraph(pos(block, 0))]);

        let (_, after) = tx.apply(&mut doc, caret(pos(block, 0))).unwrap();

        assert!(doc.node(block).unwrap().children().is_empty());
        assert_eq!(doc.node(after.focus.block).unwrap().children(), &[bold]);
        assert_eq!(doc.node(bold).unwrap().parent(), Some(after.focus.block));
        assert_text(&doc, world, "go😀");
    }
}
