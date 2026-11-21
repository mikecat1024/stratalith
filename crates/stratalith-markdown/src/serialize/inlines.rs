use super::children::{Child, children, visible_nodes};
use super::{Renderer, escaped, fence_width};
use crate::{MarkdownError, error::check_depth};
use std::fmt::Write;
use stratalith_core::{InlineKind, NodeId, NodeKind};

#[derive(Clone, Copy, PartialEq)]
enum TextEdge {
    Ordinary,
    SequenceBoundary,
    EmphasisBoundary,
}

impl TextEdge {
    fn needs_entity(self, c: char) -> bool {
        match self {
            Self::Ordinary => false,
            Self::SequenceBoundary => c.is_whitespace(),
            // The entity's '&' or ';' supplies the punctuation boundary the
            // emphasis marker needs, while decoding to the original character.
            Self::EmphasisBoundary => is_word_character(c),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum EmphasisMark {
    Asterisk,
    Underscore,
}

#[derive(Clone, Copy)]
enum EmphasisStyle {
    Emphasis,
    Strong,
}

impl EmphasisMark {
    fn delimiter(self, style: EmphasisStyle) -> &'static str {
        match (self, style) {
            (Self::Asterisk, EmphasisStyle::Emphasis) => "*",
            (Self::Asterisk, EmphasisStyle::Strong) => "**",
            (Self::Underscore, EmphasisStyle::Emphasis) => "_",
            (Self::Underscore, EmphasisStyle::Strong) => "__",
        }
    }
}

// A sequence starts at a block, after a break, or inside an inline wrapper.
#[derive(Clone, Copy)]
enum PreviousInline {
    Start {
        enclosing_mark: Option<EmphasisMark>,
    },
    Ordinary,
    Emphasis {
        mark: EmphasisMark,
        following: TextEdge,
    },
}

impl PreviousInline {
    fn text_edge(self) -> TextEdge {
        match self {
            Self::Start { .. } => TextEdge::SequenceBoundary,
            Self::Ordinary => TextEdge::Ordinary,
            Self::Emphasis { following, .. } => following,
        }
    }

    fn next_mark(self) -> EmphasisMark {
        match self {
            Self::Start {
                enclosing_mark: Some(EmphasisMark::Asterisk),
            }
            | Self::Emphasis {
                mark: EmphasisMark::Asterisk,
                ..
            } => EmphasisMark::Underscore,
            _ => EmphasisMark::Asterisk,
        }
    }
}

fn is_word_character(c: char) -> bool {
    !c.is_whitespace() && !crate::escape::punctuation(c)
}

fn write_text(out: &mut String, text: &str, left: TextEdge, right: TextEdge) {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    for (i, c) in text.char_indices() {
        let first = i == 0;
        let last = i + c.len_utf8() == text.len();
        if matches!(c, '\n' | '\r' | '\t')
            || (first && left.needs_entity(c))
            || (last && right.needs_entity(c))
        {
            write!(out, "&#{};", u32::from(c)).unwrap();
            continue;
        }

        let block_marker = left == TextEdge::SequenceBoundary
            && ((first && ">-+~=".contains(c))
                || (i == digits && (1..=9).contains(&digits) && ".)".contains(c)));
        if block_marker || "\\`*_[]!#<&".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
}

fn write_code_span(out: &mut String, code: &str) -> Result<(), MarkdownError> {
    if code.is_empty() || code.contains('\n') {
        return Err(MarkdownError::Unrepresentable(
            "code span must contain nonempty single-line text",
        ));
    }
    let fence = "`".repeat(fence_width(code, '`', 1));
    let padded = code.starts_with('`')
        || code.ends_with('`')
        || (code.starts_with(' ') && code.ends_with(' ') && code.contains(|c| c != ' '));
    let pad = if padded { " " } else { "" };
    write!(out, "{fence}{pad}{code}{pad}{fence}").unwrap();
    Ok(())
}

impl Renderer<'_> {
    fn edge_before(&self, next: Option<&Child<'_>>) -> TextEdge {
        let Some(next) = next else {
            return TextEdge::SequenceBoundary;
        };
        if next.is_break() {
            return TextEdge::SequenceBoundary;
        }
        let Child::Node(node) = next else {
            return TextEdge::Ordinary;
        };
        if !matches!(
            node.kind(),
            NodeKind::Inline(InlineKind::Strong | InlineKind::Emphasis)
        ) {
            return TextEdge::Ordinary;
        }
        // An opener directly before punctuation (including nested Markdown
        // markup) needs whitespace or punctuation on its left.
        match visible_nodes(self.0, node.children())
            .next()
            .map(|child| child.kind())
        {
            Some(NodeKind::Inline(InlineKind::Text(text)))
                if text.starts_with(is_word_character) =>
            {
                TextEdge::Ordinary
            }
            _ => TextEdge::EmphasisBoundary,
        }
    }

    fn write_emphasis(
        &self,
        out: &mut String,
        ids: &[NodeId],
        style: EmphasisStyle,
        mark: EmphasisMark,
        depth: usize,
    ) -> Result<PreviousInline, MarkdownError> {
        let delimiter = mark.delimiter(style);
        out.push_str(delimiter);
        // At a shared opening boundary, use different markers so
        // the parser can recover the order of the two wrappers.
        let mut nested = visible_nodes(self.0, ids);
        let closes_as_one_run = nested
            .next()
            .is_some_and(|child| matches!(child.kind(), NodeKind::Inline(InlineKind::Strong)))
            && nested.next().is_none();
        self.write_inlines(out, ids, (!closes_as_one_run).then_some(mark), depth + 1)?;
        let following = if matches!(mark, EmphasisMark::Underscore)
            || out.ends_with(crate::escape::punctuation)
        {
            TextEdge::EmphasisBoundary
        } else {
            TextEdge::Ordinary
        };
        out.push_str(delimiter);
        Ok(PreviousInline::Emphasis { mark, following })
    }

    pub(super) fn write_inlines(
        &self,
        out: &mut String,
        ids: &[NodeId],
        enclosing_mark: Option<EmphasisMark>,
        depth: usize,
    ) -> Result<(), MarkdownError> {
        check_depth(depth)?;
        let mut children = children(self.0, ids).peekable();
        let mut previous = PreviousInline::Start { enclosing_mark };
        while let Some(child) = children.next() {
            let node = match child {
                Child::Text(text) => {
                    write_text(
                        out,
                        &text,
                        previous.text_edge(),
                        self.edge_before(children.peek()),
                    );
                    previous = PreviousInline::Ordinary;
                    continue;
                }
                Child::Node(node) => node,
            };
            let NodeKind::Inline(kind) = node.kind() else {
                unreachable!("inline parent")
            };
            previous = match kind {
                InlineKind::Text(_) => unreachable!("text runs handled above"),
                InlineKind::Strong => self.write_emphasis(
                    out,
                    node.children(),
                    EmphasisStyle::Strong,
                    previous.next_mark(),
                    depth,
                )?,
                InlineKind::Emphasis => self.write_emphasis(
                    out,
                    node.children(),
                    EmphasisStyle::Emphasis,
                    previous.next_mark(),
                    depth,
                )?,
                InlineKind::SoftBreak => {
                    out.push('\n');
                    PreviousInline::Start {
                        enclosing_mark: None,
                    }
                }
                InlineKind::HardBreak => {
                    out.push_str("\\\n");
                    PreviousInline::Start {
                        enclosing_mark: None,
                    }
                }
                InlineKind::Code(code) => {
                    write_code_span(out, code)?;
                    PreviousInline::Ordinary
                }
                InlineKind::Link { url, title } | InlineKind::Image { url, title } => {
                    if matches!(kind, InlineKind::Image { .. }) {
                        out.push('!');
                    }
                    out.push('[');
                    self.write_inlines(out, node.children(), None, depth + 1)?;
                    out.push_str("](<");
                    out.push_str(&escaped(url, "\\<>&"));
                    out.push('>');
                    if let Some(title) = title {
                        out.push_str(" \"");
                        out.push_str(&escaped(title, "\\\"&"));
                        out.push('"');
                    }
                    out.push(')');
                    PreviousInline::Ordinary
                }
                InlineKind::Html(html) => {
                    out.push_str(html);
                    PreviousInline::Ordinary
                }
            };
        }
        Ok(())
    }
}
