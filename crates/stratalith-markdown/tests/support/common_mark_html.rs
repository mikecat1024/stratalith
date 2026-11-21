use stratalith_core::{
    BlockKind as B, Document, HeadingLevel, InlineKind as I, ListKind, NodeId, NodeKind,
};

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn uri(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~:/?#@!$&'()*+,;=%".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    escape(&out)
}

pub(super) fn render(doc: &Document, ids: &[NodeId], tight: bool, alt: bool) -> String {
    ids.iter()
        .map(|id| {
            let node = doc.node(*id).unwrap();
            let children = |tight, alt| render(doc, node.children(), tight, alt);
            match node.kind() {
                NodeKind::Block(kind) => match kind {
                    B::Paragraph if tight => children(false, false),
                    B::Paragraph => format!("<p>{}</p>\n", children(false, false)),
                    B::Heading { level } => {
                        let n = match level {
                            HeadingLevel::H1 => 1,
                            HeadingLevel::H2 => 2,
                            HeadingLevel::H3 => 3,
                            HeadingLevel::H4 => 4,
                            HeadingLevel::H5 => 5,
                            HeadingLevel::H6 => 6,
                        };
                        format!("<h{n}>{}</h{n}>\n", children(false, false))
                    }
                    B::BlockQuote => {
                        format!("<blockquote>\n{}</blockquote>\n", children(false, false))
                    }
                    B::ThematicBreak => "<hr />\n".into(),
                    B::CodeBlock { language, code } => {
                        let class = language
                            .as_deref()
                            .and_then(|s| s.split_whitespace().next())
                            .map(|s| format!(" class=\"language-{}\"", escape(s)))
                            .unwrap_or_default();
                        format!("<pre><code{class}>{}</code></pre>\n", escape(code))
                    }
                    B::Html(s) => s.clone(),
                    B::List { kind, tight } => {
                        let (tag, attr) = match kind {
                            ListKind::Bullet => ("ul", String::new()),
                            ListKind::Ordered { start: 1 } => ("ol", String::new()),
                            ListKind::Ordered { start } => ("ol", format!(" start=\"{start}\"")),
                        };
                        format!("<{tag}{attr}>\n{}</{tag}>\n", children(*tight, false))
                    }
                    B::ListItem => {
                        let mut value = String::from("<li>");
                        let mut prev_inline = false;
                        for &child in node.children() {
                            let inline = tight
                                && matches!(
                                    doc.node(child).unwrap().kind(),
                                    NodeKind::Block(B::Paragraph)
                                );
                            if !inline && (prev_inline || value == "<li>") {
                                value.push('\n');
                            }
                            value.push_str(&render(doc, &[child], tight, false));
                            prev_inline = inline;
                        }
                        value.push_str("</li>\n");
                        value
                    }
                },
                NodeKind::Inline(kind) => match kind {
                    I::Text(s) => escape(s),
                    I::SoftBreak => "\n".into(),
                    I::HardBreak if alt => "\n".into(),
                    I::HardBreak => "<br />\n".into(),
                    I::Code(s) if alt => escape(s),
                    I::Code(s) => format!("<code>{}</code>", escape(s)),
                    I::Html(s) if alt => escape(s),
                    I::Html(s) => s.clone(),
                    _ if alt => children(false, true),
                    I::Strong => format!("<strong>{}</strong>", children(false, false)),
                    I::Emphasis => format!("<em>{}</em>", children(false, false)),
                    I::Link { url, title } | I::Image { url, title } => {
                        let title = title
                            .as_ref()
                            .filter(|s| !s.is_empty())
                            .map(|s| format!(" title=\"{}\"", escape(s)))
                            .unwrap_or_default();
                        let url = uri(url);
                        if matches!(kind, I::Image { .. }) {
                            format!(
                                "<img src=\"{url}\" alt=\"{}\"{title} />",
                                children(false, true)
                            )
                        } else {
                            format!("<a href=\"{url}\"{title}>{}</a>", children(false, false))
                        }
                    }
                },
            }
        })
        .collect()
}
