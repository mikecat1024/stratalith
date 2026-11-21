//! Recognition shared by raw inline HTML and type-7 HTML blocks.

fn skip_space(s: &str) -> &str {
    s.trim_start_matches([' ', '\t', '\n', '\r', '\x0c'])
}

fn tag_name(s: &str) -> Option<(&str, &str)> {
    if !s.as_bytes().first()?.is_ascii_alphabetic() {
        return None;
    }
    let n = s
        .bytes()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == b'-')
        .count();
    Some((&s[..n], &s[n..]))
}

fn tag(input: &str) -> Option<usize> {
    let body = input.strip_prefix('<')?;
    if let Some(closing) = body.strip_prefix('/') {
        let (_, rest) = tag_name(closing)?;
        let tail = skip_space(rest).strip_prefix('>')?;
        return Some(input.len() - tail.len());
    }
    let (_, mut rest) = tag_name(body)?;
    loop {
        let next = skip_space(rest);
        if let Some(tail) = next.strip_prefix('>').or_else(|| next.strip_prefix("/>")) {
            return Some(input.len() - tail.len());
        }
        if next.len() == rest.len() {
            return None;
        }
        rest = after_attribute(next)?;
    }
}

fn after_attribute(input: &str) -> Option<&str> {
    let first = *input.as_bytes().first()?;
    if !(first.is_ascii_alphabetic() || matches!(first, b'_' | b':')) {
        return None;
    }
    let name_len = input
        .bytes()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b':' | b'.' | b'-'))
        .count();
    let rest = &input[name_len..];
    let Some(value) = skip_space(rest).strip_prefix('=') else {
        return Some(rest);
    };
    let value = skip_space(value);
    if let Some(quote @ ('\'' | '"')) = value.chars().next() {
        let (_, tail) = value[1..].split_once(quote)?;
        return Some(tail);
    }
    let end = value
        .find([' ', '\t', '\n', '\r', '\x0c', '"', '\'', '=', '<', '>', '`'])
        .unwrap_or(value.len());
    (end > 0).then_some(&value[end..])
}

pub(super) fn inline(s: &str) -> Option<usize> {
    if let Some(body) = s.strip_prefix("<!--") {
        for close in [">", "->"] {
            if body.starts_with(close) {
                return Some(4 + close.len());
            }
        }
        return body.find("-->").map(|i| 4 + i + 3);
    }
    for (open, close) in [("<?", "?>"), ("<![CDATA[", "]]>")] {
        if let Some(body) = s.strip_prefix(open) {
            return body.find(close).map(|i| open.len() + i + close.len());
        }
    }
    if s.strip_prefix("<!")
        .is_some_and(|s| s.starts_with(|c: char| c.is_ascii_alphabetic()))
    {
        return s.find('>').map(|i| i + 1);
    }
    tag(s)
}

pub(super) enum End {
    Blank,
    Literal(&'static str),
    RawTag,
}
impl End {
    pub fn closes(&self, line: &str) -> bool {
        match self {
            Self::Blank => false,
            Self::Literal(end) => line.contains(end),
            Self::RawTag => {
                let lower = line.to_ascii_lowercase();
                ["</pre>", "</script>", "</style>", "</textarea>"]
                    .iter()
                    .any(|s| lower.contains(s))
            }
        }
    }
}

pub(super) fn block(line: &str) -> Option<(End, bool)> {
    for (open, close) in [("<!--", "-->"), ("<?", "?>"), ("<![CDATA[", "]]>")] {
        if line.starts_with(open) {
            return Some((End::Literal(close), true));
        }
    }
    if line
        .strip_prefix("<!")
        .is_some_and(|s| s.starts_with(|c: char| c.is_ascii_alphabetic()))
    {
        return Some((End::Literal(">"), true));
    }
    let body = line.strip_prefix('<')?;
    let (name, rest) = tag_name(body.strip_prefix('/').unwrap_or(body))?;
    let name = name.to_ascii_lowercase();
    let boundary = rest.is_empty() || rest.starts_with([' ', '\t', '>']);
    if !body.starts_with('/')
        && boundary
        && matches!(name.as_str(), "pre" | "script" | "style" | "textarea")
    {
        return Some((End::RawTag, true));
    }
    const BLOCK_TAGS: &[&str] = &[
        "address",
        "article",
        "aside",
        "base",
        "basefont",
        "blockquote",
        "body",
        "caption",
        "center",
        "col",
        "colgroup",
        "dd",
        "details",
        "dialog",
        "dir",
        "div",
        "dl",
        "dt",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "frame",
        "frameset",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "head",
        "header",
        "hr",
        "html",
        "iframe",
        "legend",
        "li",
        "link",
        "main",
        "menu",
        "menuitem",
        "nav",
        "noframes",
        "ol",
        "optgroup",
        "option",
        "p",
        "param",
        "search",
        "section",
        "summary",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "title",
        "tr",
        "track",
        "ul",
    ];
    if (boundary || rest.starts_with("/>")) && BLOCK_TAGS.contains(&name.as_str()) {
        return Some((End::Blank, true));
    }
    tag(line)
        .filter(|&n| skip_space(&line[n..]).is_empty())
        .map(|_| (End::Blank, false))
}

pub(super) fn autolink(input: &str) -> Option<(&str, String, usize)> {
    let body = input.strip_prefix('<')?;
    let end = body.find(['>', '<', ' ', '\t', '\n'])?;
    if body.as_bytes()[end] != b'>' {
        return None;
    }
    let label = &body[..end];
    if let Some((scheme, _)) = label.split_once(':')
        && (2..=32).contains(&scheme.len())
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'))
        && !label.chars().any(|c| c.is_ascii_control())
    {
        return Some((label, label.into(), end + 2));
    }
    let (user, domain) = label.split_once('@')?;
    if user.is_empty()
        || !user
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+/=?^_`{|}~-".contains(&b))
    {
        return None;
    }
    if !domain.split('.').all(|s| {
        !s.is_empty()
            && s.len() <= 63
            && !s.starts_with('-')
            && !s.ends_with('-')
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }) {
        return None;
    }
    Some((label, format!("mailto:{label}"), end + 2))
}
