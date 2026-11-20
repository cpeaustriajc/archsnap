use super::{Map, Queue, Request, Retry, Seq, Set, Timer, Walk};

/// A value as the text output of `earnest verify` prints it in the input of a step.
///
/// A text and an enum variant are in quotes, a number and a boolean are as they are, `None` is
/// `null`, a `Some` is the value inside it, and a record is `{name: value}` in the order of the
/// field names. A [`Set`](super::Set) is `{a, b}` and a [`Map`](super::Map) is `{k -> v}` in
/// the order of [`Walk`](super::Walk), a [`Seq`](super::Seq) or a [`Queue`](super::Queue) is
/// `[a, b]`, a [`Timer`](super::Timer) or a [`Request`](super::Request) is its stage as a text,
/// and a [`Retry`](super::Retry) is its attempts. It is the text of the checker's report, so a
/// test can compare what it prints for a state with what `earnest verify` prints.
/// `#[derive(Domain)]` implements it for the enum or the struct, so every type an action can
/// take as an input has it, and [`Step::input`](super::Step::input) holds this text.
///
/// ```
/// use earnest::model::Report;
///
/// assert_eq!(3_u8.report(), "3");
/// assert_eq!(true.report(), "true");
/// assert_eq!("Kim".report(), "\"Kim\"");
/// assert_eq!(None::<u8>.report(), "null");
/// assert_eq!(Some(7_i8).report(), "7");
/// ```
pub trait Report {
    /// The value as the text output prints it.
    fn report(&self) -> String;
}

impl Report for bool {
    fn report(&self) -> String {
        self.to_string()
    }
}

macro_rules! report_integers {
    ($($integer:ty),*) => {
        $(
            impl Report for $integer {
                fn report(&self) -> String {
                    self.to_string()
                }
            }
        )*
    };
}

report_integers!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

impl Report for str {
    fn report(&self) -> String {
        let mut quoted = String::with_capacity(self.len().saturating_add(2));
        quoted.push('"');
        for letter in self.chars() {
            match letter {
                '"' => quoted.push_str("\\\""),
                '\\' => quoted.push_str("\\\\"),
                '\n' => quoted.push_str("\\n"),
                '\r' => quoted.push_str("\\r"),
                '\t' => quoted.push_str("\\t"),
                other if other.is_control() || matches!(other, '\u{2028}' | '\u{2029}') => {
                    quoted.extend(other.escape_unicode());
                }
                other => quoted.push(other),
            }
        }
        quoted.push('"');
        quoted
    }
}

impl<T: Report + ?Sized> Report for &T {
    fn report(&self) -> String {
        (**self).report()
    }
}

impl<T: Report> Report for Option<T> {
    fn report(&self) -> String {
        self.as_ref()
            .map_or_else(|| "null".to_owned(), Report::report)
    }
}

fn listed(open: &str, items: impl Iterator<Item = String>, close: &str) -> String {
    format!("{open}{}{close}", items.collect::<Vec<String>>().join(", "))
}

impl<T: Walk + Clone + Report> Report for Set<T> {
    fn report(&self) -> String {
        listed("{", self.walked().map(Report::report), "}")
    }
}

impl<K: Walk + Clone + Report, V: Clone + Report> Report for Map<K, V> {
    fn report(&self) -> String {
        let entries = self
            .entries_walked()
            .map(|(key, value)| format!("{} -> {}", key.report(), value.report()));
        listed("{", entries, "}")
    }
}

impl<T: Report, const MAX: usize> Report for Seq<T, MAX> {
    fn report(&self) -> String {
        listed("[", self.walked().map(Report::report), "]")
    }
}

impl<T: Report, const CAPACITY: usize> Report for Queue<T, CAPACITY> {
    fn report(&self) -> String {
        listed("[", self.walked().map(Report::report), "]")
    }
}

impl Report for Timer {
    fn report(&self) -> String {
        self.stage_name().report()
    }
}

impl Report for Request {
    fn report(&self) -> String {
        self.stage_name().report()
    }
}

impl<const MAX: usize> Report for Retry<MAX> {
    fn report(&self) -> String {
        self.attempts().to_string()
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Written {
    Bare(String),
    Quoted(String),
    Record(Vec<(String, Written)>),
}

struct Reading<'a> {
    rest: &'a str,
}

impl Reading<'_> {
    fn skip_spaces(&mut self) {
        self.rest = self.rest.trim_start();
    }

    fn take(&mut self, wanted: char) -> bool {
        let found = self.rest.strip_prefix(wanted);
        if let Some(after) = found {
            self.rest = after;
        }
        found.is_some()
    }

    fn quoted(&mut self) -> Option<String> {
        let mut letters = self.rest.strip_prefix('"')?.chars();
        let mut text = String::new();
        loop {
            match letters.next()? {
                '"' => break,
                '\\' => text.push(Self::escaped(&mut letters)?),
                other => text.push(other),
            }
        }
        self.rest = letters.as_str();
        Some(text)
    }

    fn escaped(letters: &mut std::str::Chars<'_>) -> Option<char> {
        Some(match letters.next()? {
            '"' => '"',
            '\\' => '\\',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => {
                if letters.next()? != '{' {
                    return None;
                }
                let mut code = String::new();
                loop {
                    match letters.next()? {
                        '}' => break,
                        digit => code.push(digit),
                    }
                }
                char::from_u32(u32::from_str_radix(&code, 16).ok()?)?
            }
            _ => return None,
        })
    }

    fn bare(&mut self) -> Option<Written> {
        let end = self.rest.find([',', '}', ':']).unwrap_or(self.rest.len());
        let word = self.rest.get(..end)?.trim();
        if word.is_empty() {
            return None;
        }
        self.rest = self.rest.get(end..)?;
        Some(Written::Bare(word.to_owned()))
    }

    fn value(&mut self) -> Option<Written> {
        self.skip_spaces();
        if self.rest.starts_with('"') {
            return self.quoted().map(Written::Quoted);
        }
        if self.rest.starts_with('{') {
            return self.record();
        }
        self.bare()
    }

    fn record(&mut self) -> Option<Written> {
        self.take('{');
        let mut fields = Vec::new();
        self.skip_spaces();
        if self.take('}') {
            return Some(Written::Record(fields));
        }
        loop {
            let Some(Written::Bare(name)) = self.bare() else {
                return None;
            };
            if !self.take(':') {
                return None;
            }
            fields.push((name, self.value()?));
            self.skip_spaces();
            if self.take('}') {
                return Some(Written::Record(fields));
            }
            if !self.take(',') {
                return None;
            }
            self.skip_spaces();
        }
    }
}

fn read(text: &str) -> Option<Written> {
    let mut reading = Reading { rest: text };
    let found = reading.value()?;
    reading.skip_spaces();
    reading.rest.is_empty().then_some(found)
}

fn agree(shown: &Written, written: &Written) -> bool {
    match (shown, written) {
        (Written::Bare(left), Written::Bare(right))
        | (Written::Quoted(left), Written::Quoted(right) | Written::Bare(right)) => left == right,
        (Written::Record(left), Written::Record(right)) => {
            left.len() == right.len()
                && left.iter().all(|(name, inside)| {
                    right
                        .iter()
                        .any(|(other, there)| other == name && agree(inside, there))
                })
        }
        _ => false,
    }
}

pub(super) fn matches(shown: &str, written: &str) -> bool {
    if shown == written {
        return true;
    }
    match (read(shown), read(written)) {
        (Some(left), Some(right)) => agree(&left, &right),
        (Some(Written::Quoted(text)), None) => text == written,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Report, matches};

    #[test]
    fn a_text_is_quoted_with_the_escapes_the_report_uses() {
        assert_eq!("Kim".report(), "\"Kim\"");
        assert_eq!("a\"b\\c".report(), "\"a\\\"b\\\\c\"");
        assert_eq!("a\nb\rc\td".report(), "\"a\\nb\\rc\\td\"");
        assert_eq!("\u{1}\u{2028}".report(), "\"\\u{1}\\u{2028}\"");
        assert_eq!("é".report(), "\"é\"", "a printable letter stays");
    }

    #[test]
    fn a_number_a_boolean_and_an_option_are_as_the_report_prints_them() {
        assert_eq!((-3_i8).report(), "-3");
        assert_eq!(u128::MAX.report(), u128::MAX.to_string());
        assert_eq!(false.report(), "false");
        assert_eq!(None::<bool>.report(), "null");
        assert_eq!(Some("Kim").report(), "\"Kim\"");
        assert_eq!(
            Some(None::<u8>).report(),
            "null",
            "the report has no `Some(None)`"
        );
    }

    #[test]
    fn a_written_value_matches_in_the_forms_the_report_and_the_json_print_it() {
        for (shown, written) in [
            ("\"Bill\"", "Bill"),
            ("\"Bill\"", "\"Bill\""),
            ("null", "null"),
            ("true", "true"),
            ("-3", "-3"),
            ("\"1\"", "1"),
            ("\"a\\\"b\"", "a\"b"),
            ("\"a\\\"b\"", "\"a\\\"b\""),
            ("\"x y, {z}\"", "x y, {z}"),
            ("{row: 1, lit: true}", "{row: 1, lit: true}"),
            ("{row: 1, lit: true}", "{lit: true, row: 1}"),
            ("{row: 1, lit: true}", "{ row: 1 , lit: true }"),
            (
                "{kind: \"Bill\", at: {row: 0}}",
                "{at: {row: 0}, kind: Bill}",
            ),
            ("{kind: \"Bill\"}", "{kind: \"Bill\"}"),
        ] {
            assert!(matches(shown, written), "`{written}` is `{shown}`");
        }
    }

    #[test]
    fn a_written_value_that_differs_does_not_match() {
        for (shown, written) in [
            ("\"Bill\"", "Card"),
            ("\"Bill\"", "null"),
            ("null", "Bill"),
            ("null", "\"null\""),
            ("3", "4"),
            ("3", "\"3\""),
            ("true", "false"),
            ("{row: 1, lit: true}", "{row: 1}"),
            ("{row: 1, lit: true}", "{row: 1, lit: false}"),
            ("{row: 1, lit: true}", "{row: 1, lit: true, extra: 1}"),
            ("{row: 1, lit: true}", "row: 1, lit: true"),
            ("{row: 1}", "{row 1}"),
            ("{row: 1}", "{row: 1"),
        ] {
            assert!(!matches(shown, written), "`{written}` is not `{shown}`");
        }
    }
}
