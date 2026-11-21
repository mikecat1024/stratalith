use stratalith_core::{BlockKind, Document, HeadingLevel, InlineKind, ListKind, NodeId, NodeKind};

// Each sibling list starts with its length. A node consists of its tag,
// tag-specific fields, and its children. Counts and string lengths are little
// endian u32, strings are UTF-8, and u32::MAX denotes an absent optional string.
const LEVELS: [HeadingLevel; 6] = [
    HeadingLevel::H1,
    HeadingLevel::H2,
    HeadingLevel::H3,
    HeadingLevel::H4,
    HeadingLevel::H5,
    HeadingLevel::H6,
];

pub(super) fn encode(document: &Document) -> Vec<u8> {
    let mut output = Writer(Vec::new());
    output.nodes(document, document.blocks());
    output.0
}

struct Writer(Vec<u8>);

impl Writer {
    fn number(&mut self, value: u32) {
        self.0.extend(value.to_le_bytes());
    }

    fn string(&mut self, value: Option<&str>) {
        if let Some(value) = value {
            self.number(value.len() as u32);
            self.0.extend(value.as_bytes());
        } else {
            self.number(u32::MAX);
        }
    }

    fn nodes(&mut self, document: &Document, ids: &[NodeId]) {
        self.number(ids.len() as u32);
        for &id in ids {
            let node = document.node(id).expect("linked document node");
            self.kind(node.kind());
            self.nodes(document, node.children());
        }
    }

    fn kind(&mut self, kind: &NodeKind) {
        let tag = match kind {
            NodeKind::Block(kind) => match kind {
                BlockKind::Paragraph => 0,
                BlockKind::BlockQuote => 1,
                BlockKind::List {
                    kind: ListKind::Bullet,
                    ..
                } => 2,
                BlockKind::List {
                    kind: ListKind::Ordered { .. },
                    ..
                } => 3,
                BlockKind::ListItem => 4,
                BlockKind::ThematicBreak => 5,
                BlockKind::Html(_) => 6,
                BlockKind::Heading { .. } => 7,
                BlockKind::CodeBlock { .. } => 8,
            },
            NodeKind::Inline(kind) => match kind {
                InlineKind::Text(_) => 9,
                InlineKind::Code(_) => 10,
                InlineKind::SoftBreak => 11,
                InlineKind::HardBreak => 12,
                InlineKind::Html(_) => 13,
                InlineKind::Strong => 14,
                InlineKind::Emphasis => 15,
                InlineKind::Link { .. } => 16,
                InlineKind::Image { .. } => 17,
            },
        };
        self.0.push(tag);
        match kind {
            NodeKind::Block(BlockKind::List { kind, tight }) => {
                if let ListKind::Ordered { start } = kind {
                    self.number(*start);
                }
                self.0.push(u8::from(*tight));
            }
            NodeKind::Block(BlockKind::Heading { level }) => {
                self.0
                    .push(LEVELS.iter().position(|item| item == level).unwrap() as u8);
            }
            NodeKind::Block(BlockKind::CodeBlock { language, code }) => {
                self.string(language.as_deref());
                self.string(Some(code));
            }
            NodeKind::Block(BlockKind::Html(value))
            | NodeKind::Inline(
                InlineKind::Text(value) | InlineKind::Code(value) | InlineKind::Html(value),
            ) => {
                self.string(Some(value));
            }
            NodeKind::Inline(
                InlineKind::Link { url, title } | InlineKind::Image { url, title },
            ) => {
                self.string(Some(url));
                self.string(title.as_deref());
            }
            _ => {}
        }
    }
}

pub(super) fn decode(input: &[u8]) -> Result<Document, String> {
    let mut reader = Reader(input);
    let mut document = Document::new();
    reader.nodes(&mut document, None, 0)?;
    if !reader.0.is_empty() {
        return Err(INVALID.into());
    }
    Ok(document)
}

const INVALID: &str = "Invalid editable document";

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], &'static str> {
        let (bytes, remaining) = self.0.split_at_checked(len).ok_or(INVALID)?;
        self.0 = remaining;
        Ok(bytes)
    }

    fn byte(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }

    fn number(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn optional_string(&mut self) -> Result<Option<String>, &'static str> {
        let len = self.number()?;
        if len == u32::MAX {
            return Ok(None);
        }
        let value = std::str::from_utf8(self.take(len as usize)?).map_err(|_| INVALID)?;
        Ok(Some(value.into()))
    }

    fn string(&mut self) -> Result<String, &'static str> {
        self.optional_string()?.ok_or(INVALID)
    }

    fn nodes(
        &mut self,
        document: &mut Document,
        parent: Option<NodeId>,
        depth: usize,
    ) -> Result<(), String> {
        let count = self.number()?;
        if count == 0 {
            return Ok(());
        }
        // List items each add a node level beyond the parser's nesting count.
        if depth >= 256 {
            return Err("Editable document is nested too deeply".into());
        }
        for index in 0..count as usize {
            let id = match (parent, self.kind()?) {
                (None, NodeKind::Block(kind)) => document.insert_block(index, kind),
                (Some(parent), NodeKind::Block(kind)) => {
                    document.insert_child_block(parent, index, kind)
                }
                (Some(parent), NodeKind::Inline(kind)) => {
                    document.insert_inline(parent, index, kind)
                }
                (None, NodeKind::Inline(_)) => return Err(INVALID.into()),
            }
            .map_err(|error| error.to_string())?;
            self.nodes(document, Some(id), depth + 1)?;
        }
        Ok(())
    }

    fn kind(&mut self) -> Result<NodeKind, &'static str> {
        let tag = self.byte()?;
        let block = match tag {
            0 => BlockKind::Paragraph,
            1 => BlockKind::BlockQuote,
            2 | 3 => BlockKind::List {
                kind: if tag == 2 {
                    ListKind::Bullet
                } else {
                    ListKind::Ordered {
                        start: self.number()?,
                    }
                },
                tight: self.byte()? != 0,
            },
            4 => BlockKind::ListItem,
            5 => BlockKind::ThematicBreak,
            6 => BlockKind::Html(self.string()?),
            7 => BlockKind::Heading {
                level: *LEVELS.get(usize::from(self.byte()?)).ok_or(INVALID)?,
            },
            8 => BlockKind::CodeBlock {
                language: self.optional_string()?,
                code: self.string()?,
            },
            _ => return self.inline(tag).map(NodeKind::Inline),
        };
        Ok(NodeKind::Block(block))
    }

    fn inline(&mut self, tag: u8) -> Result<InlineKind, &'static str> {
        Ok(match tag {
            9 => InlineKind::Text(self.string()?),
            10 => InlineKind::Code(self.string()?),
            11 => InlineKind::SoftBreak,
            12 => InlineKind::HardBreak,
            13 => InlineKind::Html(self.string()?),
            14 => InlineKind::Strong,
            15 => InlineKind::Emphasis,
            16 => InlineKind::Link {
                url: self.string()?,
                title: self.optional_string()?,
            },
            17 => InlineKind::Image {
                url: self.string()?,
                title: self.optional_string()?,
            },
            _ => return Err(INVALID),
        })
    }
}
