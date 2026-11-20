use crate::support::{Run, Sandbox};

const CONFIG_KEYWORDS: [&str; 18] = [
    "SPECIFICATION",
    "INIT",
    "NEXT",
    "INVARIANT",
    "INVARIANTS",
    "PROPERTY",
    "PROPERTIES",
    "CONSTANT",
    "CONSTANTS",
    "CONSTRAINT",
    "CONSTRAINTS",
    "ACTION_CONSTRAINT",
    "ACTION_CONSTRAINTS",
    "SYMMETRY",
    "VIEW",
    "ALIAS",
    "POSTCONDITION",
    "CHECK_DEADLOCK",
];

fn names_after(config: &str, keywords: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    let mut collecting = false;
    for word in config.split_whitespace() {
        if CONFIG_KEYWORDS.contains(&word) {
            collecting = keywords.contains(&word);
        } else if collecting {
            names.push(word.to_owned());
        }
    }
    names
}

fn defines(module: &str, name: &str) -> bool {
    module.lines().any(|line| {
        line.strip_prefix(name).is_some_and(|rest| {
            let rest = rest.trim_start();
            rest.starts_with("==") || rest.starts_with('(')
        })
    })
}

fn is_rule_of(line: &str, character: char) -> bool {
    line.len() >= 4 && line.chars().all(|found| found == character)
}

fn assert_is_a_module(module: &str, name: &str) {
    let lines: Vec<&str> = module
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let header = lines.first().copied().unwrap_or_default();
    let title = format!(" MODULE {name} ");
    assert!(
        header
            .split_once(&title)
            .is_some_and(|(before, after)| is_rule_of(before, '-') && is_rule_of(after, '-')),
        "{name}.tla opens with a `MODULE {name}` header between dashes, not `{header}`"
    );
    let footer = lines.last().copied().unwrap_or_default();
    assert!(
        is_rule_of(footer, '='),
        "{name}.tla closes with a line of `=`, not `{footer}`"
    );
    assert!(
        lines.iter().any(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|word| word == "VARIABLES" || word == "VARIABLE")
        }),
        "{name}.tla declares its variables"
    );
    for definition in ["Init", "Next"] {
        assert!(
            defines(module, definition),
            "{name}.tla defines `{definition}`"
        );
    }
}

fn assert_config_checks(config: &str, module: &str, name: &str, rules: &[&str]) {
    let specification = names_after(config, &["SPECIFICATION"]);
    let starts = names_after(config, &["INIT"]);
    let steps = names_after(config, &["NEXT"]);
    assert!(
        !specification.is_empty() || (starts == ["Init"] && steps == ["Next"]),
        "{name}.cfg names `Init` and `Next`, or a specification"
    );
    for definition in &specification {
        assert!(
            defines(module, definition),
            "{name}.tla defines the specification `{definition}` that {name}.cfg names"
        );
    }
    let invariants = names_after(config, &["INVARIANT", "INVARIANTS"]);
    for invariant in &invariants {
        assert!(
            defines(module, invariant),
            "{name}.tla defines the invariant `{invariant}` that {name}.cfg names"
        );
    }
    for rule in rules {
        let wanted = rule.to_lowercase();
        let matching = invariants
            .iter()
            .filter(|invariant| invariant.to_lowercase().contains(&wanted))
            .count();
        assert_eq!(
            matching, 1,
            "{name}.cfg names one invariant for the rule `{rule}`, among {invariants:?}"
        );
    }
}

fn export(fixture: &str) -> (Sandbox, Run) {
    let sandbox = Sandbox::with(&[fixture]);
    let file = format!("models/{fixture}");
    let run = sandbox.earnest(&["tla", &file, "--out", "build/tla"]);
    (sandbox, run)
}

#[test]
fn r06_a_model_exports_as_a_tla_module_and_its_config() {
    let (sandbox, run) = export("counter.model.ts");
    run.exits(0);
    let module = sandbox.read("build/tla/Counter.tla");
    let config = sandbox.read("build/tla/Counter.cfg");
    assert_is_a_module(&module, "Counter");
    assert_config_checks(&config, &module, "Counter", &["bounded", "even"]);
}

#[test]
fn r06_a_model_with_broken_rules_exports_an_invariant_for_each_rule() {
    let (sandbox, _) = export("oven.model.ts");
    let module = sandbox.read("build/tla/Oven.tla");
    let config = sandbox.read("build/tla/Oven.cfg");
    assert_is_a_module(&module, "Oven");
    assert_config_checks(&config, &module, "Oven", &["safe", "closed"]);
    for variable in ["door", "heat"] {
        assert!(
            module.contains(variable),
            "Oven.tla has the field `{variable}` as a variable"
        );
    }
}

#[test]
fn r06_every_model_of_a_file_gets_its_own_module() {
    let (sandbox, _) = export("switches.model.ts");
    for name in ["Porch", "Hall"] {
        let module = sandbox.read(&format!("build/tla/{name}.tla"));
        let config = sandbox.read(&format!("build/tla/{name}.cfg"));
        assert_is_a_module(&module, name);
        assert!(
            !names_after(&config, &["INVARIANT", "INVARIANTS"]).is_empty(),
            "{name}.cfg names an invariant for its rule"
        );
    }
}

#[test]
fn r06_a_rust_model_exports_the_same_way() {
    let (sandbox, _) = export("turnstile.rs");
    let module = sandbox.read("build/tla/Turnstile.tla");
    let config = sandbox.read("build/tla/Turnstile.cfg");
    assert_is_a_module(&module, "Turnstile");
    assert_config_checks(&config, &module, "Turnstile", &["locked"]);
}
