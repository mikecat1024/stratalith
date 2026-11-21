mod emphasis;
mod lexer;

use super::{
    links::{LinkTarget, References},
    syntax::{Inline, InlineSequence},
};
use crate::{MarkdownError, error::check_depth};
use emphasis::Piece;
use lexer::{LabelClosing, LabelKind, LabelOpening, Lexer, Run, Token};

enum PendingLabel {
    Open {
        opener: LabelOpening,
        piece_index: usize,
    },
    // Still pairs with ']', but an inner link has claimed its own target.
    InactiveLink,
}

pub(super) fn parse(
    source: &str,
    refs: &References,
    depth: usize,
) -> Result<Vec<Inline>, MarkdownError> {
    let mut parser = InlineParser::default();
    let mut lexer = Lexer::new(source);
    while let Some(token) = lexer.next_token() {
        parser.consume(token, refs, depth)?;
    }
    Ok(emphasis::resolve(parser.pieces, depth)?.into_nodes())
}

#[derive(Default)]
struct InlineParser {
    pieces: Vec<Piece>,
    brackets: Vec<PendingLabel>,
}

impl InlineParser {
    fn consume(
        &mut self,
        token: Token<'_, '_>,
        refs: &References,
        depth: usize,
    ) -> Result<(), MarkdownError> {
        let node = match token {
            Token::Text(s) => Inline::Text(s.to_owned()),
            Token::Entity(s) => Inline::Text(s),
            Token::Html(s) => Inline::Html(s.into()),
            Token::Code(s) => Inline::Code(s),
            Token::Auto(label, url) => Inline::Link(
                LinkTarget { url, title: None },
                InlineSequence::from_iter([Inline::Text(label.into())]),
            ),
            Token::Break { hard: true } => Inline::HardBreak,
            Token::Break { hard: false } => Inline::SoftBreak,
            Token::Run(run) => {
                self.pieces.push(Piece::Run(run));
                return Ok(());
            }
            Token::OpenLabel(opener) => {
                check_depth(depth + self.brackets.len() + 1)?;
                let literal = match opener.kind {
                    LabelKind::Link => "[",
                    LabelKind::Image => "![",
                };
                self.brackets.push(PendingLabel::Open {
                    opener,
                    piece_index: self.pieces.len(),
                });
                Inline::Text(literal.into())
            }
            Token::CloseLabel(closing) => self.close_label(closing, refs, depth)?,
        };
        if matches!(node, Inline::Link(..)) {
            // An image starts a new scope in which links are legal.
            for pending in self.brackets.iter_mut().rev() {
                match pending {
                    PendingLabel::Open { opener, .. } if opener.kind == LabelKind::Link => {
                        *pending = PendingLabel::InactiveLink;
                    }
                    // Earlier links in this scope were disabled together.
                    PendingLabel::InactiveLink | PendingLabel::Open { .. } => break,
                }
            }
        }
        self.pieces.push(Piece::Node(node));
        Ok(())
    }

    fn close_label(
        &mut self,
        closing: LabelClosing<'_, '_>,
        refs: &References,
        depth: usize,
    ) -> Result<Inline, MarkdownError> {
        let Some(PendingLabel::Open {
            opener,
            piece_index,
        }) = self.brackets.pop()
        else {
            return Ok(Inline::Text("]".into()));
        };
        let Some(target) = closing.resolve(&opener, refs) else {
            return Ok(Inline::Text("]".into()));
        };
        let label = emphasis::resolve(
            self.pieces.drain(piece_index + 1..),
            depth + self.brackets.len() + 1,
        )?;
        self.pieces.pop(); // Replace the literal opener together with its label.
        Ok(match opener.kind {
            LabelKind::Image => Inline::Image(target, label),
            LabelKind::Link => Inline::Link(target, label),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partly_closed_emphasis(width: usize) -> String {
        // Two of the three asterisks close around 'a'. One remains open.
        format!("***a** {}b", "_".repeat(width))
    }

    #[test]
    fn partly_closed_emphasis_allows_remaining_markers_at_the_nesting_limit() {
        let source = partly_closed_emphasis(127);
        let refs = References::default();

        let result = parse(&source, &refs, 0);

        assert!(result.is_ok());
    }

    #[test]
    fn partly_closed_emphasis_rejects_remaining_markers_over_the_nesting_limit() {
        let source = partly_closed_emphasis(128);
        let refs = References::default();

        let result = parse(&source, &refs, 0);

        assert!(matches!(result, Err(MarkdownError::NestingLimit)));
    }

    #[test]
    fn code_span_in_link_label_keeps_delimiters_literal() {
        let source = "[**a `]*`**](page)";
        let refs = References::default();
        let expected = [Inline::Link(
            LinkTarget {
                url: "page".into(),
                title: None,
            },
            InlineSequence::from_iter([Inline::Strong(InlineSequence::from_iter([
                Inline::Text("a ".into()),
                Inline::Code("]*".into()),
            ]))]),
        )];

        let nodes = parse(source, &refs, 0).unwrap();

        assert_eq!(nodes, expected);
    }

    #[test]
    fn inner_link_closes_before_outer_literal_brackets() {
        let source = "[outer [inner](b)](a)";
        let refs = References::default();
        let expected = [
            Inline::Text("[outer ".into()),
            Inline::Link(
                LinkTarget {
                    url: "b".into(),
                    title: None,
                },
                InlineSequence::from_iter([Inline::Text("inner".into())]),
            ),
            Inline::Text("](a)".into()),
        ];

        let nodes = parse(source, &refs, 0).unwrap();

        assert_eq!(nodes, expected);
    }
}
