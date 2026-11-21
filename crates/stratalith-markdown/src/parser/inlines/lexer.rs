use crate::escape;
use crate::parser::{
    html,
    links::{self, LinkTarget},
};

use std::collections::HashMap;

// Successful searches consume their entire span. After the first unsuccessful
// search, the last occurrence of each remaining width is known.
#[derive(Default)]
struct Backticks {
    last: HashMap<usize, usize>,
    saw_end: bool,
}

impl Backticks {
    fn closing(&mut self, source: &str, start: usize, width: usize) -> Option<usize> {
        if self.saw_end && self.last.get(&width).is_none_or(|&last| last <= start) {
            return None;
        }
        let mut offset = start + width;
        while let Some(skip) = source[offset..].find('`') {
            let next = offset + skip;
            let length = source.as_bytes()[next..]
                .iter()
                .take_while(|&&b| b == b'`')
                .count();
            if length == width {
                return Some(next);
            }
            if !self.saw_end {
                self.last.insert(length, next);
            }
            offset = next + length;
        }
        self.saw_end = true;
        None
    }
}

pub(super) struct Run {
    pub mark: char,
    pub width: usize,
    pub opens: bool,
    pub closes: bool,
}

impl Run {
    fn at(source: &str, start: usize, mark: char) -> Self {
        let rest = &source[start..];
        let width = rest.len() - rest.trim_start_matches(mark).len();
        let before = source[..start].chars().next_back();
        let after = rest[width..].chars().next();
        let left = flanking(after, before);
        let right = flanking(before, after);

        Self {
            mark,
            width,
            opens: left && (mark == '*' || !right || before.is_some_and(escape::punctuation)),
            closes: right && (mark == '*' || !left || after.is_some_and(escape::punctuation)),
        }
    }
}

// The right-flanking rule is the left-flanking rule read in reverse.
fn flanking(inner: Option<char>, outer: Option<char>) -> bool {
    inner.is_some_and(|c| {
        !c.is_whitespace()
            && (!escape::punctuation(c)
                || outer.is_none_or(|o| o.is_whitespace() || escape::punctuation(o)))
    })
}

#[derive(PartialEq, Eq)]
pub(super) enum LabelKind {
    Link,
    Image,
}

pub(super) struct LabelOpening {
    pub kind: LabelKind,
    start: usize,
}

// Only the token for ']' can consume a link target. Its borrow keeps the
// cursor at the closing bracket until the parser accepts or ignores the suffix.
pub(super) struct LabelClosing<'source, 'cursor> {
    prefix: &'source str,
    suffix: &'source str,
    offset: &'cursor mut usize,
}

impl LabelClosing<'_, '_> {
    pub(super) fn resolve(
        self,
        opening: &LabelOpening,
        refs: &links::References,
    ) -> Option<LinkTarget> {
        let label = self.prefix.get(opening.start..)?;
        let (target, consumed) =
            links::inline(self.suffix).or_else(|| refs.resolve(label, self.suffix))?;
        *self.offset += consumed;
        Some(target)
    }
}

pub(super) enum Token<'a, 'cursor> {
    Text(&'a str),
    Entity(String),
    Html(&'a str),
    Auto(&'a str, String),
    Code(String),
    Break { hard: bool },
    Run(Run),
    OpenLabel(LabelOpening),
    CloseLabel(LabelClosing<'a, 'cursor>),
}

pub(super) struct Lexer<'a> {
    source: &'a str,
    offset: usize,
    backticks: Backticks,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            offset: 0,
            backticks: Backticks::default(),
        }
    }

    pub(super) fn next_token(&mut self) -> Option<Token<'a, '_>> {
        let rest = &self.source[self.offset..];
        let c = rest.chars().next()?;
        if c == ']' {
            let prefix = &self.source[..self.offset];
            self.offset += 1;
            return Some(Token::CloseLabel(LabelClosing {
                prefix,
                suffix: &rest[1..],
                offset: &mut self.offset,
            }));
        }
        let (width, token) = match c {
            '&' => match escape::entity(rest) {
                Some((s, n)) => (n, Token::Entity(s)),
                None => (1, Token::Text("&")),
            },
            '<' => {
                if let Some((label, url, n)) = html::autolink(rest) {
                    (n, Token::Auto(label, url))
                } else if let Some(n) = html::inline(rest) {
                    (n, Token::Html(&rest[..n]))
                } else {
                    (1, Token::Text("<"))
                }
            }
            '\\' if rest.starts_with("\\\n") => (2, Token::Break { hard: true }),
            '\\' if rest[1..].starts_with(|c: char| c.is_ascii_punctuation()) => {
                (2, Token::Text(&rest[1..2]))
            }
            '\n' => (1, Token::Break { hard: false }),
            '`' => {
                let width = rest.len() - rest.trim_start_matches('`').len();
                match self.backticks.closing(self.source, self.offset, width) {
                    Some(end) => {
                        let literal = self.source[self.offset + width..end].replace('\n', " ");
                        let padded = literal.starts_with(' ')
                            && literal.ends_with(' ')
                            && literal.contains(|c| c != ' ');
                        let code = if padded {
                            literal[1..literal.len() - 1].to_owned()
                        } else {
                            literal
                        };
                        (end - self.offset + width, Token::Code(code))
                    }
                    None => (width, Token::Text(&rest[..width])),
                }
            }
            '!' if rest.starts_with("![") => (
                2,
                Token::OpenLabel(LabelOpening {
                    kind: LabelKind::Image,
                    start: self.offset + 2,
                }),
            ),
            '[' => (
                1,
                Token::OpenLabel(LabelOpening {
                    kind: LabelKind::Link,
                    start: self.offset + 1,
                }),
            ),
            '*' | '_' => {
                let run = Run::at(self.source, self.offset, c);
                (run.width, Token::Run(run))
            }
            _ => text_run(rest),
        };
        self.offset += width;
        Some(token)
    }
}

// Spaces stay in the surrounding text token unless they introduce a line break.
fn text_run<'source, 'cursor>(rest: &'source str) -> (usize, Token<'source, 'cursor>) {
    let end = rest.as_bytes()[1..]
        .iter()
        .position(|b| {
            matches!(
                b,
                b'<' | b'&' | b'\n' | b'\\' | b'`' | b'!' | b'[' | b']' | b'*' | b'_'
            )
        })
        .map_or(rest.len(), |i| i + 1);
    let text = &rest[..end];
    if rest.as_bytes().get(end) == Some(&b'\n') {
        let body = text.trim_end_matches(' ');
        if body.is_empty() {
            return (end + 1, Token::Break { hard: end >= 2 });
        }
        return (body.len(), Token::Text(body));
    }
    (end, Token::Text(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmatched_opener_leaves_later_matching_widths_available() {
        let source = "``` x `one` and ``two``";
        let mut lexer = Lexer::new(source);
        let expected_spans = ["one", "two"];

        let unmatched_is_literal = matches!(lexer.next_token(), Some(Token::Text("```")));
        let mut spans = Vec::new();
        while let Some(token) = lexer.next_token() {
            let Token::Code(code) = token else {
                continue;
            };
            spans.push(code);
        }

        assert!(unmatched_is_literal);
        assert_eq!(spans, expected_spans);
    }
}
