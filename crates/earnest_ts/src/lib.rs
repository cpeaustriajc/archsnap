//! The TypeScript reader: model file to tree.
//!
//! [`read`] parses one TypeScript model file and gives every model in it as a
//! [`Draft`](earnest_ast::Draft), with a [`Problem`] for each construct a model file may not
//! contain. It decides what the file is written with, not what it means: a name the file does
//! not define and a value of the wrong kind are left for the lowering to find.

mod bodies;
mod file;
mod members;
mod names;
mod shapes;
#[cfg(test)]
mod show;
mod terms;

use earnest_ast::{Reading, Shape};
use earnest_diagnostics::{Place, Problem};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};

const LONGEST_QUOTE: usize = 40;

pub(crate) struct Reader<'s> {
    file: &'s str,
    source: &'s str,
    line_starts: Vec<usize>,
    problems: Vec<Problem>,
    library: Option<String>,
    top_names: Vec<String>,
    shapes: Vec<(String, Shape)>,
    numbers: Vec<(String, i64)>,
    field_names: Vec<String>,
    locals: Vec<(String, String)>,
}

/// Reads the TypeScript model file `source` and gives every model it declares, in file order.
///
/// `file` is the path to print in each problem; it also picks the dialect, so a path ending
/// in `.tsx` is read as TSX and any other as TypeScript. Nothing is read from disk.
///
/// When the reading has any problem the file is unreadable and its drafts must not be
/// checked. Problems come in order of position and cover the whole file, except after a
/// syntax error, where only the syntax errors are listed. A reading without problems can
/// still fail in the lowering, which is where an undefined name is found.
#[must_use]
pub fn read(file: &str, source: &str) -> Reading {
    let allocator = Allocator::default();
    let dialect = SourceType::from_path(file).unwrap_or_else(|_| SourceType::ts());
    let parsed = Parser::new(&allocator, source, dialect).parse();
    let mut reader = Reader::new(file, source);
    for diagnostic in &parsed.diagnostics {
        let span = diagnostic.labels.first().map(oxc_span::LabeledSpan::span);
        reader.syntax_error(span, &diagnostic.message);
    }
    if !reader.problems.is_empty() || parsed.fatal_error {
        if reader.problems.is_empty() {
            reader.syntax_error(None, "the file is not TypeScript the reader can parse");
        }
        return Reading {
            drafts: Vec::new(),
            problems: reader.problems,
        };
    }
    let drafts = reader.program(&parsed.program);
    let mut problems = reader.problems;
    problems.sort_by_key(Problem::place);
    Reading { drafts, problems }
}

impl<'s> Reader<'s> {
    fn new(file: &'s str, source: &'s str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(source.match_indices('\n').map(|(index, _)| index + 1));
        Self {
            file,
            source,
            line_starts,
            problems: Vec::new(),
            library: None,
            top_names: Vec::new(),
            shapes: Vec::new(),
            numbers: Vec::new(),
            field_names: Vec::new(),
            locals: Vec::new(),
        }
    }

    pub(crate) fn place(&self, offset: u32) -> Place {
        let at = usize::try_from(offset).unwrap_or(usize::MAX);
        let line = self.line_starts.partition_point(|start| *start <= at);
        let line_start = line
            .checked_sub(1)
            .and_then(|index| self.line_starts.get(index))
            .copied()
            .unwrap_or(0);
        let before = self
            .source
            .get(line_start..at)
            .map_or(0, |text| text.chars().count());
        Place {
            line: u32::try_from(line).unwrap_or(u32::MAX),
            column: u32::try_from(before + 1).unwrap_or(u32::MAX),
        }
    }

    pub(crate) fn refuse(&mut self, offset: u32, feature: &str, message: &str) {
        let place = self.place(offset);
        let quoted = format!("`{}`", feature.replace('`', "'"));
        self.problems
            .push(Problem::new(self.file, place, &quoted, message));
    }

    pub(crate) fn text(&self, span: Span) -> &'s str {
        let start = usize::try_from(span.start).unwrap_or(usize::MAX);
        let end = usize::try_from(span.end).unwrap_or(usize::MAX);
        self.source.get(start..end).unwrap_or_default()
    }

    pub(crate) fn quoted(&self, span: Span) -> String {
        let words: Vec<&str> = self.text(span).split_whitespace().collect();
        let joined = words.join(" ");
        if joined.chars().count() <= LONGEST_QUOTE {
            return joined;
        }
        let kept: String = joined.chars().take(LONGEST_QUOTE).collect();
        format!("{kept}...")
    }

    fn syntax_error(&mut self, span: Option<Span>, message: &str) {
        let found = span.map(|at| self.quoted(at)).unwrap_or_default();
        let feature = if found.is_empty() {
            "syntax error"
        } else {
            found.as_str()
        };
        let sentence = format!("this is not TypeScript the reader can parse: {message}");
        self.refuse(span.map_or(0, |at| at.start), feature, &sentence);
    }
}

#[cfg(test)]
mod tests {
    use super::{Reader, read};
    use earnest_diagnostics::Place;
    use oxc_span::Span;

    #[test]
    fn a_place_counts_lines_and_columns_from_one() {
        let reader = Reader::new("a.ts", "ab\n\tcd\n");
        assert_eq!(
            reader.place(0),
            Place { line: 1, column: 1 },
            "the first character"
        );
        assert_eq!(
            reader.place(4),
            Place { line: 2, column: 2 },
            "after a tab on the second line"
        );
    }

    #[test]
    fn a_column_counts_characters_not_bytes() {
        let reader = Reader::new("a.ts", "\"é\" + x");
        assert_eq!(
            reader.place(7),
            Place { line: 1, column: 7 },
            "`x` is the seventh character and the eighth byte"
        );
    }

    #[test]
    fn a_long_quote_is_cut_and_joined_on_one_line() {
        let source = "one\n\ttwo three four five six seven eight nine ten eleven";
        let reader = Reader::new("a.ts", source);
        let whole = Span::new(0, u32::try_from(source.len()).expect("a short source"));
        assert_eq!(
            reader.quoted(whole),
            "one two three four five six seven eight ...",
            "forty characters, then dots"
        );
    }

    #[test]
    fn a_syntax_error_is_a_problem_with_its_place() {
        let reading = read("broken.model.ts", "const a = 1;\nconst = 2;\n");
        assert!(reading.drafts.is_empty(), "no draft from a broken file");
        let first = reading.problems.first().expect("one problem");
        assert_eq!(first.file, "broken.model.ts", "the file is named");
        assert_eq!(first.line, 2, "the line of the error");
        assert!(
            first.feature.starts_with('`') && first.feature.ends_with('`'),
            "the feature is in backticks: {}",
            first.feature
        );
        assert!(
            first.message.starts_with("this is not TypeScript"),
            "the sentence says what is wrong: {}",
            first.message
        );
    }
}
