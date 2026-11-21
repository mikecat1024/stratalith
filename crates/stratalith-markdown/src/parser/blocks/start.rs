use super::line::Line;
use crate::{heading::LEVELS, parser::html};
use stratalith_core::{HeadingLevel, ListKind};

fn heading(line: &str) -> Option<(HeadingLevel, &str)> {
    let body = line.trim_start_matches('#');
    let level = *LEVELS.get((line.len() - body.len()).checked_sub(1)?)?;
    if !body.is_empty() && !body.starts_with([' ', '\t']) {
        return None;
    }
    let content = body.trim_end_matches([' ', '\t']);
    let no_hashes = content.trim_end_matches('#');
    let content = if no_hashes.ends_with([' ', '\t']) {
        no_hashes
    } else {
        content
    };
    Some((level, content.trim_matches([' ', '\t'])))
}

pub(super) struct Fence {
    marker: char,
    width: usize,
}

impl Fence {
    fn opening(line: &str) -> Option<(Self, &str)> {
        let marker = line.chars().next()?;
        if !matches!(marker, '`' | '~') {
            return None;
        }
        let info = line.trim_start_matches(marker);
        let width = line.len() - info.len();
        if width < 3 || (marker == '`' && info.contains('`')) {
            return None;
        }
        Some((Self { marker, width }, info.trim_matches([' ', '\t'])))
    }

    pub(super) fn closes(&self, line: Line<'_>) -> bool {
        let markers = line.content().trim_end_matches([' ', '\t']);
        line.indent() <= 3
            && markers.len() >= self.width
            && markers.chars().all(|c| c == self.marker)
    }
}

fn rule(line: &str) -> bool {
    let Some(marker @ ('-' | '*' | '_')) = line.chars().next() else {
        return false;
    };
    line.chars().all(|c| c == marker || matches!(c, ' ' | '\t'))
        && line.matches(marker).count() >= 3
}

pub(super) struct ListMarker<'a> {
    pub(super) kind: ListKind,
    pub(super) symbol: u8,
    pub(super) indent: usize,
    pub(super) body: Line<'a>,
}

fn marker(line: Line<'_>) -> Option<ListMarker<'_>> {
    let s = line.content();
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    let symbol = *s.as_bytes().get(digits)?;
    let kind = match (digits, symbol) {
        (0, b'+' | b'-' | b'*') => ListKind::Bullet,
        (1..=9, b'.' | b')') => ListKind::Ordered {
            start: s[..digits].parse().ok()?,
        },
        _ => return None,
    };
    let prefix = line.indent() + digits + 1;
    let after = line.skip(prefix);
    let spaces = after.indent();
    if spaces == 0 && !after.blank() {
        return None;
    }
    let padding = if !after.blank() && (1..=4).contains(&spaces) {
        spaces
    } else {
        1
    };
    Some(ListMarker {
        kind,
        symbol,
        indent: prefix + padding,
        body: after.skip(padding.min(spaces)),
    })
}

pub(super) enum BlockStart<'a> {
    Blank,
    Indented,
    Plain,
    Heading(HeadingLevel, &'a str),
    FencedCode { fence: Fence, info: &'a str },
    Rule,
    Quote(Line<'a>),
    Item(ListMarker<'a>),
    Html(html::End, bool),
}

impl<'a> BlockStart<'a> {
    pub(super) fn read(line: Line<'a>) -> Self {
        let s = line.content();
        if s.is_empty() {
            Self::Blank
        } else if line.indent() >= 4 {
            Self::Indented
        } else if let Some((level, text)) = heading(s) {
            Self::Heading(level, text)
        } else if let Some((fence, info)) = Fence::opening(s) {
            Self::FencedCode { fence, info }
        } else if rule(s) {
            Self::Rule
        } else if s.starts_with('>') {
            let body = line.skip(line.indent() + 1);
            Self::Quote(body.skip(usize::from(body.indent() > 0)))
        } else if let Some(item) = marker(line) {
            Self::Item(item)
        } else if let Some((end, interrupt)) = html::block(s) {
            Self::Html(end, interrupt)
        } else {
            Self::Plain
        }
    }

    pub(super) fn interrupts_paragraph(&self) -> bool {
        match self {
            Self::Plain | Self::Indented => false,
            Self::Html(_, interrupt) => *interrupt,
            Self::Item(item) => {
                !item.body.blank()
                    && matches!(item.kind, ListKind::Bullet | ListKind::Ordered { start: 1 })
            }
            _ => true,
        }
    }
}

pub(super) fn setext_level(line: Line<'_>) -> Option<HeadingLevel> {
    if line.indent() > 3 {
        return None;
    }
    let s = line.content().trim_end_matches([' ', '\t']);
    match s.as_bytes().first()? {
        b'=' if s.bytes().all(|c| c == b'=') => Some(HeadingLevel::H1),
        b'-' if s.bytes().all(|c| c == b'-') => Some(HeadingLevel::H2),
        _ => None,
    }
}
