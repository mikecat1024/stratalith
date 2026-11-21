use stratalith_core::{
    Affinity, BlockKind, BlockPosition, Document, InlineKind, Node, NodeId, NodeKind, Selection,
};
use stratalith_markdown::parse;

// Byte offsets in the Markdown source after removing the fixture markers.
pub(super) enum SourceSelection {
    Caret(usize),
    Range { anchor: usize, focus: usize },
}

pub(super) fn extract_markers(input: &str) -> (String, SourceSelection) {
    let (source, markers) = optional_markers(input);
    (
        source,
        markers.expect("expected caret or selection markers"),
    )
}

pub(super) fn optional_markers(input: &str) -> (String, Option<SourceSelection>) {
    let mut source = String::with_capacity(input.len());
    let (mut caret, mut anchor, mut focus) = (None, None, None);
    let mut rest = input;
    while !rest.is_empty() {
        let endpoint = match rest.get(..2) {
            Some("$$") => &mut caret,
            Some("[|") => &mut anchor,
            Some("|]") => &mut focus,
            _ => {
                let ch = rest.chars().next().unwrap();
                source.push(ch);
                rest = &rest[ch.len_utf8()..];
                continue;
            }
        };
        assert!(
            endpoint.replace(source.len()).is_none(),
            "duplicate selection marker {}",
            &rest[..2]
        );
        rest = &rest[2..];
    }
    let selection = match (caret, anchor, focus) {
        (Some(at), None, None) => Some(SourceSelection::Caret(at)),
        (None, Some(anchor), Some(focus)) => Some(SourceSelection::Range { anchor, focus }),
        (None, None, None) => None,
        _ => panic!("use one $$ or one each of [| and |]"),
    };
    (source, selection)
}

const SENTINEL: &str = "\u{e000}";

fn probe(source: &str, offset: usize) -> Document {
    assert!(
        !source.contains(SENTINEL),
        "input contains the internal sentinel"
    );
    let mut probe = source.to_owned();
    probe.insert_str(offset, SENTINEL);
    parse(&probe).expect("parse position probe")
}

impl SourceSelection {
    pub(super) fn editable_document(&self, source: &str) -> Document {
        // Parsing a bare list marker produces a childless item. Parsing it with
        // a caret placeholder gives the editor the empty paragraph it needs.
        if let Self::Caret(at) = self {
            let mut marked = probe(source, *at);
            let placeholder = marked.nodes().find_map(|node| {
                let paragraph = marked.node(node.parent()?)?;
                let item = marked.node(paragraph.parent()?)?;
                let empty_item_text = matches!(node.kind(), NodeKind::Inline(InlineKind::Text(text)) if text == SENTINEL)
                    && matches!(paragraph.kind(), NodeKind::Block(BlockKind::Paragraph))
                    && paragraph.children().len() == 1
                    && matches!(item.kind(), NodeKind::Block(BlockKind::ListItem));
                empty_item_text.then_some(node.id())
            });
            if let Some(id) = placeholder {
                marked.remove_node(id).unwrap();
                return marked;
            }
        }
        parse(source).expect("parse input Markdown")
    }

    pub(super) fn resolve(self, source: &str, doc: &Document) -> Selection {
        // Probe one endpoint at a time. Source bytes and editor offsets differ
        // for Markdown syntax, multibyte characters, and inline objects.
        let position = |offset, marker: &str| {
            locate(doc, &probe(source, offset), SENTINEL).unwrap_or_else(|| {
                let (before, after) = source.split_at(offset);
                panic!("invalid marker {marker} at byte {offset} in {before}{marker}{after}: place it in editable text or at its boundary")
            })
        };
        match self {
            Self::Caret(at) => Selection::caret(position(at, "$$")),
            Self::Range { anchor, focus } => Selection {
                anchor: position(anchor, "[|"),
                focus: position(focus, "|]"),
            },
        }
    }
}

fn text_blocks(doc: &Document) -> impl Iterator<Item = &Node> {
    doc.nodes().filter(|node| {
        matches!(
            node.kind(),
            NodeKind::Block(
                BlockKind::Paragraph | BlockKind::Heading { .. } | BlockKind::CodeBlock { .. }
            )
        )
    })
}

fn locate(doc: &Document, marked: &Document, marker: &str) -> Option<BlockPosition> {
    if text_blocks(marked).count() != text_blocks(doc).count() {
        return None;
    }
    for (probe, original) in text_blocks(marked).zip(text_blocks(doc)) {
        let mut pieces = Vec::new();
        collect_text(marked, probe.id(), &mut pieces);
        let mut offset = 0;
        for piece in &pieces {
            if let Some((before, after)) = piece.split_once(marker) {
                let mut clean = Vec::new();
                collect_text(doc, original.id(), &mut clean);
                if pieces.concat().replacen(marker, "", 1) != clean.concat() {
                    return None;
                }
                // A marker just inside a closing delimiter belongs to the preceding inline.
                let affinity = match (before.is_empty(), after.is_empty()) {
                    (false, true) => Affinity::Upstream,
                    _ => Affinity::Downstream,
                };
                return Some(BlockPosition {
                    block: original.id(),
                    offset: offset + before.chars().count(),
                    affinity,
                });
            }
            offset += piece.chars().count();
        }
    }
    None
}

// Images, breaks, and inline HTML occupy one cursor unit regardless of source length.
fn collect_text<'a>(doc: &'a Document, id: NodeId, pieces: &mut Vec<&'a str>) {
    let node = doc.node(id).unwrap();
    match node.kind() {
        NodeKind::Block(BlockKind::CodeBlock { code: text, .. })
        | NodeKind::Inline(InlineKind::Text(text) | InlineKind::Code(text)) => pieces.push(text),
        NodeKind::Inline(
            InlineKind::Image { .. }
            | InlineKind::Html(_)
            | InlineKind::HardBreak
            | InlineKind::SoftBreak,
        ) => pieces.push("\u{fffc}"),
        _ => {
            for &child in node.children() {
                collect_text(doc, child, pieces);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "invalid marker $$ at byte 6 in **one*$$*")]
    fn rejects_caret_inside_closing_strong_delimiter() {
        let (source, markers) = extract_markers("**one*$$*");
        let doc = parse(&source).unwrap();

        markers.resolve(&source, &doc);
    }

    #[test]
    fn extracts_backward_range_in_source_bytes() {
        let input = "**日|]😀[|本**";

        let (source, selection) = extract_markers(input);

        assert_eq!(source, "**日😀本**");
        let SourceSelection::Range { anchor, focus } = selection else {
            panic!("expected a range");
        };
        assert_eq!((anchor, focus), ("**日😀".len(), "**日".len()));
    }
}
