pub(crate) const LIBRARY_MODULE: &str = "@earnest/model";

pub(crate) const DECLARATIONS: [&str; 13] = [
    "boolean", "int", "enum", "oneOf", "record", "set", "map", "seq", "queue", "lock", "timer",
    "request", "retry",
];

const PRIMITIVE_PROPERTIES: [&str; 13] = [
    "isEmpty",
    "isFull",
    "isFree",
    "isHeld",
    "isIdle",
    "isRunning",
    "hasFired",
    "isPending",
    "hasSucceeded",
    "hasFailed",
    "isSettled",
    "canRetry",
    "isExhausted",
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

pub(crate) fn is_primitive_property(name: &str) -> bool {
    PRIMITIVE_PROPERTIES.contains(&name)
}

pub(crate) fn why_outside(root: &str, property: Option<&str>) -> &'static str {
    match (root, property) {
        ("fetch" | "XMLHttpRequest" | "WebSocket" | "navigator", _) => {
            "the network is outside the library"
        }
        ("Date" | "performance", _) => "the system clock is outside the library",
        ("Math", Some("random")) | ("crypto", _) => "a random value is outside the library",
        ("Math", _) => "`Math` is outside the library; write the sum with `+`, `-`, `*` and `%`",
        ("console", _) => "printing is outside the library",
        ("setTimeout" | "setInterval", _) => {
            "a real timer is outside the library; declare one with `m.timer()`"
        }
        _ => "it is not a library primitive or a function defined in the model file",
    }
}

#[cfg(test)]
mod tests {
    use super::{is_primitive_property, snake, why_outside};

    #[test]
    fn camel_case_becomes_snake_case() {
        assert_eq!(snake("isEmpty"), "is_empty", "one capital");
        assert_eq!(snake("isHeldBy"), "is_held_by", "two capitals");
        assert_eq!(snake("size"), "size", "no capital");
    }

    #[test]
    fn only_the_library_properties_are_primitive() {
        assert!(is_primitive_property("hasFired"), "a timer property");
        assert!(!is_primitive_property("unitPrice"), "a field of a record");
    }

    #[test]
    fn a_known_global_gets_its_own_sentence() {
        assert_eq!(
            why_outside("Date", Some("now")),
            "the system clock is outside the library",
            "the clock"
        );
        assert_eq!(
            why_outside("fetch", None),
            "the network is outside the library",
            "the network"
        );
        assert_eq!(
            why_outside("Math", Some("random")),
            "a random value is outside the library",
            "a random number"
        );
    }
}
