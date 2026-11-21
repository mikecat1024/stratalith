use super::*;
use crate::{Affinity, BlockPosition, HeadingLevel, ListKind};

enum Marker {
    Heading(HeadingLevel),
    Quote,
    List(ListKind),
    Fence,
    Divider,
}

impl Marker {
    fn parse(prefix: &str) -> Option<Self> {
        Some(match prefix {
            "```" => Self::Fence,
            "---" | "—-" => Self::Divider,
            _ => match prefix.strip_suffix(' ')?.trim_start_matches(' ') {
                "#" => Self::Heading(HeadingLevel::H1),
                "##" => Self::Heading(HeadingLevel::H2),
                "###" => Self::Heading(HeadingLevel::H3),
                "####" => Self::Heading(HeadingLevel::H4),
                "\"" | "“" => Self::Quote,
                "-" | "+" | "*" => Self::List(ListKind::Bullet),
                marker => {
                    let digits = marker.strip_suffix(['.', ')'])?;
                    if digits.len() > 9 || !digits.bytes().all(|b| b.is_ascii_digit()) {
                        return None;
                    }
                    Self::List(ListKind::Ordered {
                        start: digits.parse().ok()?,
                    })
                }
            },
        })
    }
}

struct Span {
    open: usize,
    close: usize,
    width: usize,
    delimiter: char,
}

// Scan only closing runs completed by this insertion. Previous text can
// supply the opening delimiter, and content beyond the caret stays intact.
fn next_span(chars: &[Option<char>], start: usize, end: usize) -> Option<Span> {
    let mut i = 0;
    let mut opener = None;
    while i < end {
        let Some(delimiter) = chars[i] else {
            i += 1;
            continue;
        };
        if !matches!(delimiter, '*' | '_' | '`') {
            i += 1;
            continue;
        }
        let first = i;
        while i < end && chars[i] == Some(delimiter) {
            i += 1;
        }
        let run = i - first;
        if let Some((open, width, mark)) = opener
            && mark == delimiter
            && run >= width
            && first > open + width
            && !chars[first - 1].is_some_and(char::is_whitespace)
        {
            if first + width <= start
                || (delimiter == '`' && (run != 1 || chars[open + width..first].contains(&None)))
            {
                opener = None;
                continue;
            }
            return Some(Span {
                open,
                close: first,
                width,
                delimiter,
            });
        }
        let boundary = first == 0
            || !(chars[first - 1].is_some_and(char::is_alphanumeric)
                || chars[first - 1] == Some('\\'));
        if boundary
            && i < end
            && !chars[i].is_some_and(char::is_whitespace)
            && (delimiter != '`' && run <= 3 || delimiter == '`' && run == 1)
        {
            opener = Some((first, run.min(2), delimiter));
        }
    }
    None
}

impl Document {
    pub(crate) fn may_trigger_input_shortcut(text: &str) -> bool {
        text.contains([' ', '-', '*', '_', '`'])
    }

    pub(crate) fn input_shortcut(
        &self,
        start: usize,
        at: BlockPosition,
    ) -> Option<(Self, BlockPosition)> {
        if !matches!(
            self.live(at.block).kind,
            NodeKind::Block(BlockKind::Paragraph | BlockKind::Heading { .. })
        ) {
            return None;
        }
        if matches!(
            self.live(at.block).kind,
            NodeKind::Block(BlockKind::Paragraph)
        ) && let Some(&first) = self.live(at.block).children.first()
            && let NodeKind::Inline(InlineKind::Text(text)) = &self.live(first).kind
            && at.offset <= text.chars().count()
            && let Some(marker) = Marker::parse(&text[..byte_offset(text, at.offset)])
        {
            return self.convert_marker(marker, at);
        }
        let mut chars = self.shortcut_chars(at.block);
        let mut span = next_span(&chars, start, at.offset)?;
        let mut doc = self.edit_snapshot();
        let mut end = at.offset;
        loop {
            let resume = span.close - span.width;
            end -= 2 * span.width;
            doc.format_span(at.block, &span, &chars);
            chars = doc.shortcut_chars(at.block);
            let Some(next) = next_span(&chars, resume, end) else {
                break;
            };
            span = next;
        }
        let affinity = if end == span.close - span.width {
            Affinity::Downstream
        } else {
            Affinity::Upstream
        };
        Some((
            doc,
            BlockPosition {
                offset: end,
                affinity,
                ..at
            },
        ))
    }

    fn convert_marker(&self, marker: Marker, at: BlockPosition) -> Option<(Self, BlockPosition)> {
        let mut doc = self.edit_snapshot();
        let block = at.block;
        doc.delete_units(block, 0, at.offset, &mut Vec::new());
        match marker {
            Marker::Heading(level) => {
                doc.live_mut(block).kind = NodeKind::Block(BlockKind::Heading { level })
            }
            Marker::Quote => {
                let quote = doc.sibling_block(block, false, BlockKind::BlockQuote);
                doc.relocate(block, Some(quote), 0);
            }
            Marker::List(kind) => {
                let list = doc.list_for_marker(block, kind)?;
                let item = doc
                    .insert_child_block(list, doc.live(list).children.len(), BlockKind::ListItem)
                    .unwrap();
                doc.relocate(block, Some(item), 0);
            }
            Marker::Divider => {
                doc.sibling_block(block, false, BlockKind::ThematicBreak);
            }
            Marker::Fence => {
                let mut code = String::new();
                for &id in &doc.live(block).children {
                    match &doc.live(id).kind {
                        NodeKind::Inline(InlineKind::Text(s)) => code.push_str(s),
                        NodeKind::Inline(InlineKind::HardBreak | InlineKind::SoftBreak) => {
                            code.push('\n')
                        }
                        _ => return None,
                    }
                }
                doc.delete_units(block, 0, doc.block_len(block), &mut Vec::new());
                if let Some(item) = doc.item_of(block) {
                    doc.exit_item(item);
                }
                doc.live_mut(block).kind = NodeKind::Block(BlockKind::CodeBlock {
                    language: None,
                    code,
                });
            }
        }
        Some((
            doc,
            BlockPosition {
                offset: 0,
                affinity: Affinity::Downstream,
                ..at
            },
        ))
    }

    fn list_for_marker(&mut self, block: NodeId, kind: ListKind) -> Option<NodeId> {
        if let Some(&previous) = self.surrounding_siblings(block).0.last()
            && let NodeKind::Block(BlockKind::List {
                kind: preceding, ..
            }) = self.live(previous).kind
        {
            match (preceding, kind) {
                (ListKind::Bullet, ListKind::Bullet) => return Some(previous),
                (ListKind::Ordered { start }, ListKind::Ordered { start: next }) => {
                    let count = u32::try_from(self.live(previous).children.len()).ok()?;
                    return (start.checked_add(count) == Some(next)).then_some(previous);
                }
                _ => {}
            }
        }
        Some(self.sibling_block(block, false, BlockKind::List { kind, tight: true }))
    }

    fn shortcut_chars(&self, block: NodeId) -> Vec<Option<char>> {
        let mut chars = Vec::new();
        for (id, len) in self.leaves(block) {
            match &self.live(id).kind {
                NodeKind::Inline(InlineKind::Text(s)) => chars.extend(s.chars().map(Some)),
                NodeKind::Inline(InlineKind::SoftBreak | InlineKind::HardBreak) => {
                    chars.push(Some('\n'))
                }
                _ => chars.extend(std::iter::repeat_n(None, len)),
            }
        }
        chars
    }

    fn format_span(&mut self, block: NodeId, span: &Span, chars: &[Option<char>]) {
        let Span {
            open,
            close,
            width,
            delimiter,
        } = *span;
        let mut changes = Vec::new();
        self.delete_units(block, close, close + width, &mut changes);
        self.delete_units(block, open, open + width, &mut changes);
        let cut = |doc: &mut Self, offset| {
            let at = BlockPosition {
                block,
                offset,
                affinity: Affinity::Downstream,
            };
            doc.split_inlines_at(block, at.to_node_position(doc).unwrap())
        };
        let first = cut(self, open);
        let last = cut(self, close - width);
        let kind = match (delimiter, width) {
            ('`', _) => InlineKind::Code(chars[open + width..close].iter().flatten().collect()),
            (_, 2) => InlineKind::Strong,
            _ => InlineKind::Emphasis,
        };
        let children = self.live(block).children[first..last].to_vec();
        let code = matches!(kind, InlineKind::Code(_));
        let mark = self.insert_inline(block, first, kind).unwrap();
        for child in children {
            if code {
                self.remove_node(child).unwrap()
            } else {
                self.relocate(child, Some(mark), self.live(mark).children.len());
            }
        }
        self.normalize(block);
    }
}
