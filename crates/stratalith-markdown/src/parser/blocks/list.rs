use super::{
    Block, BlockParser, BlockStart, Line, LineAction, MarkdownError, References, start::ListMarker,
};
use stratalith_core::ListKind;

pub(super) struct OpenList {
    kind: ListKind,
    symbol: u8,
    completed: CompletedItems,
    current: OpenItem,
}

// A blank after the last item only makes the list loose if another item follows.
struct CompletedItems {
    blocks: Vec<Vec<Block>>,
    tight: bool,
    blank_after_last: bool,
}

impl CompletedItems {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            tight: true,
            blank_after_last: false,
        }
    }

    fn append(&mut self, mut item: BlockParser, refs: &mut References) {
        let blank_after_item = item.ends_blank();
        item.finish_open(refs);
        if self.blank_after_last || item.has_blank_between_blocks {
            self.tight = false;
        }
        self.blocks.push(item.blocks);
        self.blank_after_last = blank_after_item;
    }
}

struct OpenItem {
    blocks: BlockParser,
    content_indent: usize,
    waiting_for_content: bool,
}

impl OpenItem {
    fn start(
        marker: ListMarker<'_>,
        refs: &mut References,
        depth: usize,
    ) -> Result<Self, MarkdownError> {
        let mut blocks = BlockParser::default();
        let waiting_for_content = marker.body.blank();
        if !waiting_for_content {
            blocks.push_line(marker.body, refs, depth)?;
        }
        Ok(Self {
            blocks,
            content_indent: marker.indent,
            waiting_for_content,
        })
    }
}

impl OpenList {
    pub(super) fn new(
        first: ListMarker<'_>,
        refs: &mut References,
        depth: usize,
    ) -> Result<Self, MarkdownError> {
        Ok(Self {
            kind: first.kind,
            symbol: first.symbol,
            completed: CompletedItems::new(),
            current: OpenItem::start(first, refs, depth)?,
        })
    }

    pub(super) fn open_paragraph(&mut self) -> Option<&mut String> {
        self.current.blocks.open_paragraph()
    }

    pub(super) fn ends_blank(&self) -> bool {
        !self.current.waiting_for_content && self.current.blocks.ends_blank()
    }

    pub(super) fn finish(mut self, refs: &mut References) -> Block {
        self.completed.append(self.current.blocks, refs);
        Block::List {
            kind: self.kind,
            tight: self.completed.tight,
            items: self.completed.blocks,
        }
    }

    pub(super) fn read_line<'a>(
        &mut self,
        line: Line<'a>,
        refs: &mut References,
        depth: usize,
    ) -> Result<LineAction<'a>, MarkdownError> {
        let item = &mut self.current;
        let indent = item.content_indent;
        if line.blank() {
            item.blocks
                .push_line(line.skip(line.indent().min(indent)), refs, depth)?;
        } else if line.indent() >= indent {
            if item.waiting_for_content && item.blocks.after_blank_line {
                return Ok(LineAction::StartSibling(BlockStart::read(line)));
            }
            item.waiting_for_content = false;
            item.blocks.push_line(line.skip(indent), refs, depth)?;
        } else {
            match BlockStart::read(line) {
                BlockStart::Item(next) if next.symbol == self.symbol => {
                    self.completed
                        .append(std::mem::take(&mut item.blocks), refs);
                    self.current = OpenItem::start(next, refs, depth)?;
                }
                start @ BlockStart::Item(_) => return Ok(LineAction::StartSibling(start)),
                start => return Ok(item.blocks.lazy_continuation(line, start)),
            }
        }
        Ok(LineAction::Continue)
    }
}
