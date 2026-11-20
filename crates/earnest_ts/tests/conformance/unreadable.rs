use crate::source;
use earnest_semantic::lower;
use earnest_ts::read;

struct Outside {
    fixture: &'static str,
    model: &'static str,
    line: u32,
    column: u32,
    feature: &'static str,
}

const OUTSIDE: &[Outside] = &[
    Outside {
        fixture: "unreadable/network.model.ts",
        model: "Quote",
        line: 13,
        column: 14,
        feature: "`fetch`",
    },
    Outside {
        fixture: "unreadable/clock.model.ts",
        model: "Stamp",
        line: 8,
        column: 31,
        feature: "`Date.now`",
    },
    Outside {
        fixture: "unreadable/dice.model.ts",
        model: "Dice",
        line: 10,
        column: 25,
        feature: "`Math.random`",
    },
    Outside {
        fixture: "unreadable/loop.model.ts",
        model: "Pile",
        line: 13,
        column: 3,
        feature: "`for`",
    },
    Outside {
        fixture: "unreadable/variable.model.ts",
        model: "Score",
        line: 11,
        column: 3,
        feature: "`let`",
    },
];

#[test]
fn each_feature_outside_the_library_is_the_one_problem_of_its_file() {
    for outside in OUTSIDE {
        let file = format!("models/{}", outside.fixture);
        let reading = read(&file, &source(outside.fixture));
        let found: Vec<(&str, u32, u32, &str)> = reading
            .problems
            .iter()
            .map(|problem| {
                (
                    problem.file.as_str(),
                    problem.line,
                    problem.column,
                    problem.feature.as_str(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(file.as_str(), outside.line, outside.column, outside.feature)],
            "{}: file, line, column and feature",
            outside.fixture
        );
        let sentence = &reading.problems.first().expect("one problem").message;
        assert!(
            !sentence.is_empty() && !sentence.contains('\n'),
            "{}: a plain sentence: {sentence}",
            outside.fixture
        );
        let names: Vec<&str> = reading
            .drafts
            .iter()
            .map(|draft| draft.name.as_str())
            .collect();
        assert_eq!(
            names,
            [outside.model],
            "{}: the model is still named, so a report can say what was not checked",
            outside.fixture
        );
    }
}

#[test]
fn an_undefined_name_is_read_and_then_refused_by_the_lowering() {
    let fixture = "unreadable/undefined.model.ts";
    let reading = read(&format!("models/{fixture}"), &source(fixture));
    assert_eq!(reading.problems, [], "the reader finds nothing to refuse");
    let bill = reading.drafts.first().expect("one model");
    assert_eq!(bill.name, "Bill", "the model");
    let problems = lower(bill).expect_err("the lowering refuses the undefined name");
    let found: Vec<(&str, u32, bool)> = problems
        .iter()
        .map(|problem| {
            (
                problem.file.as_str(),
                problem.line,
                problem.feature.contains("surcharge"),
            )
        })
        .collect();
    assert_eq!(
        found,
        [("models/unreadable/undefined.model.ts", 9, true)],
        "one problem on line 9 that names `surcharge`"
    );
}
