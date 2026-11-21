use crate::build::{
    Sketch, apply, below, both, counter, int, is, is_not, not, plus, steps, texts, yes,
};
use earnest_check::{FindingKind, Limits, Outcome, Status, Verdict, explore};
use earnest_ir::{Expr, Math, Model, Op, Order, Shelf, Sweep};
use earnest_value::{Domain, Value};
use std::time::{Duration, Instant};

fn long_counter() -> Model {
    let (mut sketch, n) = counter(99);
    sketch.end("full", is(&n, 99));
    sketch.rule("neverFive", is_not(&n, 5));
    sketch.rule("neverFifty", is_not(&n, 50));
    sketch.model()
}

fn with_states(max_states: usize) -> Limits {
    Limits {
        max_states,
        ..Limits::default()
    }
}

fn with_jobs(jobs: usize) -> Limits {
    Limits {
        jobs,
        keep_graph: true,
        ..Limits::default()
    }
}

fn wide_level() -> Model {
    let mut sketch = Sketch::new();
    let every_start: Vec<i64> = (0..2000).collect();
    let n = sketch.number("n", 0, 1999, &every_start);
    let flipped = sketch.boolean("flipped", &[false]);
    sketch.action("flip", not(&flipped), &[(&flipped, yes())]);
    sketch.end("flipped", flipped.clone());
    sketch.rule("not1234", not(&both(flipped, is(&n, 1234))));
    sketch.model()
}

fn cube() -> Model {
    let mut sketch = Sketch::new();
    let axes: Vec<Expr> = ["x", "y", "z"]
        .iter()
        .map(|name| sketch.number(name, 0, 29, &[0]))
        .collect();
    for (name, axis) in ["moveX", "moveY", "moveZ"].iter().zip(&axes) {
        sketch.action(name, below(axis, 29), &[(axis, plus(axis, 1))]);
    }
    let at = |x: i64, y: i64, z: i64| both(is(&axes[0], x), both(is(&axes[1], y), is(&axes[2], z)));
    sketch.end("corner", at(29, 29, 29));
    sketch.rule("notThere", not(&at(10, 20, 5)));
    sketch.model()
}

fn unfinished_at(outcome: &Outcome, states: usize, reason: &str) {
    assert_eq!(outcome.status, Status::Unfinished, "{:?}", outcome.reason);
    assert_eq!(outcome.states, states, "the count so far");
    assert_eq!(outcome.reason.as_deref(), Some(reason), "why it stopped");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn the_default_limits_are_five_million_states_no_time_limit_and_every_rule() {
    let limits = Limits::default();
    assert_eq!(limits.max_states, 5_000_000);
    assert_eq!(limits.time_limit, None);
    assert_eq!(limits.only_rule, None);
    assert!(!limits.keep_graph);
}

#[test]
fn the_state_limit_stops_the_search_with_the_count_so_far() {
    let outcome = explore(&long_counter(), &with_states(3));
    unfinished_at(&outcome, 3, "state limit of 3");
}

#[test]
fn a_rule_not_seen_broken_before_the_limit_is_unknown() {
    let outcome = explore(&long_counter(), &with_states(3));
    assert_eq!(outcome.rules[0].verdict, Verdict::Unknown);
    assert_eq!(outcome.rules[1].verdict, Verdict::Unknown);
    assert_eq!(outcome.rules[0].path, None);
}

#[test]
fn a_rule_seen_broken_before_the_limit_fails_the_model() {
    let outcome = explore(&long_counter(), &with_states(20));
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.states, 20);
    assert_eq!(outcome.reason.as_deref(), Some("state limit of 20"));
    assert_eq!(outcome.rules[0].verdict, Verdict::Broken);
    assert_eq!(outcome.rules[1].verdict, Verdict::Unknown);
    let path = outcome.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(path.steps.len(), 5);
}

#[test]
fn a_state_limit_equal_to_the_number_of_states_does_not_stop_the_search() {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    let model = sketch.model();
    assert_eq!(explore(&model, &with_states(4)).status, Status::Pass);
    unfinished_at(&explore(&model, &with_states(3)), 3, "state limit of 3");
}

#[test]
fn the_state_limit_also_bounds_the_start_states() {
    unfinished_at(
        &explore(&wide_level(), &with_states(100)),
        100,
        "state limit of 100",
    );
}

#[test]
fn a_time_limit_that_has_already_passed_stops_the_search_at_the_start_states() {
    let limits = Limits {
        time_limit: Some(Duration::ZERO),
        ..Limits::default()
    };
    let outcome = explore(&long_counter(), &limits);
    unfinished_at(&outcome, 1, "time limit of 0 s");
    assert_eq!(outcome.rules[0].verdict, Verdict::Unknown);
}

#[test]
fn a_time_limit_that_has_already_passed_stops_the_worker_threads_too() {
    let limits = Limits {
        time_limit: Some(Duration::ZERO),
        jobs: 4,
        ..Limits::default()
    };
    unfinished_at(&explore(&wide_level(), &limits), 2000, "time limit of 0 s");
}

#[test]
fn a_time_limit_that_is_not_reached_changes_nothing() {
    let limits = Limits {
        time_limit: Some(Duration::from_hours(1)),
        ..Limits::default()
    };
    let outcome = explore(&long_counter(), &limits);
    assert_eq!(outcome.states, 100);
    assert_eq!(outcome.reason, None);
    assert_eq!(outcome.status, Status::Fail);
}

#[test]
fn a_level_of_two_thousand_states_gives_the_same_outcome_on_one_thread_and_on_four() {
    let model = wide_level();
    let single = explore(&model, &with_jobs(1));
    assert_eq!(single.states, 4000);
    assert_eq!(single.status, Status::Fail);
    let path = single.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(texts(&path.start), ["n = 1234", "flipped = false"]);
    assert_eq!(steps(path), ["flip  flipped: false -> true"]);
    assert_eq!(explore(&model, &with_jobs(4)), single);
    assert_eq!(explore(&model, &with_jobs(3)), single);
    assert_eq!(explore(&model, &with_jobs(64)), single);
}

#[test]
fn a_search_with_many_wide_levels_gives_the_same_outcome_on_one_thread_and_on_four() {
    let model = cube();
    let single = explore(&model, &with_jobs(1));
    assert_eq!(single.states, 27_000);
    assert_eq!(single.status, Status::Fail);
    let path = single.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(path.steps.len(), 35);
    assert!(single.findings.is_empty(), "{:?}", single.findings);
    assert_eq!(explore(&model, &with_jobs(4)), single);
}

#[test]
fn no_worker_threads_counts_as_one() {
    let model = wide_level();
    assert_eq!(
        explore(&model, &with_jobs(0)),
        explore(&model, &with_jobs(1))
    );
}

#[test]
fn the_state_limit_gives_the_same_outcome_on_one_thread_and_on_four() {
    let model = wide_level();
    let limited = |jobs: usize| Limits {
        max_states: 3000,
        ..with_jobs(jobs)
    };
    let single = explore(&model, &limited(1));
    assert_eq!(single.states, 3000);
    assert_eq!(single.reason.as_deref(), Some("state limit of 3000"));
    assert_eq!(explore(&model, &limited(4)), single);
}

fn start_states_zero_to_four() -> Model {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 4, &[0, 1, 2, 3, 4]);
    sketch.end("any", yes());
    sketch.rule("neverThree", is_not(&n, 3));
    sketch.model()
}

#[test]
fn a_start_state_counted_at_the_state_limit_has_its_rules_judged() {
    let outcome = explore(&start_states_zero_to_four(), &with_states(4));
    assert_eq!(outcome.states, 4, "the start states 0, 1, 2 and 3");
    assert_eq!(outcome.reason.as_deref(), Some("state limit of 4"));
    assert_eq!(outcome.status, Status::Fail, "state 3 is counted");
    assert_eq!(outcome.rules[0].verdict, Verdict::Broken);
    let path = outcome.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(texts(&path.start), ["n = 3"]);
    assert!(path.steps.is_empty(), "a start state is no step away");
}

#[test]
fn a_start_state_beyond_the_state_limit_is_neither_counted_nor_judged() {
    let outcome = explore(&start_states_zero_to_four(), &with_states(3));
    unfinished_at(&outcome, 3, "state limit of 3");
    assert_eq!(outcome.rules[0].verdict, Verdict::Unknown);
}

fn fork() -> Model {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 9, &[0]);
    sketch.action("step", is(&n, 0), &[(&n, plus(&n, 1))]);
    sketch.action("leap", is(&n, 0), &[(&n, plus(&n, 7))]);
    sketch.end("moved", not(&is(&n, 0)));
    sketch.rule("neverOne", is_not(&n, 1));
    sketch.rule("neverSeven", is_not(&n, 7));
    sketch.model()
}

#[test]
fn a_state_counted_just_before_the_state_limit_has_its_rules_judged() {
    let outcome = explore(&fork(), &with_states(2));
    assert_eq!(outcome.states, 2, "the start and the state after `step`");
    assert_eq!(outcome.reason.as_deref(), Some("state limit of 2"));
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.rules[0].verdict, Verdict::Broken, "1 is counted");
    assert_eq!(
        outcome.rules[1].verdict,
        Verdict::Unknown,
        "7 is the state the limit left out"
    );
    let path = outcome.rules[0]
        .path
        .as_ref()
        .expect("a broken rule has a path");
    assert_eq!(steps(path), ["step  n: 0 -> 1"]);
    assert!(
        outcome.findings.is_empty(),
        "no action is tried from a state the search did not go on from: {:?}",
        outcome.findings
    );
}

#[test]
fn a_state_counted_before_the_time_limit_has_its_rules_judged() {
    let (mut sketch, n) = counter(3);
    sketch.rule("neverZero", is_not(&n, 0));
    let limits = Limits {
        time_limit: Some(Duration::ZERO),
        ..Limits::default()
    };
    let outcome = explore(&sketch.model(), &limits);
    assert_eq!(outcome.states, 1, "the start state");
    assert_eq!(outcome.reason.as_deref(), Some("time limit of 0 s"));
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.rules[0].verdict, Verdict::Broken);
}

#[test]
fn the_states_judged_at_the_limit_are_the_same_on_one_thread_and_on_four() {
    let mut sketch = Sketch::new();
    let every_start: Vec<i64> = (0..2000).collect();
    let n = sketch.number("n", 0, 1999, &every_start);
    sketch.end("any", yes());
    sketch.rule("not1234", is_not(&n, 1234));
    let model = sketch.model();
    let limited = |jobs: usize| Limits {
        max_states: 1500,
        ..with_jobs(jobs)
    };
    let single = explore(&model, &limited(1));
    assert_eq!(single.states, 1500);
    assert_eq!(single.rules[0].verdict, Verdict::Broken, "1234 is counted");
    assert_eq!(explore(&model, &limited(4)), single);
}

fn every_of(count: i64, body: Expr) -> Expr {
    let items = Value::set((0..count).map(Value::Int).collect());
    Expr::Sweep {
        sweep: Sweep::Every,
        shelf: Shelf::Set,
        slot: 0,
        source: Box::new(Expr::Const(items)),
        body: Box::new(body),
    }
}

#[test]
fn a_time_limit_stops_a_rule_that_takes_billions_of_steps_in_one_state() {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    sketch.rule(
        "slow",
        every_of(2000, every_of(2000, every_of(2000, yes()))),
    );
    let limits = Limits {
        time_limit: Some(Duration::from_secs(1)),
        ..Limits::default()
    };
    let began = Instant::now();
    let outcome = explore(&sketch.model(), &limits);
    assert!(
        began.elapsed() < Duration::from_secs(20),
        "8 * 10^9 steps would take minutes: {:?}",
        began.elapsed()
    );
    assert_eq!(outcome.status, Status::Unfinished);
    assert_eq!(outcome.reason.as_deref(), Some("time limit of 1 s"));
    assert_eq!(outcome.rules[0].verdict, Verdict::Unknown);
    assert_eq!(
        outcome.states, 0,
        "no state had its rules judged, so none is counted"
    );
}

fn huge_input_space() -> Model {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 999, &[0]);
    let thousand: Vec<Value> = (0..1000).map(Value::Int).collect();
    sketch.action_with(
        "pick",
        &[
            ("a", thousand.clone()),
            ("b", thousand.clone()),
            ("c", thousand),
        ],
        yes(),
        &[(&n, Expr::Local(0))],
    );
    sketch.end("any", yes());
    sketch.model()
}

#[test]
fn an_action_with_a_billion_input_combinations_stops_at_a_small_state_limit_at_once() {
    let began = Instant::now();
    let outcome = explore(&huge_input_space(), &with_states(10));
    assert!(
        began.elapsed() < Duration::from_secs(10),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(outcome.status, Status::Unfinished);
    assert_eq!(outcome.reason.as_deref(), Some("state limit of 10"));
    assert_eq!(outcome.states, 10);
}

#[test]
fn an_action_with_a_billion_input_combinations_stops_at_the_time_limit() {
    let limits = Limits {
        time_limit: Some(Duration::from_secs(1)),
        ..Limits::default()
    };
    let began = Instant::now();
    let outcome = explore(&huge_input_space(), &limits);
    assert!(
        began.elapsed() < Duration::from_secs(20),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(outcome.status, Status::Unfinished);
    assert_eq!(outcome.reason.as_deref(), Some("time limit of 1 s"));
}

#[test]
fn a_number_computed_beyond_32_bits_is_noted_in_the_outcome() {
    let (mut sketch, n) = counter(3);
    sketch.end("full", is(&n, 3));
    let square = Expr::Apply(
        Op::Math(Math::Multiply),
        vec![plus(&n, 50_000), plus(&n, 50_000)],
    );
    sketch.rule(
        "big",
        Expr::Apply(Op::Order(Order::Greater), vec![square, int(0)]),
    );
    assert!(explore(&sketch.model(), &Limits::default()).computes_beyond_32_bits);
    let (mut small, top) = counter(3);
    small.end("full", is(&top, 3));
    assert!(!explore(&small.model(), &Limits::default()).computes_beyond_32_bits);
}

fn grows_after_a_stuck_state() -> Model {
    let mut sketch = Sketch::new();
    let j = sketch.number("j", 0, 1, &[0]);
    let q = sketch.number("q", 0, 99, &[0]);
    sketch.action("grow", both(is(&j, 0), below(&q, 99)), &[(&q, plus(&q, 1))]);
    sketch.action("step", is(&j, 0), &[(&j, int(1))]);
    sketch.model()
}

#[test]
fn a_model_with_a_stuck_state_found_before_the_state_limit_is_fail_and_says_it_stopped_early() {
    let outcome = explore(&grows_after_a_stuck_state(), &with_states(6));
    assert_eq!(outcome.status, Status::Fail, "the stuck state was found");
    assert_eq!(outcome.reason.as_deref(), Some("state limit of 6"));
    assert_eq!(outcome.findings.len(), 1, "{:?}", outcome.findings);
}

#[test]
fn a_model_with_a_fault_found_before_the_state_limit_is_fail_and_says_it_stopped_early() {
    let mut sketch = Sketch::new();
    let o = sketch.field(
        "o",
        Domain::Optional(Box::new(Domain::Int { lo: 0, hi: 1 })),
        vec![Value::Null],
    );
    let j = sketch.number("j", 0, 99, &[0]);
    let read = Expr::Apply(Op::Unwrap, vec![o]);
    sketch.action("faulty", apply(Op::Order(Order::Less), &read, int(1)), &[]);
    sketch.action("step", below(&j, 99), &[(&j, plus(&j, 1))]);
    let outcome = explore(&sketch.model(), &with_states(5));
    assert_eq!(outcome.status, Status::Fail);
    assert_eq!(outcome.reason.as_deref(), Some("state limit of 5"));
    assert_eq!(outcome.findings.len(), 1, "{:?}", outcome.findings);
}

#[test]
fn a_model_stopped_by_a_limit_with_nothing_broken_is_unfinished() {
    let (mut sketch, n) = counter(99);
    sketch.end("full", is(&n, 99));
    unfinished_at(
        &explore(&sketch.model(), &with_states(10)),
        10,
        "state limit of 10",
    );
}

#[test]
fn a_state_out_of_range_is_counted_judged_by_no_rule_and_not_searched_from() {
    let mut sketch = Sketch::new();
    let n = sketch.number("n", 0, 2, &[0]);
    sketch.action("jump", is(&n, 0), &[(&n, int(5))]);
    sketch.action("never", is(&n, 5), &[(&n, int(1))]);
    sketch.rule("low", below(&n, 3));
    let outcome = explore(&sketch.model(), &Limits::default());
    assert_eq!(
        outcome.states, 2,
        "the start state and the state out of range"
    );
    assert_eq!(
        outcome.rules[0].verdict,
        Verdict::Unknown,
        "no rule is judged in it, so the rule is not known to hold"
    );
    let kinds: Vec<_> = outcome.findings.iter().map(|found| found.kind).collect();
    assert_eq!(
        kinds,
        [FindingKind::OutOfRange],
        "and nothing is stuck or faulted after it"
    );
    assert_eq!(
        outcome.findings[0].path.steps.len(),
        1,
        "with the path to it"
    );
}

fn three_stays_then_a_move() -> Model {
    let mut sketch = Sketch::new();
    let a = sketch.number("a", 0, 1, &[0]);
    for name in ["stayOne", "stayTwo", "stayThree"] {
        sketch.action(name, is(&a, 0), &[(&a, a.clone())]);
    }
    sketch.action("go", is(&a, 0), &[(&a, int(1))]);
    sketch.rule("neverMoves", is(&a, 0));
    sketch.end("moved", is(&a, 1));
    sketch.model()
}

fn jumps_from(starts: &[i64]) -> Model {
    let mut sketch = Sketch::new();
    let x = sketch.number("x", 0, 1023, starts);
    let y = sketch.number("y", 0, 1, &[0]);
    let z = sketch.number("z", 0, 1, &[0]);
    let every_k: Vec<Value> = (0..1024).map(Value::Int).collect();
    let at_top = both(is(&x, 1023), is(&z, 0));
    sketch.action("bump", is(&y, 0), &[(&y, int(1))]);
    for (name, lands) in [("jumpOne", 0), ("jumpTwo", 0), ("lift", 1)] {
        sketch.action_with(
            name,
            &[("k", every_k.clone())],
            at_top.clone(),
            &[(&x, Expr::Local(0)), (&y, int(lands))],
        );
    }
    sketch.action("last", at_top, &[(&z, int(1))]);
    sketch.rule("flat", is(&z, 0));
    sketch.end("rest", yes());
    sketch.model()
}

#[test]
fn successors_already_known_never_use_up_the_state_limit() {
    let every_start: Vec<i64> = (0..1024).collect();
    let cases = [
        (three_stays_then_a_move(), 2),
        (jumps_from(&every_start), 2050),
    ];
    for (model, states) in &cases {
        for keep_graph in [false, true] {
            for jobs in [1, 2, 4, 10] {
                let limits = Limits {
                    max_states: *states,
                    jobs,
                    keep_graph,
                    ..Limits::default()
                };
                let outcome = explore(model, &limits);
                assert_eq!(
                    outcome.status,
                    Status::Fail,
                    "{jobs} jobs, graph {keep_graph}"
                );
                assert_eq!(outcome.states, *states, "{jobs} jobs, graph {keep_graph}");
                assert_eq!(outcome.reason, None, "{jobs} jobs, graph {keep_graph}");
                assert_eq!(outcome.rules[0].verdict, Verdict::Broken);
            }
        }
    }
}

#[test]
fn a_state_with_more_new_successors_than_the_limit_leaves_the_search_unfinished() {
    let model = jumps_from(&[1023]);
    for keep_graph in [false, true] {
        for jobs in [1, 2, 4, 10] {
            let limits = Limits {
                max_states: 100,
                jobs,
                keep_graph,
                ..Limits::default()
            };
            let outcome = explore(&model, &limits);
            assert_eq!(
                outcome.status,
                Status::Unfinished,
                "{jobs} jobs, graph {keep_graph}"
            );
            assert_eq!(outcome.reason.as_deref(), Some("state limit of 100"));
            assert_eq!(outcome.rules[0].verdict, Verdict::Unknown);
        }
    }
}
