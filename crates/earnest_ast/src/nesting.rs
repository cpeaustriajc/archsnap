use earnest_diagnostics::{Place, Problem};

/// How many levels deep a model file may nest brackets or prefix operators, and how deep any
/// expression of a model may be. A reader refuses a file past it, and so does the lowering.
pub const MOST_NESTING: usize = 1_000;

const OPENERS: &[u8] = b"([{";
const CLOSERS: &[u8] = b")]}";
const PREFIXES: &[u8] = b"!-+~&*";

/// The sentence that refuses a model file or an expression that nests deeper than
/// [`MOST_NESTING`].
pub const TOO_DEEP: &str = "the checker reads an expression nested up to 1000 levels deep, and a \
    model file that nests more is refused instead of risking a crash that would lose the results \
    of every other model in the run";

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

fn skip_comment(bytes: &[u8], at: usize) -> Option<usize> {
    match (bytes.get(at), bytes.get(at.saturating_add(1))) {
        (Some(b'/'), Some(b'/')) => Some(
            bytes
                .get(at..)
                .and_then(|rest| rest.iter().position(|byte| *byte == b'\n'))
                .map_or(bytes.len(), |end| at.saturating_add(end)),
        ),
        (Some(b'/'), Some(b'*')) => {
            let inside = bytes.get(at.saturating_add(2)..).unwrap_or_default();
            let closed = inside.windows(2).position(|pair| pair == b"*/");
            Some(closed.map_or(bytes.len(), |end| at.saturating_add(4).saturating_add(end)))
        }
        _ => None,
    }
}

/// Reports the first place where `source` nests brackets, or stacks prefix operators such as
/// `!` or `-`, more than [`MOST_NESTING`] deep, as a problem in `file`.
///
/// The parsers a reader calls recurse once for each level, so a reader calls this before it
/// parses and refuses the file when it gives a problem. It looks at the text only, outside
/// comments, and does not tell a bracket in a text from one in code, so it can only be wrong
/// for a text that holds more than a thousand brackets that are never closed.
#[must_use]
pub fn nesting_problem(file: &str, source: &str) -> Option<Problem> {
    let bytes = source.as_bytes();
    let mut depth = 0_usize;
    let mut run = 0_usize;
    let mut at = 0_usize;
    while let Some(byte) = bytes.get(at) {
        if let Some(after) = skip_comment(bytes, at) {
            at = after;
            continue;
        }
        if OPENERS.contains(byte) {
            depth = depth.saturating_add(1);
            run = 0;
        } else if CLOSERS.contains(byte) {
            depth = depth.saturating_sub(1);
            run = 0;
        } else if PREFIXES.contains(byte) {
            run = run.saturating_add(1);
        } else if !byte.is_ascii_whitespace() {
            run = 0;
        }
        if depth > MOST_NESTING || run > MOST_NESTING {
            let shown = source.get(at..).and_then(|rest| rest.chars().next());
            let feature = format!("`{}`", shown.unwrap_or_default());
            return Some(Problem::new(file, place_at(source, at), &feature, TOO_DEEP));
        }
        at = at.saturating_add(1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{MOST_NESTING, nesting_problem};

    #[test]
    fn brackets_up_to_the_limit_are_read() {
        let source = format!("{}1{}", "(".repeat(MOST_NESTING), ")".repeat(MOST_NESTING));
        assert_eq!(nesting_problem("a.ts", &source), None, "exactly the limit");
    }

    #[test]
    fn one_bracket_more_is_refused_at_that_bracket() {
        let source = format!(
            "ok\n  {}1{}",
            "[".repeat(MOST_NESTING + 1),
            "]".repeat(MOST_NESTING + 1)
        );
        let problem = nesting_problem("a.ts", &source).expect("the file nests too deep");
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
        assert!(
            nesting_problem("a.rs", &source).is_some(),
            "a thousand and one negations nest too deep"
        );
        let flat = format!("x = {}y", "a + ".repeat(MOST_NESTING * 3));
        assert_eq!(
            nesting_problem("a.rs", &flat),
            None,
            "a sum is not a run of prefix operators"
        );
    }

    #[test]
    fn a_comment_does_not_count() {
        let line = format!("// {}\nx", "(".repeat(MOST_NESTING + 5));
        let block = format!("/* {} */ x", "-".repeat(MOST_NESTING + 5));
        assert_eq!(nesting_problem("a.ts", &line), None, "a line comment");
        assert_eq!(nesting_problem("a.ts", &block), None, "a block comment");
    }

    #[test]
    fn brackets_that_close_do_not_add_up() {
        let source = "(a)".repeat(MOST_NESTING * 2);
        assert_eq!(
            nesting_problem("a.ts", &source),
            None,
            "side by side is not nested"
        );
    }
}
