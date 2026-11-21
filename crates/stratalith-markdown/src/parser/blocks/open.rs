use super::{
    BlockParser, OpenList,
    line::Line,
    start::{BlockStart, Fence, setext_level},
};
use crate::{
    MarkdownError,
    parser::{html, links::References, syntax::Block},
};
use stratalith_core::HeadingLevel;

// A closing fence belongs to the current block. A sibling's first line must be
// interpreted again at the enclosing level, using its already recognized start.
pub(super) enum LineAction<'a> {
    Continue,
    Close,
    SetextHeading(HeadingLevel, String),
    StartSibling(BlockStart<'a>),
}

pub(super) enum OpenBlock {
    Paragraph(String),
    Indented(String),
    Fenced {
        fence: Fence,
        indent: usize,
        info: Option<String>,
        body: String,
    },
    Html {
        end: html::End,
        body: String,
    },
    Quote(Box<BlockParser>),
    List(Box<OpenList>),
}

impl OpenBlock {
    pub(super) fn read_line<'a>(
        &mut self,
        line: Line<'a>,
        refs: &mut References,
        depth: usize,
    ) -> Result<LineAction<'a>, MarkdownError> {
        match self {
            Self::Paragraph(source) => {
                if let Some(level) = setext_level(line) {
                    let text = refs.collect_prefix(source).trim_matches([' ', '\t', '\n']);
                    if !text.is_empty() {
                        return Ok(LineAction::SetextHeading(level, text.to_owned()));
                    }
                    source.clear();
                }
                let start = BlockStart::read(line);
                if start.interrupts_paragraph() || source.is_empty() {
                    return Ok(LineAction::StartSibling(start));
                }
                source.push('\n');
                source.push_str(line.content());
            }
            Self::Indented(body) if line.blank() || line.indent() >= 4 => {
                line.skip(line.indent().min(4)).write(body)
            }
            Self::Fenced {
                fence,
                indent,
                body,
                ..
            } => {
                if fence.closes(line) {
                    return Ok(LineAction::Close);
                }
                line.skip(line.indent().min(*indent)).write(body);
            }
            Self::Html { end, body } if !line.blank() || !matches!(end, html::End::Blank) => {
                line.write(body);
                if end.closes(line.content()) {
                    return Ok(LineAction::Close);
                }
            }
            Self::Quote(inner) => {
                let start = BlockStart::read(line);
                if let BlockStart::Quote(body) = start {
                    inner.push_line(body, refs, depth + 1)?;
                } else {
                    return Ok(inner.lazy_continuation(line, start));
                }
            }
            Self::List(list) => return list.read_line(line, refs, depth + 1),
            Self::Html { .. } | Self::Indented(_) => {
                return Ok(LineAction::StartSibling(BlockStart::read(line)));
            }
        }
        Ok(LineAction::Continue)
    }

    pub(super) fn finish(self, refs: &mut References) -> Option<Block> {
        Some(match self {
            Self::Paragraph(mut text) => {
                let remaining = refs.collect_prefix(&text);
                let consumed = text.len() - remaining.len();
                let content_len = remaining.trim_end_matches([' ', '\t']).len();
                if content_len == 0 {
                    return None;
                }
                text.truncate(consumed + content_len);
                text.drain(..consumed);
                Block::Paragraph(text)
            }
            Self::Indented(mut body) => {
                // Keep the trailing spaces on the last content line and its newline.
                let last_content = body.trim_end_matches([' ', '\t', '\n']).len();
                if last_content == 0 {
                    body.clear()
                } else if let Some(newline) = body[last_content..].find('\n') {
                    body.truncate(last_content + newline + 1);
                }
                Block::Code { info: None, body }
            }
            Self::Fenced { info, body, .. } => Block::Code { info, body },
            Self::Html { body, .. } => Block::Html(body),
            Self::Quote(mut inner) => {
                inner.finish_open(refs);
                Block::Quote(inner.blocks)
            }
            Self::List(list) => list.finish(refs),
        })
    }
}
