use proc_macro::{Delimiter, Group, Ident, TokenStream, TokenTree};

/// Generates a separate test for each `#[case(...)]` or `#[case::name(...)]`
/// on a synchronous test function. Case arguments are ordinary Rust expressions.
///
/// ```
/// use stratalith_macros::rstest;
///
/// #[rstest]
/// #[case::empty("", 0)]
/// #[case::unicode("aé", 2)]
/// fn character_count(#[case] text: &str, #[case] expected: usize) {
///     let count = text.chars().count();
///
///     assert_eq!(count, expected);
/// }
/// ```
#[proc_macro_attribute]
pub fn rstest(options: TokenStream, item: TokenStream) -> TokenStream {
    if !options.is_empty() {
        return compile_error("#[rstest] takes no arguments");
    }
    expand(item).unwrap_or_else(compile_error)
}

fn compile_error(message: &str) -> TokenStream {
    format!("compile_error!({message:?});").parse().unwrap()
}

struct Case {
    name: Option<Ident>,
    arguments: Group,
}

impl Case {
    fn parse(attribute: Group) -> Result<Self, &'static str> {
        let tokens: Vec<_> = attribute.stream().into_iter().collect();
        let (name, arguments) = match tokens.as_slice() {
            [case, TokenTree::Group(args)] if is_ident(case, "case") => (None, args),
            [
                case,
                colon,
                colon2,
                TokenTree::Ident(name),
                TokenTree::Group(args),
            ] if is_ident(case, "case") && is_punct(colon, ':') && is_punct(colon2, ':') => {
                (Some(name.clone()), args)
            }
            _ => return Err("expected #[case(...)] or #[case::name(...)]"),
        };
        if arguments.delimiter() != Delimiter::Parenthesis {
            return Err("expected parenthesized case arguments");
        }
        Ok(Self {
            name,
            arguments: arguments.clone(),
        })
    }
}

fn is_ident(token: &TokenTree, expected: &str) -> bool {
    matches!(token, TokenTree::Ident(ident) if ident.to_string() == expected)
}

fn is_punct(token: &TokenTree, expected: char) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.as_char() == expected)
}

fn strip_case_markers(parameters: Group) -> Group {
    let mut tokens = parameters.stream().into_iter().peekable();
    let mut stripped = TokenStream::new();
    while let Some(token) = tokens.next() {
        if is_punct(&token, '#')
            && let Some(TokenTree::Group(attr)) = tokens.peek()
            && attr.delimiter() == Delimiter::Bracket
            && attr.stream().to_string() == "case"
        {
            tokens.next();
            continue;
        }
        stripped.extend([token]);
    }
    let mut result = Group::new(Delimiter::Parenthesis, stripped);
    result.set_span(parameters.span());
    result
}

fn expand(item: TokenStream) -> Result<TokenStream, &'static str> {
    let mut tokens = item.into_iter().peekable();
    let mut cases = Vec::new();
    while tokens.peek().is_some_and(|token| is_punct(token, '#')) {
        tokens.next();
        let Some(TokenTree::Group(attribute)) = tokens.next() else {
            return Err("expected a case attribute");
        };
        cases.push(Case::parse(attribute)?);
    }
    if cases.is_empty() {
        return Err("#[rstest] requires at least one case");
    }

    let Some(keyword) = tokens.next().filter(|token| is_ident(token, "fn")) else {
        return Err("expected a test function after the cases");
    };
    let Some(TokenTree::Ident(function)) = tokens.next() else {
        return Err("expected a test function name");
    };
    let Some(TokenTree::Group(parameters)) = tokens.next() else {
        return Err("expected test function parameters");
    };
    if parameters.delimiter() != Delimiter::Parenthesis {
        return Err("expected parenthesized test function parameters");
    }

    let mut output = TokenStream::from_iter([
        keyword,
        function.clone().into(),
        strip_case_markers(parameters).into(),
    ]);
    output.extend(tokens);

    let mut tests: TokenStream = "use super::*;".parse().unwrap();
    let width = cases.len().to_string().len();
    for (index, case) in cases.into_iter().enumerate() {
        let mut name = format!("case_{:0width$}", index + 1);
        if let Some(label) = case.name {
            name.push('_');
            name.push_str(label.to_string().trim_start_matches("r#"));
        }
        tests.extend(
            format!("#[test] fn {name}()")
                .parse::<TokenStream>()
                .unwrap(),
        );
        let mut call: TokenStream = "super::".parse().unwrap();
        call.extend([TokenTree::Ident(function.clone()), case.arguments.into()]);
        tests.extend([TokenTree::Group(Group::new(Delimiter::Brace, call))]);
    }
    output.extend("mod".parse::<TokenStream>().unwrap());
    output.extend([
        TokenTree::Ident(function),
        Group::new(Delimiter::Brace, tests).into(),
    ]);
    Ok(output)
}
