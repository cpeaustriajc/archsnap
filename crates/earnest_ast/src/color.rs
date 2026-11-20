const LONGEST_COLOR_NAME: usize = 32;

/// The parts of a chart a view may color, as the keys of its `colors`.
pub const COLORED_PARTS: [&str; 3] = ["broken", "start", "end"];

/// Whether `text` is a color a chart can draw: `#` and 3, 4, 6 or 8 hexadecimal digits, or a
/// color name of up to 32 letters, such as `red`.
///
/// A reader refuses a view whose color fails this, because the chart would leave it out
/// without a word. A name is judged by its letters only: whether it names a color is up to
/// whatever shows the chart.
#[must_use]
pub fn is_color(text: &str) -> bool {
    if let Some(digits) = text.strip_prefix('#') {
        return matches!(digits.len(), 3 | 4 | 6 | 8)
            && digits.bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    !text.is_empty()
        && text.len() <= LONGEST_COLOR_NAME
        && text.bytes().all(|byte| byte.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::is_color;

    #[test]
    fn a_hexadecimal_color_has_three_four_six_or_eight_digits() {
        for color in ["#fff", "#FFFF", "#c62828", "#c62828ff"] {
            assert!(is_color(color), "{color} is a color");
        }
        for color in ["#ff", "#fffff", "#fffffff", "#ggg", "#", "#12 456"] {
            assert!(!is_color(color), "{color} is not");
        }
    }

    #[test]
    fn a_name_is_letters_only_and_at_most_thirty_two() {
        assert!(is_color("red"), "a plain name");
        assert!(is_color(&"a".repeat(32)), "the longest name");
        assert!(!is_color(&"a".repeat(33)), "one letter too many");
        for color in ["", "red;", "re d", "red1", "rgb(1,2,3)", " red", "red "] {
            assert!(!is_color(color), "{color:?} is not");
        }
    }
}
