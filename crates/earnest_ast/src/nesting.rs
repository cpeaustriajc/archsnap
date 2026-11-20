use earnest_diagnostics::{Place, Problem};

/// How many levels deep a model file may nest brackets or prefix operators, and how deep any
/// expression of a model may be. A reader refuses a file past it, and so does the lowering.
pub const MOST_NESTING: usize = 1_000;

const OPENERS: &[u8] = b"([{";
const CLOSERS: &[u8] = b")]}";
const PREFIXES: &[u8] = b"!-+~&*";
const LONGEST_ESCAPE: usize = 12;

/// The sentence that refuses a model file or an expression that nests deeper than
/// [`MOST_NESTING`].
pub const TOO_DEEP: &str = "the checker reads an expression nested up to 1000 levels deep, and a \
    model file that nests more is refused instead of risking a crash that would lose the results \
    of every other model in the run";

/// Which language's comments and quoted text [`nesting_problem`] skips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Quoting {
    /// Quotes `'` and `"`, template literals with `${}` inside, and comments that do not nest.
    TypeScript,
    /// Quotes `"`, raw strings, byte and C strings, character literals, and comments that nest.
    Rust,
}

fn place_at(source: &str, offset: usize) -> Place {
    let before = source.get(..offset).unwrap_or_default();
    let line = before.matches('\n').count().saturating_add(1);
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |last| last.chars().count())
        .saturating_add(1);
    Place {
        line: u32::try_from(line).unwrap_or(u32::MAX),
        column: u32::try_from(column).unwrap_or(u32::MAX),
    }
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn skip_line(bytes: &[u8], at: usize) -> usize {
    bytes
        .get(at..)
        .and_then(|rest| rest.iter().position(|byte| *byte == b'\n'))
        .map_or(bytes.len(), |end| at.saturating_add(end))
}

fn skip_block(bytes: &[u8], at: usize, quoting: Quoting) -> usize {
    let mut depth = 1_usize;
    let mut here = at.saturating_add(2);
    while depth > 0 {
        match (bytes.get(here), bytes.get(here.saturating_add(1))) {
            (None, _) => return bytes.len(),
            (Some(b'*'), Some(b'/')) => {
                depth = depth.saturating_sub(1);
                here = here.saturating_add(2);
            }
            (Some(b'/'), Some(b'*')) if quoting == Quoting::Rust => {
                depth = depth.saturating_add(1);
                here = here.saturating_add(2);
            }
            _ => here = here.saturating_add(1),
        }
    }
    here
}

fn skip_comment(bytes: &[u8], at: usize, quoting: Quoting) -> Option<usize> {
    match (bytes.get(at), bytes.get(at.saturating_add(1))) {
        (Some(b'/'), Some(b'/')) => Some(skip_line(bytes, at)),
        (Some(b'/'), Some(b'*')) => Some(skip_block(bytes, at, quoting)),
        _ => None,
    }
}

fn skip_escaped(bytes: &[u8], from: usize, close: u8, stop_at: Option<u8>) -> usize {
    let mut here = from;
    while let Some(byte) = bytes.get(here) {
        if *byte == b'\\' {
            here = here.saturating_add(2);
        } else if *byte == close {
            return here.saturating_add(1);
        } else if stop_at == Some(*byte) {
            return here;
        } else {
            here = here.saturating_add(1);
        }
    }
    bytes.len()
}

fn raw_string_end(bytes: &[u8], at: usize) -> Option<usize> {
    let mut quote = at.saturating_add(1);
    while bytes.get(quote) == Some(&b'#') {
        quote = quote.saturating_add(1);
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote.saturating_sub(at.saturating_add(1));
    let mut here = quote.saturating_add(1);
    while let Some(byte) = bytes.get(here) {
        let after = here.saturating_add(1);
        let closing = bytes
            .get(after..after.saturating_add(hashes))
            .is_some_and(|tail| tail.iter().all(|found| *found == b'#'));
        if *byte == b'"' && closing {
            return Some(after.saturating_add(hashes));
        }
        here = after;
    }
    Some(bytes.len())
}

fn starts_a_raw_string(bytes: &[u8], at: usize) -> bool {
    let before = |index: usize| index.checked_sub(1).and_then(|found| bytes.get(found));
    let fresh = match before(at) {
        None => true,
        Some(b'b' | b'c') => !before(at.saturating_sub(1)).is_some_and(|byte| is_word(*byte)),
        Some(byte) => !is_word(*byte),
    };
    fresh && bytes.get(at) == Some(&b'r')
}

fn character_end(source: &str, at: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let after = at.saturating_add(1);
    if bytes.get(after) == Some(&b'\\') {
        let mut here = after.saturating_add(2);
        while let Some(byte) = bytes.get(here) {
            if *byte == b'\'' {
                return Some(here.saturating_add(1));
            }
            if here.saturating_sub(after) > LONGEST_ESCAPE {
                return None;
            }
            here = here.saturating_add(1);
        }
        return None;
    }
    let letter = source.get(after..)?.chars().next()?;
    let closing = after.saturating_add(letter.len_utf8());
    (bytes.get(closing) == Some(&b'\'')).then_some(closing.saturating_add(1))
}

fn skip_quoted(source: &str, at: usize, quoting: Quoting) -> Option<usize> {
    let bytes = source.as_bytes();
    let byte = *bytes.get(at)?;
    let after = at.saturating_add(1);
    match (quoting, byte) {
        (Quoting::TypeScript, b'\'' | b'"') => Some(skip_escaped(bytes, after, byte, Some(b'\n'))),
        (Quoting::Rust, b'"') => Some(skip_escaped(bytes, after, b'"', None)),
        (Quoting::Rust, b'r') if starts_a_raw_string(bytes, at) => raw_string_end(bytes, at),
        (Quoting::Rust, b'\'') => character_end(source, at),
        _ => None,
    }
}

/// `source` with each comment replaced by one space, and its quoted text kept as it is.
pub(crate) fn without_comments(source: &str, quoting: Quoting) -> String {
    let bytes = source.as_bytes();
    let mut kept = String::with_capacity(source.len());
    let mut from = 0_usize;
    let mut at = 0_usize;
    while at < bytes.len() {
        if let Some(after) = skip_comment(bytes, at, quoting) {
            kept.push_str(source.get(from..at).unwrap_or_default());
            kept.push(' ');
            at = after;
            from = after;
        } else if let Some(after) = skip_quoted(source, at, quoting) {
            at = after.max(at.saturating_add(1));
        } else if bytes.get(at) == Some(&b'`') && quoting == Quoting::TypeScript {
            at = skip_escaped(bytes, at.saturating_add(1), b'`', None);
        } else {
            at = at.saturating_add(1);
        }
    }
    kept.push_str(source.get(from..).unwrap_or_default());
    kept
}

#[derive(Default)]
struct Depth {
    brackets: usize,
    run: usize,
    substitutions: Vec<usize>,
}

impl Depth {
    fn too_deep(&self) -> bool {
        self.brackets > MOST_NESTING || self.run > MOST_NESTING
    }

    fn template(&mut self, bytes: &[u8], from: usize) -> usize {
        self.run = 0;
        let mut here = from;
        while let Some(byte) = bytes.get(here) {
            match (byte, bytes.get(here.saturating_add(1))) {
                (b'\\', _) => here = here.saturating_add(2),
                (b'`', _) => return here.saturating_add(1),
                (b'$', Some(b'{')) => {
                    self.brackets = self.brackets.saturating_add(1);
                    self.substitutions.push(self.brackets);
                    return here.saturating_add(2);
                }
                _ => here = here.saturating_add(1),
            }
        }
        bytes.len()
    }

    fn closes_substitution(&mut self, byte: u8) -> bool {
        let closes = byte == b'}' && self.substitutions.last() == Some(&self.brackets);
        if closes {
            self.substitutions.pop();
        }
        closes
    }

    fn mark(&mut self, byte: u8) {
        if OPENERS.contains(&byte) {
            self.brackets = self.brackets.saturating_add(1);
            self.run = 0;
        } else if CLOSERS.contains(&byte) {
            self.brackets = self.brackets.saturating_sub(1);
            self.run = 0;
        } else if PREFIXES.contains(&byte) {
            self.run = self.run.saturating_add(1);
        } else if !byte.is_ascii_whitespace() {
            self.run = 0;
        }
    }
}

/// Reports the first place where `source` nests brackets, or stacks prefix operators such as
/// `!` or `-`, more than [`MOST_NESTING`] deep, as a problem in `file`.
///
/// The parsers a reader calls recurse once for each level, so a reader calls this before it
/// parses and refuses the file when it gives a problem. It looks at the text only. Comments
/// and quoted text do not count, as `quoting` says them: a string, a template literal (not
/// what is inside its `${}`), a Rust raw string or a character literal can hold any number of
/// brackets and dashes without nesting anything. A bracket in a regular expression literal is
/// counted.
#[must_use]
pub fn nesting_problem(file: &str, source: &str, quoting: Quoting) -> Option<Problem> {
    let bytes = source.as_bytes();
    let mut depth = Depth::default();
    let mut at = 0_usize;
    while let Some(byte) = bytes.get(at).copied() {
        if let Some(after) = skip_comment(bytes, at, quoting) {
            at = after;
            continue;
        }
        if let Some(after) = skip_quoted(source, at, quoting) {
            depth.run = 0;
            at = after.max(at.saturating_add(1));
            continue;
        }
        if byte == b'`' && quoting == Quoting::TypeScript {
            at = depth.template(bytes, at.saturating_add(1));
            continue;
        }
        let closes = quoting == Quoting::TypeScript && depth.closes_substitution(byte);
        depth.mark(byte);
        if depth.too_deep() {
            let shown = source.get(at..).and_then(|rest| rest.chars().next());
            let feature = format!("`{}`", shown.unwrap_or_default());
            return Some(Problem::new(file, place_at(source, at), &feature, TOO_DEEP));
        }
        at = at.saturating_add(1);
        if closes {
            at = depth.template(bytes, at);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{MOST_NESTING, Quoting, nesting_problem};

    fn typescript(source: &str) -> bool {
        nesting_problem("a.ts", source, Quoting::TypeScript).is_some()
    }

    fn rust(source: &str) -> bool {
        nesting_problem("a.rs", source, Quoting::Rust).is_some()
    }

    #[test]
    fn brackets_up_to_the_limit_are_read() {
        let source = format!("{}1{}", "(".repeat(MOST_NESTING), ")".repeat(MOST_NESTING));
        assert_eq!(
            nesting_problem("a.ts", &source, Quoting::TypeScript),
            None,
            "exactly the limit"
        );
    }

    #[test]
    fn one_bracket_more_is_refused_at_that_bracket() {
        let source = format!(
            "ok\n  {}1{}",
            "[".repeat(MOST_NESTING + 1),
            "]".repeat(MOST_NESTING + 1)
        );
        let problem =
            nesting_problem("a.ts", &source, Quoting::TypeScript).expect("the file nests too deep");
        assert_eq!(
            (problem.line, problem.column, problem.feature.as_str()),
            (2, 1003, "`[`"),
            "the first bracket past the limit, with its place"
        );
        assert_eq!(problem.file, "a.ts", "the file is named");
    }

    #[test]
    fn a_run_of_prefix_operators_counts_through_white_space() {
        let source = format!("x = {}y", "! ".repeat(MOST_NESTING + 1));
        assert!(rust(&source), "a thousand and one negations nest too deep");
        let flat = format!("x = {}y", "a + ".repeat(MOST_NESTING * 3));
        assert!(!rust(&flat), "a sum is not a run of prefix operators");
    }

    #[test]
    fn a_comment_does_not_count() {
        let line = format!("// {}\nx", "(".repeat(MOST_NESTING + 5));
        let block = format!("/* {} */ x", "-".repeat(MOST_NESTING + 5));
        assert!(!typescript(&line), "a line comment");
        assert!(!typescript(&block), "a block comment");
    }

    #[test]
    fn a_rust_block_comment_nests_and_a_typescript_one_does_not() {
        let inner = "(".repeat(MOST_NESTING + 5);
        let source = format!("/* a /* b */ {inner} */ x");
        assert!(
            !rust(&source),
            "the comment ends at the second `*/` in Rust"
        );
        assert!(
            typescript(&source),
            "the comment ends at the first `*/` in TypeScript"
        );
    }

    #[test]
    fn brackets_that_close_do_not_add_up() {
        let source = "(a)".repeat(MOST_NESTING * 2);
        assert!(!typescript(&source), "side by side is not nested");
    }

    #[test]
    fn text_in_quotes_does_not_count_in_either_language() {
        let dashes = "-".repeat(MOST_NESTING + 1);
        let brackets = "(".repeat(MOST_NESTING + 1);
        for source in [
            format!("const a = \"one up {dashes}\";"),
            format!("const a = 'one up {brackets}';"),
            format!("const a = \"\\n{dashes}\";"),
            format!("const a = `{brackets}`;"),
            format!("const a = \"it\\\"s {brackets}\";"),
        ] {
            assert!(!typescript(&source), "{}", &source[..30]);
        }
        for source in [
            format!("let a = \"one up {dashes}\";"),
            format!("let a = r\"{brackets}\";"),
            format!("let a = r#\"a \" {brackets} \"#;"),
            format!("let a = br##\"{brackets}\"##;"),
            format!("let a = b\"{dashes}\";"),
            format!("let a = c\"{dashes}\";"),
            format!("let a = \"a\\\"{brackets}\";"),
        ] {
            assert!(!rust(&source), "{}", &source[..30]);
        }
    }

    #[test]
    fn a_character_literal_is_text_and_a_lifetime_is_not() {
        assert!(
            !rust(&format!("let a = '(';{}", ")".repeat(3))),
            "a bracket as a character"
        );
        assert!(
            !rust(&format!("let a = '\\'';{}", "(".repeat(5))),
            "an escaped quote"
        );
        let open = "(".repeat(MOST_NESTING + 1);
        assert!(
            rust(&format!("fn f<'a>(x: &'a u8) {open}")),
            "a lifetime is not a quote"
        );
        assert!(
            rust(&format!("fn f<'a, 'b>(x: &'a u8, y: &'b u8) {open}")),
            "two lifetimes do not make a quote between them"
        );
    }

    #[test]
    fn code_after_a_quote_still_counts() {
        let open = "(".repeat(MOST_NESTING + 1);
        assert!(
            typescript(&format!("const a = \"x\"; {open}")),
            "after a string"
        );
        assert!(
            typescript(&format!("const a = `x`; {open}")),
            "after a template"
        );
        assert!(
            rust(&format!("let a = r#\"x\"#; {open}")),
            "after a raw string"
        );
        assert!(
            typescript(&format!("const a = \"(\n{open}")),
            "after an unterminated string"
        );
    }

    #[test]
    fn a_template_substitution_is_code_and_its_text_is_not() {
        let open = "(".repeat(MOST_NESTING + 1);
        assert!(
            typescript(&format!("const a = `${{{open}}}`;")),
            "brackets in `${{}}` nest"
        );
        assert!(
            !typescript(&format!("const a = `a ${{ b }} {open} ${{ c }}`;")),
            "text between two substitutions"
        );
        assert!(
            !typescript(&format!("const a = `${{ `{open}` }}`;")),
            "a template inside a substitution"
        );
        assert!(
            typescript(&format!("const a = `${{ `${{{open}}}` }}`;")),
            "a substitution inside a template inside a substitution"
        );
    }
}
