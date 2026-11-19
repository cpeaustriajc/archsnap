use std::collections::BTreeSet;

use proptest::collection::{btree_set, vec};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

use super::{Package, Target, classify, parse};

const CASES: u32 = 256;

fn runner() -> TestRunner {
    TestRunner::new(Config {
        cases: CASES,
        ..Config::default()
    })
}

fn source_paths() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("a.ts"),
        Just("a.tsx"),
        Just("a.js"),
        Just("a.jsx"),
        Just("a.mjs"),
        Just("a.cjs"),
        Just("a.mts"),
        Just("a.cts"),
    ]
}

fn js_fragment() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("import ".to_string()),
        Just("export ".to_string()),
        Just("from ".to_string()),
        Just("import(".to_string()),
        Just("require(".to_string()),
        Just("fetch(".to_string()),
        Just("const ".to_string()),
        Just("function ".to_string()),
        Just("class ".to_string()),
        Just("async ".to_string()),
        Just("await ".to_string()),
        Just("type ".to_string()),
        Just("interface ".to_string()),
        Just("default ".to_string()),
        Just("* as ".to_string()),
        Just("new URL(".to_string()),
        Just("app.get(".to_string()),
        Just("'./a'".to_string()),
        Just("'../b/c.js'".to_string()),
        Just("\"@scope/pkg/sub\"".to_string()),
        Just("'https://example.com/api/x'".to_string()),
        Just("`/api/${id}`".to_string()),
        Just("<div a={1}>".to_string()),
        Just("</div>".to_string()),
        Just("{ ".to_string()),
        Just(" }".to_string()),
        Just("(".to_string()),
        Just(")".to_string()),
        Just(";\n".to_string()),
        Just(" = ".to_string()),
        Just(", ".to_string()),
        Just("\n".to_string()),
        "[a-zA-Z_$][a-zA-Z0-9_$]{0,6}",
        "[ -~]{0,4}",
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

fn describe(target: &Target) -> String {
    match target {
        Target::File(path) => format!("file:{path}"),
        Target::Asset => "asset".to_string(),
        Target::Package(name) => format!("package:{name}"),
        Target::Unresolved => "unresolved".to_string(),
    }
}

#[test]
fn scanner_does_not_panic_on_arbitrary_text() {
    let strategy = (source_paths(), any::<String>());
    runner()
        .run(&strategy, |(path, text)| {
            let _outcome = parse(path, &text);
            Ok(())
        })
        .unwrap();
}

#[test]
fn scanner_does_not_panic_on_js_like_text() {
    let strategy = (source_paths(), js_like_text());
    runner()
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
    runner()
        .run(&strategy, |(from, spec, files, packages)| {
            let borrowed: BTreeSet<&str> = files.iter().map(String::as_str).collect();
            let first = describe(&classify(&from, &spec, &borrowed, &packages));
            let second = describe(&classify(&from, &spec, &borrowed, &packages));
            prop_assert_eq!(first, second);
            Ok(())
        })
        .unwrap();
}
