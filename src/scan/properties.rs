use std::collections::BTreeSet;

use proptest::collection::{btree_set, vec};
use proptest::prelude::*;
use proptest::sample::select;
use proptest::test_runner::TestRunner;

use super::{Package, classify, parse};

const SOURCE_PATHS: &[&str] = &[
    "a.ts", "a.tsx", "a.js", "a.jsx", "a.mjs", "a.cjs", "a.mts", "a.cts",
];

fn source_paths() -> impl Strategy<Value = &'static str> {
    select(SOURCE_PATHS)
}

const JS_FRAGMENTS: &[&str] = &[
    "import ",
    "export ",
    "from ",
    "import(",
    "require(",
    "fetch(",
    "const ",
    "function ",
    "class ",
    "async ",
    "await ",
    "type ",
    "interface ",
    "default ",
    "* as ",
    "new URL(",
    "app.get(",
    "'./a'",
    "'../b/c.js'",
    "\"@scope/pkg/sub\"",
    "'https://example.com/api/x'",
    "`/api/${id}`",
    "<div a={1}>",
    "</div>",
    "{ ",
    " }",
    "(",
    ")",
    ";\n",
    " = ",
    ", ",
    "\n",
];

fn js_fragment() -> impl Strategy<Value = String> {
    prop_oneof![
        32 => select(JS_FRAGMENTS).prop_map(String::from),
        1 => "[a-zA-Z_$][a-zA-Z0-9_$]{0,6}",
        1 => "[ -~]{0,4}",
    ]
}

fn js_like_text() -> impl Strategy<Value = String> {
    vec(js_fragment(), 0..40).prop_map(|parts| parts.concat())
}

fn segment() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => "[a-c]{1,2}",
        1 => Just("src".to_string()),
        1 => Just("index".to_string()),
    ]
}

fn file_path() -> impl Strategy<Value = String> {
    (
        vec(segment(), 0..4),
        "[a-c]{1,2}",
        prop_oneof![
            Just("ts"),
            Just("tsx"),
            Just("js"),
            Just("mjs"),
            Just("d.ts"),
            Just("css")
        ],
    )
        .prop_map(|(dirs, stem, ext)| {
            let mut parts = dirs;
            parts.push(format!("{stem}.{ext}"));
            parts.join("/")
        })
}

fn specifier() -> impl Strategy<Value = String> {
    let prefix = prop_oneof![
        Just("./"),
        Just("../"),
        Just("../../"),
        Just("@/"),
        Just("~/"),
        Just("/"),
        Just(""),
        Just("@scope/"),
        Just("lib/"),
    ];
    let suffix = prop_oneof![
        Just(""),
        Just(".js"),
        Just(".ts"),
        Just(".css"),
        Just("?raw"),
        Just("#hash"),
        Just("/index"),
    ];
    (prefix, vec(segment(), 0..4), suffix)
        .prop_map(|(lead, parts, tail)| format!("{lead}{}{tail}", parts.join("/")))
}

fn package() -> impl Strategy<Value = Package> {
    (
        prop_oneof![
            Just("lib".to_string()),
            Just("@scope/pkg".to_string()),
            Just("a".to_string()),
            Just("b".to_string())
        ],
        vec(segment(), 0..3).prop_map(|parts| parts.join("/")),
        proptest::option::of(prop_oneof![
            Just("./a.js".to_string()),
            Just("dist/a.js".to_string()),
            Just("src/a.ts".to_string())
        ]),
    )
        .prop_map(|(name, dir, main)| Package {
            name,
            dir,
            main,
            deps: BTreeSet::new(),
        })
}

#[test]
fn scanner_does_not_panic_on_arbitrary_text() {
    let strategy = (source_paths(), any::<String>());
    TestRunner::default()
        .run(&strategy, |(path, text)| {
            let _outcome = parse(path, &text);
            Ok(())
        })
        .unwrap();
}

#[test]
fn scanner_does_not_panic_on_js_like_text() {
    let strategy = (source_paths(), js_like_text());
    TestRunner::default()
        .run(&strategy, |(path, text)| {
            let _outcome = parse(path, &text);
            Ok(())
        })
        .unwrap();
}

#[test]
fn import_resolution_is_deterministic_for_the_same_input() {
    let strategy = (
        file_path(),
        specifier(),
        btree_set(file_path(), 0..12),
        vec(package(), 0..3),
    );
    TestRunner::default()
        .run(&strategy, |(from, spec, files, packages)| {
            let borrowed: BTreeSet<&str> = files.iter().map(String::as_str).collect();
            let first = classify(&from, &spec, &borrowed, &packages);
            let second = classify(&from, &spec, &borrowed, &packages);
            prop_assert_eq!(first, second);
            Ok(())
        })
        .unwrap();
}
