//! Markdown has no marker between adjacent editor text nodes. Writing and
//! checking a document must therefore see the same sequence of text runs.

use std::borrow::Cow;
use stratalith_core::{Document, InlineKind, Node, NodeId, NodeKind};

pub(super) enum Child<'a> {
    Text(Cow<'a, str>),
    Node(&'a Node),
}

impl Child<'_> {
    pub(super) fn is_break(&self) -> bool {
        matches!(self, Self::Node(node) if matches!(node.kind(),
            NodeKind::Inline(InlineKind::HardBreak | InlineKind::SoftBreak)))
    }
}

pub(super) fn visible_nodes<'a>(
    doc: &'a Document,
    ids: &'a [NodeId],
) -> impl Iterator<Item = &'a Node> {
    ids.iter()
        .map(|&id| doc.node(id).expect("linked document node"))
        .filter(|node| text(node) != Some(""))
}

fn text(node: &Node) -> Option<&str> {
    match node.kind() {
        NodeKind::Inline(InlineKind::Text(text)) => Some(text),
        _ => None,
    }
}

pub(super) fn children<'a>(
    doc: &'a Document,
    ids: &'a [NodeId],
) -> impl Iterator<Item = Child<'a>> {
    let mut nodes = visible_nodes(doc, ids).peekable();
    std::iter::from_fn(move || {
        let first = nodes.next()?;
        let Some(first_text) = text(first) else {
            return Some(Child::Node(first));
        };
        let mut run = Cow::Borrowed(first_text);
        while let Some(next_text) = nodes.peek().and_then(|node| text(node)) {
            run.to_mut().push_str(next_text);
            nodes.next();
        }
        Some(Child::Text(run))
    })
}

pub(super) fn equivalent(left: &Document, right: &Document) -> bool {
    same_children(left, left.blocks(), right, right.blocks())
}

fn same_children(
    left: &Document,
    left_ids: &[NodeId],
    right: &Document,
    right_ids: &[NodeId],
) -> bool {
    let mut a = children(left, left_ids);
    let mut b = children(right, right_ids);
    loop {
        match (a.next(), b.next()) {
            (None, None) => return true,
            (Some(Child::Text(a)), Some(Child::Text(b))) if a == b => {}
            (Some(Child::Node(a)), Some(Child::Node(b))) if a.kind() == b.kind() => {
                if !same_children(left, a.children(), right, b.children()) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}
