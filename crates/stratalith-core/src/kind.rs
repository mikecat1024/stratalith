#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Block(BlockKind),
    Inline(InlineKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingLevel {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Ordered { start: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    BlockQuote,
    /// Contains only list items. `tight` controls paragraph rendering within items.
    List {
        kind: ListKind,
        tight: bool,
    },
    /// Contains blocks and can only be inserted into a list.
    ListItem,
    ThematicBreak,
    Html(String),
    Heading {
        level: HeadingLevel,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineKind {
    Text(String),
    Code(String),
    SoftBreak,
    HardBreak,
    Html(String),
    Strong,
    Emphasis,
    Link {
        url: String,
        title: Option<String>,
    },
    /// Inline children describe the image and supply its alternative text.
    Image {
        url: String,
        title: Option<String>,
    },
}

#[derive(PartialEq)]
enum NodeCategory {
    Block,
    ListItem,
    Inline,
}

impl NodeKind {
    /// Checks the immediate parent-child relationship, without ancestor constraints.
    pub(super) fn accepts(&self, child: &Self) -> bool {
        self.allowed_children_category() == Some(child.category())
    }

    pub(super) fn can_have_children(&self) -> bool {
        self.allowed_children_category().is_some()
    }

    fn category(&self) -> NodeCategory {
        match self {
            Self::Block(kind) => match kind {
                BlockKind::ListItem => NodeCategory::ListItem,
                BlockKind::Paragraph
                | BlockKind::BlockQuote
                | BlockKind::List { .. }
                | BlockKind::ThematicBreak
                | BlockKind::Html(_)
                | BlockKind::Heading { .. }
                | BlockKind::CodeBlock { .. } => NodeCategory::Block,
            },
            Self::Inline(_) => NodeCategory::Inline,
        }
    }

    fn allowed_children_category(&self) -> Option<NodeCategory> {
        match self {
            Self::Block(kind) => match kind {
                BlockKind::Paragraph | BlockKind::Heading { .. } => Some(NodeCategory::Inline),
                BlockKind::BlockQuote | BlockKind::ListItem => Some(NodeCategory::Block),
                BlockKind::List { .. } => Some(NodeCategory::ListItem),
                BlockKind::CodeBlock { .. } | BlockKind::ThematicBreak | BlockKind::Html(_) => None,
            },
            Self::Inline(kind) => match kind {
                InlineKind::Strong
                | InlineKind::Emphasis
                | InlineKind::Link { .. }
                | InlineKind::Image { .. } => Some(NodeCategory::Inline),
                InlineKind::Text(_)
                | InlineKind::Code(_)
                | InlineKind::SoftBreak
                | InlineKind::HardBreak
                | InlineKind::Html(_) => None,
            },
        }
    }
}

impl InlineKind {
    /// Returns the inline node's own cursor-position-based length in block-offset units.
    /// `None` tells the caller to visit the node's children. Empty text has a
    /// length of zero and can itself be the target of a cursor position.
    pub(crate) fn direct_offset_len(&self) -> Option<usize> {
        match self {
            Self::Text(text) | Self::Code(text) => Some(text.chars().count()),
            Self::Strong | Self::Emphasis | Self::Link { .. } => None,
            Self::SoftBreak | Self::HardBreak | Self::Image { .. } | Self::Html(_) => Some(1),
        }
    }
}
