mod blocks;
mod html;
mod inlines;
mod links;
mod syntax;

use crate::MarkdownError;
use stratalith_core::Document;

/// Allocates fresh document-local node IDs. Deeply nested input returns
/// [`MarkdownError::NestingLimit`] to bound parser stack usage.
pub fn parse(markdown: &str) -> Result<Document, MarkdownError> {
    let source = normalize_line_endings_and_nul(markdown);
    blocks::parse(&source)?.into_document()
}

fn normalize_line_endings_and_nul(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains(['\r', '\0']) {
        source
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\0', "�")
            .into()
    } else {
        source.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stratalith_core::builder::{
        block, document, emphasis, inline, paragraph, strong, text, text_block,
    };
    use stratalith_core::{BlockKind, HeadingLevel, InlineKind, ListKind, NodeKind};

    fn tree(doc: &Document) -> Vec<(&NodeKind, usize)> {
        doc.nodes()
            .map(|n| (n.kind(), n.children().len()))
            .collect()
    }

    #[test]
    fn parses_nested_block_and_inline_kinds() {
        let source = "## 見出し\n\n3. [**赤 *青***](book \"本\")\n   - `白`\n4. 次";
        let link = inline(
            InlineKind::Link {
                url: "book".into(),
                title: Some("本".into()),
            },
            [strong([text("赤 "), emphasis([text("青")])])],
        );
        let nested_list = block(
            BlockKind::List {
                kind: ListKind::Bullet,
                tight: true,
            },
            [block(
                BlockKind::ListItem,
                [paragraph([inline(InlineKind::Code("白".into()), [])])],
            )],
        );
        let expected_doc = document([
            text_block(
                BlockKind::Heading {
                    level: HeadingLevel::H2,
                },
                [text("見出し")],
            ),
            block(
                BlockKind::List {
                    kind: ListKind::Ordered { start: 3 },
                    tight: true,
                },
                [
                    block(BlockKind::ListItem, [paragraph([link]), nested_list]),
                    block(BlockKind::ListItem, [paragraph([text("次")])]),
                ],
            ),
        ])
        .unwrap();
        let expected = tree(&expected_doc);

        let parsed = parse(source).unwrap();

        assert_eq!(tree(&parsed), expected);
    }

    #[test]
    fn blank_line_in_fenced_code_keeps_its_list_tight() {
        let source = "- ~~~\n  a\n\n  b\n  ~~~";
        let expected = NodeKind::Block(BlockKind::List {
            kind: ListKind::Bullet,
            tight: true,
        });

        let doc = parse(source).unwrap();

        assert_eq!(doc.node(doc.blocks()[0]).unwrap().kind(), &expected);
    }

    #[test]
    fn deeply_nested_quotes_return_a_parse_error() {
        let source = format!("{}word", "> ".repeat(129));

        let result = parse(&source);

        assert!(matches!(result, Err(MarkdownError::NestingLimit)));
    }

    fn images_inside_emphasis(count: usize) -> String {
        format!("*{}x{}*", "![".repeat(count), "](p)".repeat(count))
    }

    #[test]
    fn serializes_emphasis_and_images_at_the_shared_nesting_limit() {
        let source = images_inside_emphasis(127);
        let doc = parse(&source).unwrap();

        let result = crate::serialize(&doc);

        assert!(result.is_ok());
    }

    #[test]
    fn rejects_emphasis_and_images_over_the_shared_nesting_limit() {
        let source = images_inside_emphasis(128);

        let result = parse(&source);

        assert!(matches!(result, Err(MarkdownError::NestingLimit)));
    }
}
