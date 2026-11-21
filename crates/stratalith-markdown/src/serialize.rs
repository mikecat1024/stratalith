mod children;
mod inlines;

use crate::{MarkdownError, error::check_depth, heading::LEVELS};
use children::equivalent;
use std::fmt::Write;
use stratalith_core::{BlockKind, Document, ListKind, Node, NodeId, NodeKind};

/// Emits canonical Markdown using LF line endings. Returns an error for Core
/// states whose content or structure cannot be preserved by the rendering,
/// such as empty code spans. Adjacent text nodes are one run of literal text.
pub fn serialize(doc: &Document) -> Result<String, MarkdownError> {
    let mut output = serialize_fragment(doc)?;
    if !output.is_empty() {
        output.push('\n');
    }
    // Editor-created trees can contain boundaries Markdown cannot express
    // (e.g. an empty paragraph between two paragraphs). Check before saving.
    let parsed = crate::parse(&output)?;
    if !equivalent(doc, &parsed) {
        return Err(MarkdownError::Unrepresentable(
            "Markdown cannot preserve this document's content or node boundaries",
        ));
    }
    Ok(output)
}

/// Formats selected content without appending a document terminator or requiring
/// a standalone parse to reconstruct every clipped node boundary.
///
/// ```
/// use stratalith_markdown::{parse, serialize_fragment};
///
/// let fragment = parse("😀**O**").unwrap();
///
/// assert_eq!(serialize_fragment(&fragment).unwrap(), "😀**O**");
/// ```
pub fn serialize_fragment(doc: &Document) -> Result<String, MarkdownError> {
    Renderer(doc).blocks(doc.blocks(), "\n\n", 0)
}

struct Renderer<'a>(&'a Document);

fn escaped(s: &str, punctuation: &str) -> String {
    let mut out = String::new();
    for (offset, c) in s.char_indices() {
        let at_edge = offset == 0 || offset + c.len_utf8() == s.len();
        if c.is_ascii_control() || (at_edge && c.is_whitespace()) {
            write!(out, "&#{};", u32::from(c)).unwrap();
            continue;
        }
        if punctuation.contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn fence_width(s: &str, marker: char, minimum: usize) -> usize {
    let longest = s.split(|c| c != marker).map(str::len).max().unwrap_or(0);
    minimum.max(longest + 1)
}

impl Renderer<'_> {
    fn node(&self, id: NodeId) -> &Node {
        self.0.node(id).expect("linked document node")
    }

    fn blocks(
        &self,
        ids: &[NodeId],
        separator: &str,
        depth: usize,
    ) -> Result<String, MarkdownError> {
        check_depth(depth)?;
        let mut output = String::new();
        let mut alternate = false;
        for (index, &id) in ids.iter().enumerate() {
            if index != 0 {
                output.push_str(separator);
            }
            let node = self.node(id);
            let children = node.children();
            let NodeKind::Block(kind) = node.kind() else {
                unreachable!()
            };
            match kind {
                BlockKind::Paragraph => self.write_inlines(&mut output, children, None, depth)?,
                BlockKind::Heading { level } => {
                    let n = LEVELS.iter().position(|v| v == level).unwrap() + 1;
                    let mut text = String::new();
                    self.write_inlines(&mut text, children, None, depth)?;
                    if n <= 2 && text.contains('\n') {
                        output.push_str(&text);
                        output.push_str(if n == 1 { "\n===" } else { "\n---" });
                    } else {
                        output.extend(std::iter::repeat_n('#', n));
                        output.push(' ');
                        output.push_str(&text);
                    }
                }
                BlockKind::ThematicBreak => output.push_str("***"),
                BlockKind::CodeBlock { language, code } => {
                    let info = language.as_deref().unwrap_or_default();
                    let mark = if info.contains('`') { '~' } else { '`' };
                    let fence = mark.to_string().repeat(fence_width(code, mark, 3));
                    let newline = if code.is_empty() || code.ends_with('\n') {
                        ""
                    } else {
                        "\n"
                    };
                    let info = escaped(info, "&\\");
                    let space = if info.starts_with(mark) { " " } else { "" };
                    write!(output, "{fence}{space}{info}\n{code}{newline}{fence}").unwrap();
                }
                BlockKind::BlockQuote => {
                    let body = self.blocks(children, "\n\n", depth + 1)?;
                    output.push_str("> ");
                    for (index, line) in body.split('\n').enumerate() {
                        if index > 0 {
                            output.push_str("\n> ");
                        }
                        output.push_str(line);
                    }
                }
                BlockKind::List { kind, tight } => {
                    self.write_list(&mut output, children, *kind, *tight, alternate, depth + 1)?
                }
                BlockKind::Html(html) => output.push_str(html.strip_suffix('\n').unwrap_or(html)),
                BlockKind::ListItem => unreachable!("list renders its items"),
            };
            alternate = matches!(kind, BlockKind::List { .. }) && !alternate;
        }
        Ok(output)
    }

    fn write_list(
        &self,
        out: &mut String,
        items: &[NodeId],
        kind: ListKind,
        tight: bool,
        alternate: bool,
        depth: usize,
    ) -> Result<(), MarkdownError> {
        let separator = if tight { "\n" } else { "\n\n" };
        let marker = match kind {
            ListKind::Bullet => if alternate { "+" } else { "-" }.to_owned(),
            ListKind::Ordered { start } => format!("{start}{}", if alternate { ')' } else { '.' }),
        };
        let indent = " ".repeat(marker.len() + 1);
        for (index, &item) in items.iter().enumerate() {
            if index > 0 {
                out.push_str(separator);
            }
            out.push_str(&marker);
            out.push(' ');
            let body = self.blocks(self.node(item).children(), separator, depth)?;
            let mut lines = body.split('\n');
            out.push_str(lines.next().unwrap());
            for line in lines {
                out.push('\n');
                if !line.is_empty() {
                    out.push_str(&indent);
                    out.push_str(line);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stratalith_core::HeadingLevel;
    use stratalith_core::builder::{block, document, paragraph, text, text_block};

    #[test]
    fn serializes_editor_created_nodes_with_literal_markup() {
        let doc = document([text_block(
            BlockKind::Heading {
                level: HeadingLevel::H4,
            },
            [text("a * b")],
        )])
        .unwrap();

        let markdown = serialize(&doc).unwrap();

        assert_eq!(markdown, "#### a \\* b\n");
    }

    #[test]
    fn serializes_editor_created_html_verbatim() {
        let html = "<section>\n";
        let doc = document([block(BlockKind::Html(html.into()), [])]).unwrap();

        let markdown = serialize(&doc).unwrap();

        assert_eq!(markdown, html);
    }

    #[test]
    fn refuses_to_drop_an_empty_paragraph_between_blocks() {
        let doc = document([
            text_block(BlockKind::Paragraph, [text("first")]),
            block(BlockKind::Paragraph, []),
            text_block(BlockKind::Paragraph, [text("last")]),
        ])
        .unwrap();

        let result = serialize(&doc);

        assert!(matches!(result, Err(MarkdownError::Unrepresentable(_))));
    }

    #[stratalith_macros::rstest]
    #[case::ordered_list_marker_and_cjk("1. \u{4e00}\u{4e8c}")]
    #[case::leading_spaces("  leading")]
    #[case::trailing_spaces("trailing  ")]
    #[case::emphasis_markers_and_cjk("*\u{4e00}\u{4e8c}*")]
    #[case::emoji_and_backslash("😀\\x")]
    fn splitting_editor_text_nodes_does_not_change_markdown(#[case] value: &str) {
        let whole = document([paragraph([text(value)])]).unwrap();
        let unsplit_markdown = serialize(&whole).unwrap();
        let splits: Vec<_> = value
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(value.len()))
            .map(|offset| {
                let split = document([paragraph([
                    text(""),
                    text(&value[..offset]),
                    text(""),
                    text(&value[offset..]),
                    text(""),
                ])])
                .unwrap();
                (offset, split)
            })
            .collect();
        let expected: Vec<_> = splits
            .iter()
            .map(|(offset, _)| (*offset, unsplit_markdown.clone()))
            .collect();

        let actual: Vec<_> = splits
            .into_iter()
            .map(|(offset, split)| {
                let markdown = serialize(&split)
                    .unwrap_or_else(|error| panic!("{value:?} split at {offset}: {error}"));
                (offset, markdown)
            })
            .collect();

        assert_eq!(actual, expected, "{value:?} at each character boundary");
    }
}
