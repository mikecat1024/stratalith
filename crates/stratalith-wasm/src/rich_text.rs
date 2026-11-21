use crate::{markdown::convert, tree};
use std::{cell::RefCell, time::Duration};
use stratalith_core::{
    Affinity, BlockPosition, Command, Document, EditorEngine, NodeId, NodeKind, Selection, builder,
};

thread_local! { static EDITOR: RefCell<EditorEngine> = RefCell::new(empty_editor()); }

fn empty_editor() -> EditorEngine {
    let document = builder::document([builder::paragraph([])]).unwrap();
    let selection = Selection::caret(BlockPosition {
        block: document.blocks()[0],
        offset: 0,
        affinity: Affinity::Downstream,
    });
    EditorEngine::new(document, selection).unwrap()
}

fn blocks(document: &Document) -> impl Iterator<Item = NodeId> + '_ {
    document.nodes().filter_map(|node| match node.kind() {
        NodeKind::Block(_) => Some(node.id()),
        _ => None,
    })
}

fn selection(document: &Document, coordinates: [usize; 5]) -> Result<Selection, String> {
    let endpoint = |index, offset, upstream| {
        Ok::<_, String>(BlockPosition {
            block: blocks(document)
                .nth(index)
                .ok_or("Invalid selection block")?,
            offset,
            affinity: if upstream {
                Affinity::Upstream
            } else {
                Affinity::Downstream
            },
        })
    };
    let [anchor, start, focus, end, affinity] = coordinates;
    Ok(Selection {
        anchor: endpoint(anchor, start, affinity & 1 != 0)?,
        focus: endpoint(focus, end, affinity & 2 != 0)?,
    })
}

fn state(editor: &EditorEngine) -> Vec<u8> {
    let selection = editor.selection();
    let document = editor.document();
    let mut output = Vec::new();
    for endpoint in [selection.anchor, selection.focus] {
        let index = blocks(document)
            .position(|id| id == endpoint.block)
            .unwrap();
        output.extend((index as u32).to_le_bytes());
        output.extend((endpoint.offset as u32).to_le_bytes());
    }
    let affinity = u32::from(selection.anchor.affinity == Affinity::Upstream)
        | (u32::from(selection.focus.affinity == Affinity::Upstream) << 1);
    output.extend(affinity.to_le_bytes());
    output.extend(tree::encode(document));
    output
}

#[unsafe(no_mangle)]
pub extern "C" fn rich_text_state() -> bool {
    convert(|_| Ok(EDITOR.with_borrow(state)))
}

/// Copies a range of the current editor document. Coordinates use block preorder.
#[unsafe(no_mangle)]
pub extern "C" fn selected_markdown(
    anchor: usize,
    start: usize,
    focus: usize,
    end: usize,
    affinity: usize,
) -> bool {
    convert(|_| {
        EDITOR.with_borrow(|editor| {
            let doc = editor.document();
            let range = selection(doc, [anchor, start, focus, end, affinity])?;
            let fragment = doc.extract(range).map_err(|error| error.to_string())?;
            stratalith_markdown::serialize_fragment(&fragment)
                .map(String::into_bytes)
                .map_err(|error| error.to_string())
        })
    })
}

/// Applies a rich-text command. Successful edits return both endpoints and the tree.
/// A successful no-op returns an empty buffer.
#[unsafe(no_mangle)]
pub extern "C" fn edit_rich_text(
    op: u32,
    anchor: usize,
    start: usize,
    focus: usize,
    end: usize,
    affinity: usize,
    typing_millis_plus_one: u32,
    target_anchor: usize,
    target_start: usize,
    target_focus: usize,
    target_end: usize,
    target_affinity: usize,
) -> bool {
    convert(|input| {
        EDITOR.with_borrow_mut(|editor| {
            let command = match op {
                0 => Command::InsertText(
                    std::str::from_utf8(input)
                        .map_err(|e| e.to_string())?
                        .into(),
                ),
                1 => Command::Backspace,
                2 => Command::Delete,
                3 => Command::Enter,
                4 => Command::Undo,
                5 => Command::Redo,
                6 => Command::InsertHardBreak,
                7 => Command::IndentListItem,
                8 => Command::OutdentListItem,
                9 | 10 => {
                    let target = selection(
                        editor.document(),
                        [
                            target_anchor,
                            target_start,
                            target_focus,
                            target_end,
                            target_affinity,
                        ],
                    )?;
                    let text = std::str::from_utf8(input).map_err(|e| e.to_string())?;
                    if op == 10 {
                        let Some(command) =
                            stratalith_markdown::paste_command(editor.document(), target, text)
                                .map_err(|e| e.to_string())?
                        else {
                            return Ok(Vec::new());
                        };
                        command
                    } else {
                        Command::ReplaceRange {
                            target,
                            text: text.into(),
                        }
                    }
                }
                _ => return Err("Unknown rich-text command".into()),
            };
            let selection = if matches!(command, Command::Undo | Command::Redo) {
                editor.selection()
            } else {
                selection(editor.document(), [anchor, start, focus, end, affinity])?
            };
            // Zero (also supplied by older JS callers) denotes an isolated edit.
            let typing_at = typing_millis_plus_one
                .checked_sub(1)
                .map(|ms| Duration::from_millis(ms.into()));
            let changed = editor
                .execute(command, selection, typing_at)
                .map_err(|error| error.to_string())?;
            Ok(if changed { state(editor) } else { Vec::new() })
        })
    })
}
