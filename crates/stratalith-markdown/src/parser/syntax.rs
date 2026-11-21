//! Block boundaries are resolved before inline delimiters. Only paragraphs and
//! headings carry inline input. Fenced code stays opaque in both passes.

use super::{
    inlines,
    links::{LinkTarget, References},
};
use crate::MarkdownError;
use stratalith_core::{BlockKind, Document, DocumentError, HeadingLevel, InlineKind, ListKind, NodeId};

#[cfg_attr(test, derive(Debug, PartialEq))]
pub(super) enum Block {
    Paragraph(String),
    Heading(HeadingLevel, String),
    Code {
        info: Option<String>,
        body: String,
    },
    Quote(Vec<Block>),
    List {
        kind: ListKind,
        tight: bool,
        items: Vec<Vec<Block>>,
    },
    Rule,
    Html(String),
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub(super) enum Inline {
    Text(String),
    Html(String),
    Code(String),
    SoftBreak,
    HardBreak,
    Strong(InlineSequence),
    Emphasis(InlineSequence),
    Link(LinkTarget, InlineSequence),
    Image(LinkTarget, InlineSequence),
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Default)]
pub(super) struct InlineSequence {
    nodes: Vec<Inline>,
    height: usize,
}

impl InlineSequence {
    pub(super) fn push(&mut self, node: Inline) {
        let height = match &node {
            Inline::Link(_, children)
            | Inline::Image(_, children)
            | Inline::Strong(children)
            | Inline::Emphasis(children) => children.height + 1,
            Inline::Text(_)
            | Inline::Code(_)
            | Inline::Html(_)
            | Inline::SoftBreak
            | Inline::HardBreak => 0,
        };
        self.height = self.height.max(height);
        match (self.nodes.last_mut(), node) {
            (Some(Inline::Text(last)), Inline::Text(text)) => last.push_str(&text),
            (_, node) => self.nodes.push(node),
        }
    }

    pub(super) fn height(&self) -> usize {
        self.height
    }

    pub(super) fn into_nodes(self) -> Vec<Inline> {
        self.nodes
    }
}

impl FromIterator<Inline> for InlineSequence {
    fn from_iter<I: IntoIterator<Item = Inline>>(nodes: I) -> Self {
        let mut sequence = Self::default();
        for node in nodes {
            sequence.push(node);
        }
        sequence
    }
}

pub(super) struct ParsedBlocks {
    pub(super) roots: Vec<Block>,
    pub(super) references: References,
}

impl ParsedBlocks {
    // The complete reference table is needed even for the first paragraph.
    pub(super) fn into_document(self) -> Result<Document, MarkdownError> {
        let mut doc = Document::new();
        insert_blocks(&mut doc, None, self.roots, &self.references, 0)?;
        Ok(doc)
    }
}

fn insert_blocks(
    doc: &mut Document,
    parent: Option<NodeId>,
    blocks: Vec<Block>,
    refs: &References,
    depth: usize,
) -> Result<(), MarkdownError> {
    for (index, block) in blocks.into_iter().enumerate() {
        let mut insert = |kind| match parent {
            Some(id) => doc.insert_child_block(id, index, kind),
            None => doc.insert_block(index, kind),
        };
        match block {
            Block::Paragraph(text) => {
                let id = insert(BlockKind::Paragraph)?;
                insert_inlines(doc, id, inlines::parse(&text, refs, depth)?)?;
            }
            Block::Heading(level, text) => {
                let id = insert(BlockKind::Heading { level })?;
                insert_inlines(doc, id, inlines::parse(&text, refs, depth)?)?;
            }
            Block::Quote(blocks) => {
                let id = insert(BlockKind::BlockQuote)?;
                insert_blocks(doc, Some(id), blocks, refs, depth + 1)?;
            }
            Block::List { kind, tight, items } => {
                let id = insert(BlockKind::List { kind, tight })?;
                for (index, item) in items.into_iter().enumerate() {
                    let parent = doc.insert_child_block(id, index, BlockKind::ListItem)?;
                    insert_blocks(doc, Some(parent), item, refs, depth + 1)?;
                }
            }
            Block::Code { info, body } => {
                insert(BlockKind::CodeBlock {
                    language: info,
                    code: body,
                })?;
            }
            Block::Html(html) => {
                insert(BlockKind::Html(html))?;
            }
            Block::Rule => {
                insert(BlockKind::ThematicBreak)?;
            }
        }
    }
    Ok(())
}

fn insert_inlines(
    doc: &mut Document,
    parent: NodeId,
    nodes: Vec<Inline>,
) -> Result<(), DocumentError> {
    for (index, node) in nodes.into_iter().enumerate() {
        let (kind, children) = match node {
            Inline::Text(s) => (InlineKind::Text(s), vec![]),
            Inline::Html(s) => (InlineKind::Html(s), vec![]),
            Inline::Code(s) => (InlineKind::Code(s), vec![]),
            Inline::SoftBreak => (InlineKind::SoftBreak, vec![]),
            Inline::HardBreak => (InlineKind::HardBreak, vec![]),
            Inline::Strong(nodes) => (InlineKind::Strong, nodes.into_nodes()),
            Inline::Emphasis(nodes) => (InlineKind::Emphasis, nodes.into_nodes()),
            Inline::Link(LinkTarget { url, title }, nodes) => {
                (InlineKind::Link { url, title }, nodes.into_nodes())
            }
            Inline::Image(LinkTarget { url, title }, nodes) => {
                (InlineKind::Image { url, title }, nodes.into_nodes())
            }
        };
        let id = doc.insert_inline(parent, index, kind)?;
        insert_inlines(doc, id, children)?;
    }
    Ok(())
}
