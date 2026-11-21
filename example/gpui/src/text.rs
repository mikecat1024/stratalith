use gpui::{FontStyle, FontWeight, HighlightStyle, TextLayout, UnderlineStyle, px, rgb, rgba};
use std::ops::Range;
use stratalith_core::{Affinity, BlockKind, BlockPosition, Document, InlineKind, NodeId, NodeKind};

/// Byte boundaries in the displayed string for each engine offset. Images and
/// raw HTML each occupy one engine unit even when their labels span many bytes.
pub(super) struct BlockText {
    pub id: NodeId,
    pub start: usize,
    pub text: String,
    pub boundaries: Vec<usize>,
    pub styles: Vec<(Range<usize>, HighlightStyle)>,
    pub images: Vec<String>,
    pub links: Vec<(Range<usize>, String)>,
    pub layout: Option<TextLayout>,
}

impl BlockText {
    pub fn new(doc: &Document, id: NodeId) -> Option<Self> {
        let node = doc.node(id)?;
        let mut block = Self {
            id,
            start: 0,
            text: String::new(),
            boundaries: vec![0],
            styles: Vec::new(),
            images: Vec::new(),
            links: Vec::new(),
            layout: None,
        };
        match node.kind() {
            NodeKind::Block(BlockKind::CodeBlock { code, .. }) => {
                block.push(code, false, HighlightStyle::default())
            }
            NodeKind::Block(BlockKind::Paragraph | BlockKind::Heading { .. }) => {
                for &child in node.children() {
                    block.inline(doc, child, HighlightStyle::default());
                }
            }
            _ => return None,
        }
        Some(block)
    }

    fn push(&mut self, text: &str, atomic: bool, style: HighlightStyle) {
        let start = self.text.len();
        self.text.push_str(text);
        if atomic {
            self.boundaries.push(self.text.len());
        } else {
            self.boundaries
                .extend(text.char_indices().map(|(i, c)| start + i + c.len_utf8()));
        }
        self.styles.push((start..self.text.len(), style));
    }

    fn inline(&mut self, doc: &Document, id: NodeId, mut style: HighlightStyle) {
        let node = doc.node(id).unwrap();
        let NodeKind::Inline(kind) = node.kind() else {
            return;
        };
        match kind {
            InlineKind::Text(value) => return self.push(value, false, style),
            InlineKind::Html(value) | InlineKind::Code(value) => {
                style.background_color = Some(rgba(0x88888828).into());
                return self.push(value, matches!(kind, InlineKind::Html(_)), style);
            }
            InlineKind::SoftBreak | InlineKind::HardBreak => return self.push("\n", false, style),
            InlineKind::Strong => style.font_weight = Some(FontWeight::BOLD),
            InlineKind::Emphasis => style.font_style = Some(FontStyle::Italic),
            InlineKind::Link { url, .. } => {
                let Ok(url) = gpui::http_client::Url::parse(url) else {
                    for &child in node.children() {
                        self.inline(doc, child, style);
                    }
                    return;
                };
                let start = self.text.len();
                style.color = Some(rgb(0x4989df).into());
                style.underline = Some(UnderlineStyle {
                    thickness: px(1.),
                    ..Default::default()
                });
                for &child in node.children() {
                    self.inline(doc, child, style);
                }
                if matches!(url.scheme(), "https" | "http" | "mailto") {
                    self.links.push((start..self.text.len(), url.into()));
                }
                return;
            }
            InlineKind::Image { url, .. } => {
                if let Ok(url) = gpui::http_client::Url::parse(url)
                    && matches!(url.scheme(), "http" | "https")
                {
                    self.images.push(url.into());
                }
                let start = self.text.len();
                let offset = self.boundaries.len();
                for &child in node.children() {
                    self.inline(doc, child, style);
                }
                if start == self.text.len() {
                    self.text.push('\u{fffc}');
                }
                self.boundaries.truncate(offset);
                self.boundaries.push(self.text.len());
                return;
            }
        }
        for &child in node.children() {
            self.inline(doc, child, style);
        }
    }

    pub fn word_edge(&self, byte: usize, backward: bool) -> BlockPosition {
        fn span(mut chars: impl Iterator<Item = char>) -> usize {
            let mut bytes = 0;
            let mut category = None;
            for ch in chars.by_ref() {
                let word = ch.is_alphanumeric() || ch == '_';
                if category.is_some_and(|was_word| ch.is_whitespace() || word != was_word) {
                    break;
                }
                bytes += ch.len_utf8();
                if !ch.is_whitespace() {
                    category = Some(word);
                }
            }
            bytes
        }
        let edge = if backward {
            byte - span(self.text[..byte].chars().rev())
        } else {
            byte + span(self.text[byte..].chars())
        };
        let mut position = self.position(edge);
        if !backward && self.boundaries[position.offset] < edge {
            position.offset += 1;
        }
        position
    }

    pub fn position(&self, byte: usize) -> BlockPosition {
        let offset = self
            .boundaries
            .partition_point(|&b| b <= byte)
            .saturating_sub(1);
        BlockPosition {
            block: self.id,
            offset,
            affinity: if offset == 0 {
                Affinity::Downstream
            } else {
                Affinity::Upstream
            },
        }
    }
}

/// Convert a native UTF-16 index to the preceding UTF-8 character boundary.
pub(super) fn byte_for_utf16(text: &str, index: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        units += ch.len_utf16();
        if units > index {
            return byte;
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use stratalith_markdown::parse;

    #[test]
    fn utf16_offsets_inside_a_surrogate_pair_resolve_to_its_start() {
        let text = "あ😀b";

        let offsets = (0..=4).map(|i| byte_for_utf16(text, i)).collect::<Vec<_>>();

        assert_eq!(offsets, [0, 3, 3, 7, 8]);
    }

    #[test]
    fn an_image_description_occupies_one_engine_offset() {
        let doc = parse("😀![**猫**です](https://example.org/cat.png)末").unwrap();
        let row = BlockText::new(&doc, doc.blocks()[0]).unwrap();

        let after_image = row.position("😀猫です".len());

        assert_eq!(after_image.offset, 2);
        assert_eq!(&row.text[row.boundaries[after_image.offset]..], "末");
    }
}
