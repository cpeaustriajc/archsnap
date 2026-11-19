use crate::support::{Run, Sandbox, model, models, rule};

const UNROUTABLE_PROXY: &str = "http://10.255.255.1:9";

const PROXY_VARIABLES: [&str; 6] = [
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "ALL_PROXY",
    "https_proxy",
    "http_proxy",
    "all_proxy",
];

fn verify_cut_off(sandbox: &Sandbox, tools: &Sandbox, arguments: &[&str]) -> Run {
    let mut command = sandbox.command();
    command
        .arg("verify")
        .args(arguments)
        .env("PATH", tools.path())
        .env_remove("JAVA_HOME")
        .env_remove("NODE_PATH")
        .env_remove("NO_PROXY")
        .env_remove("no_proxy");
    for variable in PROXY_VARIABLES {
        command.env(variable, UNROUTABLE_PROXY);
    }
    Run::of(&mut command)
}

#[test]
fn r18_a_first_run_needs_no_network_no_java_no_node_and_no_setup() {
    let tools = Sandbox::empty();
    let sandbox = Sandbox::with(&["counter.model.ts", "heater.model.ts", "turnstile.rs"]);
    assert_eq!(
        sandbox.entries(),
        ["models"],
        "nothing is set up before the run"
    );

    let run = verify_cut_off(&sandbox, &tools, &["--json"]);
    run.exits(1);
    let report = run.json();
    assert_eq!(models(&report).len(), 3, "every model is checked");
    assert_eq!(
        model(&report, "Counter")["states"],
        4,
        "a TypeScript model without Node"
    );
    assert_eq!(
        model(&report, "Heater")["states"],
        8,
        "a model with helper functions"
    );
    let turnstile = model(&report, "Turnstile");
    assert_eq!(turnstile["states"], 4, "a Rust model without cargo");
    assert_eq!(
        rule(turnstile, "locked")["verdict"],
        "broken",
        "its broken rule"
    );
    assert!(
        tools.entries().is_empty(),
        "the run puts no tool on the `PATH`"
    );
}

#[test]
fn r18_the_text_output_needs_nothing_either() {
    let tools = Sandbox::empty();
    let sandbox = Sandbox::with(&["counter.model.ts", "heater.model.ts"]);
    let run = verify_cut_off(&sandbox, &tools, &[]);
    run.exits(0);
    assert_eq!(run.model_lines().len(), 2, "both models are checked\n{run}");
    assert_eq!(run.summary_count("pass"), 2, "models that pass");
}
