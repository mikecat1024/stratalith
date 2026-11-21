// The opening newline and shared leading spaces belong to the Rust fixture.
// Keep blank lines, relative Markdown indentation, and trailing hard-break spaces.
pub(super) fn dedent(input: &str) -> String {
    let Some(input) = input.strip_prefix('\n') else {
        return input.to_owned();
    };
    let lines = input.split('\n');
    let indent = lines
        .clone()
        .filter(|line| !line.trim_matches(' ').is_empty())
        .map(|line| line.len() - line.trim_start_matches(' ').len())
        .min()
        .unwrap_or(0);
    lines
        .map(|line| {
            if line.trim_matches(' ').is_empty() {
                ""
            } else {
                &line[indent..]
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::dedent;
    use stratalith_macros::rstest;

    #[rstest]
    #[case("\n    - 親\n      - 子\n    ", "- 親\n  - 子\n")]
    #[case("\n    one  \n    \n    two", "one  \n\ntwo")]
    #[case("    indented code\n", "    indented code\n")]
    fn dedent_preserves_markdown_whitespace(#[case] input: &str, #[case] expected: &str) {
        let actual = dedent(input);

        assert_eq!(actual, expected);
    }
}
