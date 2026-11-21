use crate::{MarkdownError, parse};
use stratalith_core::{BlockKind, Command, Document, InlineKind, NodeKind, Selection};

/// Prepares an edit for clipboard text at the target selection. Empty text returns
/// `None` so pasting an empty clipboard preserves the selection. Code blocks
/// receive literal text. Other blocks accept Markdown formatting.
///
/// ```
/// use stratalith_core::{Affinity, BlockPosition, EditorEngine, Selection};
/// use stratalith_markdown::{parse, paste_command, serialize_fragment};
///
/// let doc = parse("Start")?;
/// let target = Selection::caret(BlockPosition {
///     block: doc.blocks()[0], offset: 5, affinity: Affinity::Upstream,
/// });
/// let mut editor = EditorEngine::new(doc, target)?;
///
/// let command = paste_command(editor.document(), target, "**bold**")?.unwrap();
/// editor.execute(command, target, None)?;
///
/// assert_eq!(serialize_fragment(editor.document())?, "Start**bold**");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn paste_command(
    doc: &Document,
    target: Selection,
    text: &str,
) -> Result<Option<Command>, MarkdownError> {
    if text.is_empty() {
        return Ok(None);
    }
    let start = target.range(doc).map_err(MarkdownError::Position)?.start();
    if !matches!(
        doc.node(start.block).unwrap().kind(),
        NodeKind::Block(BlockKind::CodeBlock { .. })
    ) {
        let fragment = parse(text)?;
        let mut nodes = fragment.nodes().map(|n| n.kind());
        let plain = matches!(nodes.next(), Some(NodeKind::Block(BlockKind::Paragraph)))
            && matches!(nodes.next(), Some(NodeKind::Inline(InlineKind::Text(value))) if value == text.trim() || value == text)
            && nodes.next().is_none();
        if !plain && !fragment.blocks().is_empty() {
            return Ok(Some(Command::Paste { target, fragment }));
        }
    }
    Ok(Some(Command::ReplaceRange {
        target,
        text: text.into(),
    }))
}
