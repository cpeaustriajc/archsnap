pub(crate) const LIBRARY_MODULE: &str = "@earnest/model";

pub(crate) const LIBRARY_QUOTES: [&str; 2] = ["\"@earnest/model\"", "'@earnest/model'"];

pub(crate) const DECLARATIONS: [&str; 13] = [
    "boolean", "int", "enum", "oneOf", "record", "set", "map", "seq", "queue", "lock", "timer",
    "request", "retry",
];

pub(crate) fn snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for letter in name.chars() {
        if letter.is_ascii_uppercase() {
            out.push('_');
            out.push(letter.to_ascii_lowercase());
        } else {
            out.push(letter);
        }
    }
    out
}

pub(crate) fn why_outside(root: &str, property: Option<&str>) -> &'static str {
    match (root, property) {
        ("fetch" | "XMLHttpRequest" | "WebSocket" | "navigator", _) => {
            "the network is outside the library: model the call with `m.request()` and its answer as an action input"
        }
        ("Date" | "performance", _) => {
            "the system clock is outside the library: model time with `m.timer()`, or as a number an action steps up"
        }
        ("Math", Some("random")) | ("crypto", _) => {
            "a random value is outside the library: model each possible value as an action input with `m.oneOf([...])` or `m.enum([...])`"
        }
        ("Math", _) => {
            "`Math` is outside the library; write the sum with `+`, `-`, `*`, `m.rem(a, b)` and `m.div(a, b)`"
        }
        ("console", _) => "printing is outside the library: leave it out of the model",
        ("setTimeout" | "setInterval", _) => {
            "a real timer is outside the library; declare one with `m.timer()`"
        }
        _ => {
            "it is not a library primitive or a function defined in the model file: write it as an arrow function in a top-level `const` of this file, or use a primitive from \"The primitives\" in the README"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{snake, why_outside};

    #[test]
    fn camel_case_becomes_snake_case() {
        assert_eq!(snake("isEmpty"), "is_empty", "one capital");
        assert_eq!(snake("isHeldBy"), "is_held_by", "two capitals");
        assert_eq!(snake("size"), "size", "no capital");
    }

    #[test]
    fn a_known_global_gets_its_own_sentence() {
        assert_eq!(
            why_outside("Date", Some("now")),
            "the system clock is outside the library: model time with `m.timer()`, or as a number an action steps up",
            "the clock"
        );
        assert_eq!(
            why_outside("fetch", None),
            "the network is outside the library: model the call with `m.request()` and its answer as an action input",
            "the network"
        );
        assert_eq!(
            why_outside("Math", Some("random")),
            "a random value is outside the library: model each possible value as an action input with `m.oneOf([...])` or `m.enum([...])`",
            "a random number"
        );
    }
}
