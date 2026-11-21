use std::borrow::Borrow;
use std::fmt::Write as _;

use super::block_position;
use crate::builder::{
    BlockBuilder, block, document, emphasis, inline, paragraph, strong, text, text_block,
};
use crate::{
    Affinity, BlockKind, BlockPosition, Command, Document, EditorEngine, HeadingLevel, InlineKind,
    ListKind, NodeId, NodeKind, Selection,
};

struct Scenario<D = Document> {
    document: D,
    selection: Selection,
}

impl Scenario {
    fn new<'a>(blocks: impl IntoIterator<Item = BlockBuilder<'a>>) -> Self {
        let document = document(blocks).unwrap();
        let last_node = document.nodes().last().unwrap();
        let selection = Selection::caret(block_position(last_node.id(), 0));
        Self {
            document,
            selection,
        }
    }

    fn with_selection(mut self, selection: Selection) -> Self {
        self.selection = selection;
        self
    }

    fn position(&self, path: &[usize], offset: usize) -> BlockPosition {
        let mut siblings = self.document.blocks();
        let (&index, parents) = path.split_last().expect("a block path cannot be empty");
        for &parent in parents {
            siblings = self.document.node(siblings[parent]).unwrap().children();
        }
        block_position(siblings[index], offset)
    }

    fn at(self, path: &[usize], offset: usize) -> Self {
        let position = self.position(path, offset);
        self.with_selection(Selection::caret(position))
    }

    fn with_caret_affinity(self, affinity: Affinity) -> Self {
        let position = BlockPosition {
            affinity,
            ..self.selection.focus
        };
        self.with_selection(Selection::caret(position))
    }

    fn select(self, path: &[usize], range: std::ops::Range<usize>) -> Self {
        let anchor = self.position(path, range.start);
        let focus = self.position(path, range.end);
        self.with_selection(Selection { anchor, focus })
    }
}

impl<D: Borrow<Document>> Scenario<D> {
    fn selection_positions(&self) -> [(usize, usize, Affinity); 2] {
        let document = self.document.borrow();
        [self.selection.anchor, self.selection.focus].map(|position| {
            let index = document
                .nodes()
                .position(|node| node.id() == position.block)
                .expect("selection block should belong to the document");
            (index, position.offset, position.affinity)
        })
    }

    fn state_lines(&self) -> Vec<String> {
        fn collect_nodes(
            lines: &mut Vec<String>,
            document: &Document,
            nodes: &[NodeId],
            positions: &[(&str, BlockPosition)],
            depth: usize,
        ) {
            for &id in nodes {
                let node = document.node(id).unwrap();
                let label = match node.kind() {
                    NodeKind::Block(BlockKind::Paragraph) if node.children().is_empty() => {
                        "Paragraph (empty)".into()
                    }
                    NodeKind::Block(kind) => format!("{kind:?}"),
                    NodeKind::Inline(kind) => format!("{kind:?}"),
                };
                lines.push(format!("{:width$}{label}", "", width = depth * 2));
                collect_nodes(lines, document, node.children(), positions, depth + 1);
                for &(label, position) in positions
                    .iter()
                    .filter(|(_, position)| position.block == id)
                {
                    lines.push(format!(
                        "{:width$}{label} at offset {} ({:?})",
                        "",
                        position.offset,
                        position.affinity,
                        width = (depth + 1) * 2
                    ));
                }
            }
        }

        let document = self.document.borrow();
        let Selection { anchor, focus } = self.selection;
        let positions: &[(&str, BlockPosition)] = match anchor == focus {
            true => &[("caret", anchor)],
            false => &[("anchor", anchor), ("focus", focus)],
        };
        let mut lines = Vec::new();
        collect_nodes(&mut lines, document, document.blocks(), positions, 0);
        lines
    }

    fn diff(&self, expected: &Scenario<impl Borrow<Document>>) -> String {
        let expected = expected.state_lines();
        let actual = self.state_lines();
        let prefix = expected
            .iter()
            .zip(&actual)
            .take_while(|(a, b)| a == b)
            .count();
        let suffix = expected[prefix..]
            .iter()
            .rev()
            .zip(actual[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();

        let mut output = String::from("--- expected\n+++ actual\n");
        for line in &expected[prefix.saturating_sub(4)..prefix] {
            writeln!(output, "  {line}").unwrap();
        }
        for (marker, lines) in [('-', &expected), ('+', &actual)] {
            for line in &lines[prefix..lines.len() - suffix] {
                writeln!(output, "{marker} {line}").unwrap();
            }
        }
        output
    }
}

impl<D: Borrow<Document>, E: Borrow<Document>> PartialEq<Scenario<E>> for Scenario<D> {
    fn eq(&self, other: &Scenario<E>) -> bool {
        self.document.borrow() == other.document.borrow()
            && self.selection_positions() == other.selection_positions()
    }
}

fn rust_code(code: &str) -> BlockBuilder<'static> {
    block(
        BlockKind::CodeBlock {
            language: Some("rust".into()),
            code: code.into(),
        },
        [],
    )
}

const WEB_URL: &str = "https://example.com/";

fn web_link<'a>(
    children: impl IntoIterator<Item = crate::builder::InlineBuilder<'a>>,
) -> crate::builder::InlineBuilder<'a> {
    inline(
        InlineKind::Link {
            url: WEB_URL.into(),
            title: None,
        },
        children,
    )
}

// In Markdown examples, $$ is the caret, [| is the anchor, and |] is the focus.
#[stratalith_macros::rstest]
#[case::backspace_converts_empty_nested_ordered_item_to_paragraph_in_parent_item(
    // ```markdown
    // 1. Hello
    //    1. $$
    // ```
    Scenario::new(
        [
            block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 1 },
                    tight: true,
                },
                [
                    block(
                        BlockKind::ListItem,
                        [
                            paragraph([text("Hello")]),
                            block(
                                BlockKind::List {
                                    kind: ListKind::Ordered { start: 1 },
                                    tight: true,
                                },
                                [block(BlockKind::ListItem, [paragraph([])])],
                            ),
                        ],
                    )
                ],
            )
        ]
    ),
    [
        (
            Command::Backspace,
            // ```markdown
            // 1. Hello
            //
            //    $$
            // ```
            Scenario::new(
                [
                    block(
                        BlockKind::List {
                            kind: ListKind::Ordered { start: 1 },
                            tight: true,
                        },
                        [
                            block(
                                BlockKind::ListItem,
                                [paragraph([text("Hello")]), paragraph([])],
                            )
                        ],
                    ),
                ]
            ),
        )
    ],
)]
// Notion block shortcuts
#[case::hash_space_starts_heading_one(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("#".into()),
            // ```markdown
            // #$$
            // ```
            Scenario::new([paragraph([text("#")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // # $$
            // ```
            Scenario::new([text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H1,
                },
                [],
            )])
            .at(&[0], 0),
        ),
    ],
)]
#[case::two_hashes_space_starts_heading_two(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("#".into()),
            // ```markdown
            // #$$
            // ```
            Scenario::new([paragraph([text("#")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("#".into()),
            // ```markdown
            // ##$$
            // ```
            Scenario::new([paragraph([text("##")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // ## $$
            // ```
            Scenario::new([text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H2,
                },
                [],
            )])
            .at(&[0], 0),
        ),
    ],
)]
#[case::three_hashes_space_starts_heading_three(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("#".into()),
            // ```markdown
            // #$$
            // ```
            Scenario::new([paragraph([text("#")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("#".into()),
            // ```markdown
            // ##$$
            // ```
            Scenario::new([paragraph([text("##")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("#".into()),
            // ```markdown
            // ###$$
            // ```
            Scenario::new([paragraph([text("###")])])
                .at(&[0], 3)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // ### $$
            // ```
            Scenario::new([text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H3,
                },
                [],
            )])
            .at(&[0], 0),
        ),
    ],
)]
#[case::four_hashes_space_starts_heading_four(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    "####"
        .chars()
        .scan(String::new(), |prefix, key| {
            prefix.push(key);
            Some((
                Command::InsertText(key.into()),
                Scenario::new([paragraph([text(prefix.as_str())])])
                    .at(&[0], prefix.chars().count())
                    .with_caret_affinity(Affinity::Upstream),
            ))
        })
        .chain([(
            Command::InsertText(" ".into()),
            // ```markdown
            // #### $$
            // ```
            Scenario::new([text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H4,
                },
                [],
            )])
            .at(&[0], 0),
        )]),
)]
#[case::five_hashes_space_keeps_text(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // Literal text in the paragraph.
    // ```markdown
    // ##### $$
    // ```
    "##### ".chars().scan(String::new(), |prefix, key| {
        prefix.push(key);
        Some((
            Command::InsertText(key.into()),
            Scenario::new([paragraph([text(prefix.as_str())])])
                .at(&[0], prefix.chars().count())
                .with_caret_affinity(Affinity::Upstream),
        ))
    }),
)]
#[case::six_hashes_space_keeps_text(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // Literal text in the paragraph.
    // ```markdown
    // ###### $$
    // ```
    "###### ".chars().scan(String::new(), |prefix, key| {
        prefix.push(key);
        Some((
            Command::InsertText(key.into()),
            Scenario::new([paragraph([text(prefix.as_str())])])
                .at(&[0], prefix.chars().count())
                .with_caret_affinity(Affinity::Upstream),
        ))
    }),
)]
#[case::seven_hashes_space_keeps_text(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // Literal text in the paragraph.
    // ```markdown
    // ####### $$
    // ```
    "####### ".chars().scan(String::new(), |prefix, key| {
        prefix.push(key);
        Some((
            Command::InsertText(key.into()),
            Scenario::new([paragraph([text(prefix.as_str())])])
                .at(&[0], prefix.chars().count())
                .with_caret_affinity(Affinity::Upstream),
        ))
    }),
)]
#[case::heading_consumes_leading_space(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText(" ".into()),
            // ```markdown
            //  $$
            // ```
            Scenario::new([paragraph([text(" ")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("#".into()),
            // ```markdown
            //  #$$
            // ```
            Scenario::new([paragraph([text(" #")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // # $$
            // ```
            Scenario::new([text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H1,
                },
                [],
            )])
            .at(&[0], 0),
        ),
    ],
)]
#[case::hash_space_mid_paragraph_stays_literal(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // Literal text in the paragraph.
    // ```markdown
    // prefix # $$
    // ```
    "prefix # ".chars().scan(String::new(), |prefix, key| {
        prefix.push(key);
        Some((
            Command::InsertText(key.into()),
            Scenario::new([paragraph([text(prefix.as_str())])])
                .at(&[0], prefix.chars().count())
                .with_caret_affinity(Affinity::Upstream),
        ))
    }),
)]
#[case::hyphen_space_starts_bulleted_list(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("-".into()),
            // ```markdown
            // -$$
            // ```
            Scenario::new([paragraph([text("-")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // - $$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([])])],
            )])
            .at(&[0, 0, 0], 0),
        ),
    ],
)]
#[case::asterisk_space_starts_bulleted_list(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("*".into()),
            // ```markdown
            // *$$
            // ```
            Scenario::new([paragraph([text("*")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // - $$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([])])],
            )])
            .at(&[0, 0, 0], 0),
        ),
    ],
)]
#[case::plus_space_starts_bulleted_list(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("+".into()),
            // ```markdown
            // +$$
            // ```
            Scenario::new([paragraph([text("+")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // - $$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([])])],
            )])
            .at(&[0, 0, 0], 0),
        ),
    ],
)]
#[case::one_period_space_starts_numbered_list(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("1".into()),
            // ```markdown
            // 1$$
            // ```
            Scenario::new([paragraph([text("1")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(".".into()),
            // ```markdown
            // 1.$$
            // ```
            Scenario::new([paragraph([text("1.")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // 1. $$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 1 },
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([])])],
            )])
            .at(&[0, 0, 0], 0),
        ),
    ],
)]
#[case::three_period_space_starts_numbered_list_at_three(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("3".into()),
            // ```markdown
            // 3$$
            // ```
            Scenario::new([paragraph([text("3")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(".".into()),
            // ```markdown
            // 3.$$
            // ```
            Scenario::new([paragraph([text("3.")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // 3. $$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 3 },
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([])])],
            )])
            .at(&[0, 0, 0], 0),
        ),
    ],
)]
#[case::zero_period_space_starts_numbered_list_at_zero(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("0".into()),
            // ```markdown
            // 0$$
            // ```
            Scenario::new([paragraph([text("0")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(".".into()),
            // ```markdown
            // 0.$$
            // ```
            Scenario::new([paragraph([text("0.")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // 0. $$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 0 },
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([])])],
            )])
            .at(&[0, 0, 0], 0),
        ),
    ],
)]
#[case::one_parenthesis_space_starts_numbered_list(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("1".into()),
            // ```markdown
            // 1$$
            // ```
            Scenario::new([paragraph([text("1")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(")".into()),
            // ```markdown
            // 1)$$
            // ```
            Scenario::new([paragraph([text("1)")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // 1. $$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 1 },
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([])])],
            )])
            .at(&[0, 0, 0], 0),
        ),
    ],
)]
#[case::nine_digit_ordered_marker_starts_list_at_that_number(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    "123456789."
        .chars()
        .scan(String::new(), |prefix, key| {
            prefix.push(key);
            Some((
                Command::InsertText(key.into()),
                Scenario::new([paragraph([text(prefix.as_str())])])
                    .at(&[0], prefix.chars().count())
                    .with_caret_affinity(Affinity::Upstream),
            ))
        })
        .chain([(
            Command::InsertText(" ".into()),
            // ```markdown
            // 123456789. $$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 123_456_789 },
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([])])],
            )])
            .at(&[0, 0, 0], 0),
        )]),
)]
#[case::third_backtick_starts_fenced_code_without_space_or_enter(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("`".into()),
            // ```markdown
            // `$$
            // ```
            Scenario::new([paragraph([text("`")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("`".into()),
            // ```markdown
            // ``$$
            // ```
            Scenario::new([paragraph([text("``")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("`".into()),
            // ````markdown
            // ```
            // $$
            // ```
            // ````
            Scenario::new([block(
                BlockKind::CodeBlock {
                    language: None,
                    code: String::new(),
                },
                [],
            )])
            .at(&[0], 0),
        ),
    ],
)]
#[case::triple_backticks_mid_paragraph_stay_literal(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // Literal text in the paragraph.
    // ````markdown
    // prefix ```$$
    // ````
    "prefix ```".chars().scan(String::new(), |prefix, key| {
        prefix.push(key);
        Some((
            Command::InsertText(key.into()),
            Scenario::new([paragraph([text(prefix.as_str())])])
                .at(&[0], prefix.chars().count())
                .with_caret_affinity(Affinity::Upstream),
        ))
    }),
)]
#[case::tilde_fence_stays_literal(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("~".into()),
            // ```markdown
            // ~$$
            // ```
            Scenario::new([paragraph([text("~")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("~".into()),
            // ```markdown
            // ~~$$
            // ```
            Scenario::new([paragraph([text("~~")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("~".into()),
            // Literal text in the paragraph.
            // ```markdown
            // ~~~$$
            // ```
            Scenario::new([paragraph([text("~~~")])])
                .at(&[0], 3)
                .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::asterisk_divider_stays_literal(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("*".into()),
            // ```markdown
            // *$$
            // ```
            Scenario::new([paragraph([text("*")])])
                .at(&[0], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("*".into()),
            // ```markdown
            // **$$
            // ```
            Scenario::new([paragraph([text("**")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("*".into()),
            // Literal text in the paragraph.
            // ```markdown
            // ***$$
            // ```
            Scenario::new([paragraph([text("***")])])
                .at(&[0], 3)
                .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
// Notion applies typography to the displayed prefix. Both the displayed
// prefix and the ASCII input can trigger the structural shortcut.
#[case::smart_quote_then_space_starts_quote(
    // ```markdown
    // “$$
    // ```
    Scenario::new([paragraph([text("“")])])
        .at(&[0], 1)
        .with_caret_affinity(Affinity::Upstream),
    [(
        Command::InsertText(" ".into()),
        // ```markdown
        // > $$
        // ```
        Scenario::new([block(BlockKind::BlockQuote, [paragraph([])])]).at(&[0, 0], 0),
    )],
)]
#[case::straight_quote_then_space_starts_quote(
    // ```markdown
    // "$$
    // ```
    Scenario::new([paragraph([text("\"")])])
        .at(&[0], 1)
        .with_caret_affinity(Affinity::Upstream),
    [(
        Command::InsertText(" ".into()),
        // ```markdown
        // > $$
        // ```
        Scenario::new([block(BlockKind::BlockQuote, [paragraph([])])]).at(&[0, 0], 0),
    )],
)]
#[case::emdash_then_hyphen_creates_divider_and_opens_new_paragraph(
    // ```markdown
    // —$$
    // ```
    Scenario::new([paragraph([text("—")])])
        .at(&[0], 1)
        .with_caret_affinity(Affinity::Upstream),
    [(
        Command::InsertText("-".into()),
        // ```markdown
        // ---
        //
        // $$
        // ```
        Scenario::new([block(BlockKind::ThematicBreak, []), paragraph([])]).at(&[1], 0),
    )],
)]
#[case::two_hyphens_then_third_create_divider_and_open_new_paragraph(
    // ```markdown
    // --$$
    // ```
    Scenario::new([paragraph([text("--")])])
        .at(&[0], 2)
        .with_caret_affinity(Affinity::Upstream),
    [(
        Command::InsertText("-".into()),
        // ```markdown
        // ---
        //
        // $$
        // ```
        Scenario::new([block(BlockKind::ThematicBreak, []), paragraph([])]).at(&[1], 0),
    )],
)]
#[case::hash_without_space_stays_text(
    // ```markdown
    // 10710
    // ```
    Scenario::new([paragraph([text("#")])])
        .at(&[0], 1)
        .with_caret_affinity(Affinity::Upstream),
    [(
        Command::Enter,
        // The first paragraph remains literal.
        // ```markdown
        // #
        //
        // $$
        // ```
        Scenario::new([paragraph([text("#")]), paragraph([])]).at(&[1], 0),
    )],
)]
#[case::tilde_fence_stays_text(
    // Literal text in the paragraph.
    // ```markdown
    // ~~~$$
    // ```
    Scenario::new([paragraph([text("~~~")])])
        .at(&[0], 3)
        .with_caret_affinity(Affinity::Upstream),
    [(
        Command::Enter,
        // The first paragraph remains literal.
        // ```markdown
        // ~~~
        //
        // $$
        // ```
        Scenario::new([paragraph([text("~~~")]), paragraph([])]).at(&[1], 0),
    )],
)]
#[case::asterisk_divider_stays_text(
    // Literal text in the paragraph.
    // ```markdown
    // ***$$
    // ```
    Scenario::new([paragraph([text("***")])])
        .at(&[0], 3)
        .with_caret_affinity(Affinity::Upstream),
    [(
        Command::Enter,
        // The first paragraph remains literal.
        // ```markdown
        // ***
        //
        // $$
        // ```
        Scenario::new([paragraph([text("***")]), paragraph([])]).at(&[1], 0),
    )],
)]
#[case::equals_on_following_paragraph_does_not_make_setext_heading(
    // ```markdown
    // above
    //
    // ===$$
    // ```
    Scenario::new([paragraph([text("above")]), paragraph([text("===")])]).at(&[1], 3),
    [(
        Command::Enter,
        // ```markdown
        // above
        //
        // ===
        //
        // $$
        // ```
        Scenario::new([
            paragraph([text("above")]),
            paragraph([text("===")]),
            paragraph([]),
        ])
        .at(&[2], 0),
    )],
)]
#[case::hyphen_typed_after_divider_shortcut_goes_in_next_paragraph(
    // ```markdown
    // —$$
    // ```
    Scenario::new([paragraph([text("—")])])
        .at(&[0], 1)
        .with_caret_affinity(Affinity::Upstream),
    [
        (
            Command::InsertText("-".into()),
            // ```markdown
            // ---
            //
            // $$
            // ```
            Scenario::new([block(BlockKind::ThematicBreak, []), paragraph([])]).at(&[1], 0),
        ),
        (
            Command::InsertText("-".into()),
            // ```markdown
            // ---
            //
            // -$$
            // ```
            Scenario::new([block(BlockKind::ThematicBreak, []), paragraph([text("-")])])
                .at(&[1], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::divider_on_following_paragraph_leaves_previous_paragraph_unchanged(
    // ```markdown
    // above
    //
    // —$$
    // ```
    Scenario::new([paragraph([text("above")]), paragraph([text("—")])]).at(&[1], 1),
    [(
        Command::InsertText("-".into()),
        // ```markdown
        // above
        //
        // ---
        //
        // $$
        // ```
        Scenario::new([
            paragraph([text("above")]),
            block(BlockKind::ThematicBreak, []),
            paragraph([]),
        ])
        .at(&[2], 0),
    )],
)]
#[case::complete_heading_marker_in_one_insertion_starts_heading(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [(
        Command::InsertText("# ".into()),
        // ```markdown
        // # $$
        // ```
        Scenario::new([text_block(
            BlockKind::Heading {
                level: HeadingLevel::H1,
            },
            [],
        )])
        .at(&[0], 0),
    )],
)]
#[case::undo_and_redo_heading_shortcut_keep_the_trigger_space(
    // ```markdown
    // 10710
    // ```
    Scenario::new([paragraph([text("#")])])
        .at(&[0], 1)
        .with_caret_affinity(Affinity::Upstream),
    [
        (
            Command::InsertText(" ".into()),
            // ```markdown
            // # $$
            // ```
            Scenario::new([text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H1,
                },
                [],
            )])
            .at(&[0], 0),
        ),
        (
            Command::Undo,
            // Undo restores the literal marker and space.
            // ```markdown
            // # $$
            // ```
            Scenario::new([paragraph([text("# ")])])
                .at(&[0], 2)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::Redo,
            // ```markdown
            // # $$
            // ```
            Scenario::new([text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H1,
                },
                [],
            )])
            .at(&[0], 0),
        ),
    ],
)]
#[case::heading_keeps_suffix(
    // ```markdown
    // #$$body
    // ```
    Scenario::new([paragraph([text("#body")])]).at(&[0], 1),
    [(
        Command::InsertText(" ".into()),
        // ```markdown
        // # $$body
        // ```
        Scenario::new([text_block(
            BlockKind::Heading {
                level: HeadingLevel::H1,
            },
            [text("body")],
        )])
        .at(&[0], 0),
    )],
)]
#[case::bulleted_item_keeps_suffix(
    // ```markdown
    // -$$body
    // ```
    Scenario::new([paragraph([text("-body")])]).at(&[0], 1),
    [(
        Command::InsertText(" ".into()),
        // ```markdown
        // - $$body
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [block(BlockKind::ListItem, [paragraph([text("body")])])],
        )])
        .at(&[0, 0, 0], 0),
    )],
)]
#[case::space_replaces_selection_then_converts_heading_marker(
    // ```markdown
    // #[|remove|]body
    // ```
    Scenario::new([paragraph([text("#removebody")])]).select(&[0], 1..7),
    [(
        Command::InsertText(" ".into()),
        // ```markdown
        // # $$body
        // ```
        Scenario::new([text_block(
            BlockKind::Heading {
                level: HeadingLevel::H1,
            },
            [text("body")],
        )])
        .at(&[0], 0),
    )],
)]
#[case::hash_space_after_hard_break_stays_literal(
    // ```markdown
    // prefix\
    // 10710
    // ```
    Scenario::new([paragraph([
        text("prefix"),
        inline(InlineKind::HardBreak, []),
        text("#"),
    ])])
    .at(&[0], 8),
    [(
        Command::InsertText(" ".into()),
        // The hash and space stay in the original paragraph.
        // ```markdown
        // prefix\
        // # $$
        // ```
        Scenario::new([paragraph([
            text("prefix"),
            inline(InlineKind::HardBreak, []),
            text("# "),
        ])])
        .at(&[0], 9)
        .with_caret_affinity(Affinity::Upstream),
    )],
)]
#[case::nonconsecutive_number_below_numbered_list_stays_literal(
    // ```markdown
    // 1. first
    //
    // 3.$$
    // ```
    Scenario::new([
        block(
            BlockKind::List {
                kind: ListKind::Ordered { start: 1 },
                tight: true,
            },
            [block(BlockKind::ListItem, [paragraph([text("first")])])],
        ),
        paragraph([text("3.")]),
    ])
    .at(&[1], 2),
    [(
        Command::InsertText(" ".into()),
        // The second block stays a literal paragraph.
        // ```markdown
        // 1. first
        //
        // 3. $$
        // ```
        Scenario::new([
            block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 1 },
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([text("first")])])],
            ),
            paragraph([text("3. ")]),
        ])
        .at(&[1], 3)
        .with_caret_affinity(Affinity::Upstream),
    )],
)]
#[case::consecutive_number_continues_numbered_list(
    // ```markdown
    // 1. first
    //
    // 2.$$
    // ```
    Scenario::new([
        block(
            BlockKind::List {
                kind: ListKind::Ordered { start: 1 },
                tight: true,
            },
            [block(BlockKind::ListItem, [paragraph([text("first")])])],
        ),
        paragraph([text("2.")]),
    ])
    .at(&[1], 2),
    [(
        Command::InsertText(" ".into()),
        // ```markdown
        // 1. first
        // 2. $$
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Ordered { start: 1 },
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([text("first")])]),
                block(BlockKind::ListItem, [paragraph([])]),
            ],
        )])
        .at(&[0, 1, 0], 0),
    )],
)]
#[case::another_bullet_marker_continues_bulleted_list(
    // ```markdown
    // - first
    //
    // *$$
    // ```
    Scenario::new([
        block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [block(BlockKind::ListItem, [paragraph([text("first")])])],
        ),
        paragraph([text("*")]),
    ])
    .at(&[1], 1),
    [(
        Command::InsertText(" ".into()),
        // ```markdown
        // - first
        // - $$
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([text("first")])]),
                block(BlockKind::ListItem, [paragraph([])]),
            ],
        )])
        .at(&[0, 1, 0], 0),
    )],
)]
#[case::third_backtick_replaces_last_ordered_item_with_code_after_list(
    // ```markdown
    // 1. first
    // 2. second
    // 3. ``$$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Ordered { start: 1 },
            tight: true,
        },
        [
            block(BlockKind::ListItem, [paragraph([text("first")])]),
            block(BlockKind::ListItem, [paragraph([text("second")])]),
            block(BlockKind::ListItem, [paragraph([text("``")])]),
        ],
    )])
    .at(&[0, 2, 0], 2),
    [(
        Command::InsertText("`".into()),
        // ````markdown
        // 1. first
        // 2. second
        //
        // ```
        // $$
        // ```
        // ````
        Scenario::new([
            block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 1 },
                    tight: true,
                },
                [
                    block(BlockKind::ListItem, [paragraph([text("first")])]),
                    block(BlockKind::ListItem, [paragraph([text("second")])]),
                ],
            ),
            block(
                BlockKind::CodeBlock {
                    language: None,
                    code: String::new(),
                },
                [],
            ),
        ])
        .at(&[1], 0),
    )],
)]
// Inline formatting shortcuts and literal input.
// https://www.notion.com/help/keyboard-shortcuts#markdown-style
#[case::bold_with_two_asterisks(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // \*\*bold\*$$
        // ```
        let typing = "**bold*"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"**bold*"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        // ```markdown
        // **bold**tail$$
        // ```
        let tail = "tail"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"tail"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([strong([text("bold")]), text(prefix)])]).at(&[0], 4 + index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        typing
            .chain([(
                Command::InsertText("*".into()),
                // ```markdown
                // **bold**$$
                // ```
                Scenario::new([paragraph([strong([text("bold")])])]).at(&[0], 4),
            )])
            .chain(tail)
    },
)]
#[case::italic_with_one_asterisk(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // \*italic$$
        // ```
        let typing = "*italic"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"*italic"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        // ```markdown
        // *italic*tail$$
        // ```
        let tail = "tail"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"tail"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([emphasis([text("italic")]), text(prefix)])]).at(&[0], 6 + index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        typing
            .chain([(
                Command::InsertText("*".into()),
                // ```markdown
                // *italic*$$
                // ```
                Scenario::new([paragraph([emphasis([text("italic")])])]).at(&[0], 6),
            )])
            .chain(tail)
    },
)]
#[case::inline_code_with_one_backtick(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // \`code$$
        // ```
        let typing = "`code"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"`code"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        // ```markdown
        // `code`tail$$
        // ```
        let tail = "tail"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"tail"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([
                        inline(InlineKind::Code("code".into()), []),
                        text(prefix),
                    ])]).at(&[0], 4 + index + 1).with_caret_affinity(Affinity::Upstream),
                )
            });
        typing
            .chain([(
                Command::InsertText("`".into()),
                // ```markdown
                // `code`$$
                // ```
                Scenario::new([paragraph([inline(InlineKind::Code("code".into()), [])])]).at(&[0], 4),
            )])
            .chain(tail)
    },
)]
#[case::punctuation_before_and_after_italic(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // pre \(\*x$$
        // ```
        let typing = "pre (*x"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"pre (*x"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        typing.chain([
            (
                Command::InsertText("*".into()),
                // ```markdown
                // pre (*x*$$
                // ```
                Scenario::new([paragraph([text("pre ("), emphasis([text("x")])])]).at(&[0], 6),
            ),
            (
                Command::InsertText(")".into()),
                // ```markdown
                // pre (*x*)$$
                // ```
                Scenario::new([paragraph([text("pre ("), emphasis([text("x")]), text(")")])]).at(&[0], 7)
                    .with_caret_affinity(Affinity::Upstream),
            ),
        ])
    },
)]
#[case::inline_code_between_plain_words(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // pre \`x$$
        // ```
        let typing = "pre `x"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"pre `x"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        // ```markdown
        // pre `x` again$$
        // ```
        let tail = " again"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &" again"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([
                        text("pre "),
                        inline(InlineKind::Code("x".into()), []),
                        text(prefix),
                    ])]).at(&[0], 5 + index + 1).with_caret_affinity(Affinity::Upstream),
                )
            });
        typing
            .chain([(
                Command::InsertText("`".into()),
                // ```markdown
                // pre `x`$$
                // ```
                Scenario::new([paragraph([text("pre "), inline(InlineKind::Code("x".into()), [])])]).at(&[0], 5),
            )])
            .chain(tail)
    },
)]
#[case::complete_bold_span(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("**paste**".into()),
            // ```markdown
            // **paste**$$
            // ```
            Scenario::new([paragraph([strong([text("paste")])])]).at(&[0], 5),
        ),
    ],
)]
#[case::underscore_emphasis_and_strong_in_committed_fragment(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    [
        (
            Command::InsertText("_italic_ __bold__".into()),
            // ```markdown
            // _italic_ __bold__$$
            // ```
            Scenario::new([paragraph([
                emphasis([text("italic")]),
                text(" "),
                strong([text("bold")]),
            ])])
            .at(&[0], 11),
        ),
    ],
)]
#[case::strong_in_heading(
    // ```markdown
    // # \*\*head\*$$
    // ```
    Scenario::new([text_block(BlockKind::Heading { level: HeadingLevel::H1 }, [text("**head*")])])
    .at(&[0], 7)
    .with_caret_affinity(Affinity::Upstream),
    [
        (
            Command::InsertText("*".into()),
            // ```markdown
            // # **head**$$
            // ```
            Scenario::new([text_block(BlockKind::Heading { level: HeadingLevel::H1 }, [strong([text("head")])])])
            .at(&[0], 4),
        ),
        (
            Command::InsertText("tail".into()),
            // ```markdown
            // # **head**tail$$
            // ```
            Scenario::new([text_block(BlockKind::Heading { level: HeadingLevel::H1 }, [strong([text("head")]), text("tail")])])
            .at(&[0], 8)
            .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::emphasis_in_quote(
    // ```markdown
    // > \*quote$$
    // ```
    Scenario::new([block(BlockKind::BlockQuote, [paragraph([text("*quote")])])])
    .at(&[0, 0], 6)
    .with_caret_affinity(Affinity::Upstream),
    [
        (
            Command::InsertText("*".into()),
            // ```markdown
            // > *quote*$$
            // ```
            Scenario::new([block(BlockKind::BlockQuote, [paragraph([emphasis([text("quote")])])])])
            .at(&[0, 0], 5),
        ),
        (
            Command::InsertText("tail".into()),
            // ```markdown
            // > *quote*tail$$
            // ```
            Scenario::new([block(BlockKind::BlockQuote, [paragraph([emphasis([text("quote")]), text("tail")])])])
            .at(&[0, 0], 9)
            .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::code_in_bullet_item(
    // ```markdown
    // - \`list$$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [block(BlockKind::ListItem, [paragraph([text("`list")])])],
    )])
    .at(&[0, 0, 0], 5)
    .with_caret_affinity(Affinity::Upstream),
    [
        (
            Command::InsertText("`".into()),
            // ```markdown
            // - `list`$$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([inline(InlineKind::Code("list".into()), [])])])],
            )])
            .at(&[0, 0, 0], 4),
        ),
        (
            Command::InsertText("tail".into()),
            // ```markdown
            // - `list`tail$$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([inline(InlineKind::Code("list".into()), []), text("tail")])])],
            )])
            .at(&[0, 0, 0], 8)
            .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::underscore_emphasis_in_bullet_item(
    // ```markdown
    // - \_one$$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [block(BlockKind::ListItem, [paragraph([text("_one")])])],
    )])
    .at(&[0, 0, 0], 4)
    .with_caret_affinity(Affinity::Upstream),
    [
        (
            Command::InsertText("_".into()),
            // ```markdown
            // - *one*$$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([emphasis([text("one")])])])],
            )])
            .at(&[0, 0, 0], 3),
        ),
        (
            Command::InsertText("tail".into()),
            // ```markdown
            // - *one*tail$$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([emphasis([text("one")]), text("tail")])])],
            )])
            .at(&[0, 0, 0], 7)
            .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::double_underscore_strong_in_bullet_item(
    // ```markdown
    // - \_\_two\_$$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [block(BlockKind::ListItem, [paragraph([text("__two_")])])],
    )])
    .at(&[0, 0, 0], 6)
    .with_caret_affinity(Affinity::Upstream),
    [
        (
            Command::InsertText("_".into()),
            // ```markdown
            // - **two**$$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([strong([text("two")])])])],
            )])
            .at(&[0, 0, 0], 3),
        ),
        (
            Command::InsertText("tail".into()),
            // ```markdown
            // - **two**tail$$
            // ```
            Scenario::new([block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([strong([text("two")]), text("tail")])])],
            )])
            .at(&[0, 0, 0], 7)
            .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::bold_opening_attached_to_word(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // pre\*\*bold\*\*tail$$
    // ```
    "pre**bold**tail"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"pre**bold**tail"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::italic_opening_attached_to_word(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // x\*y\*z$$
    // ```
    "x*y*z"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"x*y*z"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::space_after_italic_opening(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // pre \* x\*$$
    // ```
    "pre * x*"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"pre * x*"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::space_before_italic_closing(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // pre \*x \*$$
    // ```
    "pre *x *"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"pre *x *"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::backslash_before_opening_asterisk(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // \\\*word\*$$
    // ```
    "\\*word*"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"\\*word*"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::asterisk_delimiters_without_content(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // pre \*\*\*\*$$
    // ```
    "pre ****"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"pre ****"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::double_backticks(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // \`\`code\`\`tail$$
    // ```
    "``code``tail"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"``code``tail"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::space_on_both_sides_inside_backticks(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // pre \` x \`$$
    // ```
    "pre ` x `"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"pre ` x `"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::double_tildes(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // \~\~strike\~\~tail$$
    // ```
    "~~strike~~tail"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"~~strike~~tail"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::inline_markdown_link(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    // ```markdown
    // \[label\]\(https://example.com\)$$
    // ```
    "[label](https://example.com)"
        .char_indices()
        .enumerate()
        .map(move |(index, (start, character))| {
            let prefix = &"[label](https://example.com)"[..start + character.len_utf8()];
            (
                Command::InsertText(character.to_string()),
                Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                    .with_caret_affinity(Affinity::Upstream),
            )
        }),
)]
#[case::triple_asterisks_apply_bold_then_italic(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // \*\*\*both\*$$
        // ```
        let typing = "***both*"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"***both*"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        // ```markdown
        // ***both***tail$$
        // ```
        let tail = "tail"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"tail"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([emphasis([strong([text("both")])]), text(prefix)])]).at(&[0], 5 + index)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        typing
            .chain([
                (
                    Command::InsertText("*".into()),
                    // ```markdown
                    // **\*both**$$
                    // ```
                    Scenario::new([paragraph([strong([text("*both")])])]).at(&[0], 5),
                ),
                (
                    Command::InsertText("*".into()),
                    // ```markdown
                    // ***both***$$
                    // ```
                    Scenario::new([paragraph([emphasis([strong([text("both")])])])]).at(&[0], 4),
                ),
            ])
            .chain(tail)
    },
)]
#[case::typing_closing_delimiter_before_existing_suffix_keeps_suffix_plain(
    // ```markdown
    // pre \*\*bold\*$$end
    // ```
    Scenario::new([paragraph([text("pre **bold*end")])]).at(&[0], 11),
    [
        (
            Command::InsertText("*".into()),
            // ```markdown
            // pre **bold**$$end
            // ```
            Scenario::new([paragraph([text("pre "), strong([text("bold")]), text("end")])]).at(&[0], 8),
        ),
    ],
)]
#[case::typing_italic_after_bold_preserves_both_marks(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // pre \*\*x\*$$
        // ```
        let opening = "pre **x*"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"pre **x*"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        // ```markdown
        // pre **x** and \*y$$
        // ```
        let next_mark = " and *y"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &" and *y"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text("pre "), strong([text("x")]), text(prefix)])]).at(&[0], 6 + index)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        opening
            .chain([(
                Command::InsertText("*".into()),
                // ```markdown
                // pre **x**$$
                // ```
                Scenario::new([paragraph([text("pre "), strong([text("x")])])]).at(&[0], 5),
            )])
            .chain(next_mark)
            .chain([(
                Command::InsertText("*".into()),
                // ```markdown
                // pre **x** and *y*$$
                // ```
                Scenario::new([paragraph([
                    text("pre "), strong([text("x")]), text(" and "), emphasis([text("y")]),
                ])]).at(&[0], 11),
            )])
    },
)]
#[case::typing_closing_delimiter_bolds_unicode_at_scalar_offsets(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // **€😀**tail$$
        // ```
        let tail = "tail"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"tail"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([strong([text("€😀")]), text(prefix)])]).at(&[0], 3 + index)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        [
            (
                Command::InsertText("*".into()),
                // ```markdown
                // \*$$
                // ```
                Scenario::new([paragraph([text("*")])]).at(&[0], 1)
                    .with_caret_affinity(Affinity::Upstream),
            ),
            (
                Command::InsertText("*".into()),
                // ```markdown
                // \*\*$$
                // ```
                Scenario::new([paragraph([text("**")])]).at(&[0], 2)
                    .with_caret_affinity(Affinity::Upstream),
            ),
            (
                Command::InsertText("€😀".into()),
                // ```markdown
                // \*\*€😀$$
                // ```
                Scenario::new([paragraph([text("**€😀")])]).at(&[0], 4)
                    .with_caret_affinity(Affinity::Upstream),
            ),
            (
                Command::InsertText("*".into()),
                // ```markdown
                // \*\*€😀\*$$
                // ```
                Scenario::new([paragraph([text("**€😀*")])]).at(&[0], 5)
                    .with_caret_affinity(Affinity::Upstream),
            ),
            (
                Command::InsertText("*".into()),
                // ```markdown
                // **€😀**$$
                // ```
                Scenario::new([paragraph([strong([text("€😀")])])]).at(&[0], 2),
            ),
        ]
        .into_iter()
        .chain(tail)
    },
)]
#[case::typing_closing_delimiter_bolds_across_a_hard_break(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // \*\*between two$$
        // ```
        let before_break = "**between two"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"**between two"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        // ```markdown
        // \*\*between two\
        // lines\*$$
        // ```
        let after_break = "lines*"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"lines*"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([
                        text("**between two"),
                        inline(InlineKind::HardBreak, []),
                        text(prefix),
                    ])]).at(&[0], 15 + index).with_caret_affinity(Affinity::Upstream),
                )
            });
        before_break
            .chain([(
                Command::InsertHardBreak,
                // ```markdown
                // \*\*between two\
                // $$
                // ```
                Scenario::new([paragraph([text("**between two"), inline(InlineKind::HardBreak, [])])]).at(&[0], 14),
            )])
            .chain(after_break)
            .chain([(
                Command::InsertText("*".into()),
                // ```markdown
                // **between two\
                // lines**$$
                // ```
                Scenario::new([paragraph([strong([
                    text("between two"),
                    inline(InlineKind::HardBreak, []),
                    text("lines"),
                ])])]).at(&[0], 17),
            )])
    },
)]
#[case::undo_and_redo_inline_shortcut_toggle_formatting_and_literal_delimiters(
    // ```markdown
    // $$
    // ```
    Scenario::new([paragraph([])]),
    {
        // ```markdown
        // \*\*undo\*$$
        // ```
        let typing = "**undo*"
            .char_indices()
            .enumerate()
            .map(move |(index, (start, character))| {
                let prefix = &"**undo*"[..start + character.len_utf8()];
                (
                    Command::InsertText(character.to_string()),
                    Scenario::new([paragraph([text(prefix)])]).at(&[0], index + 1)
                        .with_caret_affinity(Affinity::Upstream),
                )
            });
        typing.chain([
            (
                Command::InsertText("*".into()),
                // ```markdown
                // **undo**$$
                // ```
                Scenario::new([paragraph([strong([text("undo")])])]).at(&[0], 4),
            ),
            (
                Command::Undo,
                // ```markdown
                // \*\*undo\*\*$$
                // ```
                Scenario::new([paragraph([text("**undo**")])]).at(&[0], 8)
                    .with_caret_affinity(Affinity::Upstream),
            ),
            (
                Command::Redo,
                // ```markdown
                // **undo**$$
                // ```
                Scenario::new([paragraph([strong([text("undo")])])]).at(&[0], 4),
            ),
        ])
    },
)]
#[case::backspace_after_inline_shortcut_moves_caret_into_formatted_text(
    // ```markdown
    // \*\*erase\*$$
    // ```
    Scenario::new([paragraph([text("**erase*")])]).at(&[0], 8)
        .with_caret_affinity(Affinity::Upstream),
    [
        (
            Command::InsertText("*".into()),
            // ```markdown
            // **erase**$$
            // ```
            Scenario::new([paragraph([strong([text("erase")])])]).at(&[0], 5),
        ),
        (
            Command::Backspace,
            // ```markdown
            // **eras$$**
            // ```
            Scenario::new([paragraph([strong([text("eras")])])]).at(&[0], 4)
                .with_caret_affinity(Affinity::Upstream),
        ),
        (
            Command::InsertText("z".into()),
            // ```markdown
            // **erasz$$**
            // ```
            Scenario::new([paragraph([strong([text("erasz")])])]).at(&[0], 5)
                .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::hard_break_inside_bullet_item_keeps_one_item(
    // ```markdown
    // - ab$$CD
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [block(BlockKind::ListItem, [paragraph([text("abCD")])])],
    )])
    .at(&[0, 0, 0], 2),
    [(
        Command::InsertHardBreak,
        // ```markdown
        // - ab\
        //   $$CD
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [block(
                BlockKind::ListItem,
                [paragraph([
                    text("ab"),
                    inline(InlineKind::HardBreak, []),
                    text("CD"),
                ])],
            )],
        )])
        .at(&[0, 0, 0], 3),
    )],
)]
#[case::hard_break_inside_quote_keeps_one_paragraph(
    // ```markdown
    // > ab$$CD
    // ```
    Scenario::new([block(BlockKind::BlockQuote, [paragraph([text("abCD")])])]).at(&[0, 0], 2),
    [(
        Command::InsertHardBreak,
        // ```markdown
        // > ab\
        // > $$CD
        // ```
        Scenario::new([block(
            BlockKind::BlockQuote,
            [paragraph([
                text("ab"),
                inline(InlineKind::HardBreak, []),
                text("CD"),
            ])],
        )])
        .at(&[0, 0], 3),
    )],
)]
#[case::indent_bullet_item_moves_under_previous_item(
    // ```markdown
    // - parent
    // - ch$$ild
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [
            block(BlockKind::ListItem, [paragraph([text("parent")])]),
            block(BlockKind::ListItem, [paragraph([text("child")])]),
        ],
    )])
    .at(&[0, 1, 0], 2),
    [(
        Command::IndentListItem,
        // ```markdown
        // - parent
        //   - ch$$ild
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [block(
                BlockKind::ListItem,
                [
                    paragraph([text("parent")]),
                    block(
                        BlockKind::List {
                            kind: ListKind::Bullet,
                            tight: true,
                        },
                        [block(BlockKind::ListItem, [paragraph([text("child")])])],
                    ),
                ],
            )],
        )])
        .at(&[0, 0, 1, 0, 0], 2),
    )],
)]
#[case::indent_bullet_item_keeps_child_and_following_sibling_at_their_depths(
    // ```markdown
    // - before
    // - ta$$rget
    //   - child
    // - after
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [
            block(BlockKind::ListItem, [paragraph([text("before")])]),
            block(
                BlockKind::ListItem,
                [
                    paragraph([text("target")]),
                    block(
                        BlockKind::List {
                            kind: ListKind::Bullet,
                            tight: true,
                        },
                        [block(BlockKind::ListItem, [paragraph([text("child")])])],
                    ),
                ],
            ),
            block(BlockKind::ListItem, [paragraph([text("after")])]),
        ],
    )])
    .at(&[0, 1, 0], 2),
    [(
        Command::IndentListItem,
        // ```markdown
        // - before
        //   - ta$$rget
        //   - child
        // - after
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(
                    BlockKind::ListItem,
                    [
                        paragraph([text("before")]),
                        block(
                            BlockKind::List {
                                kind: ListKind::Bullet,
                                tight: true,
                            },
                            [
                                block(BlockKind::ListItem, [paragraph([text("target")])]),
                                block(BlockKind::ListItem, [paragraph([text("child")])]),
                            ],
                        ),
                    ],
                ),
                block(BlockKind::ListItem, [paragraph([text("after")])]),
            ],
        )])
        .at(&[0, 0, 1, 0, 0], 2),
    )],
)]
#[case::indent_middle_bullet_leaves_following_sibling_at_top_level(
    // ```markdown
    // - before
    // - ta$$rget
    // - after
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [
            block(BlockKind::ListItem, [paragraph([text("before")])]),
            block(BlockKind::ListItem, [paragraph([text("target")])]),
            block(BlockKind::ListItem, [paragraph([text("after")])]),
        ],
    )])
    .at(&[0, 1, 0], 2),
    [(
        Command::IndentListItem,
        // ```markdown
        // - before
        //   - ta$$rget
        // - after
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(
                    BlockKind::ListItem,
                    [
                        paragraph([text("before")]),
                        block(
                            BlockKind::List {
                                kind: ListKind::Bullet,
                                tight: true,
                            },
                            [block(BlockKind::ListItem, [paragraph([text("target")])])],
                        ),
                    ],
                ),
                block(BlockKind::ListItem, [paragraph([text("after")])]),
            ],
        )])
        .at(&[0, 0, 1, 0, 0], 2),
    )],
)]
#[case::indent_first_nested_bullet_keeps_its_depth(
    // ```markdown
    // - before
    //   - ta$$rget
    // - after
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [
            block(
                BlockKind::ListItem,
                [
                    paragraph([text("before")]),
                    block(
                        BlockKind::List {
                            kind: ListKind::Bullet,
                            tight: true,
                        },
                        [block(BlockKind::ListItem, [paragraph([text("target")])])],
                    ),
                ],
            ),
            block(BlockKind::ListItem, [paragraph([text("after")])]),
        ],
    )])
    .at(&[0, 0, 1, 0, 0], 2),
    [(
        Command::IndentListItem,
        // ```markdown
        // - before
        //   - ta$$rget
        // - after
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(
                    BlockKind::ListItem,
                    [
                        paragraph([text("before")]),
                        block(
                            BlockKind::List {
                                kind: ListKind::Bullet,
                                tight: true,
                            },
                            [block(BlockKind::ListItem, [paragraph([text("target")])])],
                        ),
                    ],
                ),
                block(BlockKind::ListItem, [paragraph([text("after")])]),
            ],
        )])
        .at(&[0, 0, 1, 0, 0], 2),
    )],
)]
#[case::outdent_first_nested_bullet_keeps_following_bullet_at_its_depth(
    // ```markdown
    // - before
    //   - ta$$rget
    //   - after
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [block(
        BlockKind::ListItem,
        [
            paragraph([text("before")]),
            block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [
                    block(BlockKind::ListItem, [paragraph([text("target")])]),
                    block(BlockKind::ListItem, [paragraph([text("after")])]),
                ],
            ),
        ],
    )],
    )])
    .at(&[0, 0, 1, 0, 0], 2),
    [(
        Command::OutdentListItem,
        // ```markdown
        // - before
        // - ta$$rget
        //   - after
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([text("before")])]),
                block(
                    BlockKind::ListItem,
                    [
                        paragraph([text("target")]),
                        block(
                            BlockKind::List {
                                kind: ListKind::Bullet,
                                tight: true,
                            },
                            [block(BlockKind::ListItem, [paragraph([text("after")])])],
                        ),
                    ],
                ),
            ],
        )])
        .at(&[0, 1, 0], 2),
    )],
)]
#[case::outdent_numbered_item_promotes_one_level(
    // ```markdown
    // 1. parent
    //    1. ch$$ild
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Ordered { start: 1 },
            tight: true,
        },
        [block(
            BlockKind::ListItem,
            [
                paragraph([text("parent")]),
                block(
                    BlockKind::List {
                        kind: ListKind::Ordered { start: 1 },
                        tight: true,
                    },
                    [block(BlockKind::ListItem, [paragraph([text("child")])])],
                ),
            ],
        )],
    )])
    .at(&[0, 0, 1, 0, 0], 2),
    [(
        Command::OutdentListItem,
        // ```markdown
        // 1. parent
        // 2. ch$$ild
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Ordered { start: 1 },
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([text("parent")])]),
                block(BlockKind::ListItem, [paragraph([text("child")])]),
            ],
        )])
        .at(&[0, 1, 0], 2),
    )],
)]
#[case::enter_after_heading_creates_paragraph(
    // ```markdown
    // # Heading$$
    // ```
    Scenario::new([text_block(
        BlockKind::Heading {
            level: HeadingLevel::H1,
        },
        [text("Heading")],
    )])
    .at(&[0], 7),
    [(
        Command::Enter,
        // ```markdown
        // # Heading
        //
        // $$
        // ```
        Scenario::new([
            text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H1,
                },
                [text("Heading")],
            ),
            paragraph([]),
        ])
        .at(&[1], 0),
    )],
)]
#[case::enter_in_middle_of_heading_creates_paragraph_with_suffix(
    // ```markdown
    // # ab$$CD
    // ```
    Scenario::new([text_block(
        BlockKind::Heading {
            level: HeadingLevel::H1,
        },
        [text("abCD")],
    )])
    .at(&[0], 2),
    [(
        Command::Enter,
        // ```markdown
        // # ab
        //
        // $$CD
        // ```
        Scenario::new([
            text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H1,
                },
                [text("ab")],
            ),
            paragraph([text("CD")]),
        ])
        .at(&[1], 0),
    )],
)]
#[case::enter_at_heading_start_inserts_paragraph_before_heading(
    // ```markdown
    // # $$abcd
    // ```
    Scenario::new([text_block(
        BlockKind::Heading {
            level: HeadingLevel::H1,
        },
        [text("abcd")],
    )])
    .at(&[0], 0),
    [(
        Command::Enter,
        // The first block is an empty paragraph.
        // ```markdown
        //
        // # $$abcd
        // ```
        Scenario::new([
            paragraph([]),
            text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H1,
                },
                [text("abcd")],
            ),
        ])
        .at(&[1], 0),
    )],
)]
#[case::backspace_at_heading_start_joins_previous_plain_paragraph(
    // ```markdown
    // before
    //
    // # $$abcd
    // ```
    Scenario::new([
        paragraph([text("before")]),
        text_block(
            BlockKind::Heading {
                level: HeadingLevel::H1,
            },
            [text("abcd")],
        ),
    ])
    .at(&[1], 0),
    [(
        Command::Backspace,
        // ```markdown
        // before$$abcd
        // ```
        Scenario::new([paragraph([text("beforeabcd")])]).at(&[0], 6),
    )],
)]
#[case::enter_after_quote_creates_plain_paragraph(
    // ```markdown
    // > abcd$$
    // ```
    Scenario::new([block(BlockKind::BlockQuote, [paragraph([text("abcd")])])]).at(&[0, 0], 4),
    [(
        Command::Enter,
        // ```markdown
        // > abcd
        //
        // $$
        // ```
        Scenario::new([
            block(BlockKind::BlockQuote, [paragraph([text("abcd")])]),
            paragraph([]),
        ])
        .at(&[1], 0),
    )],
)]
#[case::enter_in_middle_of_quote_moves_suffix_to_plain_paragraph(
    // ```markdown
    // > ab$$cd
    // ```
    Scenario::new([block(BlockKind::BlockQuote, [paragraph([text("abcd")])])]).at(&[0, 0], 2),
    [(
        Command::Enter,
        // ```markdown
        // > ab
        //
        // $$cd
        // ```
        Scenario::new([
            block(BlockKind::BlockQuote, [paragraph([text("ab")])]),
            paragraph([text("cd")]),
        ])
        .at(&[1], 0),
    )],
)]
#[case::backspace_at_quote_start_joins_previous_plain_paragraph(
    // ```markdown
    // before
    //
    // > $$abcd
    // ```
    Scenario::new([
        paragraph([text("before")]),
        block(BlockKind::BlockQuote, [paragraph([text("abcd")])]),
    ])
    .at(&[1, 0], 0),
    [(
        Command::Backspace,
        // ```markdown
        // before$$abcd
        // ```
        Scenario::new([paragraph([text("beforeabcd")])]).at(&[0], 6),
    )],
)]
#[case::enter_continues_bullet_list_at_item_end(
    // ```markdown
    // - alpha$$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [block(BlockKind::ListItem, [paragraph([text("alpha")])])],
    )])
    .at(&[0, 0, 0], 5),
    [(
        Command::Enter,
        // ```markdown
        // - alpha
        // - $$
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([text("alpha")])]),
                block(BlockKind::ListItem, [paragraph([])]),
            ],
        )])
        .at(&[0, 1, 0], 0),
    )],
)]
#[case::enter_splits_bullet_item_at_caret(
    // ```markdown
    // - ab$$CD
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [block(BlockKind::ListItem, [paragraph([text("abCD")])])],
    )])
    .at(&[0, 0, 0], 2),
    [(
        Command::Enter,
        // ```markdown
        // - ab
        // - $$CD
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([text("ab")])]),
                block(BlockKind::ListItem, [paragraph([text("CD")])]),
            ],
        )])
        .at(&[0, 1, 0], 0),
    )],
)]
#[case::enter_exits_bullet_list_from_empty_final_item(
    // ```markdown
    // - alpha
    // - $$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [
            block(BlockKind::ListItem, [paragraph([text("alpha")])]),
            block(BlockKind::ListItem, [paragraph([])]),
        ],
    )]),
    [(
        Command::Enter,
        // ```markdown
        // - alpha
        //
        // $$
        // ```
        Scenario::new([
            block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([text("alpha")])])],
            ),
            paragraph([]),
        ])
        .at(&[1], 0),
    )],
)]
#[case::enter_at_start_of_bullet_item_inserts_empty_item_before_it(
    // ```markdown
    // - $$abcd
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [block(BlockKind::ListItem, [paragraph([text("abcd")])])],
    )])
    .at(&[0, 0, 0], 0),
    [(
        Command::Enter,
        // ```markdown
        // -
        // - $$abcd
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([])]),
                block(BlockKind::ListItem, [paragraph([text("abcd")])]),
            ],
        )])
        .at(&[0, 1, 0], 0),
    )],
)]
#[case::backspace_at_bullet_start_after_plain_paragraph_removes_bullet(
    // ```markdown
    // before
    //
    // - $$abcd
    // ```
    Scenario::new([
        paragraph([text("before")]),
        block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [block(BlockKind::ListItem, [paragraph([text("abcd")])])],
        ),
    ])
    .at(&[1, 0, 0], 0),
    [(
        Command::Backspace,
        // ```markdown
        // before
        //
        // $$abcd
        // ```
        Scenario::new([paragraph([text("before")]), paragraph([text("abcd")])]).at(&[1], 0),
    )],
)]
#[case::backspace_at_second_bullet_start_keeps_it_separate_as_plain_paragraph(
    // ```markdown
    // - before
    // - $$abcd
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [
            block(BlockKind::ListItem, [paragraph([text("before")])]),
            block(BlockKind::ListItem, [paragraph([text("abcd")])]),
        ],
    )])
    .at(&[0, 1, 0], 0),
    [(
        Command::Backspace,
        // ```markdown
        // - before
        //
        // $$abcd
        // ```
        Scenario::new([
            block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([text("before")])])],
            ),
            paragraph([text("abcd")]),
        ])
        .at(&[1], 0),
    )],
)]
#[case::enter_inside_code_keeps_newline_in_code_block(
    // ````markdown
    // ```
    // ab$$CD
    // ```
    // ````
    Scenario::new([block(
        BlockKind::CodeBlock {
            language: None,
            code: "abCD".into(),
        },
        [],
    )])
    .at(&[0], 2),
    [(
        Command::Enter,
        // ````markdown
        // ```
        // ab
        // $$CD
        // ```
        // ````
        Scenario::new([block(
            BlockKind::CodeBlock {
                language: None,
                code: "ab\nCD".into(),
            },
            [],
        )])
        .at(&[0], 3),
    )],
)]
#[case::enter_at_nested_numbered_item_end_continues_nested_list(
    // ```markdown
    // 1. parent
    //    1. child$$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Ordered { start: 1 },
            tight: true,
        },
        [block(
            BlockKind::ListItem,
            [
                paragraph([text("parent")]),
                block(
                    BlockKind::List {
                        kind: ListKind::Ordered { start: 1 },
                        tight: true,
                    },
                    [block(BlockKind::ListItem, [paragraph([text("child")])])],
                ),
            ],
        )],
    )])
    .at(&[0, 0, 1, 0, 0], 5),
    [(
        Command::Enter,
        // ```markdown
        // 1. parent
        //    1. child
        //    2. $$
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Ordered { start: 1 },
                tight: true,
            },
            [block(
                BlockKind::ListItem,
                [
                    paragraph([text("parent")]),
                    block(
                        BlockKind::List {
                            kind: ListKind::Ordered { start: 1 },
                            tight: true,
                        },
                        [
                            block(BlockKind::ListItem, [paragraph([text("child")])]),
                            block(BlockKind::ListItem, [paragraph([])]),
                        ],
                    ),
                ],
            )],
        )])
        .at(&[0, 0, 1, 1, 0], 0),
    )],
)]
#[case::enter_on_empty_nested_numbered_item_promotes_it_one_level(
    // ```markdown
    // 1. parent
    //    1. child
    //    2. $$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Ordered { start: 1 },
            tight: true,
        },
        [block(
            BlockKind::ListItem,
            [
                paragraph([text("parent")]),
                block(
                    BlockKind::List {
                        kind: ListKind::Ordered { start: 1 },
                        tight: true,
                    },
                    [
                        block(BlockKind::ListItem, [paragraph([text("child")])]),
                        block(BlockKind::ListItem, [paragraph([])]),
                    ],
                ),
            ],
        )],
    )])
    .at(&[0, 0, 1, 1, 0], 0),
    [(
        Command::Enter,
        // ```markdown
        // 1. parent
        //    1. child
        // 2. $$
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Ordered { start: 1 },
                tight: true,
            },
            [
                block(
                    BlockKind::ListItem,
                    [
                        paragraph([text("parent")]),
                        block(
                            BlockKind::List {
                                kind: ListKind::Ordered { start: 1 },
                                tight: true,
                            },
                            [block(BlockKind::ListItem, [paragraph([text("child")])])],
                        ),
                    ],
                ),
                block(BlockKind::ListItem, [paragraph([])]),
            ],
        )])
        .at(&[0, 1, 0], 0),
    )],
)]
#[case::backspace_on_empty_middle_nested_numbered_item_splits_nested_list(
    // ```markdown
    // 1. parent
    //    1. child
    //    2. $$
    //    3. third
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Ordered { start: 1 },
            tight: true,
        },
        [block(
            BlockKind::ListItem,
            [
                paragraph([text("parent")]),
                block(
                    BlockKind::List {
                        kind: ListKind::Ordered { start: 1 },
                        tight: true,
                    },
                    [
                        block(BlockKind::ListItem, [paragraph([text("child")])]),
                        block(BlockKind::ListItem, [paragraph([])]),
                        block(BlockKind::ListItem, [paragraph([text("third")])]),
                    ],
                ),
            ],
        )],
    )])
    .at(&[0, 0, 1, 1, 0], 0),
    [(
        Command::Backspace,
        // ```markdown
        // 1. parent
        //    1. child
        //
        //    $$
        //
        //    1. third
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Ordered { start: 1 },
                tight: true,
            },
            [block(
                BlockKind::ListItem,
                [
                    paragraph([text("parent")]),
                    block(
                        BlockKind::List {
                            kind: ListKind::Ordered { start: 1 },
                            tight: true,
                        },
                        [block(BlockKind::ListItem, [paragraph([text("child")])])],
                    ),
                    paragraph([]),
                    block(
                        BlockKind::List {
                            kind: ListKind::Ordered { start: 1 },
                            tight: true,
                        },
                        [block(BlockKind::ListItem, [paragraph([text("third")])])],
                    ),
                ],
            )],
        )])
        .at(&[0, 0, 2], 0),
    )],
)]
#[case::enter_at_numbered_item_end_adds_next_item(
    // ```markdown
    // 1. first
    // 2. third$$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Ordered { start: 1 },
            tight: true,
        },
        [
            block(BlockKind::ListItem, [paragraph([text("first")])]),
            block(BlockKind::ListItem, [paragraph([text("third")])]),
        ],
    )])
    .at(&[0, 1, 0], 5),
    [(
        Command::Enter,
        // ```markdown
        // 1. first
        // 2. third
        // 3. $$
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Ordered { start: 1 },
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([text("first")])]),
                block(BlockKind::ListItem, [paragraph([text("third")])]),
                block(BlockKind::ListItem, [paragraph([])]),
            ],
        )])
        .at(&[0, 2, 0], 0),
    )],
)]
#[case::enter_on_empty_top_level_numbered_item_exits_list(
    // ```markdown
    // 1. first
    // 2. third
    // 3. $$
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Ordered { start: 1 },
            tight: true,
        },
        [
            block(BlockKind::ListItem, [paragraph([text("first")])]),
            block(BlockKind::ListItem, [paragraph([text("third")])]),
            block(BlockKind::ListItem, [paragraph([])]),
        ],
    )])
    .at(&[0, 2, 0], 0),
    [(
        Command::Enter,
        // ```markdown
        // 1. first
        // 2. third
        //
        // $$
        // ```
        Scenario::new([
            block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 1 },
                    tight: true,
                },
                [
                    block(BlockKind::ListItem, [paragraph([text("first")])]),
                    block(BlockKind::ListItem, [paragraph([text("third")])]),
                ],
            ),
            paragraph([]),
        ])
        .at(&[1], 0),
    )],
)]
#[case::delete_at_paragraph_end_joins_following_heading_as_plain_text(
    // ```markdown
    // before$$
    //
    // # abcd
    // ```
    Scenario::new([
        paragraph([text("before")]),
        text_block(
            BlockKind::Heading {
                level: HeadingLevel::H1,
            },
            [text("abcd")],
        ),
    ])
    .at(&[0], 6),
    [(
        Command::Delete,
        // ```markdown
        // before$$abcd
        // ```
        Scenario::new([paragraph([text("beforeabcd")])]).at(&[0], 6),
    )],
)]
#[case::delete_at_paragraph_end_joins_following_quote_as_plain_text(
    // ```markdown
    // before$$
    //
    // > abcd
    // ```
    Scenario::new([
        paragraph([text("before")]),
        block(BlockKind::BlockQuote, [paragraph([text("abcd")])]),
    ])
    .at(&[0], 6),
    [(
        Command::Delete,
        // ```markdown
        // before$$abcd
        // ```
        Scenario::new([paragraph([text("beforeabcd")])]).at(&[0], 6),
    )],
)]
#[case::delete_at_paragraph_end_joins_first_bullet_and_keeps_rest_of_list(
    // ```markdown
    // before$$
    //
    // - abcd
    // - next
    // ```
    Scenario::new([
        paragraph([text("before")]),
        block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([text("abcd")])]),
                block(BlockKind::ListItem, [paragraph([text("next")])]),
            ],
        ),
    ])
    .at(&[0], 6),
    [(
        Command::Delete,
        // ```markdown
        // before$$abcd
        //
        // - next
        // ```
        Scenario::new([
            paragraph([text("beforeabcd")]),
            block(
                BlockKind::List {
                    kind: ListKind::Bullet,
                    tight: true,
                },
                [block(BlockKind::ListItem, [paragraph([text("next")])])],
            ),
        ])
        .at(&[0], 6),
    )],
)]
#[case::enter_replaces_heading_selection_with_split(
    // ```markdown
    // # ab[|kill|]xy
    // ```
    Scenario::new([text_block(
        BlockKind::Heading {
            level: HeadingLevel::H1,
        },
        [text("abkillxy")],
    )])
    .select(&[0], 2..6),
    [(
        Command::Enter,
        // ```markdown
        // # ab
        //
        // $$xy
        // ```
        Scenario::new([
            text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H1,
                },
                [text("ab")],
            ),
            paragraph([text("xy")]),
        ])
        .at(&[1], 0),
    )],
)]
#[case::enter_replaces_bullet_item_selection_with_split(
    // ```markdown
    // - cd[|mark|]uv
    // ```
    Scenario::new([block(
        BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        },
        [block(BlockKind::ListItem, [paragraph([text("cdmarkuv")])])],
    )])
    .select(&[0, 0, 0], 2..6),
    [(
        Command::Enter,
        // ```markdown
        // - cd
        // - $$uv
        // ```
        Scenario::new([block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [
                block(BlockKind::ListItem, [paragraph([text("cd")])]),
                block(BlockKind::ListItem, [paragraph([text("uv")])]),
            ],
        )])
        .at(&[0, 1, 0], 0),
    )],
)]
#[case::enter_at_quote_start_inserts_paragraph_and_keeps_caret_in_quote(
    // ```markdown
    // > $$anchor
    // ```
    Scenario::new([block(BlockKind::BlockQuote, [paragraph([text("anchor")])])]).at(&[0, 0], 0),
    [(
        Command::Enter,
        // The first block is an empty paragraph.
        // ```markdown
        //
        // > $$anchor
        // ```
        Scenario::new([
            paragraph([]),
            block(BlockKind::BlockQuote, [paragraph([text("anchor")])]),
        ])
        .at(&[1, 0], 0),
    )],
)]
#[case::enter_on_empty_quote_changes_it_to_paragraph(
    // ```markdown
    // > $$
    // ```
    Scenario::new([block(BlockKind::BlockQuote, [paragraph([])])]).at(&[0, 0], 0),
    [(
        Command::Enter,
        // ```markdown
        // $$
        // ```
        Scenario::new([paragraph([])]),
    )],
)]
#[case::backspace_at_start_of_nonempty_code_stays_in_code(
    // ````markdown
    // before
    //
    // ```
    // $$program
    // ```
    // ````
    Scenario::new([
        paragraph([text("before")]),
        block(
            BlockKind::CodeBlock {
                language: None,
                code: "program".into(),
            },
            [],
        ),
    ])
    .at(&[1], 0),
    [(
        Command::Backspace,
        // ````markdown
        // before
        //
        // ```
        // $$program
        // ```
        // ````
        Scenario::new([
            paragraph([text("before")]),
            block(
                BlockKind::CodeBlock {
                    language: None,
                    code: "program".into(),
                },
                [],
            ),
        ])
        .at(&[1], 0),
    )],
)]
#[case::emphasis_delimiter_inside_code_stays_literal(
    // ````markdown
    // ```
    // **bold*$$
    // ```
    // ````
    Scenario::new([block(
        BlockKind::CodeBlock {
            language: None,
            code: "**bold*".into(),
        },
        [],
    )])
    .at(&[0], 7),
    [(
        Command::InsertText("*".into()),
        // ````markdown
        // ```
        // **bold**$$
        // ```
        // ````
        Scenario::new([block(
            BlockKind::CodeBlock {
                language: None,
                code: "**bold**".into(),
            },
            [],
        )])
        .at(&[0], 8)
        .with_caret_affinity(Affinity::Upstream),
    )],
)]
#[case::hard_break_inside_heading_one(
    // ```markdown
    // alpha$$beta
    // ===
    // ```
    Scenario::new([text_block(
        BlockKind::Heading {
            level: HeadingLevel::H1,
        },
        [text("alphabeta")],
    )])
    .at(&[0], 5),
    [(
        Command::InsertHardBreak,
        // ```markdown
        // alpha\
        // $$beta
        // ===
        // ```
        Scenario::new([text_block(
            BlockKind::Heading {
                level: HeadingLevel::H1,
            },
            [
                text("alpha"),
                inline(InlineKind::HardBreak, []),
                text("beta"),
            ],
        )])
        .at(&[0], 6),
    )],
)]
#[case::hard_break_inside_heading_two(
    // ```markdown
    // alpha$$beta
    // ---
    // ```
    Scenario::new([text_block(
        BlockKind::Heading {
            level: HeadingLevel::H2,
        },
        [text("alphabeta")],
    )])
    .at(&[0], 5),
    [(
        Command::InsertHardBreak,
        // ```markdown
        // alpha\
        // $$beta
        // ---
        // ```
        Scenario::new([text_block(
            BlockKind::Heading {
                level: HeadingLevel::H2,
            },
            [
                text("alpha"),
                inline(InlineKind::HardBreak, []),
                text("beta"),
            ],
        )])
        .at(&[0], 6),
    )],
)]
#[case::hard_break_exits_code_at_start(
    Scenario::new([rust_code("abc")]).at(&[0], 0),
    [(
        Command::InsertHardBreak,
        Scenario::new([rust_code(""), paragraph([text("abc")])]).at(&[1], 0),
    )],
)]
#[case::hard_break_exits_code_at_end(
    Scenario::new([rust_code("abc")]).at(&[0], 3),
    [(
        Command::InsertHardBreak,
        Scenario::new([rust_code("abc"), paragraph([])]).at(&[1], 0),
    )],
)]
#[case::hard_break_exits_code_and_types_before_suffix(
    Scenario::new([rust_code("€😀b")]).at(&[0], 1),
    [
        (
            Command::InsertHardBreak,
            Scenario::new([rust_code("€"), paragraph([text("😀b")])]).at(&[1], 0),
        ),
        (
            Command::InsertText("X".into()),
            Scenario::new([rust_code("€"), paragraph([text("X😀b")])])
                .at(&[1], 1)
                .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::hard_break_exits_selected_code_inside_quote(
    Scenario::new([block(BlockKind::BlockQuote, [rust_code("abcde")])]).select(&[0, 0], 1..3),
    [
        (
            Command::InsertHardBreak,
            Scenario::new([block(
                BlockKind::BlockQuote,
                [rust_code("a"), paragraph([text("de")])],
            )])
            .at(&[0, 1], 0),
        ),
        (
            Command::Undo,
            Scenario::new([block(BlockKind::BlockQuote, [rust_code("abcde")])])
                .select(&[0, 0], 1..3),
        ),
        (
            Command::Redo,
            Scenario::new([block(
                BlockKind::BlockQuote,
                [rust_code("a"), paragraph([text("de")])],
            )])
            .at(&[0, 1], 0),
        ),
    ],
)]
#[case::hard_break_exits_code_preserves_line_breaks_in_suffix(
    Scenario::new([rust_code("ab\ncd")]).at(&[0], 1),
    [(
        Command::InsertHardBreak,
        Scenario::new([
            rust_code("a"),
            paragraph([text("b"), inline(InlineKind::HardBreak, []), text("cd")]),
        ])
        .at(&[1], 0),
    )],
)]
#[case::url_links_selected_label_then_types_after_link(
    Scenario::new([paragraph([text("labelme")])]).select(&[0], 0..7),
    [
        (
            Command::InsertText(WEB_URL.into()),
            Scenario::new([paragraph([web_link([text("labelme")])])]).at(&[0], 7),
        ),
        (
            Command::InsertText("!".into()),
            Scenario::new([paragraph([web_link([text("labelme")]), text("!")])])
                .at(&[0], 8)
                .with_caret_affinity(Affinity::Upstream),
        ),
    ],
)]
#[case::url_links_selected_formatted_text_and_preserves_neighbors(
    Scenario::new([
        paragraph([text("A😀"), strong([text("BC")]), text("D")]),
        paragraph([text("end")]),
    ])
    .select(&[0], 1..3),
    [
        (
            Command::InsertText(WEB_URL.into()),
            Scenario::new([
                paragraph([
                    text("A"),
                    web_link([text("😀"), strong([text("B")])]),
                    strong([text("C")]),
                    text("D"),
                ]),
                paragraph([text("end")]),
            ])
            .at(&[0], 3),
        ),
        (
            Command::Undo,
            Scenario::new([
                paragraph([text("A😀"), strong([text("BC")]), text("D")]),
                paragraph([text("end")]),
            ])
            .select(&[0], 1..3),
        ),
        (
            Command::Redo,
            Scenario::new([
                paragraph([
                    text("A"),
                    web_link([text("😀"), strong([text("B")])]),
                    strong([text("C")]),
                    text("D"),
                ]),
                paragraph([text("end")]),
            ])
            .at(&[0], 3),
        ),
    ],
)]
#[case::url_at_caret_is_text(
    Scenario::new([paragraph([])]).at(&[0], 0),
    [(
        Command::InsertText(WEB_URL.into()),
        Scenario::new([paragraph([text(WEB_URL)])])
            .at(&[0], WEB_URL.len())
            .with_caret_affinity(Affinity::Upstream),
    )],
)]
#[case::url_in_a_sentence_replaces_label_as_text(
    Scenario::new([paragraph([text("labelme")])]).select(&[0], 0..7),
    [(
        Command::InsertText("https://example.com/ path".into()),
        Scenario::new([paragraph([text("https://example.com/ path")])])
            .at(&[0], 25)
            .with_caret_affinity(Affinity::Upstream),
    )],
)]
#[case::url_replaces_selected_code_as_text(
    Scenario::new([rust_code("labelme")]).select(&[0], 0..7),
    [(
        Command::InsertText(WEB_URL.into()),
        Scenario::new([rust_code(WEB_URL)])
            .at(&[0], WEB_URL.len())
            .with_caret_affinity(Affinity::Upstream),
    )],
)]
fn scenario(#[case] input: Scenario, #[case] steps: impl IntoIterator<Item = (Command, Scenario)>) {
    let mut editor = EditorEngine::new(input.document, input.selection).unwrap();

    for (index, (command, expected)) in steps.into_iter().enumerate() {
        let step = index + 1;

        let result = editor.execute(command, editor.selection(), None);

        result.unwrap_or_else(|error| panic!("step {step}: {error}"));
        let actual = Scenario {
            document: editor.document(),
            selection: editor.selection(),
        };
        assert!(
            actual == expected,
            "editor state after step {step}\n{}",
            actual.diff(&expected)
        );
    }
}
