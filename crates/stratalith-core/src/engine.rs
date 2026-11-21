use crate::{
    Affinity, BeforeEditPosition, BlockKind, BlockPosition, Document, Edit, EditError, NodeKind,
    PositionError, Selection, Transaction,
};

use std::time::Duration;

const TYPING_GROUP_DELAY: Duration = Duration::from_millis(500);

pub enum Command {
    /// Replaces the selection. A web URL links a nonempty inline selection
    /// when the label can be wrapped in a link. An empty string leaves it intact.
    InsertText(String),
    /// Replaces a host-resolved range, recording the dispatch selection for Undo.
    /// Empty content deletes the range. A collapsed empty replacement is a no-op.
    ReplaceRange {
        target: Selection,
        text: String,
    },
    /// Inserts parsed blocks at the target range as one undoable edit.
    Paste {
        target: Selection,
        fragment: Document,
    },
    /// Deletes the selection or the preceding unit of [`BlockPosition`].
    /// At a block boundary, unwraps a list item or joins adjacent inline content.
    Backspace,
    /// Deletes the selection or the following unit of [`BlockPosition`].
    /// At a block boundary, joins adjacent inline content in document order.
    Delete,
    /// Replaces the selection with a paragraph split or a newline in a code block.
    /// Continues list items and moves heading or quoted suffixes into a paragraph.
    Enter,
    /// Insert a hard break. Within one code block, move the surviving suffix
    /// to a sibling paragraph and place the caret at its start.
    InsertHardBreak,
    /// Nest the item containing the selection focus under its previous sibling.
    IndentListItem,
    /// Promote the focused item one list level. A top-level item stays in place.
    OutdentListItem,
    Undo,
    Redo,
}

/// Owns the document, selection, and local undo history for one editing session.
///
/// Timestamped consecutive insertions at a caret form one undo step when each insertion
/// occurs within 500 ms of the previous one. Moving the selection,
/// replacing a range, or applying a different edit starts a separate step.
/// Undo restores the original selection, including its direction and affinities.
/// Redo restores the caret left by the edit.
///
/// ```
/// use stratalith_core::builder::{document, paragraph};
/// use stratalith_core::{Affinity, BlockPosition, Command, EditorEngine, Selection};
///
/// let doc = document([paragraph([])])?;
/// let start = BlockPosition { block: doc.blocks()[0], offset: 0, affinity: Affinity::Downstream };
/// let mut editor = EditorEngine::new(doc, Selection::caret(start))?;
///
/// editor.execute(Command::InsertText("日本".into()), editor.selection(), None)?;
/// editor.execute(Command::Undo, editor.selection(), None)?;
/// let selection_after_undo = editor.selection();
/// editor.execute(Command::Redo, editor.selection(), None)?;
///
/// assert_eq!(selection_after_undo, Selection::caret(start));
/// assert_eq!(editor.selection().focus.offset, 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct EditorEngine {
    document: Document,
    selection: Selection,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    last_typing_at: Option<Duration>,
}

// Only the inactive document is stored. Undo and redo exchange it with the live one.
struct HistoryEntry {
    document: Document,
    before: Selection,
    after: Selection,
}

enum Direction {
    Backward,
    Forward,
}

enum Replacement {
    Text(String),
    LineBreak,
    HardBreak,
}

enum TextInput {
    Plain(String),
    Url(String),
}

enum LineBreakAction {
    HardBreak,
    ExitCodeBlock(crate::Range),
}

impl LineBreakAction {
    fn for_selection(document: &Document, range: crate::Range) -> Option<Self> {
        let in_code = |at: BlockPosition| {
            matches!(
                document.node(at.block).unwrap().kind(),
                NodeKind::Block(BlockKind::CodeBlock { .. })
            )
        };
        match (in_code(range.start()), in_code(range.end())) {
            (true, true) if range.start().block == range.end().block => {
                Some(Self::ExitCodeBlock(range))
            }
            (false, false) => Some(Self::HardBreak),
            _ => None,
        }
    }
}

impl TextInput {
    fn classify(text: String) -> Self {
        if is_web_url(&text) {
            Self::Url(text)
        } else {
            Self::Plain(text)
        }
    }
}

fn is_web_url(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once("://") else {
        return false;
    };
    if !(scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http"))
        || text
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\\'))
    {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap();
    let host_port = authority.rsplit('@').next().unwrap();
    let (host, port) = if let Some(ipv6) = host_port.strip_prefix('[') {
        let Some((address, tail)) = ipv6.split_once(']') else {
            return false;
        };
        if address.parse::<std::net::Ipv6Addr>().is_err() {
            return false;
        }
        (address, tail)
    } else {
        let end = host_port.find(':').unwrap_or(host_port.len());
        let (host, port) = host_port.split_at(end);
        if host.contains(['[', ']', '|', '^', '%']) {
            return false;
        }
        (host, port)
    };
    !host.is_empty()
        && (port.is_empty()
            || port
                .strip_prefix(':')
                .is_some_and(|p| p.parse::<u16>().is_ok()))
}

impl Replacement {
    fn resulting_caret_affinity(&self) -> Affinity {
        match self {
            // Keep subsequent typing in the inline context of the inserted text.
            Self::Text(_) => Affinity::Upstream,
            // Continue typing at the start of the new line.
            Self::LineBreak | Self::HardBreak => Affinity::Downstream,
        }
    }
}

impl EditorEngine {
    pub fn new(document: Document, selection: Selection) -> Result<Self, PositionError> {
        selection.range(&document)?;

        Ok(Self {
            document,
            selection,
            undo: Vec::new(),
            redo: Vec::new(),
            last_typing_at: None,
        })
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Report a caret move or range change that occurs between commands. Moving
    /// away and back still breaks a typing group, even though the next command
    /// sees the original caret. Reporting the same selection leaves the group intact.
    /// Selection changes themselves create no undo entry.
    pub fn set_selection(&mut self, selection: Selection) -> Result<(), PositionError> {
        if self.selection != selection {
            selection.range(&self.document)?;

            self.selection = selection;
            self.last_typing_at = None;
        }
        Ok(())
    }

    /// Records a replacement document and both selections in the undo history.
    /// The imported document receives fresh node IDs.
    /// `typing_at` uses the same monotonic clock and grouping rules as [`Self::execute`].
    ///
    /// ```
    /// use stratalith_core::{Affinity, BlockPosition, EditorEngine, Selection, builder};
    ///
    /// let original = builder::document([builder::paragraph([])])?;
    /// let replacement = builder::document([builder::paragraph([builder::text("a")])])?;
    /// let caret = |block, offset| Selection::caret(BlockPosition {
    ///     block, offset, affinity: Affinity::Downstream,
    /// });
    /// let before = caret(original.blocks()[0], 0);
    /// let after = caret(replacement.blocks()[0], 1);
    /// let mut editor = EditorEngine::new(original, before)?;
    ///
    /// editor.replace_document(before, &replacement, after, None)?;
    ///
    /// assert_eq!(editor.selection().focus.offset, 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn replace_document(
        &mut self,
        before: Selection,
        replacement: &Document,
        after: Selection,
        typing_at: Option<Duration>,
    ) -> Result<(), PositionError> {
        before.range(&self.document)?;
        after.range(replacement)?;
        let (document, after) = self.document.import_document(replacement, after);
        self.commit(document, before, after, typing_at);
        Ok(())
    }

    /// Uses the host selection at dispatch. Undo and redo restore the selection
    /// recorded in history. The supplied selection must be valid in the current document.
    /// Returns true when the document was edited or a history entry restored.
    /// Errors and no-ops preserve the editor state and history.
    ///
    /// Supply elapsed time from a common monotonic origin to group consecutive typing.
    /// With `None`, a text insertion forms its own undo step.
    pub fn execute(
        &mut self,
        command: Command,
        selection: Selection,
        now: Option<Duration>,
    ) -> Result<bool, EditError> {
        if selection != self.selection {
            selection.range(&self.document)?;
        }
        match command {
            Command::Undo => {
                let Some(mut entry) = self.undo.pop() else {
                    return Ok(false);
                };
                entry.document = self.document.restore_snapshot_document(entry.document);
                self.selection = entry.before;
                self.redo.push(entry);
                self.last_typing_at = None;
                Ok(true)
            }
            Command::Redo => {
                let Some(mut entry) = self.redo.pop() else {
                    return Ok(false);
                };
                entry.document = self.document.restore_snapshot_document(entry.document);
                self.selection = entry.after;
                self.undo.push(entry);
                self.last_typing_at = None;
                Ok(true)
            }
            Command::InsertText(text) if text.is_empty() => Ok(false),
            Command::InsertText(text) => self.replace_text(selection, selection, text, now),
            Command::ReplaceRange { target, text } => {
                target.range(&self.document)?;
                self.replace_text(selection, target, text, now)
            }
            Command::Paste { target, fragment } => {
                if fragment.blocks().is_empty() {
                    return Ok(false);
                }
                let mut document = self.document.edit_snapshot();
                let after = document.insert_fragment(target, &fragment)?;
                self.commit(document, selection, Selection::caret(after), None);
                Ok(true)
            }
            Command::InsertHardBreak => {
                let range = selection.range(&self.document)?;
                match LineBreakAction::for_selection(&self.document, range) {
                    Some(LineBreakAction::ExitCodeBlock(range)) => {
                        let mut document = self.document.edit_snapshot();
                        let block = document.exit_code_block(&range);
                        let after = Selection::caret(BlockPosition {
                            block,
                            offset: 0,
                            affinity: Affinity::Downstream,
                        });
                        self.commit(document, selection, after, None);
                        Ok(true)
                    }
                    Some(LineBreakAction::HardBreak) => {
                        self.replace_selection(selection, selection, Replacement::HardBreak, now)
                    }
                    None => Ok(false),
                }
            }
            Command::IndentListItem => self.move_list_item(selection, Edit::IndentListItem),
            Command::OutdentListItem => self.move_list_item(selection, Edit::OutdentListItem),
            Command::Enter => {
                self.replace_selection(selection, selection, Replacement::LineBreak, now)
            }
            Command::Backspace => self.delete(selection, Direction::Backward),
            Command::Delete => self.delete(selection, Direction::Forward),
        }
    }

    fn move_list_item(
        &mut self,
        selection: Selection,
        edit: fn(crate::NodeId) -> Edit,
    ) -> Result<bool, EditError> {
        let item = std::iter::successors(self.document.node(selection.focus.block), |node| {
            node.parent().and_then(|id| self.document.node(id))
        })
        .find(|node| matches!(node.kind(), NodeKind::Block(BlockKind::ListItem)));
        let Some(item) = item else { return Ok(false) };
        match Transaction::new([edit(item.id())]).stage(&self.document) {
            Ok((document, _)) => {
                // Moving the subtree preserves block IDs and offsets at both endpoints.
                self.commit(document, selection, selection, None);
                Ok(true)
            }
            Err(EditError::CannotMoveListItem(_)) => Ok(false),
            Err(error) => Err(error),
        }
    }

    // A caret may have two affinities at the same offset. The focus determines
    // where typing goes. A nonempty selection is replaced at its ordered start.
    fn selected_start(&self, selection: Selection) -> Result<(BlockPosition, bool), PositionError> {
        let Selection { anchor, focus } = selection;
        if anchor.block == focus.block && anchor.offset == focus.offset {
            Ok((focus, true))
        } else {
            Ok((selection.range(&self.document)?.start(), false))
        }
    }

    fn replace_text(
        &mut self,
        before: Selection,
        target: Selection,
        text: String,
        now: Option<Duration>,
    ) -> Result<bool, EditError> {
        if text.is_empty() {
            let (at, collapsed) = self.selected_start(target)?;
            if collapsed {
                return Ok(false);
            }
            return self.apply(
                Transaction::new([Edit::Delete(target)]),
                before,
                at,
                at.affinity,
                None,
            );
        }
        let text = match TextInput::classify(text) {
            TextInput::Url(url) => {
                let range = target.range(&self.document)?;
                if let Some(document) = self.document.linked_selection(&range, &url) {
                    let after = Selection::caret(BlockPosition {
                        affinity: Affinity::Downstream,
                        ..range.end()
                    });
                    self.commit(document, before, after, None);
                    return Ok(true);
                }
                url
            }
            TextInput::Plain(text) => text,
        };
        self.replace_selection(before, target, Replacement::Text(text), now)
    }

    fn replace_selection(
        &mut self,
        before: Selection,
        selection: Selection,
        replacement: Replacement,
        now: Option<Duration>,
    ) -> Result<bool, EditError> {
        let (at, collapsed) = self.selected_start(selection)?;
        let resulting_caret_affinity = replacement.resulting_caret_affinity();
        let is_typing = matches!(replacement, Replacement::Text(_));
        let check_shortcut = matches!(&replacement, Replacement::Text(text)
            if Document::may_trigger_input_shortcut(text));
        let edit = match replacement {
            Replacement::Text(text) => Edit::InsertText { at, text },
            Replacement::HardBreak => Edit::InsertHardBreak(at),
            Replacement::LineBreak
                if matches!(
                    self.document.node(at.block).unwrap().kind(),
                    NodeKind::Block(BlockKind::CodeBlock { .. })
                ) =>
            {
                Edit::InsertText {
                    at,
                    text: "\n".into(),
                }
            }
            Replacement::LineBreak => {
                let mut document = self.document.edit_snapshot();
                if !collapsed {
                    document.apply_edit(Edit::Delete(selection), &mut Vec::new())?;
                }
                let after = document.enter_at(at)?;
                self.commit(document, before, Selection::caret(after), None);
                return Ok(true);
            }
        };
        let mut edits = Vec::new();
        if !collapsed {
            edits.push(Edit::Delete(selection));
        }
        edits.push(edit);
        // Follow the insertion or split even when the input targeted upstream text.
        let cursor = BlockPosition {
            affinity: Affinity::Downstream,
            ..at
        };
        self.apply(
            Transaction::new(edits),
            before,
            cursor,
            resulting_caret_affinity,
            now.filter(|_| is_typing && collapsed && before == selection),
        )?;
        if check_shortcut
            && let Some((document, caret)) = self
                .document
                .input_shortcut(at.offset, self.selection.focus)
        {
            self.commit(document, self.selection, Selection::caret(caret), None);
        }
        Ok(true)
    }

    fn delete(&mut self, selection: Selection, direction: Direction) -> Result<bool, EditError> {
        let (at, collapsed) = self.selected_start(selection)?;
        if collapsed
            && matches!(direction, Direction::Backward)
            && let Some((document, caret)) = self.document.exit_item_on_backspace(at)
        {
            self.commit(document, selection, Selection::caret(caret), None);
            return Ok(true);
        }
        let edit = if collapsed {
            let Some(edit) = self.build_delete_edit_at_caret(at, &direction) else {
                return Ok(false);
            };
            edit
        } else {
            Edit::Delete(selection)
        };
        let affinity = if collapsed
            && matches!(direction, Direction::Backward)
            && at.offset > 0
            && self.document.in_mark_before(at)
        {
            Affinity::Upstream
        } else {
            at.affinity
        };
        self.apply(Transaction::new([edit]), selection, at, affinity, None)
    }

    fn build_delete_edit_at_caret(&self, at: BlockPosition, direction: &Direction) -> Option<Edit> {
        let offset = match direction {
            Direction::Backward => at.offset.checked_sub(1),
            Direction::Forward => {
                (at.offset < self.document.block_len(at.block)).then(|| at.offset + 1)
            }
        };
        if let Some(offset) = offset {
            return Some(Edit::Delete(Selection {
                anchor: at,
                focus: BlockPosition { offset, ..at },
            }));
        }

        self.document
            .boundary_join(at, matches!(direction, Direction::Backward))
            .map(Edit::Delete)
    }

    fn apply(
        &mut self,
        transaction: Transaction,
        before: Selection,
        cursor: BlockPosition,
        resulting_caret_affinity: Affinity,
        typing_at: Option<Duration>,
    ) -> Result<bool, EditError> {
        let (document, changes) = transaction.stage(&self.document)?;
        let mapped = changes
            .get_position_after_edits(BeforeEditPosition::new(cursor))
            .into_position();
        let after = Selection::caret(BlockPosition {
            affinity: resulting_caret_affinity,
            ..mapped
        });
        self.commit(document, before, after, typing_at);
        Ok(true)
    }

    fn commit(
        &mut self,
        document: Document,
        before: Selection,
        after: Selection,
        typing_at: Option<Duration>,
    ) {
        let before_document = std::mem::replace(&mut self.document, document);
        self.record_edit_history(
            HistoryEntry {
                document: before_document,
                before,
                after,
            },
            typing_at,
        );
        self.selection = after;
    }

    fn record_edit_history(&mut self, entry: HistoryEntry, typing_at: Option<Duration>) {
        let continues_typing = typing_at
            .zip(self.last_typing_at)
            .and_then(|(now, last)| now.checked_sub(last))
            .is_some_and(|gap| gap <= TYPING_GROUP_DELAY);
        if let Some(previous) = self
            .undo
            .last_mut()
            .filter(|previous| continues_typing && previous.after == entry.before)
        {
            previous.after = entry.after;
        } else {
            self.undo.push(entry);
        }
        self.redo.clear();
        self.last_typing_at = typing_at;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{block, document, paragraph, strong, text, text_block};
    use crate::{HeadingLevel, InlineKind, NodeId};

    mod scenarios;

    fn block_position(block: NodeId, offset: usize) -> BlockPosition {
        BlockPosition {
            block,
            offset,
            affinity: Affinity::Downstream,
        }
    }

    fn editor_at(document: Document, at: BlockPosition) -> EditorEngine {
        EditorEngine::new(document, Selection::caret(at)).unwrap()
    }

    fn assert_text(editor: &EditorEngine, id: NodeId, value: &str) {
        assert_eq!(
            editor.document.node(id).unwrap().kind(),
            &NodeKind::Inline(InlineKind::Text(value.into()))
        );
    }

    #[test]
    fn document_replacement_restores_the_imported_selection_on_redo() {
        let original = document([paragraph([text("old")])]).unwrap();
        let replacement = document([paragraph([text("新😀")])]).unwrap();
        let old_block = original.blocks()[0];
        let before = Selection::caret(block_position(old_block, 2));
        let new_block = replacement.blocks()[0];
        let after = Selection {
            anchor: block_position(new_block, 2),
            focus: block_position(new_block, 1),
        };
        let mut editor = EditorEngine::new(original, before).unwrap();
        editor
            .replace_document(before, &replacement, after, None)
            .unwrap();
        let imported = editor.selection();
        editor.execute(Command::Undo, imported, None).unwrap();

        let redone = editor.execute(Command::Redo, before, None).unwrap();

        assert!(redone);
        assert_eq!(editor.selection(), imported);
        assert_eq!((imported.anchor.offset, imported.focus.offset), (2, 1));
        assert_ne!(imported.anchor.block, old_block);
        assert!(editor.document().node(old_block).is_none());
        let paragraph = editor.document().node(imported.anchor.block).unwrap();
        assert_text(&editor, paragraph.children()[0], "新😀");
    }

    #[test]
    fn typing_groups_use_the_gap_since_the_last_insertion() {
        let doc = document([paragraph([])]).unwrap();
        let at = block_position(doc.blocks()[0], 0);
        let mut editor = editor_at(doc, at);

        for (text, millis) in [("a", 0), ("b", 500), ("c", 1000), ("d", 1501)] {
            editor
                .execute(
                    Command::InsertText(text.into()),
                    editor.selection(),
                    Some(Duration::from_millis(millis)),
                )
                .unwrap();
        }
        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();

        let id = editor.document.node(at.block).unwrap().children()[0];
        assert_text(&editor, id, "abc");
        assert_eq!(editor.undo.len(), 1);
    }

    #[test]
    fn untimed_typing_is_separate_from_both_neighbors() {
        let mut id = None;
        let doc = document([paragraph([text("x").capture(&mut id)])]).unwrap();
        let at = block_position(doc.blocks()[0], 1);
        let mut editor = editor_at(doc, at);
        let id = id.unwrap();

        for (text, time) in [
            ("a", Some(Duration::ZERO)),
            ("b", None),
            ("c", Some(Duration::ZERO)),
        ] {
            editor
                .execute(Command::InsertText(text.into()), editor.selection(), time)
                .unwrap();
        }
        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();
        let after_first_undo = editor.selection();
        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();

        assert_eq!(after_first_undo.focus.offset, 3);
        assert_text(&editor, id, "xa");
    }

    #[test]
    fn consecutive_typing_at_a_formatting_boundary_stays_in_one_context_and_undo_step() {
        let mut bold_text = None;
        let doc = document([paragraph([
            strong([text("a").capture(&mut bold_text)]),
            text("b"),
        ])])
        .unwrap();
        let at = BlockPosition {
            affinity: Affinity::Upstream,
            ..block_position(doc.blocks()[0], 1)
        };
        let mut editor = editor_at(doc, at);
        let now = Some(Duration::ZERO);
        editor
            .execute(Command::InsertText("日".into()), editor.selection(), now)
            .unwrap();
        editor.set_selection(editor.selection()).unwrap();
        editor
            .execute(Command::InsertText("😀".into()), editor.selection(), now)
            .unwrap();
        let typed = editor.selection();

        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();
        let before = editor.selection();
        let more = editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();
        editor
            .execute(Command::Redo, editor.selection(), None)
            .unwrap();

        assert!(!more);
        assert_eq!(before, Selection::caret(at));
        assert_eq!(typed, Selection::caret(BlockPosition { offset: 3, ..at }));
        assert_eq!(editor.selection(), typed);
        assert_text(&editor, bold_text.unwrap(), "a日😀");
    }

    #[test]
    fn moving_the_caret_separates_typing_groups_and_redo_restores_the_edit_caret() {
        let doc = document([paragraph([])]).unwrap();
        let at = block_position(doc.blocks()[0], 0);
        let mut editor = editor_at(doc, at);
        let now = Some(Duration::ZERO);
        editor
            .execute(Command::InsertText("a".into()), editor.selection(), now)
            .unwrap();
        let id = editor.document.node(at.block).unwrap().children()[0];
        let after_a = editor.selection();
        editor.set_selection(Selection::caret(at)).unwrap();
        editor.set_selection(after_a).unwrap();
        editor
            .execute(Command::InsertText("b".into()), editor.selection(), now)
            .unwrap();
        let typed = editor.selection();
        editor.set_selection(Selection::caret(at)).unwrap();

        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();
        assert_text(&editor, id, "a");
        assert_eq!(editor.selection(), after_a);

        editor
            .execute(Command::Redo, editor.selection(), None)
            .unwrap();
        assert_text(&editor, id, "ab");
        assert_eq!(editor.selection(), typed);
    }

    #[test]
    fn replacing_a_backward_cross_paragraph_selection_is_one_undo_step() {
        let (mut a, mut b) = (None, None);
        let doc = document([
            paragraph([text("abc").capture(&mut a)]),
            paragraph([text("def").capture(&mut b)]),
        ])
        .unwrap();
        let (a, b) = (a.unwrap(), b.unwrap());
        let (left, right) = (doc.blocks()[0], doc.blocks()[1]);
        let selection = Selection {
            anchor: block_position(right, 2),
            focus: block_position(left, 1),
        };
        let mut editor = editor_at(doc, block_position(left, 0));

        editor
            .execute(Command::InsertText("😀".into()), selection, None)
            .unwrap();
        assert_eq!(editor.document.blocks(), &[left]);
        assert_text(&editor, a, "a😀f");
        assert_eq!(editor.selection().focus.offset, 2);

        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();
        assert_eq!(editor.document.blocks(), &[left, right]);
        assert_text(&editor, a, "abc");
        assert_text(&editor, b, "def");
        assert_eq!(editor.selection(), selection);
    }

    #[stratalith_macros::rstest]
    #[case::backspace(Command::Backspace, "a界", 1)]
    #[case::delete(Command::Delete, "a😀", 2)]
    fn directional_deletion_at_a_caret_removes_one_scalar(
        #[case] command: Command,
        #[case] expected: &str,
        #[case] offset: usize,
    ) {
        let mut id = None;
        let doc = document([paragraph([text("a😀界").capture(&mut id)])]).unwrap();
        let at = block_position(doc.blocks()[0], 2);
        let mut editor = editor_at(doc, at);

        editor.execute(command, editor.selection(), None).unwrap();

        assert_text(&editor, id.unwrap(), expected);
        assert_eq!(
            editor.selection(),
            Selection::caret(BlockPosition { offset, ..at })
        );
    }

    #[stratalith_macros::rstest]
    #[case::backspace(Command::Backspace)]
    #[case::delete(Command::Delete)]
    fn directional_deletion_removes_the_entire_selection(#[case] command: Command) {
        let mut id = None;
        let doc = document([paragraph([text("a😀界b").capture(&mut id)])]).unwrap();
        let block = doc.blocks()[0];
        let mut editor = EditorEngine::new(
            doc,
            Selection {
                anchor: block_position(block, 3),
                focus: block_position(block, 1),
            },
        )
        .unwrap();

        editor.execute(command, editor.selection(), None).unwrap();

        assert_text(&editor, id.unwrap(), "ab");
        assert_eq!(
            editor.selection(),
            Selection::caret(block_position(block, 1))
        );
    }

    #[stratalith_macros::rstest]
    #[case::backspace(Command::Backspace, 1, 0)]
    #[case::delete(Command::Delete, 0, 1)]
    fn directional_deletion_joins_sibling_paragraphs_at_the_boundary(
        #[case] command: Command,
        #[case] paragraph_index: usize,
        #[case] offset: usize,
    ) {
        let mut node = None;
        let doc = document([block(
            BlockKind::BlockQuote,
            [
                paragraph([text("左").capture(&mut node)]),
                paragraph([text("右")]),
            ],
        )])
        .unwrap();
        let parent = doc.blocks()[0];
        let siblings = doc.node(parent).unwrap().children();
        let left = siblings[0];
        let at = block_position(siblings[paragraph_index], offset);
        let mut editor = editor_at(doc, at);

        editor.execute(command, editor.selection(), None).unwrap();

        assert_eq!(editor.document.node(parent).unwrap().children(), &[left]);
        assert_text(&editor, node.unwrap(), "左右");
        assert_eq!(
            editor.selection(),
            Selection::caret(block_position(left, 1))
        );
    }

    #[test]
    fn enter_replaces_the_selection_and_redo_restores_the_split_ids() {
        let mut id = None;
        let doc = document([paragraph([text("abcd").capture(&mut id)])]).unwrap();
        let left = doc.blocks()[0];
        let selection = Selection {
            anchor: block_position(left, 1),
            focus: block_position(left, 3),
        };
        let mut editor = EditorEngine::new(doc, selection).unwrap();
        editor
            .execute(Command::Enter, editor.selection(), None)
            .unwrap();
        let right = editor.document.blocks()[1];
        let suffix = editor.document.node(right).unwrap().children()[0];

        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();
        assert_text(&editor, id.unwrap(), "abcd");
        assert_eq!(editor.selection(), selection);

        editor
            .execute(Command::Redo, editor.selection(), None)
            .unwrap();
        assert_eq!(editor.document.blocks(), &[left, right]);
        assert_text(&editor, id.unwrap(), "a");
        assert_text(&editor, suffix, "d");
        assert_eq!(
            editor.selection(),
            Selection::caret(block_position(right, 0))
        );
    }

    #[test]
    fn a_new_edit_after_undo_discards_redo_without_reusing_abandoned_ids() {
        let doc = document([paragraph([])]).unwrap();
        let at = block_position(doc.blocks()[0], 0);
        let mut editor = editor_at(doc, at);
        editor
            .execute(Command::Enter, editor.selection(), None)
            .unwrap();
        let abandoned = editor.selection().focus;
        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();

        editor
            .execute(Command::Enter, editor.selection(), None)
            .unwrap();
        let stale = editor.set_selection(Selection::caret(abandoned));
        let redone = editor
            .execute(Command::Redo, editor.selection(), None)
            .unwrap();

        assert!(matches!(stale, Err(PositionError::NodeNotFound(id)) if id == abandoned.block));
        assert!(!redone);
        assert_ne!(editor.selection().focus.block, abandoned.block);
    }

    #[stratalith_macros::rstest]
    #[case::backspace_at_document_start(Command::Backspace)]
    #[case::delete_at_document_end(Command::Delete)]
    #[case::empty_insertion(Command::InsertText(String::new()))]
    fn no_op_preserves_redo(#[case] command: Command) {
        let doc = document([paragraph([])]).unwrap();
        let at = block_position(doc.blocks()[0], 0);
        let mut editor = editor_at(doc, at);
        editor
            .execute(Command::InsertText("x".into()), editor.selection(), None)
            .unwrap();
        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();

        let changed = editor.execute(command, editor.selection(), None).unwrap();

        assert!(!changed);
        assert_eq!(editor.document(), &document([paragraph([])]).unwrap());
        assert_eq!(editor.selection(), Selection::caret(at));

        let redone = editor
            .execute(Command::Redo, editor.selection(), None)
            .unwrap();

        assert!(redone);
        assert_eq!(
            editor.document(),
            &document([paragraph([text("x")])]).unwrap()
        );
        assert_eq!(
            editor.selection(),
            Selection::caret(BlockPosition {
                offset: 1,
                affinity: Affinity::Upstream,
                ..at
            })
        );
    }

    #[test]
    fn failed_split_after_range_deletion_preserves_document_selection_and_redo() {
        let mut id = None;
        let doc = document([text_block(
            BlockKind::Heading {
                level: HeadingLevel::H1,
            },
            [text("abc").capture(&mut id)],
        )])
        .unwrap();
        let block = doc.blocks()[0];
        let selection = Selection {
            anchor: block_position(block, 0),
            focus: block_position(block, 2),
        };
        let mut editor = EditorEngine::new(doc, selection).unwrap();
        editor
            .execute(Command::Delete, editor.selection(), None)
            .unwrap();
        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();

        let other_range = Selection {
            focus: block_position(block, 1),
            ..selection
        };
        let transaction = Transaction::new([
            Edit::Delete(other_range),
            Edit::SplitParagraph(other_range.anchor),
        ]);

        let split = editor.apply(
            transaction,
            other_range,
            other_range.anchor,
            Affinity::Downstream,
            None,
        );

        assert!(matches!(split, Err(EditError::ExpectedParagraph(id)) if id == block));
        assert_text(&editor, id.unwrap(), "abc");
        assert_eq!(editor.selection(), selection);
        assert!(
            editor
                .execute(Command::Redo, editor.selection(), None)
                .unwrap()
        );
        assert_text(&editor, id.unwrap(), "c");
    }

    #[test]
    fn enter_across_paragraph_and_code_preserves_document_selection_and_redo() {
        let doc = document([
            paragraph([text("ab")]),
            block(
                BlockKind::CodeBlock {
                    language: None,
                    code: "cd".into(),
                },
                [],
            ),
        ])
        .unwrap();
        let at = block_position(doc.blocks()[0], 1);
        let range = Selection {
            anchor: at,
            focus: block_position(doc.blocks()[1], 1),
        };
        let mut editor = editor_at(doc, at);
        editor
            .execute(Command::InsertText("x".into()), editor.selection(), None)
            .unwrap();
        editor
            .execute(Command::Undo, editor.selection(), None)
            .unwrap();
        let before = editor.document().edit_snapshot();

        let result = editor.execute(Command::Enter, range, None);

        assert!(matches!(result, Err(EditError::IncompatibleBlockContents)));
        assert_eq!(editor.document(), &before);
        assert_eq!(editor.selection(), Selection::caret(at));
        assert!(
            editor
                .execute(Command::Redo, editor.selection(), None)
                .unwrap()
        );
        assert_eq!(editor.selection().focus.offset, 2);
    }

    #[test]
    fn enter_inside_code_inserts_a_newline_in_the_same_block() {
        let code = BlockKind::CodeBlock {
            language: Some("rs".into()),
            code: "日😀".into(),
        };
        let doc = document([block(code, [])]).unwrap();
        let at = block_position(doc.blocks()[0], 1);
        let mut editor = editor_at(doc, at);

        editor
            .execute(Command::Enter, editor.selection(), None)
            .unwrap();

        assert_eq!(editor.document.blocks(), &[at.block]);
        assert_eq!(
            editor.document.node(at.block).unwrap().kind(),
            &NodeKind::Block(BlockKind::CodeBlock {
                language: Some("rs".into()),
                code: "日\n😀".into(),
            }),
        );
        assert_eq!(
            editor.selection(),
            Selection::caret(block_position(at.block, 2))
        );
    }
}
