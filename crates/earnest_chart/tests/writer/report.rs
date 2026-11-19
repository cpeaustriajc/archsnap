use crate::support::{
    broken, change, edge, end, graph, named, node, outcome, rule, start, step, text,
};
use earnest_chart::{
    ChangeKind, RuleChange, RuleChanges, RuleText, Section, detail_lines, path_lines, report,
    rule_change_lines,
};
use earnest_check::{Finding, FindingKind, Outcome, Path, Shown, Status};
use serde_json::json;

fn number(value: i64) -> Shown {
    Shown {
        text: value.to_string(),
        json: json!(value),
    }
}

fn counter() -> Outcome {
    Outcome {
        rules: vec![
            rule("bounded", "The count never passes three", None),
            rule("even", "The count is never five", None),
        ],
        graph: Some(graph(
            vec![
                start("count = 0"),
                node("count = 1"),
                node("count = 2"),
                end("count = 3"),
            ],
            vec![
                edge(0, 1, &["add one"]),
                edge(1, 2, &["add one"]),
                edge(2, 3, &["add one"]),
            ],
        )),
        ..outcome(Status::Pass, 4)
    }
}

fn cold_oven() -> Vec<earnest_check::Named> {
    vec![named("door", text("Closed")), named("heat", text("Off"))]
}

fn heat_on() -> earnest_check::Step {
    step(
        "start",
        Vec::new(),
        vec![change("heat", text("Off"), text("On"))],
    )
}

fn door_open() -> earnest_check::Step {
    step(
        "open",
        Vec::new(),
        vec![change("door", text("Closed"), text("Open"))],
    )
}

fn oven() -> Outcome {
    let hot_and_open = Path {
        start: cold_oven(),
        steps: vec![heat_on(), door_open()],
    };
    Outcome {
        rules: vec![
            rule(
                "safe",
                "The heat is never on with the door open",
                Some(hot_and_open.clone()),
            ),
            rule("warm", "The heat can be on", None),
            rule(
                "closed",
                "The door stays closed",
                Some(Path {
                    start: cold_oven(),
                    steps: vec![door_open()],
                }),
            ),
        ],
        findings: vec![Finding {
            kind: FindingKind::Stuck,
            message: "no action can happen here, and the model does not mark it as an end"
                .to_owned(),
            path: hot_and_open,
        }],
        graph: Some(graph(
            vec![
                start("door = Closed, heat = Off"),
                broken("door = Open, heat = Off", &["closed"]),
                node("door = Closed, heat = On"),
                broken("door = Open, heat = On", &["safe", "closed"]),
            ],
            vec![
                edge(0, 1, &["the cook opens the door"]),
                edge(0, 2, &["the cook starts the heat"]),
                edge(1, 0, &["the cook closes the door"]),
                edge(2, 3, &["the cook opens the door"]),
            ],
        )),
        ..outcome(Status::Fail, 4)
    }
}

#[test]
fn a_passing_model_has_a_heading_a_result_line_and_a_chart() {
    let counter = counter();
    let expected = r#"## Counter

**pass** · Counter · 4 states · `models/counter.model.ts`

```mermaid
stateDiagram-v2
    state "count = 0" as s0
    state "count = 1" as s1
    state "count = 2" as s2
    state "count = 3" as s3
    [*] --> s0
    s0 --> s1 : add one
    s1 --> s2 : add one
    s2 --> s3 : add one
    s3 --> [*]
```"#;
    let sections = [Section {
        name: "Counter",
        file: "models/counter.model.ts",
        outcome: &counter,
    }];
    assert_eq!(report(&sections, None), expected);
}

const OVEN_SECTION: &str = r#"## Oven

**fail** · Oven · 4 states · `models/oven.model.ts`

```mermaid
stateDiagram-v2
    state "door = Closed, heat = Off" as s0
    state "door = Open, heat = Off" as s1
    state "door = Closed, heat = On" as s2
    state "door = Open, heat = On" as s3
    [*] --> s0
    s0 --> s1 : the cook opens the door
    s0 --> s2 : the cook starts the heat
    s1 --> s0 : the cook closes the door
    s2 --> s3 : the cook opens the door
    classDef broken fill:#c62828,color:#ffffff
    class s1,s3 broken
```

```text
broken  safe  "The heat is never on with the door open"
  start  door = "Closed", heat = "Off"
  1. start  heat: "Off" -> "On"
  2. open  door: "Closed" -> "Open"
broken  closed  "The door stays closed"
  start  door = "Closed", heat = "Off"
  1. open  door: "Closed" -> "Open"
stuck  no action can happen here, and the model does not mark it as an end
  start  door = "Closed", heat = "Off"
  1. start  heat: "Off" -> "On"
  2. open  door: "Closed" -> "Open"
```"#;

#[test]
fn a_failing_model_has_the_path_to_each_broken_rule_and_each_finding() {
    let oven = oven();
    let sections = [Section {
        name: "Oven",
        file: "models/oven.model.ts",
        outcome: &oven,
    }];
    assert_eq!(report(&sections, None), OVEN_SECTION);
}

#[test]
fn the_sections_follow_one_another_in_the_order_given() {
    let oven = oven();
    let counter = Outcome {
        graph: None,
        ..counter()
    };
    let sections = [
        Section {
            name: "Oven",
            file: "models/oven.model.ts",
            outcome: &oven,
        },
        Section {
            name: "Counter",
            file: "models/counter.model.ts",
            outcome: &counter,
        },
    ];
    let expected = format!(
        "{OVEN_SECTION}\n\n## Counter\n\n**pass** · Counter · 4 states · `models/counter.model.ts`"
    );
    assert_eq!(report(&sections, None), expected);
    assert_eq!(report(&[], None), "", "no model, no text");
}

fn side<'a>(says: &'a str, holds: &'a str) -> RuleText<'a> {
    RuleText { says, holds }
}

fn rewritten<'a>(
    id: &'a str,
    change: ChangeKind,
    was: Option<RuleText<'a>>,
    now: Option<RuleText<'a>>,
) -> RuleChange<'a> {
    RuleChange {
        model: "Tank",
        id,
        change,
        was,
        now,
    }
}

#[test]
fn a_report_with_rule_changes_ends_with_them_in_the_form_of_the_text_output() {
    let tank = outcome(Status::Pass, 4);
    let sections = [Section {
        name: "Tank",
        file: "models/tank.model.ts",
        outcome: &tank,
    }];
    let changes = [
        rewritten(
            "fresh",
            ChangeKind::Added,
            None,
            Some(side("The level is never seven", "s.level !== 7")),
        ),
        rewritten(
            "gone",
            ChangeKind::Removed,
            Some(side("The level is never five", "s.level !== 5")),
            None,
        ),
        rewritten(
            "capped",
            ChangeKind::Loosened,
            Some(side("The level is at most two", "s.level <= 2")),
            Some(side("The level is at most three", "s.level <= 3")),
        ),
        rewritten(
            "floor",
            ChangeKind::Tightened,
            Some(side("The level is at least zero", "s.level >= 0")),
            Some(side("The level is at least one", "s.level >= 1")),
        ),
        rewritten(
            "steady",
            ChangeKind::Changed,
            Some(side("The level stays within the tank", "s.level <= 3")),
            Some(side("The level stays within the tank", "s.level < 4")),
        ),
        rewritten(
            "named",
            ChangeKind::Reworded,
            Some(side("The level is small", "s.level < 4")),
            Some(side("The level is tiny", "s.level < 4")),
        ),
    ];
    let expected = r#"## Tank

**pass** · Tank · 4 states · `models/tank.model.ts`

## Rule changes

```text
rule changes since origin/main
  added      Tank.fresh  "The level is never seven"
  removed    Tank.gone  "The level is never five"
  loosened   Tank.capped
    was  "The level is at most two"  s.level <= 2
    now  "The level is at most three"  s.level <= 3
  tightened  Tank.floor
    was  "The level is at least zero"  s.level >= 0
    now  "The level is at least one"  s.level >= 1
  changed    Tank.steady
    was  "The level stays within the tank"  s.level <= 3
    now  "The level stays within the tank"  s.level < 4
  reworded   Tank.named
    was  "The level is small"  s.level < 4
    now  "The level is tiny"  s.level < 4
```"#;
    let since = RuleChanges {
        base: "origin/main",
        changes: &changes,
    };
    assert_eq!(report(&sections, Some(&since)), expected);
}

#[test]
fn a_comparison_that_found_no_change_says_none() {
    let since = RuleChanges {
        base: "HEAD",
        changes: &[],
    };
    assert_eq!(
        rule_change_lines(&since),
        ["rule changes since HEAD", "  none"]
    );
    assert_eq!(
        report(&[], Some(&since)),
        "## Rule changes\n\n```text\nrule changes since HEAD\n  none\n```"
    );
}

#[test]
fn a_rule_change_prints_whichever_sides_it_has() {
    let changes = [
        rewritten("bare", ChangeKind::Added, None, None),
        rewritten(
            "half",
            ChangeKind::Changed,
            None,
            Some(side("Now", "(s) =>\n    s.level < 4")),
        ),
    ];
    let since = RuleChanges {
        base: "main",
        changes: &changes,
    };
    assert_eq!(
        rule_change_lines(&since),
        [
            "rule changes since main",
            "  added      Tank.bare",
            "  changed    Tank.half",
            "    now  \"Now\"  (s) => s.level < 4",
        ]
    );
}

#[test]
fn an_unfinished_model_says_how_far_it_got_and_why_it_stopped() {
    let ledger = Outcome {
        reason: Some("state limit of 1000".to_owned()),
        ..outcome(Status::Unfinished, 1000)
    };
    let sections = [Section {
        name: "Ledger",
        file: "src/ledger.model.ts",
        outcome: &ledger,
    }];
    assert_eq!(
        report(&sections, None),
        "## Ledger\n\n**unfinished** · Ledger · 1000 states so far · state limit of 1000 · `src/ledger.model.ts`"
    );
}

#[test]
fn a_path_prints_the_input_of_a_step_before_its_changes() {
    let path = Path {
        start: vec![
            named(
                "box",
                Shown {
                    text: "[]".to_owned(),
                    json: json!([]),
                },
            ),
            named("sent", number(0)),
        ],
        steps: vec![
            step(
                "deliver",
                vec![named("letter", text("Bill")), named("copies", number(2))],
                vec![
                    change(
                        "box",
                        Shown {
                            text: "[]".to_owned(),
                            json: json!([]),
                        },
                        Shown {
                            text: "[\"Bill\"]".to_owned(),
                            json: json!(["Bill"]),
                        },
                    ),
                    change("sent", number(0), number(-1)),
                ],
            ),
            step("wait", Vec::new(), Vec::new()),
        ],
    };
    assert_eq!(
        path_lines(&path),
        [
            "start  box = [], sent = 0",
            "1. deliver  letter = \"Bill\", copies = 2  box: [] -> [\"Bill\"], sent: 0 -> -1",
            "2. wait",
        ]
    );
}

#[test]
fn a_path_of_no_steps_is_its_start_line() {
    let at_start = Path {
        start: vec![named("level", number(0))],
        steps: Vec::new(),
    };
    assert_eq!(path_lines(&at_start), ["start  level = 0"]);
    let no_fields = Path {
        start: Vec::new(),
        steps: Vec::new(),
    };
    assert_eq!(path_lines(&no_fields), ["start"]);
}

#[test]
fn each_kind_of_finding_prints_under_its_own_words() {
    let at_start = Path {
        start: vec![named("count", number(4))],
        steps: Vec::new(),
    };
    let finding = |kind, message: &str| Finding {
        kind,
        message: message.to_owned(),
        path: at_start.clone(),
    };
    let tally = Outcome {
        findings: vec![
            finding(FindingKind::Stuck, "nothing can happen"),
            finding(FindingKind::OutOfRange, "count is 4, outside 0 to 3"),
            finding(FindingKind::Fault, "the rule reads a field of null"),
        ],
        ..outcome(Status::Fail, 5)
    };
    assert_eq!(
        detail_lines(&tally),
        [
            "stuck  nothing can happen",
            "  start  count = 4",
            "out of range  count is 4, outside 0 to 3",
            "  start  count = 4",
            "fault  the rule reads a field of null",
            "  start  count = 4",
        ]
    );
    assert!(detail_lines(&outcome(Status::Pass, 1)).is_empty());
}

#[test]
fn text_a_developer_wrote_cannot_open_or_close_a_block_or_a_heading() {
    let hostile = "```\n## Injected\n<script>";
    let path = Path {
        start: vec![named(hostile, text(hostile))],
        steps: vec![step(
            hostile,
            vec![named(hostile, text(hostile))],
            vec![change(hostile, text(hostile), text(hostile))],
        )],
    };
    let model = Outcome {
        rules: vec![rule(hostile, hostile, Some(path.clone()))],
        findings: vec![Finding {
            kind: FindingKind::Fault,
            message: hostile.to_owned(),
            path,
        }],
        reason: Some(hostile.to_owned()),
        ..outcome(Status::Fail, 2)
    };
    let sections = [Section {
        name: "My_Model # 1\n## Injected",
        file: "src/`odd`\n```.ts",
        outcome: &model,
    }];
    let changes = [RuleChange {
        model: hostile,
        id: hostile,
        change: ChangeKind::Loosened,
        was: Some(side(hostile, hostile)),
        now: Some(side(hostile, hostile)),
    }];
    let since = RuleChanges {
        base: hostile,
        changes: &changes,
    };
    let written = report(&sections, Some(&since));
    let flat = "``` ## Injected <script>";
    let expected = format!(
        r#"## My\_Model \# 1 \#\# Injected

**fail** · My\_Model \# 1 \#\# Injected · 2 states · \`\`\` \#\# Injected \<script\> · ````src/`odd` ```.ts````

```text
broken  {flat}  "{flat}"
  start  {flat} = "{flat}"
  1. {flat}  {flat} = "{flat}"  {flat}: "{flat}" -> "{flat}"
fault  {flat}
  start  {flat} = "{flat}"
  1. {flat}  {flat} = "{flat}"  {flat}: "{flat}" -> "{flat}"
```

## Rule changes

```text
rule changes since {flat}
  loosened   {flat}.{flat}
    was  "{flat}"  {flat}
    now  "{flat}"  {flat}
```"#
    );
    assert_eq!(written, expected);
    let fences: Vec<&str> = written
        .lines()
        .filter(|line| line.trim_start().starts_with("```"))
        .collect();
    assert_eq!(fences, ["```text", "```", "```text", "```"]);
    let headings = written
        .lines()
        .filter(|line| line.trim_start().starts_with('#'))
        .count();
    assert_eq!(headings, 2);
}
