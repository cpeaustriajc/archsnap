const FENCE: &str = "```";

fn breaks_a_line(character: char) -> bool {
    character.is_control() || matches!(character, '\u{2028}' | '\u{2029}')
}

/// The text on one line: each line break or other control character is one space, and the
/// white space around it is dropped. A name that is printed on a line of its own, such as a
/// model's or a file's, goes through this so it can't start a line that reads as another
/// result.
#[must_use]
pub fn one_line(text: &str) -> String {
    text.split(breaks_a_line)
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn prose(text: &str) -> String {
    let mut safe = String::new();
    for character in one_line(text).chars() {
        if character.is_ascii_punctuation() {
            safe.push('\\');
        }
        safe.push(character);
    }
    safe
}

pub(crate) fn code_span(text: &str) -> String {
    let text = one_line(text);
    let longest_run = text
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let ticks = "`".repeat(longest_run.saturating_add(1));
    if text.is_empty() {
        return "` `".to_owned();
    }
    let pad = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{ticks}{pad}{text}{pad}{ticks}")
}

pub(crate) fn fenced(lines: &[String]) -> String {
    format!("{FENCE}text\n{}\n{FENCE}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::{code_span, fenced, one_line, prose};

    #[test]
    fn a_line_break_and_the_indent_around_it_become_one_space() {
        assert_eq!(
            one_line("(s) =>\r\n    s.level <= 2\n"),
            "(s) => s.level <= 2",
            "source over two lines"
        );
        assert_eq!(one_line("two  spaces"), "two  spaces", "no line break");
    }

    #[test]
    fn every_control_character_and_line_separator_is_one_space() {
        assert_eq!(
            one_line("a\u{b}b\u{c}c\u{85}d\u{2028}e\u{2029}f\tg\u{1b}h"),
            "a b c d e f g h",
            "vertical tab, form feed, next line, separators, tab and escape"
        );
    }

    #[test]
    fn prose_escapes_every_character_markdown_reads() {
        assert_eq!(prose("Cart"), "Cart", "a plain name");
        assert_eq!(
            prose("# *My*_model_ <b> [x](y) `z` & 1. @me\n- item"),
            "\\# \\*My\\*\\_model\\_ \\<b\\> \\[x\\]\\(y\\) \\`z\\` \\& 1\\. \\@me \\- item",
            "markup, a link, code, an entity, a list and a mention"
        );
        assert_eq!(prose("模型 é"), "模型 é", "text outside ASCII is kept");
    }

    #[test]
    fn a_code_span_is_longer_than_any_run_of_backticks_inside() {
        assert_eq!(code_span("src/a.model.ts"), "`src/a.model.ts`", "plain");
        assert_eq!(code_span("a`b"), "``a`b``", "one backtick inside");
        assert_eq!(code_span("a``b`c"), "```a``b`c```", "a run of two inside");
        assert_eq!(code_span("`a"), "`` `a ``", "a backtick at the edge");
        assert_eq!(code_span(""), "` `", "nothing");
        assert_eq!(code_span("a\nb"), "`a b`", "a line break");
    }

    #[test]
    fn a_fenced_block_holds_its_lines() {
        assert_eq!(
            fenced(&["one".to_owned(), "  two".to_owned()]),
            "```text\none\n  two\n```",
            "two lines"
        );
    }
}
