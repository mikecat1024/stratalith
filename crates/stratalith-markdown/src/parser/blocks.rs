mod line;
mod list;
mod open;
mod start;

use super::{
    links::References,
    syntax::{Block, ParsedBlocks},
};
use crate::{MarkdownError, error::check_depth, escape};
use line::Line;
use list::OpenList;
use open::{LineAction, OpenBlock};
use start::BlockStart;

// Each block sequence has one open rightmost branch. New lines first try to
// extend that branch. A rejected line starts its next sibling at this level.
#[derive(Default)]
struct BlockParser {
    blocks: Vec<Block>,
    open: Option<OpenBlock>,
    after_blank_line: bool,
    has_blank_between_blocks: bool,
}

pub(super) fn parse(input: &str) -> Result<ParsedBlocks, MarkdownError> {
    let mut root = BlockParser::default();
    let mut references = References::default();
    for line in input.lines() {
        root.push_line(Line::new(line), &mut references, 0)?;
    }
    root.finish_open(&mut references);
    Ok(ParsedBlocks {
        roots: root.blocks,
        references,
    })
}

impl BlockParser {
    fn ends_blank(&self) -> bool {
        self.after_blank_line
            || matches!(&self.open, Some(OpenBlock::List(list)) if list.ends_blank())
    }

    // A missing container prefix is allowed only along the open branch
    // leading to a paragraph.
    fn open_paragraph(&mut self) -> Option<&mut String> {
        match &mut self.open {
            Some(OpenBlock::Paragraph(source)) => Some(source),
            Some(OpenBlock::Quote(inner)) => inner.open_paragraph(),
            Some(OpenBlock::List(list)) => list.open_paragraph(),
            _ => None,
        }
    }

    fn lazy_continuation<'a>(&mut self, line: Line<'a>, start: BlockStart<'a>) -> LineAction<'a> {
        if !start.interrupts_paragraph()
            && let Some(source) = self.open_paragraph()
        {
            source.push('\n');
            source.push_str(line.content());
            LineAction::Continue
        } else {
            LineAction::StartSibling(start)
        }
    }

    fn finish_open(&mut self, refs: &mut References) {
        if let Some(block) = self.open.take().and_then(|open| open.finish(refs)) {
            self.blocks.push(block);
        }
    }

    fn push_line(
        &mut self,
        line: Line<'_>,
        refs: &mut References,
        depth: usize,
    ) -> Result<(), MarkdownError> {
        check_depth(depth)?;
        let action = match &mut self.open {
            Some(open) => open.read_line(line, refs, depth)?,
            None => LineAction::StartSibling(BlockStart::read(line)),
        };
        match action {
            LineAction::Continue => {}
            LineAction::Close => self.finish_open(refs),
            LineAction::SetextHeading(level, text) => {
                self.open = None;
                self.blocks.push(Block::Heading(level, text));
            }
            LineAction::StartSibling(start) => self.begin(line, start, refs, depth)?,
        }
        Ok(())
    }

    fn begin(
        &mut self,
        line: Line<'_>,
        start: BlockStart<'_>,
        refs: &mut References,
        depth: usize,
    ) -> Result<(), MarkdownError> {
        let after_blank_line = self.ends_blank();
        self.finish_open(refs);
        if matches!(start, BlockStart::Blank) {
            self.after_blank_line = true;
            return Ok(());
        }
        self.has_blank_between_blocks |= after_blank_line && !self.blocks.is_empty();
        self.after_blank_line = false;
        self.open = match start {
            BlockStart::Blank => unreachable!(),
            BlockStart::Plain => Some(OpenBlock::Paragraph(line.content().into())),
            BlockStart::Heading(level, text) => {
                self.blocks.push(Block::Heading(level, text.into()));
                None
            }
            BlockStart::Rule => {
                self.blocks.push(Block::Rule);
                None
            }
            BlockStart::Indented => {
                let mut body = String::new();
                line.skip(4).write(&mut body);
                Some(OpenBlock::Indented(body))
            }
            BlockStart::FencedCode { fence, info } => Some(OpenBlock::Fenced {
                fence,
                indent: line.indent(),
                info: (!info.is_empty()).then(|| escape::decode(info)),
                body: String::new(),
            }),
            BlockStart::Html(end, _) => {
                let mut body = String::new();
                line.write(&mut body);
                if end.closes(line.content()) {
                    self.blocks.push(Block::Html(body));
                    None
                } else {
                    Some(OpenBlock::Html { end, body })
                }
            }
            BlockStart::Quote(body) => {
                let mut inner = BlockParser::default();
                inner.push_line(body, refs, depth + 1)?;
                Some(OpenBlock::Quote(Box::new(inner)))
            }
            BlockStart::Item(first) => Some(OpenBlock::List(Box::new(OpenList::new(
                first,
                refs,
                depth + 1,
            )?))),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stratalith_core::ListKind;

    #[test]
    fn collects_inline_source_only_in_text_blocks() {
        let source = ["> - **label**", ">   ~~~md", ">   **literal**", ">   ~~~"].join("\n");
        let expected = [Block::Quote(vec![Block::List {
            kind: ListKind::Bullet,
            tight: true,
            items: vec![vec![
                Block::Paragraph("**label**".to_owned()),
                Block::Code {
                    info: Some("md".to_owned()),
                    body: "**literal**\n".to_owned(),
                },
            ]],
        }])];

        let blocks = parse(&source).unwrap().roots;

        assert_eq!(blocks, expected);
    }
}
