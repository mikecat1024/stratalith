//! WebAssembly entry points for plain-text editing, rich-text editing and Markdown conversion.
//! Plain-text host positions and patch ranges are UTF-16 offsets.

#![cfg(target_arch = "wasm32")]

mod markdown;
mod rich_text;
mod tree;

use std::{cell::RefCell, time::Duration};
use stratalith_core::{
    Affinity, BlockPosition, Command, Document, EditorEngine, InlineKind, NodeId, NodeKind,
    Selection, builder,
};

struct View {
    text: String,
    // UTF-8 start of each paragraph in the joined text.
    blocks: Vec<(NodeId, usize)>,
}

impl View {
    fn render(document: &Document) -> Self {
        let mut text = String::new();
        let mut blocks = Vec::new();
        for &id in document.blocks() {
            if !blocks.is_empty() {
                text.push('\n');
            }
            blocks.push((id, text.len()));
            for &child in document.node(id).unwrap().children() {
                match document.node(child).unwrap().kind() {
                    NodeKind::Inline(InlineKind::Text(value)) => text.push_str(value),
                    NodeKind::Inline(InlineKind::HardBreak) => text.push('\n'),
                    _ => unreachable!("unexpected inline in plain-text editor"),
                }
            }
        }
        Self { text, blocks }
    }

    fn position(&self, offset: usize) -> Option<BlockPosition> {
        let mut units = 0;
        let mut byte = 0;
        for ch in self.text.chars() {
            if units >= offset {
                break;
            }
            units += ch.len_utf16();
            byte += ch.len_utf8();
        }
        if units != offset {
            return None;
        }
        let index = self.blocks.partition_point(|&(_, start)| start <= byte) - 1;
        let (block, start) = self.blocks[index];
        Some(BlockPosition {
            block,
            offset: self.text[start..byte].chars().count(),
            affinity: Affinity::Downstream,
        })
    }

    fn offset(&self, position: BlockPosition) -> usize {
        let &(_, start) = self
            .blocks
            .iter()
            .find(|&&(id, _)| id == position.block)
            .unwrap();
        self.text[..start].encode_utf16().count()
            + self.text[start..]
                .chars()
                .take(position.offset)
                .map(char::len_utf16)
                .sum::<usize>()
    }
}

// Trim equal scalar values so a replacement never splits a surrogate pair.
fn patch<'a>(before: &str, after: &'a str) -> (usize, usize, &'a str) {
    let prefix: usize = before
        .chars()
        .zip(after.chars())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum();
    let (old, new) = (&before[prefix..], &after[prefix..]);
    let suffix: usize = old
        .chars()
        .rev()
        .zip(new.chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum();
    let start = before[..prefix].encode_utf16().count();
    (
        start,
        start + old[..old.len() - suffix].encode_utf16().count(),
        &new[..new.len() - suffix],
    )
}

struct Session {
    engine: EditorEngine,
    view: View,
    input: Vec<u8>,
}

impl Session {
    fn new() -> Self {
        let document = builder::document([builder::paragraph([])]).unwrap();
        let view = View::render(&document);
        let engine =
            EditorEngine::new(document, Selection::caret(view.position(0).unwrap())).unwrap();
        Self {
            engine,
            view,
            input: Vec::new(),
        }
    }

    fn selection(&self, anchor: usize, focus: usize) -> Option<Selection> {
        // Preserve the core's affinity when the browser echoes our selection.
        let current = self.engine.selection();
        let endpoint = |offset, previous| {
            if self.view.offset(previous) == offset {
                Some(previous)
            } else {
                self.view.position(offset)
            }
        };
        Some(Selection {
            anchor: endpoint(anchor, current.anchor)?,
            focus: endpoint(focus, current.focus)?,
        })
    }
}

thread_local! { static SESSION: RefCell<Session> = RefCell::new(Session::new()); }

/// Writable UTF-8 input for the next InsertText. A subsequent resize invalidates the pointer.
#[unsafe(no_mangle)]
pub extern "C" fn prepare_input(len: usize) -> *mut u8 {
    SESSION.with_borrow_mut(|s| {
        s.input.resize(len, 0);
        s.input.as_mut_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn select(anchor: usize, focus: usize) -> bool {
    SESSION.with_borrow_mut(|s| {
        s.selection(anchor, focus)
            .is_some_and(|selection| s.engine.set_selection(selection).is_ok())
    })
}

#[link(wasm_import_module = "host")]
unsafe extern "C" {
    fn update(start: usize, end: usize, utf8: *const u8, len: usize, anchor: usize, focus: usize);
}

/// Opcodes 0..5 are InsertText, Backspace, Delete, Enter, Undo, Redo.
/// Opcodes 6..8 are InsertHardBreak, IndentListItem, OutdentListItem.
/// A successful command (including a no-op) synchronously delivers one patch.
/// Invalid commands, text or positions return false without invoking the host.
#[unsafe(no_mangle)]
pub extern "C" fn execute(op: u32, anchor: usize, focus: usize, millis: u32) -> bool {
    SESSION.with_borrow_mut(|s| {
        let Some(selection) = s.selection(anchor, focus) else {
            return false;
        };
        let command = match op {
            0 => {
                let Ok(text) = std::str::from_utf8(&s.input) else {
                    return false;
                };
                Command::InsertText(text.to_owned())
            }
            1 => Command::Backspace,
            2 => Command::Delete,
            3 => Command::Enter,
            4 => Command::Undo,
            5 => Command::Redo,
            6 => Command::InsertHardBreak,
            7 => Command::IndentListItem,
            8 => Command::OutdentListItem,
            _ => return false,
        };
        let Ok(changed) = s.engine.execute(
            command,
            selection,
            Some(Duration::from_millis(millis.into())),
        ) else {
            return false;
        };
        if !changed {
            // A no-op keeps the host selection, including a move since the last selection event.
            s.engine.set_selection(selection).unwrap();
        }
        let next = View::render(s.engine.document());
        let (start, end, insert) = patch(&s.view.text, &next.text);
        let selection = s.engine.selection();
        // SAFETY: host reads these bytes synchronously, before next is moved or dropped.
        unsafe {
            update(
                start,
                end,
                insert.as_ptr(),
                insert.len(),
                next.offset(selection.anchor),
                next.offset(selection.focus),
            )
        };
        s.view = next;
        true
    })
}
