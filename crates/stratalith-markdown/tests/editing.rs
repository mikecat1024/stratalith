#[path = "support/dedent.rs"]
mod dedent;
#[path = "support/marked_markdown.rs"]
mod marked_markdown;

use dedent::dedent;
use stratalith_core::{Command, Document, EditorEngine, Selection};
use stratalith_macros::stratest;
use stratalith_markdown::{parse, serialize};

#[stratest]
#[case::restore_list(
    "
        - **one**
        - two
    ",
    "
        - **one**
        - two
    "
)]
#[case::restore_empty("", "")]
#[case::restore_literal_dollars(
    "`$$`",
    "
        `$$`
    "
)]
fn restore(#[case] input: &str, #[case] expected: &str) {
    let document = parse(&dedent(input)).unwrap();

    let output = serialize(&document).unwrap();

    assert_eq!(output, dedent(expected));
}

// In editing cases, $$ is the caret, [| is the anchor, and |] is the focus.
// Expected output markers also check the selection after each command.
#[stratest]
#[case::insert_in_quoted_heading(
    "> # €😀$$t",
    vec![(
        Command::InsertText("s".into()),
        "
            > # €😀st
        "
    )]
)]
#[case::delete_backward_range(
    "
        a|]bc

        de[|f
    ",
    vec![(
        Command::Delete,
        "
            af
        "
    )]
)]
#[case::delete_forward_range(
    "
        a[|bc

        de|]f
    ",
    vec![(
        Command::Delete,
        "
            af
        "
    )]
)]
#[case::insert_at_end_of_bold(
    "**one$$** two",
    vec![(
        Command::InsertText("s".into()),
        "
            **ones** two
        "
    )]
)]
#[case::insert_at_next_of_bold(
    "**one**$$ two",
    vec![(
        Command::InsertText("s".into()),
        "
            **one**s two
        "
    )]
)]
#[case::insert_between_literal_stars(
    "`one*$$*`",
    vec![(
        Command::InsertText("s".into()),
        "
            `one*s*`
        "
    )]
)]
#[case::hard_break_replaces_backward_range(
    "€|]t[|s",
    vec![(
        Command::InsertHardBreak,
        r"
            €\
            s
        "
    )]
)]
#[case::hard_break_inside_emphasis(
    "*€$$s*",
    vec![(
        Command::InsertHardBreak,
        r"
            *€\
            s*
        "
    )]
)]
#[case::enter_in_code_block(
    "
        ```
        €$$s
        ```",
    vec![(
        Command::Enter,
        "
            ```
            €
            s
            ```
        "
    )]
)]
#[case::url_links_selected_label(
    "before [|labelme|] after",
    vec![(
        Command::InsertText("https://example.com/".into()),
        "before [labelme](<https://example.com/>) after\n",
    )]
)]
#[case::indent_second_item(
    "- A\n- B|]😀[|",
    vec![
        (Command::IndentListItem, "- A\n  - B|]😀[|\n"),
        (Command::Undo, "- A\n- B|]😀[|\n"),
        (Command::Redo, "- A\n  - B|]😀[|\n"),
    ]
)]
#[case::indent_extends_nested_list(
    "
        - A
          - B
        - C$$",
    vec![(
        Command::IndentListItem,
        "
            - A
              - B
              - C
        "
    )]
)]
#[case::outdent_only_child(
    "
        - A
          - B$$",
    vec![(
        Command::OutdentListItem,
        "
            - A
            - B
        "
    )]
)]
#[case::outdent_middle_child(
    "
        - A
          - B
          - C$$
          - D
        - E",
    vec![(
        Command::OutdentListItem,
        "
            - A
              - B
            - C
              - D
            - E
        "
    )]
)]
#[case::backspace_empty_nested_list_item_keeps_following_item(
    "
        - Hello
          - $$
          - Next
    ",
    vec![(
        Command::Backspace,
        "
            - Hello$$
              - Next
        "
    )]
)]
#[case::indent_first_item(
    "- $$A",
    vec![(Command::IndentListItem, "- A\n")]
)]
#[case::outdent_top_level(
    "- $$A",
    vec![(Command::OutdentListItem, "- A\n")]
)]
fn editing(#[case] input: &str, #[case] steps: Vec<(Command, &str)>) {
    let (source, markers) = marked_markdown::extract_markers(&dedent(input));
    let doc = markers.editable_document(&source);
    let selection = markers.resolve(&source, &doc);
    let mut editor = EditorEngine::new(doc, selection).unwrap();

    for (index, (command, expected)) in steps.into_iter().enumerate() {
        let before = editor.selection();
        let expected = dedent(expected);

        editor.execute(command, before, None).unwrap();

        assert_editor_state(&editor, &expected, index + 1);
    }
}

fn assert_editor_state(editor: &EditorEngine, expected: &str, step: usize) {
    let (expected, markers) = marked_markdown::optional_markers(expected);
    assert_eq!(
        serialize(editor.document()).unwrap(),
        expected,
        "Markdown after step {step}"
    );
    if let Some(markers) = markers {
        let expected_doc = markers.editable_document(&expected);
        let expected_selection = markers.resolve(&expected, &expected_doc);
        // Node IDs are local to each document. Compare endpoints in document order.
        let endpoints = |doc: &Document, selection: Selection| {
            [selection.anchor, selection.focus].map(|pos| {
                (
                    doc.nodes().position(|node| node.id() == pos.block).unwrap(),
                    pos.offset,
                )
            })
        };
        assert_eq!(
            endpoints(editor.document(), editor.selection()),
            endpoints(&expected_doc, expected_selection),
            "selection after step {step}"
        );
    }
}
