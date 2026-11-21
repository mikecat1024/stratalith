// Stable libtest discovers individual #[test] functions at compile time.
use std::{env, fmt::Write, fs, path::PathBuf};

const INLINE_PARTS: &[(&str, &str)] = &[
    ("word", "word"),
    ("cjk", "\u{4e00}"),
    ("space", " "),
    ("punctuation", ". "),
    ("asterisk_emphasis", "*a*"),
    ("underscore_emphasis", "_b_"),
    ("asterisk_strong", "**c**"),
    ("underscore_strong", "__d__"),
    ("code", "`e`"),
    ("link", "[f](url)"),
    ("image", "![g](img)"),
    ("entity", "&amp;"),
    ("html", "<i>"),
    ("escape", "\\*"),
    ("nested_emphasis", "***a***"),
    ("newline", "\n"),
    ("blank_line", "\n\n"),
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let mut tests = String::from("mod adjacent_inlines {\n");
    // Both neighbors can affect how Markdown parses a fragment. For example,
    // rewriting word*a*word as word_a_word would lose the emphasis. Each ordered
    // triple checks that serializing and reparsing preserves the original parse tree.
    for &(first_name, first) in INLINE_PARTS {
        for &(middle_name, middle) in INLINE_PARTS {
            for &(last_name, last) in INLINE_PARTS {
                writeln!(
                    tests,
                    "#[test] fn {first_name}_{middle_name}_{last_name}() {{
                    super::check_inline_boundaries(concat!({first:?}, {middle:?}, {last:?}));
                }}"
                )
                .unwrap();
            }
        }
    }
    tests.push_str("}\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::write(output.join("markdown_cases.rs"), tests).expect("write Markdown test registrations");
}
