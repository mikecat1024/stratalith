//! Examples from CommonMark 0.31.2 by John MacFarlane and contributors.
//! Copyright (C) 2014-16 John MacFarlane. Adapted from the
//! [official JSON distribution](https://spec.commonmark.org/0.31.2/spec.json)
//! under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).

#[path = "support/common_mark_html.rs"]
mod html;

use stratalith_core::{Document, NodeKind};
use stratalith_markdown::{parse, serialize};

fn tree(doc: &Document) -> Vec<(&NodeKind, usize)> {
    doc.nodes()
        .map(|node| (node.kind(), node.children().len()))
        .collect()
}

macro_rules! common_mark_examples {
    ($($name:ident($source:literal, $html:literal)),+ $(,)?) => {
        #[stratalith_macros::rstest]
        $(#[case::$name($source, $html)])+
        fn parse_html(#[case] source: &str, #[case] expected: &str) {
            let doc = parse(source).unwrap();

            let actual = html::render(&doc, doc.blocks(), false, false);

            assert_eq!(actual, expected, "{source:?}");
        }

        #[stratalith_macros::rstest]
        $(#[case::$name($source)])+
        fn round_trip(#[case] source: &str) {
            let doc = parse(source).unwrap();
            let expected = tree(&doc);

            let markdown = serialize(&doc).unwrap();
            let reparsed = parse(&markdown).unwrap();

            assert_eq!(tree(&reparsed), expected, "{source:?} -> {markdown:?}");
        }
    };
}

common_mark_examples! {
    // Tabs
    tabs_001(
        "\tfoo\tbaz\t\tbim\n",
        "<pre><code>foo\tbaz\t\tbim\n</code></pre>\n"
    ),
    tabs_002(
        "  \tfoo\tbaz\t\tbim\n",
        "<pre><code>foo\tbaz\t\tbim\n</code></pre>\n"
    ),
    tabs_003(
        "    a\ta\n    ὐ\ta\n",
        "<pre><code>a\ta\nὐ\ta\n</code></pre>\n"
    ),
    tabs_004(
        "  - foo\n\n\tbar\n",
        "<ul>\n<li>\n<p>foo</p>\n<p>bar</p>\n</li>\n</ul>\n"
    ),
    tabs_005(
        "- foo\n\n\t\tbar\n",
        "<ul>\n<li>\n<p>foo</p>\n<pre><code>  bar\n</code></pre>\n</li>\n</ul>\n"
    ),
    tabs_006(
        ">\t\tfoo\n",
        "<blockquote>\n<pre><code>  foo\n</code></pre>\n</blockquote>\n"
    ),
    tabs_007(
        "-\t\tfoo\n",
        "<ul>\n<li>\n<pre><code>  foo\n</code></pre>\n</li>\n</ul>\n"
    ),
    tabs_008(
        "    foo\n\tbar\n",
        "<pre><code>foo\nbar\n</code></pre>\n"
    ),
    tabs_009(
        " - foo\n   - bar\n\t - baz\n",
        "<ul>\n<li>foo\n<ul>\n<li>bar\n<ul>\n<li>baz</li>\n</ul>\n</li>\n</ul>\n</li>\n</ul>\n"
    ),
    tabs_010(
        "#\tFoo\n",
        "<h1>Foo</h1>\n"
    ),
    tabs_011(
        "*\t*\t*\t\n",
        "<hr />\n"
    ),

    // Backslash escapes
    backslash_escapes_012(
        "\\!\\\"\\#\\$\\%\\&\\'\\(\\)\\*\\+\\,\\-\\.\\/\\:\\;\\<\\=\\>\\?\\@\\[\\\\\\]\\^\\_\\`\\{\\|\\}\\~\n",
        "<p>!&quot;#$%&amp;'()*+,-./:;&lt;=&gt;?@[\\]^_`{|}~</p>\n"
    ),
    backslash_escapes_013(
        "\\\t\\A\\a\\ \\3\\φ\\«\n",
        "<p>\\\t\\A\\a\\ \\3\\φ\\«</p>\n"
    ),
    backslash_escapes_014(
        "\\*not emphasized*\n\\<br/> not a tag\n\\[not a link](/foo)\n\\`not code`\n1\\. not a list\n\\* not a list\n\\# not a heading\n\\[foo]: /url \"not a reference\"\n\\&ouml; not a character entity\n",
        "<p>*not emphasized*\n&lt;br/&gt; not a tag\n[not a link](/foo)\n`not code`\n1. not a list\n* not a list\n# not a heading\n[foo]: /url &quot;not a reference&quot;\n&amp;ouml; not a character entity</p>\n"
    ),
    backslash_escapes_015(
        "\\\\*emphasis*\n",
        "<p>\\<em>emphasis</em></p>\n"
    ),
    backslash_escapes_016(
        "foo\\\nbar\n",
        "<p>foo<br />\nbar</p>\n"
    ),
    backslash_escapes_017(
        "`` \\[\\` ``\n",
        "<p><code>\\[\\`</code></p>\n"
    ),
    backslash_escapes_018(
        "    \\[\\]\n",
        "<pre><code>\\[\\]\n</code></pre>\n"
    ),
    backslash_escapes_019(
        "~~~\n\\[\\]\n~~~\n",
        "<pre><code>\\[\\]\n</code></pre>\n"
    ),
    backslash_escapes_020(
        "<https://example.com?find=\\*>\n",
        "<p><a href=\"https://example.com?find=%5C*\">https://example.com?find=\\*</a></p>\n"
    ),
    backslash_escapes_021(
        "<a href=\"/bar\\/)\">\n",
        "<a href=\"/bar\\/)\">\n"
    ),
    backslash_escapes_022(
        "[foo](/bar\\* \"ti\\*tle\")\n",
        "<p><a href=\"/bar*\" title=\"ti*tle\">foo</a></p>\n"
    ),
    backslash_escapes_023(
        "[foo]\n\n[foo]: /bar\\* \"ti\\*tle\"\n",
        "<p><a href=\"/bar*\" title=\"ti*tle\">foo</a></p>\n"
    ),
    backslash_escapes_024(
        "``` foo\\+bar\nfoo\n```\n",
        "<pre><code class=\"language-foo+bar\">foo\n</code></pre>\n"
    ),

    // Entity and numeric character references
    entity_and_numeric_character_references_025(
        "&nbsp; &amp; &copy; &AElig; &Dcaron;\n&frac34; &HilbertSpace; &DifferentialD;\n&ClockwiseContourIntegral; &ngE;\n",
        "<p>  &amp; © Æ Ď\n¾ ℋ ⅆ\n∲ ≧̸</p>\n"
    ),
    entity_and_numeric_character_references_026(
        "&#35; &#1234; &#992; &#0;\n",
        "<p># Ӓ Ϡ �</p>\n"
    ),
    entity_and_numeric_character_references_027(
        "&#X22; &#XD06; &#xcab;\n",
        "<p>&quot; ആ ಫ</p>\n"
    ),
    entity_and_numeric_character_references_028(
        "&nbsp &x; &#; &#x;\n&#87654321;\n&#abcdef0;\n&ThisIsNotDefined; &hi?;\n",
        "<p>&amp;nbsp &amp;x; &amp;#; &amp;#x;\n&amp;#87654321;\n&amp;#abcdef0;\n&amp;ThisIsNotDefined; &amp;hi?;</p>\n"
    ),
    entity_and_numeric_character_references_029(
        "&copy\n",
        "<p>&amp;copy</p>\n"
    ),
    entity_and_numeric_character_references_030(
        "&MadeUpEntity;\n",
        "<p>&amp;MadeUpEntity;</p>\n"
    ),
    entity_and_numeric_character_references_031(
        "<a href=\"&ouml;&ouml;.html\">\n",
        "<a href=\"&ouml;&ouml;.html\">\n"
    ),
    entity_and_numeric_character_references_032(
        "[foo](/f&ouml;&ouml; \"f&ouml;&ouml;\")\n",
        "<p><a href=\"/f%C3%B6%C3%B6\" title=\"föö\">foo</a></p>\n"
    ),
    entity_and_numeric_character_references_033(
        "[foo]\n\n[foo]: /f&ouml;&ouml; \"f&ouml;&ouml;\"\n",
        "<p><a href=\"/f%C3%B6%C3%B6\" title=\"föö\">foo</a></p>\n"
    ),
    entity_and_numeric_character_references_034(
        "``` f&ouml;&ouml;\nfoo\n```\n",
        "<pre><code class=\"language-föö\">foo\n</code></pre>\n"
    ),
    entity_and_numeric_character_references_035(
        "`f&ouml;&ouml;`\n",
        "<p><code>f&amp;ouml;&amp;ouml;</code></p>\n"
    ),
    entity_and_numeric_character_references_036(
        "    f&ouml;f&ouml;\n",
        "<pre><code>f&amp;ouml;f&amp;ouml;\n</code></pre>\n"
    ),
    entity_and_numeric_character_references_037(
        "&#42;foo&#42;\n*foo*\n",
        "<p>*foo*\n<em>foo</em></p>\n"
    ),
    entity_and_numeric_character_references_038(
        "&#42; foo\n\n* foo\n",
        "<p>* foo</p>\n<ul>\n<li>foo</li>\n</ul>\n"
    ),
    entity_and_numeric_character_references_039(
        "foo&#10;&#10;bar\n",
        "<p>foo\n\nbar</p>\n"
    ),
    entity_and_numeric_character_references_040(
        "&#9;foo\n",
        "<p>\tfoo</p>\n"
    ),
    entity_and_numeric_character_references_041(
        "[a](url &quot;tit&quot;)\n",
        "<p>[a](url &quot;tit&quot;)</p>\n"
    ),

    // Precedence
    precedence_042(
        "- `one\n- two`\n",
        "<ul>\n<li>`one</li>\n<li>two`</li>\n</ul>\n"
    ),

    // Thematic breaks
    thematic_breaks_043(
        "***\n---\n___\n",
        "<hr />\n<hr />\n<hr />\n"
    ),
    thematic_breaks_044(
        "+++\n",
        "<p>+++</p>\n"
    ),
    thematic_breaks_045(
        "===\n",
        "<p>===</p>\n"
    ),
    thematic_breaks_046(
        "--\n**\n__\n",
        "<p>--\n**\n__</p>\n"
    ),
    thematic_breaks_047(
        " ***\n  ***\n   ***\n",
        "<hr />\n<hr />\n<hr />\n"
    ),
    thematic_breaks_048(
        "    ***\n",
        "<pre><code>***\n</code></pre>\n"
    ),
    thematic_breaks_049(
        "Foo\n    ***\n",
        "<p>Foo\n***</p>\n"
    ),
    thematic_breaks_050(
        "_____________________________________\n",
        "<hr />\n"
    ),
    thematic_breaks_051(
        " - - -\n",
        "<hr />\n"
    ),
    thematic_breaks_052(
        " **  * ** * ** * **\n",
        "<hr />\n"
    ),
    thematic_breaks_053(
        "-     -      -      -\n",
        "<hr />\n"
    ),
    thematic_breaks_054(
        "- - - -    \n",
        "<hr />\n"
    ),
    thematic_breaks_055(
        "_ _ _ _ a\n\na------\n\n---a---\n",
        "<p>_ _ _ _ a</p>\n<p>a------</p>\n<p>---a---</p>\n"
    ),
    thematic_breaks_056(
        " *-*\n",
        "<p><em>-</em></p>\n"
    ),
    thematic_breaks_057(
        "- foo\n***\n- bar\n",
        "<ul>\n<li>foo</li>\n</ul>\n<hr />\n<ul>\n<li>bar</li>\n</ul>\n"
    ),
    thematic_breaks_058(
        "Foo\n***\nbar\n",
        "<p>Foo</p>\n<hr />\n<p>bar</p>\n"
    ),
    thematic_breaks_059(
        "Foo\n---\nbar\n",
        "<h2>Foo</h2>\n<p>bar</p>\n"
    ),
    thematic_breaks_060(
        "* Foo\n* * *\n* Bar\n",
        "<ul>\n<li>Foo</li>\n</ul>\n<hr />\n<ul>\n<li>Bar</li>\n</ul>\n"
    ),
    thematic_breaks_061(
        "- Foo\n- * * *\n",
        "<ul>\n<li>Foo</li>\n<li>\n<hr />\n</li>\n</ul>\n"
    ),

    // ATX headings
    atx_headings_062(
        "# foo\n## foo\n### foo\n#### foo\n##### foo\n###### foo\n",
        "<h1>foo</h1>\n<h2>foo</h2>\n<h3>foo</h3>\n<h4>foo</h4>\n<h5>foo</h5>\n<h6>foo</h6>\n"
    ),
    atx_headings_063(
        "####### foo\n",
        "<p>####### foo</p>\n"
    ),
    atx_headings_064(
        "#5 bolt\n\n#hashtag\n",
        "<p>#5 bolt</p>\n<p>#hashtag</p>\n"
    ),
    atx_headings_065(
        "\\## foo\n",
        "<p>## foo</p>\n"
    ),
    atx_headings_066(
        "# foo *bar* \\*baz\\*\n",
        "<h1>foo <em>bar</em> *baz*</h1>\n"
    ),
    atx_headings_067(
        "#                  foo                     \n",
        "<h1>foo</h1>\n"
    ),
    atx_headings_068(
        " ### foo\n  ## foo\n   # foo\n",
        "<h3>foo</h3>\n<h2>foo</h2>\n<h1>foo</h1>\n"
    ),
    atx_headings_069(
        "    # foo\n",
        "<pre><code># foo\n</code></pre>\n"
    ),
    atx_headings_070(
        "foo\n    # bar\n",
        "<p>foo\n# bar</p>\n"
    ),
    atx_headings_071(
        "## foo ##\n  ###   bar    ###\n",
        "<h2>foo</h2>\n<h3>bar</h3>\n"
    ),
    atx_headings_072(
        "# foo ##################################\n##### foo ##\n",
        "<h1>foo</h1>\n<h5>foo</h5>\n"
    ),
    atx_headings_073(
        "### foo ###     \n",
        "<h3>foo</h3>\n"
    ),
    atx_headings_074(
        "### foo ### b\n",
        "<h3>foo ### b</h3>\n"
    ),
    atx_headings_075(
        "# foo#\n",
        "<h1>foo#</h1>\n"
    ),
    atx_headings_076(
        "### foo \\###\n## foo #\\##\n# foo \\#\n",
        "<h3>foo ###</h3>\n<h2>foo ###</h2>\n<h1>foo #</h1>\n"
    ),
    atx_headings_077(
        "****\n## foo\n****\n",
        "<hr />\n<h2>foo</h2>\n<hr />\n"
    ),
    atx_headings_078(
        "Foo bar\n# baz\nBar foo\n",
        "<p>Foo bar</p>\n<h1>baz</h1>\n<p>Bar foo</p>\n"
    ),
    atx_headings_079(
        "## \n#\n### ###\n",
        "<h2></h2>\n<h1></h1>\n<h3></h3>\n"
    ),

    // Setext headings
    setext_headings_080(
        "Foo *bar*\n=========\n\nFoo *bar*\n---------\n",
        "<h1>Foo <em>bar</em></h1>\n<h2>Foo <em>bar</em></h2>\n"
    ),
    setext_headings_081(
        "Foo *bar\nbaz*\n====\n",
        "<h1>Foo <em>bar\nbaz</em></h1>\n"
    ),
    setext_headings_082(
        "  Foo *bar\nbaz*\t\n====\n",
        "<h1>Foo <em>bar\nbaz</em></h1>\n"
    ),
    setext_headings_083(
        "Foo\n-------------------------\n\nFoo\n=\n",
        "<h2>Foo</h2>\n<h1>Foo</h1>\n"
    ),
    setext_headings_084(
        "   Foo\n---\n\n  Foo\n-----\n\n  Foo\n  ===\n",
        "<h2>Foo</h2>\n<h2>Foo</h2>\n<h1>Foo</h1>\n"
    ),
    setext_headings_085(
        "    Foo\n    ---\n\n    Foo\n---\n",
        "<pre><code>Foo\n---\n\nFoo\n</code></pre>\n<hr />\n"
    ),
    setext_headings_086(
        "Foo\n   ----      \n",
        "<h2>Foo</h2>\n"
    ),
    setext_headings_087(
        "Foo\n    ---\n",
        "<p>Foo\n---</p>\n"
    ),
    setext_headings_088(
        "Foo\n= =\n\nFoo\n--- -\n",
        "<p>Foo\n= =</p>\n<p>Foo</p>\n<hr />\n"
    ),
    setext_headings_089(
        "Foo  \n-----\n",
        "<h2>Foo</h2>\n"
    ),
    setext_headings_090(
        "Foo\\\n----\n",
        "<h2>Foo\\</h2>\n"
    ),
    setext_headings_091(
        "`Foo\n----\n`\n\n<a title=\"a lot\n---\nof dashes\"/>\n",
        "<h2>`Foo</h2>\n<p>`</p>\n<h2>&lt;a title=&quot;a lot</h2>\n<p>of dashes&quot;/&gt;</p>\n"
    ),
    setext_headings_092(
        "> Foo\n---\n",
        "<blockquote>\n<p>Foo</p>\n</blockquote>\n<hr />\n"
    ),
    setext_headings_093(
        "> foo\nbar\n===\n",
        "<blockquote>\n<p>foo\nbar\n===</p>\n</blockquote>\n"
    ),
    setext_headings_094(
        "- Foo\n---\n",
        "<ul>\n<li>Foo</li>\n</ul>\n<hr />\n"
    ),
    setext_headings_095(
        "Foo\nBar\n---\n",
        "<h2>Foo\nBar</h2>\n"
    ),
    setext_headings_096(
        "---\nFoo\n---\nBar\n---\nBaz\n",
        "<hr />\n<h2>Foo</h2>\n<h2>Bar</h2>\n<p>Baz</p>\n"
    ),
    setext_headings_097(
        "\n====\n",
        "<p>====</p>\n"
    ),
    setext_headings_098(
        "---\n---\n",
        "<hr />\n<hr />\n"
    ),
    setext_headings_099(
        "- foo\n-----\n",
        "<ul>\n<li>foo</li>\n</ul>\n<hr />\n"
    ),
    setext_headings_100(
        "    foo\n---\n",
        "<pre><code>foo\n</code></pre>\n<hr />\n"
    ),
    setext_headings_101(
        "> foo\n-----\n",
        "<blockquote>\n<p>foo</p>\n</blockquote>\n<hr />\n"
    ),
    setext_headings_102(
        "\\> foo\n------\n",
        "<h2>&gt; foo</h2>\n"
    ),
    setext_headings_103(
        "Foo\n\nbar\n---\nbaz\n",
        "<p>Foo</p>\n<h2>bar</h2>\n<p>baz</p>\n"
    ),
    setext_headings_104(
        "Foo\nbar\n\n---\n\nbaz\n",
        "<p>Foo\nbar</p>\n<hr />\n<p>baz</p>\n"
    ),
    setext_headings_105(
        "Foo\nbar\n* * *\nbaz\n",
        "<p>Foo\nbar</p>\n<hr />\n<p>baz</p>\n"
    ),
    setext_headings_106(
        "Foo\nbar\n\\---\nbaz\n",
        "<p>Foo\nbar\n---\nbaz</p>\n"
    ),

    // Indented code blocks
    indented_code_blocks_107(
        "    a simple\n      indented code block\n",
        "<pre><code>a simple\n  indented code block\n</code></pre>\n"
    ),
    indented_code_blocks_108(
        "  - foo\n\n    bar\n",
        "<ul>\n<li>\n<p>foo</p>\n<p>bar</p>\n</li>\n</ul>\n"
    ),
    indented_code_blocks_109(
        "1.  foo\n\n    - bar\n",
        "<ol>\n<li>\n<p>foo</p>\n<ul>\n<li>bar</li>\n</ul>\n</li>\n</ol>\n"
    ),
    indented_code_blocks_110(
        "    <a/>\n    *hi*\n\n    - one\n",
        "<pre><code>&lt;a/&gt;\n*hi*\n\n- one\n</code></pre>\n"
    ),
    indented_code_blocks_111(
        "    chunk1\n\n    chunk2\n  \n \n \n    chunk3\n",
        "<pre><code>chunk1\n\nchunk2\n\n\n\nchunk3\n</code></pre>\n"
    ),
    indented_code_blocks_112(
        "    chunk1\n      \n      chunk2\n",
        "<pre><code>chunk1\n  \n  chunk2\n</code></pre>\n"
    ),
    indented_code_blocks_113(
        "Foo\n    bar\n\n",
        "<p>Foo\nbar</p>\n"
    ),
    indented_code_blocks_114(
        "    foo\nbar\n",
        "<pre><code>foo\n</code></pre>\n<p>bar</p>\n"
    ),
    indented_code_blocks_115(
        "# Heading\n    foo\nHeading\n------\n    foo\n----\n",
        "<h1>Heading</h1>\n<pre><code>foo\n</code></pre>\n<h2>Heading</h2>\n<pre><code>foo\n</code></pre>\n<hr />\n"
    ),
    indented_code_blocks_116(
        "        foo\n    bar\n",
        "<pre><code>    foo\nbar\n</code></pre>\n"
    ),
    indented_code_blocks_117(
        "\n    \n    foo\n    \n\n",
        "<pre><code>foo\n</code></pre>\n"
    ),
    indented_code_blocks_118(
        "    foo  \n",
        "<pre><code>foo  \n</code></pre>\n"
    ),

    // Fenced code blocks
    fenced_code_blocks_119(
        "```\n<\n >\n```\n",
        "<pre><code>&lt;\n &gt;\n</code></pre>\n"
    ),
    fenced_code_blocks_120(
        "~~~\n<\n >\n~~~\n",
        "<pre><code>&lt;\n &gt;\n</code></pre>\n"
    ),
    fenced_code_blocks_121(
        "``\nfoo\n``\n",
        "<p><code>foo</code></p>\n"
    ),
    fenced_code_blocks_122(
        "```\naaa\n~~~\n```\n",
        "<pre><code>aaa\n~~~\n</code></pre>\n"
    ),
    fenced_code_blocks_123(
        "~~~\naaa\n```\n~~~\n",
        "<pre><code>aaa\n```\n</code></pre>\n"
    ),
    fenced_code_blocks_124(
        "````\naaa\n```\n``````\n",
        "<pre><code>aaa\n```\n</code></pre>\n"
    ),
    fenced_code_blocks_125(
        "~~~~\naaa\n~~~\n~~~~\n",
        "<pre><code>aaa\n~~~\n</code></pre>\n"
    ),
    fenced_code_blocks_126(
        "```\n",
        "<pre><code></code></pre>\n"
    ),
    fenced_code_blocks_127(
        "`````\n\n```\naaa\n",
        "<pre><code>\n```\naaa\n</code></pre>\n"
    ),
    fenced_code_blocks_128(
        "> ```\n> aaa\n\nbbb\n",
        "<blockquote>\n<pre><code>aaa\n</code></pre>\n</blockquote>\n<p>bbb</p>\n"
    ),
    fenced_code_blocks_129(
        "```\n\n  \n```\n",
        "<pre><code>\n  \n</code></pre>\n"
    ),
    fenced_code_blocks_130(
        "```\n```\n",
        "<pre><code></code></pre>\n"
    ),
    fenced_code_blocks_131(
        " ```\n aaa\naaa\n```\n",
        "<pre><code>aaa\naaa\n</code></pre>\n"
    ),
    fenced_code_blocks_132(
        "  ```\naaa\n  aaa\naaa\n  ```\n",
        "<pre><code>aaa\naaa\naaa\n</code></pre>\n"
    ),
    fenced_code_blocks_133(
        "   ```\n   aaa\n    aaa\n  aaa\n   ```\n",
        "<pre><code>aaa\n aaa\naaa\n</code></pre>\n"
    ),
    fenced_code_blocks_134(
        "    ```\n    aaa\n    ```\n",
        "<pre><code>```\naaa\n```\n</code></pre>\n"
    ),
    fenced_code_blocks_135(
        "```\naaa\n  ```\n",
        "<pre><code>aaa\n</code></pre>\n"
    ),
    fenced_code_blocks_136(
        "   ```\naaa\n  ```\n",
        "<pre><code>aaa\n</code></pre>\n"
    ),
    fenced_code_blocks_137(
        "```\naaa\n    ```\n",
        "<pre><code>aaa\n    ```\n</code></pre>\n"
    ),
    fenced_code_blocks_138(
        "``` ```\naaa\n",
        "<p><code> </code>\naaa</p>\n"
    ),
    fenced_code_blocks_139(
        "~~~~~~\naaa\n~~~ ~~\n",
        "<pre><code>aaa\n~~~ ~~\n</code></pre>\n"
    ),
    fenced_code_blocks_140(
        "foo\n```\nbar\n```\nbaz\n",
        "<p>foo</p>\n<pre><code>bar\n</code></pre>\n<p>baz</p>\n"
    ),
    fenced_code_blocks_141(
        "foo\n---\n~~~\nbar\n~~~\n# baz\n",
        "<h2>foo</h2>\n<pre><code>bar\n</code></pre>\n<h1>baz</h1>\n"
    ),
    fenced_code_blocks_142(
        "```ruby\ndef foo(x)\n  return 3\nend\n```\n",
        "<pre><code class=\"language-ruby\">def foo(x)\n  return 3\nend\n</code></pre>\n"
    ),
    fenced_code_blocks_143(
        "~~~~    ruby startline=3 $%@#$\ndef foo(x)\n  return 3\nend\n~~~~~~~\n",
        "<pre><code class=\"language-ruby\">def foo(x)\n  return 3\nend\n</code></pre>\n"
    ),
    fenced_code_blocks_144(
        "````;\n````\n",
        "<pre><code class=\"language-;\"></code></pre>\n"
    ),
    fenced_code_blocks_145(
        "``` aa ```\nfoo\n",
        "<p><code>aa</code>\nfoo</p>\n"
    ),
    fenced_code_blocks_146(
        "~~~ aa ``` ~~~\nfoo\n~~~\n",
        "<pre><code class=\"language-aa\">foo\n</code></pre>\n"
    ),
    fenced_code_blocks_147(
        "```\n``` aaa\n```\n",
        "<pre><code>``` aaa\n</code></pre>\n"
    ),

    // HTML blocks
    html_blocks_148(
        "<table><tr><td>\n<pre>\n**Hello**,\n\n_world_.\n</pre>\n</td></tr></table>\n",
        "<table><tr><td>\n<pre>\n**Hello**,\n<p><em>world</em>.\n</pre></p>\n</td></tr></table>\n"
    ),
    html_blocks_149(
        "<table>\n  <tr>\n    <td>\n           hi\n    </td>\n  </tr>\n</table>\n\nokay.\n",
        "<table>\n  <tr>\n    <td>\n           hi\n    </td>\n  </tr>\n</table>\n<p>okay.</p>\n"
    ),
    html_blocks_150(
        " <div>\n  *hello*\n         <foo><a>\n",
        " <div>\n  *hello*\n         <foo><a>\n"
    ),
    html_blocks_151(
        "</div>\n*foo*\n",
        "</div>\n*foo*\n"
    ),
    html_blocks_152(
        "<DIV CLASS=\"foo\">\n\n*Markdown*\n\n</DIV>\n",
        "<DIV CLASS=\"foo\">\n<p><em>Markdown</em></p>\n</DIV>\n"
    ),
    html_blocks_153(
        "<div id=\"foo\"\n  class=\"bar\">\n</div>\n",
        "<div id=\"foo\"\n  class=\"bar\">\n</div>\n"
    ),
    html_blocks_154(
        "<div id=\"foo\" class=\"bar\n  baz\">\n</div>\n",
        "<div id=\"foo\" class=\"bar\n  baz\">\n</div>\n"
    ),
    html_blocks_155(
        "<div>\n*foo*\n\n*bar*\n",
        "<div>\n*foo*\n<p><em>bar</em></p>\n"
    ),
    html_blocks_156(
        "<div id=\"foo\"\n*hi*\n",
        "<div id=\"foo\"\n*hi*\n"
    ),
    html_blocks_157(
        "<div class\nfoo\n",
        "<div class\nfoo\n"
    ),
    html_blocks_158(
        "<div *???-&&&-<---\n*foo*\n",
        "<div *???-&&&-<---\n*foo*\n"
    ),
    html_blocks_159(
        "<div><a href=\"bar\">*foo*</a></div>\n",
        "<div><a href=\"bar\">*foo*</a></div>\n"
    ),
    html_blocks_160(
        "<table><tr><td>\nfoo\n</td></tr></table>\n",
        "<table><tr><td>\nfoo\n</td></tr></table>\n"
    ),
    html_blocks_161(
        "<div></div>\n``` c\nint x = 33;\n```\n",
        "<div></div>\n``` c\nint x = 33;\n```\n"
    ),
    html_blocks_162(
        "<a href=\"foo\">\n*bar*\n</a>\n",
        "<a href=\"foo\">\n*bar*\n</a>\n"
    ),
    html_blocks_163(
        "<Warning>\n*bar*\n</Warning>\n",
        "<Warning>\n*bar*\n</Warning>\n"
    ),
    html_blocks_164(
        "<i class=\"foo\">\n*bar*\n</i>\n",
        "<i class=\"foo\">\n*bar*\n</i>\n"
    ),
    html_blocks_165(
        "</ins>\n*bar*\n",
        "</ins>\n*bar*\n"
    ),
    html_blocks_166(
        "<del>\n*foo*\n</del>\n",
        "<del>\n*foo*\n</del>\n"
    ),
    html_blocks_167(
        "<del>\n\n*foo*\n\n</del>\n",
        "<del>\n<p><em>foo</em></p>\n</del>\n"
    ),
    html_blocks_168(
        "<del>*foo*</del>\n",
        "<p><del><em>foo</em></del></p>\n"
    ),
    html_blocks_169(
        "<pre language=\"haskell\"><code>\nimport Text.HTML.TagSoup\n\nmain :: IO ()\nmain = print $ parseTags tags\n</code></pre>\nokay\n",
        "<pre language=\"haskell\"><code>\nimport Text.HTML.TagSoup\n\nmain :: IO ()\nmain = print $ parseTags tags\n</code></pre>\n<p>okay</p>\n"
    ),
    html_blocks_170(
        "<script type=\"text/javascript\">\n// JavaScript example\n\ndocument.getElementById(\"demo\").innerHTML = \"Hello JavaScript!\";\n</script>\nokay\n",
        "<script type=\"text/javascript\">\n// JavaScript example\n\ndocument.getElementById(\"demo\").innerHTML = \"Hello JavaScript!\";\n</script>\n<p>okay</p>\n"
    ),
    html_blocks_171(
        "<textarea>\n\n*foo*\n\n_bar_\n\n</textarea>\n",
        "<textarea>\n\n*foo*\n\n_bar_\n\n</textarea>\n"
    ),
    html_blocks_172(
        "<style\n  type=\"text/css\">\nh1 {color:red;}\n\np {color:blue;}\n</style>\nokay\n",
        "<style\n  type=\"text/css\">\nh1 {color:red;}\n\np {color:blue;}\n</style>\n<p>okay</p>\n"
    ),
    html_blocks_173(
        "<style\n  type=\"text/css\">\n\nfoo\n",
        "<style\n  type=\"text/css\">\n\nfoo\n"
    ),
    html_blocks_174(
        "> <div>\n> foo\n\nbar\n",
        "<blockquote>\n<div>\nfoo\n</blockquote>\n<p>bar</p>\n"
    ),
    html_blocks_175(
        "- <div>\n- foo\n",
        "<ul>\n<li>\n<div>\n</li>\n<li>foo</li>\n</ul>\n"
    ),
    html_blocks_176(
        "<style>p{color:red;}</style>\n*foo*\n",
        "<style>p{color:red;}</style>\n<p><em>foo</em></p>\n"
    ),
    html_blocks_177(
        "<!-- foo -->*bar*\n*baz*\n",
        "<!-- foo -->*bar*\n<p><em>baz</em></p>\n"
    ),
    html_blocks_178(
        "<script>\nfoo\n</script>1. *bar*\n",
        "<script>\nfoo\n</script>1. *bar*\n"
    ),
    html_blocks_179(
        "<!-- Foo\n\nbar\n   baz -->\nokay\n",
        "<!-- Foo\n\nbar\n   baz -->\n<p>okay</p>\n"
    ),
    html_blocks_180(
        "<?php\n\n  echo '>';\n\n?>\nokay\n",
        "<?php\n\n  echo '>';\n\n?>\n<p>okay</p>\n"
    ),
    html_blocks_181(
        "<!DOCTYPE html>\n",
        "<!DOCTYPE html>\n"
    ),
    html_blocks_182(
        "<![CDATA[\nfunction matchwo(a,b)\n{\n  if (a < b && a < 0) then {\n    return 1;\n\n  } else {\n\n    return 0;\n  }\n}\n]]>\nokay\n",
        "<![CDATA[\nfunction matchwo(a,b)\n{\n  if (a < b && a < 0) then {\n    return 1;\n\n  } else {\n\n    return 0;\n  }\n}\n]]>\n<p>okay</p>\n"
    ),
    html_blocks_183(
        "  <!-- foo -->\n\n    <!-- foo -->\n",
        "  <!-- foo -->\n<pre><code>&lt;!-- foo --&gt;\n</code></pre>\n"
    ),
    html_blocks_184(
        "  <div>\n\n    <div>\n",
        "  <div>\n<pre><code>&lt;div&gt;\n</code></pre>\n"
    ),
    html_blocks_185(
        "Foo\n<div>\nbar\n</div>\n",
        "<p>Foo</p>\n<div>\nbar\n</div>\n"
    ),
    html_blocks_186(
        "<div>\nbar\n</div>\n*foo*\n",
        "<div>\nbar\n</div>\n*foo*\n"
    ),
    html_blocks_187(
        "Foo\n<a href=\"bar\">\nbaz\n",
        "<p>Foo\n<a href=\"bar\">\nbaz</p>\n"
    ),
    html_blocks_188(
        "<div>\n\n*Emphasized* text.\n\n</div>\n",
        "<div>\n<p><em>Emphasized</em> text.</p>\n</div>\n"
    ),
    html_blocks_189(
        "<div>\n*Emphasized* text.\n</div>\n",
        "<div>\n*Emphasized* text.\n</div>\n"
    ),
    html_blocks_190(
        "<table>\n\n<tr>\n\n<td>\nHi\n</td>\n\n</tr>\n\n</table>\n",
        "<table>\n<tr>\n<td>\nHi\n</td>\n</tr>\n</table>\n"
    ),
    html_blocks_191(
        "<table>\n\n  <tr>\n\n    <td>\n      Hi\n    </td>\n\n  </tr>\n\n</table>\n",
        "<table>\n  <tr>\n<pre><code>&lt;td&gt;\n  Hi\n&lt;/td&gt;\n</code></pre>\n  </tr>\n</table>\n"
    ),

    // Link reference definitions
    link_reference_definitions_192(
        "[foo]: /url \"title\"\n\n[foo]\n",
        "<p><a href=\"/url\" title=\"title\">foo</a></p>\n"
    ),
    link_reference_definitions_193(
        "   [foo]: \n      /url  \n           'the title'  \n\n[foo]\n",
        "<p><a href=\"/url\" title=\"the title\">foo</a></p>\n"
    ),
    link_reference_definitions_194(
        "[Foo*bar\\]]:my_(url) 'title (with parens)'\n\n[Foo*bar\\]]\n",
        "<p><a href=\"my_(url)\" title=\"title (with parens)\">Foo*bar]</a></p>\n"
    ),
    link_reference_definitions_195(
        "[Foo bar]:\n<my url>\n'title'\n\n[Foo bar]\n",
        "<p><a href=\"my%20url\" title=\"title\">Foo bar</a></p>\n"
    ),
    link_reference_definitions_196(
        "[foo]: /url '\ntitle\nline1\nline2\n'\n\n[foo]\n",
        "<p><a href=\"/url\" title=\"\ntitle\nline1\nline2\n\">foo</a></p>\n"
    ),
    link_reference_definitions_197(
        "[foo]: /url 'title\n\nwith blank line'\n\n[foo]\n",
        "<p>[foo]: /url 'title</p>\n<p>with blank line'</p>\n<p>[foo]</p>\n"
    ),
    link_reference_definitions_198(
        "[foo]:\n/url\n\n[foo]\n",
        "<p><a href=\"/url\">foo</a></p>\n"
    ),
    link_reference_definitions_199(
        "[foo]:\n\n[foo]\n",
        "<p>[foo]:</p>\n<p>[foo]</p>\n"
    ),
    link_reference_definitions_200(
        "[foo]: <>\n\n[foo]\n",
        "<p><a href=\"\">foo</a></p>\n"
    ),
    link_reference_definitions_201(
        "[foo]: <bar>(baz)\n\n[foo]\n",
        "<p>[foo]: <bar>(baz)</p>\n<p>[foo]</p>\n"
    ),
    link_reference_definitions_202(
        "[foo]: /url\\bar\\*baz \"foo\\\"bar\\baz\"\n\n[foo]\n",
        "<p><a href=\"/url%5Cbar*baz\" title=\"foo&quot;bar\\baz\">foo</a></p>\n"
    ),
    link_reference_definitions_203(
        "[foo]\n\n[foo]: url\n",
        "<p><a href=\"url\">foo</a></p>\n"
    ),
    link_reference_definitions_204(
        "[foo]\n\n[foo]: first\n[foo]: second\n",
        "<p><a href=\"first\">foo</a></p>\n"
    ),
    link_reference_definitions_205(
        "[FOO]: /url\n\n[Foo]\n",
        "<p><a href=\"/url\">Foo</a></p>\n"
    ),
    link_reference_definitions_206(
        "[ΑΓΩ]: /φου\n\n[αγω]\n",
        "<p><a href=\"/%CF%86%CE%BF%CF%85\">αγω</a></p>\n"
    ),
    link_reference_definitions_207(
        "[foo]: /url\n",
        ""
    ),
    link_reference_definitions_208(
        "[\nfoo\n]: /url\nbar\n",
        "<p>bar</p>\n"
    ),
    link_reference_definitions_209(
        "[foo]: /url \"title\" ok\n",
        "<p>[foo]: /url &quot;title&quot; ok</p>\n"
    ),
    link_reference_definitions_210(
        "[foo]: /url\n\"title\" ok\n",
        "<p>&quot;title&quot; ok</p>\n"
    ),
    link_reference_definitions_211(
        "    [foo]: /url \"title\"\n\n[foo]\n",
        "<pre><code>[foo]: /url &quot;title&quot;\n</code></pre>\n<p>[foo]</p>\n"
    ),
    link_reference_definitions_212(
        "```\n[foo]: /url\n```\n\n[foo]\n",
        "<pre><code>[foo]: /url\n</code></pre>\n<p>[foo]</p>\n"
    ),
    link_reference_definitions_213(
        "Foo\n[bar]: /baz\n\n[bar]\n",
        "<p>Foo\n[bar]: /baz</p>\n<p>[bar]</p>\n"
    ),
    link_reference_definitions_214(
        "# [Foo]\n[foo]: /url\n> bar\n",
        "<h1><a href=\"/url\">Foo</a></h1>\n<blockquote>\n<p>bar</p>\n</blockquote>\n"
    ),
    link_reference_definitions_215(
        "[foo]: /url\nbar\n===\n[foo]\n",
        "<h1>bar</h1>\n<p><a href=\"/url\">foo</a></p>\n"
    ),
    link_reference_definitions_216(
        "[foo]: /url\n===\n[foo]\n",
        "<p>===\n<a href=\"/url\">foo</a></p>\n"
    ),
    link_reference_definitions_217(
        "[foo]: /foo-url \"foo\"\n[bar]: /bar-url\n  \"bar\"\n[baz]: /baz-url\n\n[foo],\n[bar],\n[baz]\n",
        "<p><a href=\"/foo-url\" title=\"foo\">foo</a>,\n<a href=\"/bar-url\" title=\"bar\">bar</a>,\n<a href=\"/baz-url\">baz</a></p>\n"
    ),
    link_reference_definitions_218(
        "[foo]\n\n> [foo]: /url\n",
        "<p><a href=\"/url\">foo</a></p>\n<blockquote>\n</blockquote>\n"
    ),

    // Paragraphs
    paragraphs_219(
        "aaa\n\nbbb\n",
        "<p>aaa</p>\n<p>bbb</p>\n"
    ),
    paragraphs_220(
        "aaa\nbbb\n\nccc\nddd\n",
        "<p>aaa\nbbb</p>\n<p>ccc\nddd</p>\n"
    ),
    paragraphs_221(
        "aaa\n\n\nbbb\n",
        "<p>aaa</p>\n<p>bbb</p>\n"
    ),
    paragraphs_222(
        "  aaa\n bbb\n",
        "<p>aaa\nbbb</p>\n"
    ),
    paragraphs_223(
        "aaa\n             bbb\n                                       ccc\n",
        "<p>aaa\nbbb\nccc</p>\n"
    ),
    paragraphs_224(
        "   aaa\nbbb\n",
        "<p>aaa\nbbb</p>\n"
    ),
    paragraphs_225(
        "    aaa\nbbb\n",
        "<pre><code>aaa\n</code></pre>\n<p>bbb</p>\n"
    ),
    paragraphs_226(
        "aaa     \nbbb     \n",
        "<p>aaa<br />\nbbb</p>\n"
    ),

    // Blank lines
    blank_lines_227(
        "  \n\naaa\n  \n\n# aaa\n\n  \n",
        "<p>aaa</p>\n<h1>aaa</h1>\n"
    ),

    // Block quotes
    block_quotes_228(
        "> # Foo\n> bar\n> baz\n",
        "<blockquote>\n<h1>Foo</h1>\n<p>bar\nbaz</p>\n</blockquote>\n"
    ),
    block_quotes_229(
        "># Foo\n>bar\n> baz\n",
        "<blockquote>\n<h1>Foo</h1>\n<p>bar\nbaz</p>\n</blockquote>\n"
    ),
    block_quotes_230(
        "   > # Foo\n   > bar\n > baz\n",
        "<blockquote>\n<h1>Foo</h1>\n<p>bar\nbaz</p>\n</blockquote>\n"
    ),
    block_quotes_231(
        "    > # Foo\n    > bar\n    > baz\n",
        "<pre><code>&gt; # Foo\n&gt; bar\n&gt; baz\n</code></pre>\n"
    ),
    block_quotes_232(
        "> # Foo\n> bar\nbaz\n",
        "<blockquote>\n<h1>Foo</h1>\n<p>bar\nbaz</p>\n</blockquote>\n"
    ),
    block_quotes_233(
        "> bar\nbaz\n> foo\n",
        "<blockquote>\n<p>bar\nbaz\nfoo</p>\n</blockquote>\n"
    ),
    block_quotes_234(
        "> foo\n---\n",
        "<blockquote>\n<p>foo</p>\n</blockquote>\n<hr />\n"
    ),
    block_quotes_235(
        "> - foo\n- bar\n",
        "<blockquote>\n<ul>\n<li>foo</li>\n</ul>\n</blockquote>\n<ul>\n<li>bar</li>\n</ul>\n"
    ),
    block_quotes_236(
        ">     foo\n    bar\n",
        "<blockquote>\n<pre><code>foo\n</code></pre>\n</blockquote>\n<pre><code>bar\n</code></pre>\n"
    ),
    block_quotes_237(
        "> ```\nfoo\n```\n",
        "<blockquote>\n<pre><code></code></pre>\n</blockquote>\n<p>foo</p>\n<pre><code></code></pre>\n"
    ),
    block_quotes_238(
        "> foo\n    - bar\n",
        "<blockquote>\n<p>foo\n- bar</p>\n</blockquote>\n"
    ),
    block_quotes_239(
        ">\n",
        "<blockquote>\n</blockquote>\n"
    ),
    block_quotes_240(
        ">\n>  \n> \n",
        "<blockquote>\n</blockquote>\n"
    ),
    block_quotes_241(
        ">\n> foo\n>  \n",
        "<blockquote>\n<p>foo</p>\n</blockquote>\n"
    ),
    block_quotes_242(
        "> foo\n\n> bar\n",
        "<blockquote>\n<p>foo</p>\n</blockquote>\n<blockquote>\n<p>bar</p>\n</blockquote>\n"
    ),
    block_quotes_243(
        "> foo\n> bar\n",
        "<blockquote>\n<p>foo\nbar</p>\n</blockquote>\n"
    ),
    block_quotes_244(
        "> foo\n>\n> bar\n",
        "<blockquote>\n<p>foo</p>\n<p>bar</p>\n</blockquote>\n"
    ),
    block_quotes_245(
        "foo\n> bar\n",
        "<p>foo</p>\n<blockquote>\n<p>bar</p>\n</blockquote>\n"
    ),
    block_quotes_246(
        "> aaa\n***\n> bbb\n",
        "<blockquote>\n<p>aaa</p>\n</blockquote>\n<hr />\n<blockquote>\n<p>bbb</p>\n</blockquote>\n"
    ),
    block_quotes_247(
        "> bar\nbaz\n",
        "<blockquote>\n<p>bar\nbaz</p>\n</blockquote>\n"
    ),
    block_quotes_248(
        "> bar\n\nbaz\n",
        "<blockquote>\n<p>bar</p>\n</blockquote>\n<p>baz</p>\n"
    ),
    block_quotes_249(
        "> bar\n>\nbaz\n",
        "<blockquote>\n<p>bar</p>\n</blockquote>\n<p>baz</p>\n"
    ),
    block_quotes_250(
        "> > > foo\nbar\n",
        "<blockquote>\n<blockquote>\n<blockquote>\n<p>foo\nbar</p>\n</blockquote>\n</blockquote>\n</blockquote>\n"
    ),
    block_quotes_251(
        ">>> foo\n> bar\n>>baz\n",
        "<blockquote>\n<blockquote>\n<blockquote>\n<p>foo\nbar\nbaz</p>\n</blockquote>\n</blockquote>\n</blockquote>\n"
    ),
    block_quotes_252(
        ">     code\n\n>    not code\n",
        "<blockquote>\n<pre><code>code\n</code></pre>\n</blockquote>\n<blockquote>\n<p>not code</p>\n</blockquote>\n"
    ),

    // List items
    list_items_253(
        "A paragraph\nwith two lines.\n\n    indented code\n\n> A block quote.\n",
        "<p>A paragraph\nwith two lines.</p>\n<pre><code>indented code\n</code></pre>\n<blockquote>\n<p>A block quote.</p>\n</blockquote>\n"
    ),
    list_items_254(
        "1.  A paragraph\n    with two lines.\n\n        indented code\n\n    > A block quote.\n",
        "<ol>\n<li>\n<p>A paragraph\nwith two lines.</p>\n<pre><code>indented code\n</code></pre>\n<blockquote>\n<p>A block quote.</p>\n</blockquote>\n</li>\n</ol>\n"
    ),
    list_items_255(
        "- one\n\n two\n",
        "<ul>\n<li>one</li>\n</ul>\n<p>two</p>\n"
    ),
    list_items_256(
        "- one\n\n  two\n",
        "<ul>\n<li>\n<p>one</p>\n<p>two</p>\n</li>\n</ul>\n"
    ),
    list_items_257(
        " -    one\n\n     two\n",
        "<ul>\n<li>one</li>\n</ul>\n<pre><code> two\n</code></pre>\n"
    ),
    list_items_258(
        " -    one\n\n      two\n",
        "<ul>\n<li>\n<p>one</p>\n<p>two</p>\n</li>\n</ul>\n"
    ),
    list_items_259(
        "   > > 1.  one\n>>\n>>     two\n",
        "<blockquote>\n<blockquote>\n<ol>\n<li>\n<p>one</p>\n<p>two</p>\n</li>\n</ol>\n</blockquote>\n</blockquote>\n"
    ),
    list_items_260(
        ">>- one\n>>\n  >  > two\n",
        "<blockquote>\n<blockquote>\n<ul>\n<li>one</li>\n</ul>\n<p>two</p>\n</blockquote>\n</blockquote>\n"
    ),
    list_items_261(
        "-one\n\n2.two\n",
        "<p>-one</p>\n<p>2.two</p>\n"
    ),
    list_items_262(
        "- foo\n\n\n  bar\n",
        "<ul>\n<li>\n<p>foo</p>\n<p>bar</p>\n</li>\n</ul>\n"
    ),
    list_items_263(
        "1.  foo\n\n    ```\n    bar\n    ```\n\n    baz\n\n    > bam\n",
        "<ol>\n<li>\n<p>foo</p>\n<pre><code>bar\n</code></pre>\n<p>baz</p>\n<blockquote>\n<p>bam</p>\n</blockquote>\n</li>\n</ol>\n"
    ),
    list_items_264(
        "- Foo\n\n      bar\n\n\n      baz\n",
        "<ul>\n<li>\n<p>Foo</p>\n<pre><code>bar\n\n\nbaz\n</code></pre>\n</li>\n</ul>\n"
    ),
    list_items_265(
        "123456789. ok\n",
        "<ol start=\"123456789\">\n<li>ok</li>\n</ol>\n"
    ),
    list_items_266(
        "1234567890. not ok\n",
        "<p>1234567890. not ok</p>\n"
    ),
    list_items_267(
        "0. ok\n",
        "<ol start=\"0\">\n<li>ok</li>\n</ol>\n"
    ),
    list_items_268(
        "003. ok\n",
        "<ol start=\"3\">\n<li>ok</li>\n</ol>\n"
    ),
    list_items_269(
        "-1. not ok\n",
        "<p>-1. not ok</p>\n"
    ),
    list_items_270(
        "- foo\n\n      bar\n",
        "<ul>\n<li>\n<p>foo</p>\n<pre><code>bar\n</code></pre>\n</li>\n</ul>\n"
    ),
    list_items_271(
        "  10.  foo\n\n           bar\n",
        "<ol start=\"10\">\n<li>\n<p>foo</p>\n<pre><code>bar\n</code></pre>\n</li>\n</ol>\n"
    ),
    list_items_272(
        "    indented code\n\nparagraph\n\n    more code\n",
        "<pre><code>indented code\n</code></pre>\n<p>paragraph</p>\n<pre><code>more code\n</code></pre>\n"
    ),
    list_items_273(
        "1.     indented code\n\n   paragraph\n\n       more code\n",
        "<ol>\n<li>\n<pre><code>indented code\n</code></pre>\n<p>paragraph</p>\n<pre><code>more code\n</code></pre>\n</li>\n</ol>\n"
    ),
    list_items_274(
        "1.      indented code\n\n   paragraph\n\n       more code\n",
        "<ol>\n<li>\n<pre><code> indented code\n</code></pre>\n<p>paragraph</p>\n<pre><code>more code\n</code></pre>\n</li>\n</ol>\n"
    ),
    list_items_275(
        "   foo\n\nbar\n",
        "<p>foo</p>\n<p>bar</p>\n"
    ),
    list_items_276(
        "-    foo\n\n  bar\n",
        "<ul>\n<li>foo</li>\n</ul>\n<p>bar</p>\n"
    ),
    list_items_277(
        "-  foo\n\n   bar\n",
        "<ul>\n<li>\n<p>foo</p>\n<p>bar</p>\n</li>\n</ul>\n"
    ),
    list_items_278(
        "-\n  foo\n-\n  ```\n  bar\n  ```\n-\n      baz\n",
        "<ul>\n<li>foo</li>\n<li>\n<pre><code>bar\n</code></pre>\n</li>\n<li>\n<pre><code>baz\n</code></pre>\n</li>\n</ul>\n"
    ),
    list_items_279(
        "-   \n  foo\n",
        "<ul>\n<li>foo</li>\n</ul>\n"
    ),
    list_items_280(
        "-\n\n  foo\n",
        "<ul>\n<li></li>\n</ul>\n<p>foo</p>\n"
    ),
    list_items_281(
        "- foo\n-\n- bar\n",
        "<ul>\n<li>foo</li>\n<li></li>\n<li>bar</li>\n</ul>\n"
    ),
    list_items_282(
        "- foo\n-   \n- bar\n",
        "<ul>\n<li>foo</li>\n<li></li>\n<li>bar</li>\n</ul>\n"
    ),
    list_items_283(
        "1. foo\n2.\n3. bar\n",
        "<ol>\n<li>foo</li>\n<li></li>\n<li>bar</li>\n</ol>\n"
    ),
    list_items_284(
        "*\n",
        "<ul>\n<li></li>\n</ul>\n"
    ),
    list_items_285(
        "foo\n*\n\nfoo\n1.\n",
        "<p>foo\n*</p>\n<p>foo\n1.</p>\n"
    ),
    list_items_286(
        " 1.  A paragraph\n     with two lines.\n\n         indented code\n\n     > A block quote.\n",
        "<ol>\n<li>\n<p>A paragraph\nwith two lines.</p>\n<pre><code>indented code\n</code></pre>\n<blockquote>\n<p>A block quote.</p>\n</blockquote>\n</li>\n</ol>\n"
    ),
    list_items_287(
        "  1.  A paragraph\n      with two lines.\n\n          indented code\n\n      > A block quote.\n",
        "<ol>\n<li>\n<p>A paragraph\nwith two lines.</p>\n<pre><code>indented code\n</code></pre>\n<blockquote>\n<p>A block quote.</p>\n</blockquote>\n</li>\n</ol>\n"
    ),
    list_items_288(
        "   1.  A paragraph\n       with two lines.\n\n           indented code\n\n       > A block quote.\n",
        "<ol>\n<li>\n<p>A paragraph\nwith two lines.</p>\n<pre><code>indented code\n</code></pre>\n<blockquote>\n<p>A block quote.</p>\n</blockquote>\n</li>\n</ol>\n"
    ),
    list_items_289(
        "    1.  A paragraph\n        with two lines.\n\n            indented code\n\n        > A block quote.\n",
        "<pre><code>1.  A paragraph\n    with two lines.\n\n        indented code\n\n    &gt; A block quote.\n</code></pre>\n"
    ),
    list_items_290(
        "  1.  A paragraph\nwith two lines.\n\n          indented code\n\n      > A block quote.\n",
        "<ol>\n<li>\n<p>A paragraph\nwith two lines.</p>\n<pre><code>indented code\n</code></pre>\n<blockquote>\n<p>A block quote.</p>\n</blockquote>\n</li>\n</ol>\n"
    ),
    list_items_291(
        "  1.  A paragraph\n    with two lines.\n",
        "<ol>\n<li>A paragraph\nwith two lines.</li>\n</ol>\n"
    ),
    list_items_292(
        "> 1. > Blockquote\ncontinued here.\n",
        "<blockquote>\n<ol>\n<li>\n<blockquote>\n<p>Blockquote\ncontinued here.</p>\n</blockquote>\n</li>\n</ol>\n</blockquote>\n"
    ),
    list_items_293(
        "> 1. > Blockquote\n> continued here.\n",
        "<blockquote>\n<ol>\n<li>\n<blockquote>\n<p>Blockquote\ncontinued here.</p>\n</blockquote>\n</li>\n</ol>\n</blockquote>\n"
    ),
    list_items_294(
        "- foo\n  - bar\n    - baz\n      - boo\n",
        "<ul>\n<li>foo\n<ul>\n<li>bar\n<ul>\n<li>baz\n<ul>\n<li>boo</li>\n</ul>\n</li>\n</ul>\n</li>\n</ul>\n</li>\n</ul>\n"
    ),
    list_items_295(
        "- foo\n - bar\n  - baz\n   - boo\n",
        "<ul>\n<li>foo</li>\n<li>bar</li>\n<li>baz</li>\n<li>boo</li>\n</ul>\n"
    ),
    list_items_296(
        "10) foo\n    - bar\n",
        "<ol start=\"10\">\n<li>foo\n<ul>\n<li>bar</li>\n</ul>\n</li>\n</ol>\n"
    ),
    list_items_297(
        "10) foo\n   - bar\n",
        "<ol start=\"10\">\n<li>foo</li>\n</ol>\n<ul>\n<li>bar</li>\n</ul>\n"
    ),
    list_items_298(
        "- - foo\n",
        "<ul>\n<li>\n<ul>\n<li>foo</li>\n</ul>\n</li>\n</ul>\n"
    ),
    list_items_299(
        "1. - 2. foo\n",
        "<ol>\n<li>\n<ul>\n<li>\n<ol start=\"2\">\n<li>foo</li>\n</ol>\n</li>\n</ul>\n</li>\n</ol>\n"
    ),
    list_items_300(
        "- # Foo\n- Bar\n  ---\n  baz\n",
        "<ul>\n<li>\n<h1>Foo</h1>\n</li>\n<li>\n<h2>Bar</h2>\nbaz</li>\n</ul>\n"
    ),

    // Lists
    lists_301(
        "- foo\n- bar\n+ baz\n",
        "<ul>\n<li>foo</li>\n<li>bar</li>\n</ul>\n<ul>\n<li>baz</li>\n</ul>\n"
    ),
    lists_302(
        "1. foo\n2. bar\n3) baz\n",
        "<ol>\n<li>foo</li>\n<li>bar</li>\n</ol>\n<ol start=\"3\">\n<li>baz</li>\n</ol>\n"
    ),
    lists_303(
        "Foo\n- bar\n- baz\n",
        "<p>Foo</p>\n<ul>\n<li>bar</li>\n<li>baz</li>\n</ul>\n"
    ),
    lists_304(
        "The number of windows in my house is\n14.  The number of doors is 6.\n",
        "<p>The number of windows in my house is\n14.  The number of doors is 6.</p>\n"
    ),
    lists_305(
        "The number of windows in my house is\n1.  The number of doors is 6.\n",
        "<p>The number of windows in my house is</p>\n<ol>\n<li>The number of doors is 6.</li>\n</ol>\n"
    ),
    lists_306(
        "- foo\n\n- bar\n\n\n- baz\n",
        "<ul>\n<li>\n<p>foo</p>\n</li>\n<li>\n<p>bar</p>\n</li>\n<li>\n<p>baz</p>\n</li>\n</ul>\n"
    ),
    lists_307(
        "- foo\n  - bar\n    - baz\n\n\n      bim\n",
        "<ul>\n<li>foo\n<ul>\n<li>bar\n<ul>\n<li>\n<p>baz</p>\n<p>bim</p>\n</li>\n</ul>\n</li>\n</ul>\n</li>\n</ul>\n"
    ),
    lists_308(
        "- foo\n- bar\n\n<!-- -->\n\n- baz\n- bim\n",
        "<ul>\n<li>foo</li>\n<li>bar</li>\n</ul>\n<!-- -->\n<ul>\n<li>baz</li>\n<li>bim</li>\n</ul>\n"
    ),
    lists_309(
        "-   foo\n\n    notcode\n\n-   foo\n\n<!-- -->\n\n    code\n",
        "<ul>\n<li>\n<p>foo</p>\n<p>notcode</p>\n</li>\n<li>\n<p>foo</p>\n</li>\n</ul>\n<!-- -->\n<pre><code>code\n</code></pre>\n"
    ),
    lists_310(
        "- a\n - b\n  - c\n   - d\n  - e\n - f\n- g\n",
        "<ul>\n<li>a</li>\n<li>b</li>\n<li>c</li>\n<li>d</li>\n<li>e</li>\n<li>f</li>\n<li>g</li>\n</ul>\n"
    ),
    lists_311(
        "1. a\n\n  2. b\n\n   3. c\n",
        "<ol>\n<li>\n<p>a</p>\n</li>\n<li>\n<p>b</p>\n</li>\n<li>\n<p>c</p>\n</li>\n</ol>\n"
    ),
    lists_312(
        "- a\n - b\n  - c\n   - d\n    - e\n",
        "<ul>\n<li>a</li>\n<li>b</li>\n<li>c</li>\n<li>d\n- e</li>\n</ul>\n"
    ),
    lists_313(
        "1. a\n\n  2. b\n\n    3. c\n",
        "<ol>\n<li>\n<p>a</p>\n</li>\n<li>\n<p>b</p>\n</li>\n</ol>\n<pre><code>3. c\n</code></pre>\n"
    ),
    lists_314(
        "- a\n- b\n\n- c\n",
        "<ul>\n<li>\n<p>a</p>\n</li>\n<li>\n<p>b</p>\n</li>\n<li>\n<p>c</p>\n</li>\n</ul>\n"
    ),
    lists_315(
        "* a\n*\n\n* c\n",
        "<ul>\n<li>\n<p>a</p>\n</li>\n<li></li>\n<li>\n<p>c</p>\n</li>\n</ul>\n"
    ),
    lists_316(
        "- a\n- b\n\n  c\n- d\n",
        "<ul>\n<li>\n<p>a</p>\n</li>\n<li>\n<p>b</p>\n<p>c</p>\n</li>\n<li>\n<p>d</p>\n</li>\n</ul>\n"
    ),
    lists_317(
        "- a\n- b\n\n  [ref]: /url\n- d\n",
        "<ul>\n<li>\n<p>a</p>\n</li>\n<li>\n<p>b</p>\n</li>\n<li>\n<p>d</p>\n</li>\n</ul>\n"
    ),
    lists_318(
        "- a\n- ```\n  b\n\n\n  ```\n- c\n",
        "<ul>\n<li>a</li>\n<li>\n<pre><code>b\n\n\n</code></pre>\n</li>\n<li>c</li>\n</ul>\n"
    ),
    lists_319(
        "- a\n  - b\n\n    c\n- d\n",
        "<ul>\n<li>a\n<ul>\n<li>\n<p>b</p>\n<p>c</p>\n</li>\n</ul>\n</li>\n<li>d</li>\n</ul>\n"
    ),
    lists_320(
        "* a\n  > b\n  >\n* c\n",
        "<ul>\n<li>a\n<blockquote>\n<p>b</p>\n</blockquote>\n</li>\n<li>c</li>\n</ul>\n"
    ),
    lists_321(
        "- a\n  > b\n  ```\n  c\n  ```\n- d\n",
        "<ul>\n<li>a\n<blockquote>\n<p>b</p>\n</blockquote>\n<pre><code>c\n</code></pre>\n</li>\n<li>d</li>\n</ul>\n"
    ),
    lists_322(
        "- a\n",
        "<ul>\n<li>a</li>\n</ul>\n"
    ),
    lists_323(
        "- a\n  - b\n",
        "<ul>\n<li>a\n<ul>\n<li>b</li>\n</ul>\n</li>\n</ul>\n"
    ),
    lists_324(
        "1. ```\n   foo\n   ```\n\n   bar\n",
        "<ol>\n<li>\n<pre><code>foo\n</code></pre>\n<p>bar</p>\n</li>\n</ol>\n"
    ),
    lists_325(
        "* foo\n  * bar\n\n  baz\n",
        "<ul>\n<li>\n<p>foo</p>\n<ul>\n<li>bar</li>\n</ul>\n<p>baz</p>\n</li>\n</ul>\n"
    ),
    lists_326(
        "- a\n  - b\n  - c\n\n- d\n  - e\n  - f\n",
        "<ul>\n<li>\n<p>a</p>\n<ul>\n<li>b</li>\n<li>c</li>\n</ul>\n</li>\n<li>\n<p>d</p>\n<ul>\n<li>e</li>\n<li>f</li>\n</ul>\n</li>\n</ul>\n"
    ),

    // Inlines
    inlines_327(
        "`hi`lo`\n",
        "<p><code>hi</code>lo`</p>\n"
    ),

    // Code spans
    code_spans_328(
        "`foo`\n",
        "<p><code>foo</code></p>\n"
    ),
    code_spans_329(
        "`` foo ` bar ``\n",
        "<p><code>foo ` bar</code></p>\n"
    ),
    code_spans_330(
        "` `` `\n",
        "<p><code>``</code></p>\n"
    ),
    code_spans_331(
        "`  ``  `\n",
        "<p><code> `` </code></p>\n"
    ),
    code_spans_332(
        "` a`\n",
        "<p><code> a</code></p>\n"
    ),
    code_spans_333(
        "` b `\n",
        "<p><code> b </code></p>\n"
    ),
    code_spans_334(
        "` `\n`  `\n",
        "<p><code> </code>\n<code>  </code></p>\n"
    ),
    code_spans_335(
        "``\nfoo\nbar  \nbaz\n``\n",
        "<p><code>foo bar   baz</code></p>\n"
    ),
    code_spans_336(
        "``\nfoo \n``\n",
        "<p><code>foo </code></p>\n"
    ),
    code_spans_337(
        "`foo   bar \nbaz`\n",
        "<p><code>foo   bar  baz</code></p>\n"
    ),
    code_spans_338(
        "`foo\\`bar`\n",
        "<p><code>foo\\</code>bar`</p>\n"
    ),
    code_spans_339(
        "``foo`bar``\n",
        "<p><code>foo`bar</code></p>\n"
    ),
    code_spans_340(
        "` foo `` bar `\n",
        "<p><code>foo `` bar</code></p>\n"
    ),
    code_spans_341(
        "*foo`*`\n",
        "<p>*foo<code>*</code></p>\n"
    ),
    code_spans_342(
        "[not a `link](/foo`)\n",
        "<p>[not a <code>link](/foo</code>)</p>\n"
    ),
    code_spans_343(
        "`<a href=\"`\">`\n",
        "<p><code>&lt;a href=&quot;</code>&quot;&gt;`</p>\n"
    ),
    code_spans_344(
        "<a href=\"`\">`\n",
        "<p><a href=\"`\">`</p>\n"
    ),
    code_spans_345(
        "`<https://foo.bar.`baz>`\n",
        "<p><code>&lt;https://foo.bar.</code>baz&gt;`</p>\n"
    ),
    code_spans_346(
        "<https://foo.bar.`baz>`\n",
        "<p><a href=\"https://foo.bar.%60baz\">https://foo.bar.`baz</a>`</p>\n"
    ),
    code_spans_347(
        "```foo``\n",
        "<p>```foo``</p>\n"
    ),
    code_spans_348(
        "`foo\n",
        "<p>`foo</p>\n"
    ),
    code_spans_349(
        "`foo``bar``\n",
        "<p>`foo<code>bar</code></p>\n"
    ),

    // Emphasis and strong emphasis
    emphasis_and_strong_emphasis_350(
        "*foo bar*\n",
        "<p><em>foo bar</em></p>\n"
    ),
    emphasis_and_strong_emphasis_351(
        "a * foo bar*\n",
        "<p>a * foo bar*</p>\n"
    ),
    emphasis_and_strong_emphasis_352(
        "a*\"foo\"*\n",
        "<p>a*&quot;foo&quot;*</p>\n"
    ),
    emphasis_and_strong_emphasis_353(
        "* a *\n",
        "<p>* a *</p>\n"
    ),
    emphasis_and_strong_emphasis_354(
        "*$*alpha.\n\n*£*bravo.\n\n*€*charlie.\n",
        "<p>*$*alpha.</p>\n<p>*£*bravo.</p>\n<p>*€*charlie.</p>\n"
    ),
    emphasis_and_strong_emphasis_355(
        "foo*bar*\n",
        "<p>foo<em>bar</em></p>\n"
    ),
    emphasis_and_strong_emphasis_356(
        "5*6*78\n",
        "<p>5<em>6</em>78</p>\n"
    ),
    emphasis_and_strong_emphasis_357(
        "_foo bar_\n",
        "<p><em>foo bar</em></p>\n"
    ),
    emphasis_and_strong_emphasis_358(
        "_ foo bar_\n",
        "<p>_ foo bar_</p>\n"
    ),
    emphasis_and_strong_emphasis_359(
        "a_\"foo\"_\n",
        "<p>a_&quot;foo&quot;_</p>\n"
    ),
    emphasis_and_strong_emphasis_360(
        "foo_bar_\n",
        "<p>foo_bar_</p>\n"
    ),
    emphasis_and_strong_emphasis_361(
        "5_6_78\n",
        "<p>5_6_78</p>\n"
    ),
    emphasis_and_strong_emphasis_362(
        "пристаням_стремятся_\n",
        "<p>пристаням_стремятся_</p>\n"
    ),
    emphasis_and_strong_emphasis_363(
        "aa_\"bb\"_cc\n",
        "<p>aa_&quot;bb&quot;_cc</p>\n"
    ),
    emphasis_and_strong_emphasis_364(
        "foo-_(bar)_\n",
        "<p>foo-<em>(bar)</em></p>\n"
    ),
    emphasis_and_strong_emphasis_365(
        "_foo*\n",
        "<p>_foo*</p>\n"
    ),
    emphasis_and_strong_emphasis_366(
        "*foo bar *\n",
        "<p>*foo bar *</p>\n"
    ),
    emphasis_and_strong_emphasis_367(
        "*foo bar\n*\n",
        "<p>*foo bar\n*</p>\n"
    ),
    emphasis_and_strong_emphasis_368(
        "*(*foo)\n",
        "<p>*(*foo)</p>\n"
    ),
    emphasis_and_strong_emphasis_369(
        "*(*foo*)*\n",
        "<p><em>(<em>foo</em>)</em></p>\n"
    ),
    emphasis_and_strong_emphasis_370(
        "*foo*bar\n",
        "<p><em>foo</em>bar</p>\n"
    ),
    emphasis_and_strong_emphasis_371(
        "_foo bar _\n",
        "<p>_foo bar _</p>\n"
    ),
    emphasis_and_strong_emphasis_372(
        "_(_foo)\n",
        "<p>_(_foo)</p>\n"
    ),
    emphasis_and_strong_emphasis_373(
        "_(_foo_)_\n",
        "<p><em>(<em>foo</em>)</em></p>\n"
    ),
    emphasis_and_strong_emphasis_374(
        "_foo_bar\n",
        "<p>_foo_bar</p>\n"
    ),
    emphasis_and_strong_emphasis_375(
        "_пристаням_стремятся\n",
        "<p>_пристаням_стремятся</p>\n"
    ),
    emphasis_and_strong_emphasis_376(
        "_foo_bar_baz_\n",
        "<p><em>foo_bar_baz</em></p>\n"
    ),
    emphasis_and_strong_emphasis_377(
        "_(bar)_.\n",
        "<p><em>(bar)</em>.</p>\n"
    ),
    emphasis_and_strong_emphasis_378(
        "**foo bar**\n",
        "<p><strong>foo bar</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_379(
        "** foo bar**\n",
        "<p>** foo bar**</p>\n"
    ),
    emphasis_and_strong_emphasis_380(
        "a**\"foo\"**\n",
        "<p>a**&quot;foo&quot;**</p>\n"
    ),
    emphasis_and_strong_emphasis_381(
        "foo**bar**\n",
        "<p>foo<strong>bar</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_382(
        "__foo bar__\n",
        "<p><strong>foo bar</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_383(
        "__ foo bar__\n",
        "<p>__ foo bar__</p>\n"
    ),
    emphasis_and_strong_emphasis_384(
        "__\nfoo bar__\n",
        "<p>__\nfoo bar__</p>\n"
    ),
    emphasis_and_strong_emphasis_385(
        "a__\"foo\"__\n",
        "<p>a__&quot;foo&quot;__</p>\n"
    ),
    emphasis_and_strong_emphasis_386(
        "foo__bar__\n",
        "<p>foo__bar__</p>\n"
    ),
    emphasis_and_strong_emphasis_387(
        "5__6__78\n",
        "<p>5__6__78</p>\n"
    ),
    emphasis_and_strong_emphasis_388(
        "пристаням__стремятся__\n",
        "<p>пристаням__стремятся__</p>\n"
    ),
    emphasis_and_strong_emphasis_389(
        "__foo, __bar__, baz__\n",
        "<p><strong>foo, <strong>bar</strong>, baz</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_390(
        "foo-__(bar)__\n",
        "<p>foo-<strong>(bar)</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_391(
        "**foo bar **\n",
        "<p>**foo bar **</p>\n"
    ),
    emphasis_and_strong_emphasis_392(
        "**(**foo)\n",
        "<p>**(**foo)</p>\n"
    ),
    emphasis_and_strong_emphasis_393(
        "*(**foo**)*\n",
        "<p><em>(<strong>foo</strong>)</em></p>\n"
    ),
    emphasis_and_strong_emphasis_394(
        "**Gomphocarpus (*Gomphocarpus physocarpus*, syn.\n*Asclepias physocarpa*)**\n",
        "<p><strong>Gomphocarpus (<em>Gomphocarpus physocarpus</em>, syn.\n<em>Asclepias physocarpa</em>)</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_395(
        "**foo \"*bar*\" foo**\n",
        "<p><strong>foo &quot;<em>bar</em>&quot; foo</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_396(
        "**foo**bar\n",
        "<p><strong>foo</strong>bar</p>\n"
    ),
    emphasis_and_strong_emphasis_397(
        "__foo bar __\n",
        "<p>__foo bar __</p>\n"
    ),
    emphasis_and_strong_emphasis_398(
        "__(__foo)\n",
        "<p>__(__foo)</p>\n"
    ),
    emphasis_and_strong_emphasis_399(
        "_(__foo__)_\n",
        "<p><em>(<strong>foo</strong>)</em></p>\n"
    ),
    emphasis_and_strong_emphasis_400(
        "__foo__bar\n",
        "<p>__foo__bar</p>\n"
    ),
    emphasis_and_strong_emphasis_401(
        "__пристаням__стремятся\n",
        "<p>__пристаням__стремятся</p>\n"
    ),
    emphasis_and_strong_emphasis_402(
        "__foo__bar__baz__\n",
        "<p><strong>foo__bar__baz</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_403(
        "__(bar)__.\n",
        "<p><strong>(bar)</strong>.</p>\n"
    ),
    emphasis_and_strong_emphasis_404(
        "*foo [bar](/url)*\n",
        "<p><em>foo <a href=\"/url\">bar</a></em></p>\n"
    ),
    emphasis_and_strong_emphasis_405(
        "*foo\nbar*\n",
        "<p><em>foo\nbar</em></p>\n"
    ),
    emphasis_and_strong_emphasis_406(
        "_foo __bar__ baz_\n",
        "<p><em>foo <strong>bar</strong> baz</em></p>\n"
    ),
    emphasis_and_strong_emphasis_407(
        "_foo _bar_ baz_\n",
        "<p><em>foo <em>bar</em> baz</em></p>\n"
    ),
    emphasis_and_strong_emphasis_408(
        "__foo_ bar_\n",
        "<p><em><em>foo</em> bar</em></p>\n"
    ),
    emphasis_and_strong_emphasis_409(
        "*foo *bar**\n",
        "<p><em>foo <em>bar</em></em></p>\n"
    ),
    emphasis_and_strong_emphasis_410(
        "*foo **bar** baz*\n",
        "<p><em>foo <strong>bar</strong> baz</em></p>\n"
    ),
    emphasis_and_strong_emphasis_411(
        "*foo**bar**baz*\n",
        "<p><em>foo<strong>bar</strong>baz</em></p>\n"
    ),
    emphasis_and_strong_emphasis_412(
        "*foo**bar*\n",
        "<p><em>foo**bar</em></p>\n"
    ),
    emphasis_and_strong_emphasis_413(
        "***foo** bar*\n",
        "<p><em><strong>foo</strong> bar</em></p>\n"
    ),
    emphasis_and_strong_emphasis_414(
        "*foo **bar***\n",
        "<p><em>foo <strong>bar</strong></em></p>\n"
    ),
    emphasis_and_strong_emphasis_415(
        "*foo**bar***\n",
        "<p><em>foo<strong>bar</strong></em></p>\n"
    ),
    emphasis_and_strong_emphasis_416(
        "foo***bar***baz\n",
        "<p>foo<em><strong>bar</strong></em>baz</p>\n"
    ),
    emphasis_and_strong_emphasis_417(
        "foo******bar*********baz\n",
        "<p>foo<strong><strong><strong>bar</strong></strong></strong>***baz</p>\n"
    ),
    emphasis_and_strong_emphasis_418(
        "*foo **bar *baz* bim** bop*\n",
        "<p><em>foo <strong>bar <em>baz</em> bim</strong> bop</em></p>\n"
    ),
    emphasis_and_strong_emphasis_419(
        "*foo [*bar*](/url)*\n",
        "<p><em>foo <a href=\"/url\"><em>bar</em></a></em></p>\n"
    ),
    emphasis_and_strong_emphasis_420(
        "** is not an empty emphasis\n",
        "<p>** is not an empty emphasis</p>\n"
    ),
    emphasis_and_strong_emphasis_421(
        "**** is not an empty strong emphasis\n",
        "<p>**** is not an empty strong emphasis</p>\n"
    ),
    emphasis_and_strong_emphasis_422(
        "**foo [bar](/url)**\n",
        "<p><strong>foo <a href=\"/url\">bar</a></strong></p>\n"
    ),
    emphasis_and_strong_emphasis_423(
        "**foo\nbar**\n",
        "<p><strong>foo\nbar</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_424(
        "__foo _bar_ baz__\n",
        "<p><strong>foo <em>bar</em> baz</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_425(
        "__foo __bar__ baz__\n",
        "<p><strong>foo <strong>bar</strong> baz</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_426(
        "____foo__ bar__\n",
        "<p><strong><strong>foo</strong> bar</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_427(
        "**foo **bar****\n",
        "<p><strong>foo <strong>bar</strong></strong></p>\n"
    ),
    emphasis_and_strong_emphasis_428(
        "**foo *bar* baz**\n",
        "<p><strong>foo <em>bar</em> baz</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_429(
        "**foo*bar*baz**\n",
        "<p><strong>foo<em>bar</em>baz</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_430(
        "***foo* bar**\n",
        "<p><strong><em>foo</em> bar</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_431(
        "**foo *bar***\n",
        "<p><strong>foo <em>bar</em></strong></p>\n"
    ),
    emphasis_and_strong_emphasis_432(
        "**foo *bar **baz**\nbim* bop**\n",
        "<p><strong>foo <em>bar <strong>baz</strong>\nbim</em> bop</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_433(
        "**foo [*bar*](/url)**\n",
        "<p><strong>foo <a href=\"/url\"><em>bar</em></a></strong></p>\n"
    ),
    emphasis_and_strong_emphasis_434(
        "__ is not an empty emphasis\n",
        "<p>__ is not an empty emphasis</p>\n"
    ),
    emphasis_and_strong_emphasis_435(
        "____ is not an empty strong emphasis\n",
        "<p>____ is not an empty strong emphasis</p>\n"
    ),
    emphasis_and_strong_emphasis_436(
        "foo ***\n",
        "<p>foo ***</p>\n"
    ),
    emphasis_and_strong_emphasis_437(
        "foo *\\**\n",
        "<p>foo <em>*</em></p>\n"
    ),
    emphasis_and_strong_emphasis_438(
        "foo *_*\n",
        "<p>foo <em>_</em></p>\n"
    ),
    emphasis_and_strong_emphasis_439(
        "foo *****\n",
        "<p>foo *****</p>\n"
    ),
    emphasis_and_strong_emphasis_440(
        "foo **\\***\n",
        "<p>foo <strong>*</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_441(
        "foo **_**\n",
        "<p>foo <strong>_</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_442(
        "**foo*\n",
        "<p>*<em>foo</em></p>\n"
    ),
    emphasis_and_strong_emphasis_443(
        "*foo**\n",
        "<p><em>foo</em>*</p>\n"
    ),
    emphasis_and_strong_emphasis_444(
        "***foo**\n",
        "<p>*<strong>foo</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_445(
        "****foo*\n",
        "<p>***<em>foo</em></p>\n"
    ),
    emphasis_and_strong_emphasis_446(
        "**foo***\n",
        "<p><strong>foo</strong>*</p>\n"
    ),
    emphasis_and_strong_emphasis_447(
        "*foo****\n",
        "<p><em>foo</em>***</p>\n"
    ),
    emphasis_and_strong_emphasis_448(
        "foo ___\n",
        "<p>foo ___</p>\n"
    ),
    emphasis_and_strong_emphasis_449(
        "foo _\\__\n",
        "<p>foo <em>_</em></p>\n"
    ),
    emphasis_and_strong_emphasis_450(
        "foo _*_\n",
        "<p>foo <em>*</em></p>\n"
    ),
    emphasis_and_strong_emphasis_451(
        "foo _____\n",
        "<p>foo _____</p>\n"
    ),
    emphasis_and_strong_emphasis_452(
        "foo __\\___\n",
        "<p>foo <strong>_</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_453(
        "foo __*__\n",
        "<p>foo <strong>*</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_454(
        "__foo_\n",
        "<p>_<em>foo</em></p>\n"
    ),
    emphasis_and_strong_emphasis_455(
        "_foo__\n",
        "<p><em>foo</em>_</p>\n"
    ),
    emphasis_and_strong_emphasis_456(
        "___foo__\n",
        "<p>_<strong>foo</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_457(
        "____foo_\n",
        "<p>___<em>foo</em></p>\n"
    ),
    emphasis_and_strong_emphasis_458(
        "__foo___\n",
        "<p><strong>foo</strong>_</p>\n"
    ),
    emphasis_and_strong_emphasis_459(
        "_foo____\n",
        "<p><em>foo</em>___</p>\n"
    ),
    emphasis_and_strong_emphasis_460(
        "**foo**\n",
        "<p><strong>foo</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_461(
        "*_foo_*\n",
        "<p><em><em>foo</em></em></p>\n"
    ),
    emphasis_and_strong_emphasis_462(
        "__foo__\n",
        "<p><strong>foo</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_463(
        "_*foo*_\n",
        "<p><em><em>foo</em></em></p>\n"
    ),
    emphasis_and_strong_emphasis_464(
        "****foo****\n",
        "<p><strong><strong>foo</strong></strong></p>\n"
    ),
    emphasis_and_strong_emphasis_465(
        "____foo____\n",
        "<p><strong><strong>foo</strong></strong></p>\n"
    ),
    emphasis_and_strong_emphasis_466(
        "******foo******\n",
        "<p><strong><strong><strong>foo</strong></strong></strong></p>\n"
    ),
    emphasis_and_strong_emphasis_467(
        "***foo***\n",
        "<p><em><strong>foo</strong></em></p>\n"
    ),
    emphasis_and_strong_emphasis_468(
        "_____foo_____\n",
        "<p><em><strong><strong>foo</strong></strong></em></p>\n"
    ),
    emphasis_and_strong_emphasis_469(
        "*foo _bar* baz_\n",
        "<p><em>foo _bar</em> baz_</p>\n"
    ),
    emphasis_and_strong_emphasis_470(
        "*foo __bar *baz bim__ bam*\n",
        "<p><em>foo <strong>bar *baz bim</strong> bam</em></p>\n"
    ),
    emphasis_and_strong_emphasis_471(
        "**foo **bar baz**\n",
        "<p>**foo <strong>bar baz</strong></p>\n"
    ),
    emphasis_and_strong_emphasis_472(
        "*foo *bar baz*\n",
        "<p>*foo <em>bar baz</em></p>\n"
    ),
    emphasis_and_strong_emphasis_473(
        "*[bar*](/url)\n",
        "<p>*<a href=\"/url\">bar*</a></p>\n"
    ),
    emphasis_and_strong_emphasis_474(
        "_foo [bar_](/url)\n",
        "<p>_foo <a href=\"/url\">bar_</a></p>\n"
    ),
    emphasis_and_strong_emphasis_475(
        "*<img src=\"foo\" title=\"*\"/>\n",
        "<p>*<img src=\"foo\" title=\"*\"/></p>\n"
    ),
    emphasis_and_strong_emphasis_476(
        "**<a href=\"**\">\n",
        "<p>**<a href=\"**\"></p>\n"
    ),
    emphasis_and_strong_emphasis_477(
        "__<a href=\"__\">\n",
        "<p>__<a href=\"__\"></p>\n"
    ),
    emphasis_and_strong_emphasis_478(
        "*a `*`*\n",
        "<p><em>a <code>*</code></em></p>\n"
    ),
    emphasis_and_strong_emphasis_479(
        "_a `_`_\n",
        "<p><em>a <code>_</code></em></p>\n"
    ),
    emphasis_and_strong_emphasis_480(
        "**a<https://foo.bar/?q=**>\n",
        "<p>**a<a href=\"https://foo.bar/?q=**\">https://foo.bar/?q=**</a></p>\n"
    ),
    emphasis_and_strong_emphasis_481(
        "__a<https://foo.bar/?q=__>\n",
        "<p>__a<a href=\"https://foo.bar/?q=__\">https://foo.bar/?q=__</a></p>\n"
    ),

    // Links
    links_482(
        "[link](/uri \"title\")\n",
        "<p><a href=\"/uri\" title=\"title\">link</a></p>\n"
    ),
    links_483(
        "[link](/uri)\n",
        "<p><a href=\"/uri\">link</a></p>\n"
    ),
    links_484(
        "[](./target.md)\n",
        "<p><a href=\"./target.md\"></a></p>\n"
    ),
    links_485(
        "[link]()\n",
        "<p><a href=\"\">link</a></p>\n"
    ),
    links_486(
        "[link](<>)\n",
        "<p><a href=\"\">link</a></p>\n"
    ),
    links_487(
        "[]()\n",
        "<p><a href=\"\"></a></p>\n"
    ),
    links_488(
        "[link](/my uri)\n",
        "<p>[link](/my uri)</p>\n"
    ),
    links_489(
        "[link](</my uri>)\n",
        "<p><a href=\"/my%20uri\">link</a></p>\n"
    ),
    links_490(
        "[link](foo\nbar)\n",
        "<p>[link](foo\nbar)</p>\n"
    ),
    links_491(
        "[link](<foo\nbar>)\n",
        "<p>[link](<foo\nbar>)</p>\n"
    ),
    links_492(
        "[a](<b)c>)\n",
        "<p><a href=\"b)c\">a</a></p>\n"
    ),
    links_493(
        "[link](<foo\\>)\n",
        "<p>[link](&lt;foo&gt;)</p>\n"
    ),
    links_494(
        "[a](<b)c\n[a](<b)c>\n[a](<b>c)\n",
        "<p>[a](&lt;b)c\n[a](&lt;b)c&gt;\n[a](<b>c)</p>\n"
    ),
    links_495(
        "[link](\\(foo\\))\n",
        "<p><a href=\"(foo)\">link</a></p>\n"
    ),
    links_496(
        "[link](foo(and(bar)))\n",
        "<p><a href=\"foo(and(bar))\">link</a></p>\n"
    ),
    links_497(
        "[link](foo(and(bar))\n",
        "<p>[link](foo(and(bar))</p>\n"
    ),
    links_498(
        "[link](foo\\(and\\(bar\\))\n",
        "<p><a href=\"foo(and(bar)\">link</a></p>\n"
    ),
    links_499(
        "[link](<foo(and(bar)>)\n",
        "<p><a href=\"foo(and(bar)\">link</a></p>\n"
    ),
    links_500(
        "[link](foo\\)\\:)\n",
        "<p><a href=\"foo):\">link</a></p>\n"
    ),
    links_501(
        "[link](#fragment)\n\n[link](https://example.com#fragment)\n\n[link](https://example.com?foo=3#frag)\n",
        "<p><a href=\"#fragment\">link</a></p>\n<p><a href=\"https://example.com#fragment\">link</a></p>\n<p><a href=\"https://example.com?foo=3#frag\">link</a></p>\n"
    ),
    links_502(
        "[link](foo\\bar)\n",
        "<p><a href=\"foo%5Cbar\">link</a></p>\n"
    ),
    links_503(
        "[link](foo%20b&auml;)\n",
        "<p><a href=\"foo%20b%C3%A4\">link</a></p>\n"
    ),
    links_504(
        "[link](\"title\")\n",
        "<p><a href=\"%22title%22\">link</a></p>\n"
    ),
    links_505(
        "[link](/url \"title\")\n[link](/url 'title')\n[link](/url (title))\n",
        "<p><a href=\"/url\" title=\"title\">link</a>\n<a href=\"/url\" title=\"title\">link</a>\n<a href=\"/url\" title=\"title\">link</a></p>\n"
    ),
    links_506(
        "[link](/url \"title \\\"&quot;\")\n",
        "<p><a href=\"/url\" title=\"title &quot;&quot;\">link</a></p>\n"
    ),
    links_507(
        "[link](/url \"title\")\n",
        "<p><a href=\"/url%C2%A0%22title%22\">link</a></p>\n"
    ),
    links_508(
        "[link](/url \"title \"and\" title\")\n",
        "<p>[link](/url &quot;title &quot;and&quot; title&quot;)</p>\n"
    ),
    links_509(
        "[link](/url 'title \"and\" title')\n",
        "<p><a href=\"/url\" title=\"title &quot;and&quot; title\">link</a></p>\n"
    ),
    links_510(
        "[link](   /uri\n  \"title\"  )\n",
        "<p><a href=\"/uri\" title=\"title\">link</a></p>\n"
    ),
    links_511(
        "[link] (/uri)\n",
        "<p>[link] (/uri)</p>\n"
    ),
    links_512(
        "[link [foo [bar]]](/uri)\n",
        "<p><a href=\"/uri\">link [foo [bar]]</a></p>\n"
    ),
    links_513(
        "[link] bar](/uri)\n",
        "<p>[link] bar](/uri)</p>\n"
    ),
    links_514(
        "[link [bar](/uri)\n",
        "<p>[link <a href=\"/uri\">bar</a></p>\n"
    ),
    links_515(
        "[link \\[bar](/uri)\n",
        "<p><a href=\"/uri\">link [bar</a></p>\n"
    ),
    links_516(
        "[link *foo **bar** `#`*](/uri)\n",
        "<p><a href=\"/uri\">link <em>foo <strong>bar</strong> <code>#</code></em></a></p>\n"
    ),
    links_517(
        "[![moon](moon.jpg)](/uri)\n",
        "<p><a href=\"/uri\"><img src=\"moon.jpg\" alt=\"moon\" /></a></p>\n"
    ),
    links_518(
        "[foo [bar](/uri)](/uri)\n",
        "<p>[foo <a href=\"/uri\">bar</a>](/uri)</p>\n"
    ),
    links_519(
        "[foo *[bar [baz](/uri)](/uri)*](/uri)\n",
        "<p>[foo <em>[bar <a href=\"/uri\">baz</a>](/uri)</em>](/uri)</p>\n"
    ),
    links_520(
        "![[[foo](uri1)](uri2)](uri3)\n",
        "<p><img src=\"uri3\" alt=\"[foo](uri2)\" /></p>\n"
    ),
    links_521(
        "*[foo*](/uri)\n",
        "<p>*<a href=\"/uri\">foo*</a></p>\n"
    ),
    links_522(
        "[foo *bar](baz*)\n",
        "<p><a href=\"baz*\">foo *bar</a></p>\n"
    ),
    links_523(
        "*foo [bar* baz]\n",
        "<p><em>foo [bar</em> baz]</p>\n"
    ),
    links_524(
        "[foo <bar attr=\"](baz)\">\n",
        "<p>[foo <bar attr=\"](baz)\"></p>\n"
    ),
    links_525(
        "[foo`](/uri)`\n",
        "<p>[foo<code>](/uri)</code></p>\n"
    ),
    links_526(
        "[foo<https://example.com/?search=](uri)>\n",
        "<p>[foo<a href=\"https://example.com/?search=%5D(uri)\">https://example.com/?search=](uri)</a></p>\n"
    ),
    links_527(
        "[foo][bar]\n\n[bar]: /url \"title\"\n",
        "<p><a href=\"/url\" title=\"title\">foo</a></p>\n"
    ),
    links_528(
        "[link [foo [bar]]][ref]\n\n[ref]: /uri\n",
        "<p><a href=\"/uri\">link [foo [bar]]</a></p>\n"
    ),
    links_529(
        "[link \\[bar][ref]\n\n[ref]: /uri\n",
        "<p><a href=\"/uri\">link [bar</a></p>\n"
    ),
    links_530(
        "[link *foo **bar** `#`*][ref]\n\n[ref]: /uri\n",
        "<p><a href=\"/uri\">link <em>foo <strong>bar</strong> <code>#</code></em></a></p>\n"
    ),
    links_531(
        "[![moon](moon.jpg)][ref]\n\n[ref]: /uri\n",
        "<p><a href=\"/uri\"><img src=\"moon.jpg\" alt=\"moon\" /></a></p>\n"
    ),
    links_532(
        "[foo [bar](/uri)][ref]\n\n[ref]: /uri\n",
        "<p>[foo <a href=\"/uri\">bar</a>]<a href=\"/uri\">ref</a></p>\n"
    ),
    links_533(
        "[foo *bar [baz][ref]*][ref]\n\n[ref]: /uri\n",
        "<p>[foo <em>bar <a href=\"/uri\">baz</a></em>]<a href=\"/uri\">ref</a></p>\n"
    ),
    links_534(
        "*[foo*][ref]\n\n[ref]: /uri\n",
        "<p>*<a href=\"/uri\">foo*</a></p>\n"
    ),
    links_535(
        "[foo *bar][ref]*\n\n[ref]: /uri\n",
        "<p><a href=\"/uri\">foo *bar</a>*</p>\n"
    ),
    links_536(
        "[foo <bar attr=\"][ref]\">\n\n[ref]: /uri\n",
        "<p>[foo <bar attr=\"][ref]\"></p>\n"
    ),
    links_537(
        "[foo`][ref]`\n\n[ref]: /uri\n",
        "<p>[foo<code>][ref]</code></p>\n"
    ),
    links_538(
        "[foo<https://example.com/?search=][ref]>\n\n[ref]: /uri\n",
        "<p>[foo<a href=\"https://example.com/?search=%5D%5Bref%5D\">https://example.com/?search=][ref]</a></p>\n"
    ),
    links_539(
        "[foo][BaR]\n\n[bar]: /url \"title\"\n",
        "<p><a href=\"/url\" title=\"title\">foo</a></p>\n"
    ),
    links_540(
        "[ẞ]\n\n[SS]: /url\n",
        "<p><a href=\"/url\">ẞ</a></p>\n"
    ),
    links_541(
        "[Foo\n  bar]: /url\n\n[Baz][Foo bar]\n",
        "<p><a href=\"/url\">Baz</a></p>\n"
    ),
    links_542(
        "[foo] [bar]\n\n[bar]: /url \"title\"\n",
        "<p>[foo] <a href=\"/url\" title=\"title\">bar</a></p>\n"
    ),
    links_543(
        "[foo]\n[bar]\n\n[bar]: /url \"title\"\n",
        "<p>[foo]\n<a href=\"/url\" title=\"title\">bar</a></p>\n"
    ),
    links_544(
        "[foo]: /url1\n\n[foo]: /url2\n\n[bar][foo]\n",
        "<p><a href=\"/url1\">bar</a></p>\n"
    ),
    links_545(
        "[bar][foo\\!]\n\n[foo!]: /url\n",
        "<p>[bar][foo!]</p>\n"
    ),
    links_546(
        "[foo][ref[]\n\n[ref[]: /uri\n",
        "<p>[foo][ref[]</p>\n<p>[ref[]: /uri</p>\n"
    ),
    links_547(
        "[foo][ref[bar]]\n\n[ref[bar]]: /uri\n",
        "<p>[foo][ref[bar]]</p>\n<p>[ref[bar]]: /uri</p>\n"
    ),
    links_548(
        "[[[foo]]]\n\n[[[foo]]]: /url\n",
        "<p>[[[foo]]]</p>\n<p>[[[foo]]]: /url</p>\n"
    ),
    links_549(
        "[foo][ref\\[]\n\n[ref\\[]: /uri\n",
        "<p><a href=\"/uri\">foo</a></p>\n"
    ),
    links_550(
        "[bar\\\\]: /uri\n\n[bar\\\\]\n",
        "<p><a href=\"/uri\">bar\\</a></p>\n"
    ),
    links_551(
        "[]\n\n[]: /uri\n",
        "<p>[]</p>\n<p>[]: /uri</p>\n"
    ),
    links_552(
        "[\n ]\n\n[\n ]: /uri\n",
        "<p>[\n]</p>\n<p>[\n]: /uri</p>\n"
    ),
    links_553(
        "[foo][]\n\n[foo]: /url \"title\"\n",
        "<p><a href=\"/url\" title=\"title\">foo</a></p>\n"
    ),
    links_554(
        "[*foo* bar][]\n\n[*foo* bar]: /url \"title\"\n",
        "<p><a href=\"/url\" title=\"title\"><em>foo</em> bar</a></p>\n"
    ),
    links_555(
        "[Foo][]\n\n[foo]: /url \"title\"\n",
        "<p><a href=\"/url\" title=\"title\">Foo</a></p>\n"
    ),
    links_556(
        "[foo] \n[]\n\n[foo]: /url \"title\"\n",
        "<p><a href=\"/url\" title=\"title\">foo</a>\n[]</p>\n"
    ),
    links_557(
        "[foo]\n\n[foo]: /url \"title\"\n",
        "<p><a href=\"/url\" title=\"title\">foo</a></p>\n"
    ),
    links_558(
        "[*foo* bar]\n\n[*foo* bar]: /url \"title\"\n",
        "<p><a href=\"/url\" title=\"title\"><em>foo</em> bar</a></p>\n"
    ),
    links_559(
        "[[*foo* bar]]\n\n[*foo* bar]: /url \"title\"\n",
        "<p>[<a href=\"/url\" title=\"title\"><em>foo</em> bar</a>]</p>\n"
    ),
    links_560(
        "[[bar [foo]\n\n[foo]: /url\n",
        "<p>[[bar <a href=\"/url\">foo</a></p>\n"
    ),
    links_561(
        "[Foo]\n\n[foo]: /url \"title\"\n",
        "<p><a href=\"/url\" title=\"title\">Foo</a></p>\n"
    ),
    links_562(
        "[foo] bar\n\n[foo]: /url\n",
        "<p><a href=\"/url\">foo</a> bar</p>\n"
    ),
    links_563(
        "\\[foo]\n\n[foo]: /url \"title\"\n",
        "<p>[foo]</p>\n"
    ),
    links_564(
        "[foo*]: /url\n\n*[foo*]\n",
        "<p>*<a href=\"/url\">foo*</a></p>\n"
    ),
    links_565(
        "[foo][bar]\n\n[foo]: /url1\n[bar]: /url2\n",
        "<p><a href=\"/url2\">foo</a></p>\n"
    ),
    links_566(
        "[foo][]\n\n[foo]: /url1\n",
        "<p><a href=\"/url1\">foo</a></p>\n"
    ),
    links_567(
        "[foo]()\n\n[foo]: /url1\n",
        "<p><a href=\"\">foo</a></p>\n"
    ),
    links_568(
        "[foo](not a link)\n\n[foo]: /url1\n",
        "<p><a href=\"/url1\">foo</a>(not a link)</p>\n"
    ),
    links_569(
        "[foo][bar][baz]\n\n[baz]: /url\n",
        "<p>[foo]<a href=\"/url\">bar</a></p>\n"
    ),
    links_570(
        "[foo][bar][baz]\n\n[baz]: /url1\n[bar]: /url2\n",
        "<p><a href=\"/url2\">foo</a><a href=\"/url1\">baz</a></p>\n"
    ),
    links_571(
        "[foo][bar][baz]\n\n[baz]: /url1\n[foo]: /url2\n",
        "<p>[foo]<a href=\"/url1\">bar</a></p>\n"
    ),

    // Images
    images_572(
        "![foo](/url \"title\")\n",
        "<p><img src=\"/url\" alt=\"foo\" title=\"title\" /></p>\n"
    ),
    images_573(
        "![foo *bar*]\n\n[foo *bar*]: train.jpg \"train & tracks\"\n",
        "<p><img src=\"train.jpg\" alt=\"foo bar\" title=\"train &amp; tracks\" /></p>\n"
    ),
    images_574(
        "![foo ![bar](/url)](/url2)\n",
        "<p><img src=\"/url2\" alt=\"foo bar\" /></p>\n"
    ),
    images_575(
        "![foo [bar](/url)](/url2)\n",
        "<p><img src=\"/url2\" alt=\"foo bar\" /></p>\n"
    ),
    images_576(
        "![foo *bar*][]\n\n[foo *bar*]: train.jpg \"train & tracks\"\n",
        "<p><img src=\"train.jpg\" alt=\"foo bar\" title=\"train &amp; tracks\" /></p>\n"
    ),
    images_577(
        "![foo *bar*][foobar]\n\n[FOOBAR]: train.jpg \"train & tracks\"\n",
        "<p><img src=\"train.jpg\" alt=\"foo bar\" title=\"train &amp; tracks\" /></p>\n"
    ),
    images_578(
        "![foo](train.jpg)\n",
        "<p><img src=\"train.jpg\" alt=\"foo\" /></p>\n"
    ),
    images_579(
        "My ![foo bar](/path/to/train.jpg  \"title\"   )\n",
        "<p>My <img src=\"/path/to/train.jpg\" alt=\"foo bar\" title=\"title\" /></p>\n"
    ),
    images_580(
        "![foo](<url>)\n",
        "<p><img src=\"url\" alt=\"foo\" /></p>\n"
    ),
    images_581(
        "![](/url)\n",
        "<p><img src=\"/url\" alt=\"\" /></p>\n"
    ),
    images_582(
        "![foo][bar]\n\n[bar]: /url\n",
        "<p><img src=\"/url\" alt=\"foo\" /></p>\n"
    ),
    images_583(
        "![foo][bar]\n\n[BAR]: /url\n",
        "<p><img src=\"/url\" alt=\"foo\" /></p>\n"
    ),
    images_584(
        "![foo][]\n\n[foo]: /url \"title\"\n",
        "<p><img src=\"/url\" alt=\"foo\" title=\"title\" /></p>\n"
    ),
    images_585(
        "![*foo* bar][]\n\n[*foo* bar]: /url \"title\"\n",
        "<p><img src=\"/url\" alt=\"foo bar\" title=\"title\" /></p>\n"
    ),
    images_586(
        "![Foo][]\n\n[foo]: /url \"title\"\n",
        "<p><img src=\"/url\" alt=\"Foo\" title=\"title\" /></p>\n"
    ),
    images_587(
        "![foo] \n[]\n\n[foo]: /url \"title\"\n",
        "<p><img src=\"/url\" alt=\"foo\" title=\"title\" />\n[]</p>\n"
    ),
    images_588(
        "![foo]\n\n[foo]: /url \"title\"\n",
        "<p><img src=\"/url\" alt=\"foo\" title=\"title\" /></p>\n"
    ),
    images_589(
        "![*foo* bar]\n\n[*foo* bar]: /url \"title\"\n",
        "<p><img src=\"/url\" alt=\"foo bar\" title=\"title\" /></p>\n"
    ),
    images_590(
        "![[foo]]\n\n[[foo]]: /url \"title\"\n",
        "<p>![[foo]]</p>\n<p>[[foo]]: /url &quot;title&quot;</p>\n"
    ),
    images_591(
        "![Foo]\n\n[foo]: /url \"title\"\n",
        "<p><img src=\"/url\" alt=\"Foo\" title=\"title\" /></p>\n"
    ),
    images_592(
        "!\\[foo]\n\n[foo]: /url \"title\"\n",
        "<p>![foo]</p>\n"
    ),
    images_593(
        "\\![foo]\n\n[foo]: /url \"title\"\n",
        "<p>!<a href=\"/url\" title=\"title\">foo</a></p>\n"
    ),

    // Autolinks
    autolinks_594(
        "<http://foo.bar.baz>\n",
        "<p><a href=\"http://foo.bar.baz\">http://foo.bar.baz</a></p>\n"
    ),
    autolinks_595(
        "<https://foo.bar.baz/test?q=hello&id=22&boolean>\n",
        "<p><a href=\"https://foo.bar.baz/test?q=hello&amp;id=22&amp;boolean\">https://foo.bar.baz/test?q=hello&amp;id=22&amp;boolean</a></p>\n"
    ),
    autolinks_596(
        "<irc://foo.bar:2233/baz>\n",
        "<p><a href=\"irc://foo.bar:2233/baz\">irc://foo.bar:2233/baz</a></p>\n"
    ),
    autolinks_597(
        "<MAILTO:FOO@BAR.BAZ>\n",
        "<p><a href=\"MAILTO:FOO@BAR.BAZ\">MAILTO:FOO@BAR.BAZ</a></p>\n"
    ),
    autolinks_598(
        "<a+b+c:d>\n",
        "<p><a href=\"a+b+c:d\">a+b+c:d</a></p>\n"
    ),
    autolinks_599(
        "<made-up-scheme://foo,bar>\n",
        "<p><a href=\"made-up-scheme://foo,bar\">made-up-scheme://foo,bar</a></p>\n"
    ),
    autolinks_600(
        "<https://../>\n",
        "<p><a href=\"https://../\">https://../</a></p>\n"
    ),
    autolinks_601(
        "<localhost:5001/foo>\n",
        "<p><a href=\"localhost:5001/foo\">localhost:5001/foo</a></p>\n"
    ),
    autolinks_602(
        "<https://foo.bar/baz bim>\n",
        "<p>&lt;https://foo.bar/baz bim&gt;</p>\n"
    ),
    autolinks_603(
        "<https://example.com/\\[\\>\n",
        "<p><a href=\"https://example.com/%5C%5B%5C\">https://example.com/\\[\\</a></p>\n"
    ),
    autolinks_604(
        "<foo@bar.example.com>\n",
        "<p><a href=\"mailto:foo@bar.example.com\">foo@bar.example.com</a></p>\n"
    ),
    autolinks_605(
        "<foo+special@Bar.baz-bar0.com>\n",
        "<p><a href=\"mailto:foo+special@Bar.baz-bar0.com\">foo+special@Bar.baz-bar0.com</a></p>\n"
    ),
    autolinks_606(
        "<foo\\+@bar.example.com>\n",
        "<p>&lt;foo+@bar.example.com&gt;</p>\n"
    ),
    autolinks_607(
        "<>\n",
        "<p>&lt;&gt;</p>\n"
    ),
    autolinks_608(
        "< https://foo.bar >\n",
        "<p>&lt; https://foo.bar &gt;</p>\n"
    ),
    autolinks_609(
        "<m:abc>\n",
        "<p>&lt;m:abc&gt;</p>\n"
    ),
    autolinks_610(
        "<foo.bar.baz>\n",
        "<p>&lt;foo.bar.baz&gt;</p>\n"
    ),
    autolinks_611(
        "https://example.com\n",
        "<p>https://example.com</p>\n"
    ),
    autolinks_612(
        "foo@bar.example.com\n",
        "<p>foo@bar.example.com</p>\n"
    ),

    // Raw HTML
    raw_html_613(
        "<a><bab><c2c>\n",
        "<p><a><bab><c2c></p>\n"
    ),
    raw_html_614(
        "<a/><b2/>\n",
        "<p><a/><b2/></p>\n"
    ),
    raw_html_615(
        "<a  /><b2\ndata=\"foo\" >\n",
        "<p><a  /><b2\ndata=\"foo\" ></p>\n"
    ),
    raw_html_616(
        "<a foo=\"bar\" bam = 'baz <em>\"</em>'\n_boolean zoop:33=zoop:33 />\n",
        "<p><a foo=\"bar\" bam = 'baz <em>\"</em>'\n_boolean zoop:33=zoop:33 /></p>\n"
    ),
    raw_html_617(
        "Foo <responsive-image src=\"foo.jpg\" />\n",
        "<p>Foo <responsive-image src=\"foo.jpg\" /></p>\n"
    ),
    raw_html_618(
        "<33> <__>\n",
        "<p>&lt;33&gt; &lt;__&gt;</p>\n"
    ),
    raw_html_619(
        "<a h*#ref=\"hi\">\n",
        "<p>&lt;a h*#ref=&quot;hi&quot;&gt;</p>\n"
    ),
    raw_html_620(
        "<a href=\"hi'> <a href=hi'>\n",
        "<p>&lt;a href=&quot;hi'&gt; &lt;a href=hi'&gt;</p>\n"
    ),
    raw_html_621(
        "< a><\nfoo><bar/ >\n<foo bar=baz\nbim!bop />\n",
        "<p>&lt; a&gt;&lt;\nfoo&gt;&lt;bar/ &gt;\n&lt;foo bar=baz\nbim!bop /&gt;</p>\n"
    ),
    raw_html_622(
        "<a href='bar'title=title>\n",
        "<p>&lt;a href='bar'title=title&gt;</p>\n"
    ),
    raw_html_623(
        "</a></foo >\n",
        "<p></a></foo ></p>\n"
    ),
    raw_html_624(
        "</a href=\"foo\">\n",
        "<p>&lt;/a href=&quot;foo&quot;&gt;</p>\n"
    ),
    raw_html_625(
        "foo <!-- this is a --\ncomment - with hyphens -->\n",
        "<p>foo <!-- this is a --\ncomment - with hyphens --></p>\n"
    ),
    raw_html_626(
        "foo <!--> foo -->\n\nfoo <!---> foo -->\n",
        "<p>foo <!--> foo --&gt;</p>\n<p>foo <!---> foo --&gt;</p>\n"
    ),
    raw_html_627(
        "foo <?php echo $a; ?>\n",
        "<p>foo <?php echo $a; ?></p>\n"
    ),
    raw_html_628(
        "foo <!ELEMENT br EMPTY>\n",
        "<p>foo <!ELEMENT br EMPTY></p>\n"
    ),
    raw_html_629(
        "foo <![CDATA[>&<]]>\n",
        "<p>foo <![CDATA[>&<]]></p>\n"
    ),
    raw_html_630(
        "foo <a href=\"&ouml;\">\n",
        "<p>foo <a href=\"&ouml;\"></p>\n"
    ),
    raw_html_631(
        "foo <a href=\"\\*\">\n",
        "<p>foo <a href=\"\\*\"></p>\n"
    ),
    raw_html_632(
        "<a href=\"\\\"\">\n",
        "<p>&lt;a href=&quot;&quot;&quot;&gt;</p>\n"
    ),

    // Hard line breaks
    hard_line_breaks_633(
        "foo  \nbaz\n",
        "<p>foo<br />\nbaz</p>\n"
    ),
    hard_line_breaks_634(
        "foo\\\nbaz\n",
        "<p>foo<br />\nbaz</p>\n"
    ),
    hard_line_breaks_635(
        "foo       \nbaz\n",
        "<p>foo<br />\nbaz</p>\n"
    ),
    hard_line_breaks_636(
        "foo  \n     bar\n",
        "<p>foo<br />\nbar</p>\n"
    ),
    hard_line_breaks_637(
        "foo\\\n     bar\n",
        "<p>foo<br />\nbar</p>\n"
    ),
    hard_line_breaks_638(
        "*foo  \nbar*\n",
        "<p><em>foo<br />\nbar</em></p>\n"
    ),
    hard_line_breaks_639(
        "*foo\\\nbar*\n",
        "<p><em>foo<br />\nbar</em></p>\n"
    ),
    hard_line_breaks_640(
        "`code  \nspan`\n",
        "<p><code>code   span</code></p>\n"
    ),
    hard_line_breaks_641(
        "`code\\\nspan`\n",
        "<p><code>code\\ span</code></p>\n"
    ),
    hard_line_breaks_642(
        "<a href=\"foo  \nbar\">\n",
        "<p><a href=\"foo  \nbar\"></p>\n"
    ),
    hard_line_breaks_643(
        "<a href=\"foo\\\nbar\">\n",
        "<p><a href=\"foo\\\nbar\"></p>\n"
    ),
    hard_line_breaks_644(
        "foo\\\n",
        "<p>foo\\</p>\n"
    ),
    hard_line_breaks_645(
        "foo  \n",
        "<p>foo</p>\n"
    ),
    hard_line_breaks_646(
        "### foo\\\n",
        "<h3>foo\\</h3>\n"
    ),
    hard_line_breaks_647(
        "### foo  \n",
        "<h3>foo</h3>\n"
    ),

    // Soft line breaks
    soft_line_breaks_648(
        "foo\nbaz\n",
        "<p>foo\nbaz</p>\n"
    ),
    soft_line_breaks_649(
        "foo \n baz\n",
        "<p>foo\nbaz</p>\n"
    ),

    // Textual content
    textual_content_650(
        "hello $.;'there\n",
        "<p>hello $.;'there</p>\n"
    ),
    textual_content_651(
        "Foo χρῆν\n",
        "<p>Foo χρῆν</p>\n"
    ),
    textual_content_652(
        "Multiple     spaces\n",
        "<p>Multiple     spaces</p>\n"
    ),
}
