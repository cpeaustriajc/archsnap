//! The TypeScript reader: model file to tree.
//!
//! [`read`] parses one TypeScript model file and gives every model in it as a
//! [`Draft`](earnest_ast::Draft), with a [`Problem`] for each construct a model file may not
//! contain. It decides what the file is written with, not what it means: a name the file does
//! not define and a value of the wrong kind are left for the lowering to find.

mod bodies;
#[cfg(test)]
mod checks;
mod file;
#[cfg(test)]
mod forms;
mod members;
mod names;
mod shapes;
#[cfg(test)]
mod show;
mod terms;

use earnest_ast::{MOST_NESTING, Quoting, Reading, Shape, TOO_DEEP, nesting_problem};
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
    plain_values: Vec<String>,
    models: Vec<String>,
    locals: Vec<(String, String)>,
    uninitialised: Vec<(String, usize)>,
    step_places: Vec<(String, u32)>,
    depth: usize,
}

/// Reads the TypeScript model file `source` and gives every model it declares, in file order.
///
/// `file` is the path to print in each problem; it also picks the dialect, so a path ending
/// in `.tsx` is read as TSX and any other as TypeScript. Nothing is read from disk.
///
/// A top-level `const` is read as exactly one of: a model, an action, a rule, an end or a view
/// made by calling the library with the model as its first argument, as in `m.rule(Model, ...)`,
/// or by calling the model directly; a declaration; a helper function; or a
/// number, text or boolean written out or computed from earlier constants. Any other value is
/// a problem, whether or not something reads the constant, and so is the name of a model used
/// as a value. In an action, each function's own parameters decide which inputs it has in
/// scope. A rule's `text` is in the normal form of `earnest_ast::condition_text`.
///
/// When the reading has any problem the file is unreadable and its drafts must not be
/// checked. Problems come in order of position and cover the whole file, except after a
/// syntax error, where only the syntax errors are listed. A reading without problems can
/// still fail in the lowering, which is where an undefined name is found.
#[must_use]
pub fn read(file: &str, source: &str) -> Reading {
    if let Some(problem) = nesting_problem(file, source, Quoting::TypeScript) {
        return Reading {
            drafts: Vec::new(),
            problems: vec![problem],
        };
    }
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

/// Whether the TypeScript file `source` imports or may import `@earnest/model`, so it is a
/// model file and must be read, or refused as unreadable, and never left out.
///
/// It is true when a static `import` or an `export ... from` names `@earnest/model` once the
/// escapes of the specifier are resolved, as JavaScript does (`"@earnest\x2fmodel"` is the
/// library), and when the file holds the specifier written plainly in either kind of quote.
/// It is also true when the file can't be parsed, makes a dynamic `import()` or calls
/// `require`, and mentions `earnest` anywhere: the file can't be shown to leave the library
/// alone, and the reader then refuses what it can't read. A file nested too deep to parse is
/// a model file only when it mentions `earnest`, because its text can't be resolved. A file
/// that reaches the library in a way its text does not show, such as a joined specifier, is
/// not found here: the caller decides by the file's name and by whether it was asked for.
/// `file` picks the dialect as it does for [`read`].
#[must_use]
pub fn reaches_library(file: &str, source: &str) -> bool {
    let plainly = names::LIBRARY_QUOTES
        .iter()
        .any(|quoted| source.contains(quoted));
    if plainly {
        return true;
    }
    let mentions = source.contains("earnest");
    if !mentions && !source.contains('\\') {
        return false;
    }
    if nesting_problem(file, source, Quoting::TypeScript).is_some() {
        return mentions;
    }
    let allocator = Allocator::default();
    let dialect = SourceType::from_path(file).unwrap_or_else(|_| SourceType::ts());
    let parsed = Parser::new(&allocator, source, dialect).parse();
    let imports = parsed
        .module_record
        .requested_modules
        .keys()
        .any(|specifier| specifier.as_str() == names::LIBRARY_MODULE);
    let unclear = parsed.fatal_error
        || !parsed.diagnostics.is_empty()
        || !parsed.module_record.dynamic_imports.is_empty()
        || source.contains("require");
    imports || (unclear && mentions)
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
            plain_values: Vec::new(),
            models: Vec::new(),
            locals: Vec::new(),
            uninitialised: Vec::new(),
            step_places: Vec::new(),
            depth: 0,
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

    pub(crate) fn too_deep(&mut self, span: Span) -> bool {
        if self.depth < MOST_NESTING {
            return false;
        }
        let written = self.quoted(Span::new(span.start, span.start.saturating_add(1)));
        self.refuse(span.start, &written, TOO_DEEP);
        true
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
