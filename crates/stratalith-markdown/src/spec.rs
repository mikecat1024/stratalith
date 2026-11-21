use super::*;
use stratalith_core::{Document, NodeKind};

include!(concat!(env!("OUT_DIR"), "/markdown_cases.rs"));

fn tree(doc: &Document) -> Vec<(&NodeKind, usize)> {
    doc.nodes()
        .map(|n| (n.kind(), n.children().len()))
        .collect()
}

#[stratalith_macros::rstest]
#[case::entity_whitespace_in_emphasis("*&nbsp;word&nbsp;*")]
#[case::line_endings_in_link("[label](<a&#10;b> \"title&#13;\")")]
#[case::entities_in_fence_info("~~~&#32;rust&#10;example&#32;\ncode\n~~~")]
#[case::empty_document("")]
#[case::empty_heading("##")]
#[case::heading_with_cjk_and_crlf("###### 日本語 ###\r\n\r\n---")]
#[case::nested_emphasis("__Hello *world*__ and ***both*** and **_nested_**")]
#[case::soft_and_hard_breaks("before\nsecond  \nthird\\\nfourth")]
#[case::escapes_and_literal_markers(r"\# word \*literal\* \[unclosed and foo_bar_baz")]
#[case::image_and_link("![*alt*](<a b> \"caption\") and [read](path_(v1))")]
#[case::literal_fenced_code("~~~rust ignore\n\n```\n**literal**\n~~~")]
#[case::nested_lists_in_quote("> ++\n>\n> 12) a\n>     - b\n> 13) c")]
#[case::loose_list_and_paragraph("- first\n\n  second paragraph\n\n- next\n\nend")]
#[case::adjacent_lists("* first\n+ separate\n1. ordered\n2. next")]
#[case::code_spans_with_backticks_and_spaces("`` `a` `` `   ` end")]
#[case::unclosed_link_and_emphasis("text [incomplete]( x **open")]
#[case::list_with_link_rule_and_sublist("3. [`]`](page)\n   - ***\n   - nested")]
#[case::loose_nested_list("- one\n  - a\n\n  - b\n- two")]
#[case::list_with_blank_line_in_fenced_code("- ~~~\n  a\n\n  b\n  ~~~\n- next")]
#[case::quote_with_loose_list_and_separate_quote("> - first\n>\n>   second\n\n> separate")]
fn parsed_documents_survive_canonical_serialization(#[case] source: &str) {
    let doc = parse(source).unwrap();
    let expected = tree(&doc);

    let serialized = serialize(&doc).unwrap();
    let reparsed = parse(&serialized).unwrap();

    assert_eq!(tree(&reparsed), expected, "{source:?} -> {serialized:?}");
}

fn check_inline_boundaries(source: &str) {
    let original = parse(source).unwrap();
    let expected = tree(&original);

    let output = serialize(&original).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    let reparsed = parse(&output).unwrap();

    assert_eq!(tree(&reparsed), expected, "{source:?} -> {output:?}");
}
