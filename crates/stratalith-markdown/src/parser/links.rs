use crate::escape;
use std::collections::HashMap;

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub(super) struct LinkTarget {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Default)]
pub(super) struct References {
    by_label: HashMap<String, LinkTarget>,
}

impl References {
    // Definitions at the start of a paragraph disappear from its visible text.
    // When labels repeat, the first definition in document order wins.
    pub(super) fn collect_prefix<'a>(&mut self, mut text: &'a str) -> &'a str {
        while let Some((label, target, rest)) = definition(text) {
            self.by_label.entry(label).or_insert(target);
            text = rest;
        }
        text
    }

    pub(super) fn resolve(&self, text: &str, suffix: &str) -> Option<(LinkTarget, usize)> {
        if self.by_label.is_empty() {
            return None;
        }
        let (label, consumed) = match ReferenceLabel::bracketed(suffix) {
            Some((ReferenceLabel(""), tail)) => {
                (ReferenceLabel::whole(text)?, suffix.len() - tail.len())
            }
            Some((label, tail)) => (label, suffix.len() - tail.len()),
            None => (ReferenceLabel::whole(text)?, 0),
        };
        self.by_label
            .get(&label.normalized())
            .cloned()
            .map(|target| (target, consumed))
    }
}

struct ReferenceLabel<'a>(&'a str);

impl<'a> ReferenceLabel<'a> {
    fn bracketed(source: &'a str) -> Option<(Self, &'a str)> {
        let (label, tail) = Self::scan(source.strip_prefix('[')?)?;
        Some((label, tail.strip_prefix(']')?))
    }

    fn whole(text: &'a str) -> Option<Self> {
        let (label, tail) = Self::scan(text)?;
        tail.is_empty().then_some(label)
    }

    // A label has at most 999 source characters, including backslashes.
    fn scan(body: &'a str) -> Option<(Self, &'a str)> {
        let mut escaped = false;
        for (length, (offset, c)) in body.char_indices().enumerate() {
            if !escaped && matches!(c, '[' | ']') {
                return Some((Self(&body[..offset]), &body[offset..]));
            }
            if length >= 999 {
                return None;
            }
            escaped = !escaped && c == '\\';
        }
        Some((Self(body), ""))
    }

    fn normalized(&self) -> String {
        let mut out = String::new();
        for word in self.0.split_whitespace() {
            if !out.is_empty() {
                out.push(' ')
            }
            for c in word.chars() {
                escape::fold(c, &mut out);
            }
        }
        out
    }
}

fn whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n')
}

fn unescaped_end(input: &str, stop: impl Fn(char) -> bool) -> usize {
    let mut chars = input.char_indices();
    while let Some((i, c)) = chars.next() {
        if stop(c) {
            return i;
        }
        if c == '\\'
            && chars
                .clone()
                .next()
                .is_some_and(|(_, c)| c.is_ascii_punctuation())
        {
            chars.next();
        }
    }
    input.len()
}

fn destination(s: &str) -> Option<(String, &str)> {
    if let Some(body) = s.strip_prefix('<') {
        let n = unescaped_end(body, |c| matches!(c, '<' | '>' | '\n'));
        return Some((escape::decode(&body[..n]), body[n..].strip_prefix('>')?));
    }
    let mut depth = 0usize;
    let mut chars = s.char_indices();
    let mut n = s.len();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' if chars
                .clone()
                .next()
                .is_some_and(|(_, c)| c.is_ascii_punctuation()) =>
            {
                chars.next();
            }
            '(' => depth += 1,
            ')' if depth != 0 => depth -= 1,
            c if c == ')' || c.is_ascii_control() || c == ' ' => {
                n = i;
                break;
            }
            _ => {}
        }
    }
    (depth == 0).then(|| (escape::decode(&s[..n]), &s[n..]))
}

fn title(s: &str) -> Option<(String, &str)> {
    let open = s.chars().next()?;
    let close = match open {
        '\'' | '"' => open,
        '(' => ')',
        _ => return None,
    };
    let body = &s[1..];
    let n = unescaped_end(body, |c| c == close || (open == '(' && c == '('));
    Some((escape::decode(&body[..n]), body[n..].strip_prefix(close)?))
}

pub(super) fn inline(s: &str) -> Option<(LinkTarget, usize)> {
    let body = s.strip_prefix('(')?.trim_start_matches(whitespace);
    let (url, rest) = destination(body)?;
    let trimmed = rest.trim_start_matches(whitespace);
    let (title, tail) = if trimmed.len() != rest.len() && !trimmed.starts_with(')') {
        let (title, tail) = title(trimmed)?;
        (Some(title), tail.trim_start_matches(whitespace))
    } else {
        (None, trimmed)
    };
    let tail = tail.strip_prefix(')')?;
    Some((LinkTarget { url, title }, s.len() - tail.len()))
}

fn definition(s: &str) -> Option<(String, LinkTarget, &str)> {
    let (label, tail) = ReferenceLabel::bracketed(s)?;
    let before = tail.strip_prefix(':')?.trim_start_matches(whitespace);
    let name = label.normalized();
    if name.is_empty() {
        return None;
    }
    let (url, rest) = destination(before)?;
    if before.len() == rest.len() {
        return None;
    }
    let trimmed = rest.trim_start_matches(whitespace);
    if rest.len() != trimmed.len()
        && let Some((title, tail)) = title(trimmed)
        && let Some(tail) = end_line(tail)
    {
        return Some((
            name,
            LinkTarget {
                url,
                title: Some(title),
            },
            tail,
        ));
    }
    Some((name, LinkTarget { url, title: None }, end_line(rest)?))
}

fn end_line(s: &str) -> Option<&str> {
    let s = s.trim_start_matches([' ', '\t']);
    if s.is_empty() {
        Some(s)
    } else {
        s.strip_prefix('\n')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[stratalith_macros::rstest]
    #[case::at_limit(997, true)]
    #[case::over_limit(998, false)]
    fn bracketed_label_length_counts_unicode_scalars_and_both_escape_characters(
        #[case] prefix_length: usize,
        #[case] expected: bool,
    ) {
        let source = format!(r"[{}\]]", "€".repeat(prefix_length));

        let label = ReferenceLabel::bracketed(&source);

        assert_eq!(label.is_some(), expected);
    }

    #[stratalith_macros::rstest]
    #[case::at_limit(997, true)]
    #[case::over_limit(998, false)]
    fn shortcut_label_length_counts_unicode_scalars_and_both_escape_characters(
        #[case] prefix_length: usize,
        #[case] expected: bool,
    ) {
        let source = format!(r"{}\]", "€".repeat(prefix_length));

        let label = ReferenceLabel::whole(&source);

        assert_eq!(label.is_some(), expected);
    }
}
