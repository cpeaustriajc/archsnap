use crate::build::{Sketch, bind, body_of, helpers_of, input, int, no, one, two, yes};
use earnest_ir::{Expr, Math, Op, Order, SeqOp, Shelf, Sweep};
use earnest_value::{Domain, Value};

fn small() -> Domain {
    Domain::Int { lo: 0, hi: 1 }
}

fn variables_of(module: &str) -> String {
    module
        .lines()
        .find_map(|line| line.strip_prefix("VARIABLES "))
        .unwrap_or_else(|| panic!("the module declares its variables: {module}"))
        .to_owned()
}

#[test]
fn a_field_named_like_a_word_of_the_language_or_of_the_export_gets_a_number() {
    let mut sketch = Sketch::new("Words");
    let keyword = sketch.number("IF", 0, 1, 0);
    sketch.number("Init", 0, 1, 0);
    sketch.number("Next", 0, 1, 0);
    sketch.number("Len", 0, 1, 0);
    sketch.number("InRange", 0, 1, 0);
    sketch.action(
        "bump",
        yes(),
        &[(&keyword, two(Op::Math(Math::Add), &keyword, int(1)))],
    );
    let module = sketch.written().module;
    assert_eq!(
        variables_of(&module),
        "IF_2, Init_2, Next_2, Len_2, InRange_2"
    );
    assert_eq!(
        body_of(&module, "Action_bump"),
        [
            "IF InRange",
            "THEN /\\ IF_2' = (IF_2 + 1)",
            "     /\\ UNCHANGED <<Init_2, Next_2, Len_2, InRange_2>>",
            "ELSE FALSE",
        ]
    );
    assert_eq!(
        body_of(&module, "Init"),
        [
            "/\\ IF_2 = 0",
            "/\\ Init_2 = 0",
            "/\\ Next_2 = 0",
            "/\\ Len_2 = 0",
            "/\\ InRange_2 = 0"
        ]
    );
}

#[test]
fn two_fields_that_spell_the_same_in_the_language_stay_two_variables() {
    let mut sketch = Sketch::new("Twins");
    sketch.number("price$", 0, 1, 0);
    sketch.number("price_", 0, 1, 0);
    sketch.number("2nd", 0, 1, 0);
    sketch.number("WF_x", 0, 1, 0);
    assert_eq!(
        variables_of(&sketch.written().module),
        "price_, price__2, v2nd, vWF_x"
    );
}

#[test]
fn a_helper_parameter_steps_aside_for_a_field_with_its_name() {
    let mut sketch = Sketch::new("Shadow");
    let seq = sketch.field(
        "seq",
        Domain::Seq {
            item: Box::new(small()),
            max: 2,
        },
        vec![Value::seq(Vec::new())],
    );
    sketch.rule("shorter", one(Op::Seq(SeqOp::Shift), &seq));
    let module = sketch.written().module;
    assert_eq!(
        helpers_of(&module),
        ["SeqShift(seq_2) == IF Len(seq_2) = 0 THEN seq_2 ELSE Tail(seq_2)"]
    );
    assert_eq!(
        body_of(&module, "Rule_shorter"),
        ["InRange => SeqShift(seq)"]
    );
}

#[test]
fn an_input_or_a_local_named_like_a_field_gets_a_number() {
    let mut sketch = Sketch::new("Shadow");
    let who = sketch.number("who", 0, 1, 0);
    let item = sketch.field("item", Domain::Set(Box::new(small())), Vec::new());
    sketch.number("local", 0, 1, 0);
    sketch.number("Keep", 0, 1, 0);
    let tested = Expr::Sweep {
        sweep: Sweep::Every,
        shelf: Shelf::Set,
        slot: 1,
        source: Box::new(item),
        body: Box::new(two(Op::Equal, &Expr::Local(1), Expr::Local(0))),
    };
    sketch.action_with(
        "pick",
        vec![input("who", 0, small())],
        tested,
        &[(&who, bind(2, int(1), Expr::Local(2)))],
    );
    let kept = Expr::Sweep {
        sweep: Sweep::Filter,
        shelf: Shelf::Seq,
        slot: 0,
        source: Box::new(Expr::SeqOf(vec![int(1)])),
        body: Box::new(two(Op::Order(Order::Less), &Expr::Local(0), int(1))),
    };
    sketch.rule("kept", kept);
    let module = sketch.written().module;
    assert_eq!(
        body_of(&module, "Action_pick"),
        [
            "\\E who_2 \\in {0, 1} :",
            "    IF InRange /\\ (\\A item_2 \\in item : (item_2 = who_2))",
            "    THEN /\\ who' = (LET local_2 == 1 IN local_2)",
            "         /\\ UNCHANGED <<item, local, Keep>>",
            "    ELSE FALSE",
        ]
    );
    assert_eq!(
        body_of(&module, "Rule_kept"),
        ["InRange => (LET Keep_2(item_2) == (item_2 < 1) IN SelectSeq(<<1>>, Keep_2))"]
    );
}

#[test]
fn the_same_input_name_is_free_again_in_the_next_action() {
    let mut sketch = Sketch::new("Twice");
    let n = sketch.number("n", 0, 1, 0);
    for id in ["first", "second"] {
        sketch.action_with(
            id,
            vec![input("to", 0, small())],
            yes(),
            &[(&n, Expr::Local(0))],
        );
    }
    let module = sketch.written().module;
    for action in ["Action_first", "Action_second"] {
        assert_eq!(
            body_of(&module, action),
            [
                "\\E to \\in {0, 1} :",
                "    IF InRange",
                "    THEN /\\ n' = to",
                "    ELSE FALSE",
            ]
        );
    }
}

#[test]
fn an_action_a_rule_and_an_end_with_one_id_are_three_definitions() {
    let mut sketch = Sketch::new("Door");
    let open = sketch.field("open", Domain::Bool, vec![Value::Bool(false)]);
    sketch.action("open", yes(), &[(&open, yes())]);
    sketch.rule("open", yes());
    sketch.end("open", open.clone());
    let files = sketch.written();
    assert_eq!(body_of(&files.module, "Rule_open"), ["InRange => TRUE"]);
    assert_eq!(body_of(&files.module, "End_open"), ["open"]);
    assert_eq!(
        body_of(&files.module, "Next"),
        ["\\/ Action_open", "\\/ Stop"]
    );
    assert_eq!(
        body_of(&files.module, "Stop"),
        [
            "IF (~InRange) \\/ End_open",
            "THEN UNCHANGED vars",
            "ELSE FALSE"
        ]
    );
    assert!(
        files.config.ends_with("INVARIANT Rule_open\n"),
        "{}",
        files.config
    );
}

#[test]
fn a_rule_whose_invariant_name_a_field_has_taken_still_carries_its_id() {
    let mut sketch = Sketch::new("Taken");
    sketch.number("Rule_safe", 0, 1, 0);
    sketch.rule("safe", yes());
    sketch.rule("say it", no());
    let files = sketch.written();
    assert_eq!(body_of(&files.module, "Rule_safe_2"), ["InRange => TRUE"]);
    assert_eq!(body_of(&files.module, "Rule_say_it"), ["InRange => FALSE"]);
    assert!(
        files
            .config
            .ends_with("INVARIANT Rule_safe_2\nINVARIANT Rule_say_it\n"),
        "{}",
        files.config
    );
}

#[test]
fn the_module_is_named_after_the_model_as_far_as_the_language_can_spell_it() {
    let spaced = Sketch::new("Add to cart").written();
    assert_eq!(spaced.name, "Add_to_cart");
    assert!(
        spaced.module.starts_with("---- MODULE Add_to_cart ----\n"),
        "{}",
        spaced.module
    );
    assert_eq!(Sketch::new("Next").written().name, "Next_2");
    assert_eq!(Sketch::new("Sequences").written().name, "Sequences_2");
}

#[test]
fn a_field_named_like_its_model_leaves_the_name_to_the_module() {
    let mut sketch = Sketch::new("Counter");
    sketch.number("Counter", 0, 1, 0);
    let files = sketch.written();
    assert_eq!(files.name, "Counter");
    assert_eq!(variables_of(&files.module), "Counter_2");
}

#[test]
fn a_record_field_keeps_its_name_unless_the_language_owns_the_word() {
    let mut sketch = Sketch::new("Labels");
    let domain = sketch.record(&[("x", small()), ("IF", small()), ("a b", small())]);
    let x = sketch.sym("x");
    let keyword = sketch.sym("IF");
    let spaced = sketch.sym("a b");
    let start = Value::record(vec![
        (x, Value::Int(0)),
        (keyword, Value::Int(1)),
        (spaced, Value::Int(0)),
    ]);
    let field = sketch.field("x", domain, vec![start]);
    sketch.rule("read", Expr::Get(Box::new(field.clone()), x));
    sketch.rule("word", Expr::Get(Box::new(field), keyword));
    let module = sketch.written().module;
    assert_eq!(
        body_of(&module, "Init"),
        ["/\\ x = [x |-> 0, IF_2 |-> 1, a_b |-> 0]"]
    );
    assert_eq!(
        body_of(&module, "InRange"),
        ["/\\ ((x.x \\in 0..1) /\\ (x.IF_2 \\in 0..1) /\\ (x.a_b \\in 0..1))"]
    );
    assert_eq!(body_of(&module, "Rule_read"), ["InRange => x.x"]);
    assert_eq!(body_of(&module, "Rule_word"), ["InRange => x.IF_2"]);
}

#[test]
fn a_text_value_is_never_renamed() {
    let mut sketch = Sketch::new("Texts");
    let words = sketch.one_of(&["IF", "Init", "a b"]);
    let start = sketch.text("IF");
    sketch.field("word", words, vec![start]);
    let module = sketch.written().module;
    assert_eq!(
        body_of(&module, "InRange"),
        ["/\\ (word \\in {\"IF\", \"Init\", \"a b\"})"]
    );
    assert_eq!(body_of(&module, "Init"), ["/\\ word = \"IF\""]);
}

#[test]
fn a_text_value_that_reads_like_a_helper_call_defines_no_helper() {
    let mut sketch = Sketch::new("Texts");
    let words = sketch.one_of(&["MapGet(book, key)", "Fault"]);
    let start = sketch.text("Fault");
    sketch.field("word", words, vec![start]);
    let module = sketch.written().module;
    assert_eq!(helpers_of(&module), Vec::<String>::new());
    assert_eq!(body_of(&module, "Init"), ["/\\ word = \"Fault\""]);
}

#[test]
fn a_record_field_named_like_a_helper_or_a_definition_gets_a_number() {
    let mut sketch = Sketch::new("Labels");
    let domain = sketch.record(&[("Fault", small()), ("Init", small())]);
    let fault = sketch.sym("Fault");
    let init = sketch.sym("Init");
    let start = Value::record(vec![(fault, Value::Int(0)), (init, Value::Int(1))]);
    sketch.field("held", domain, vec![start]);
    let module = sketch.written().module;
    assert_eq!(
        body_of(&module, "Init"),
        ["/\\ held = [Fault_2 |-> 0, Init_2 |-> 1]"]
    );
    assert_eq!(helpers_of(&module), Vec::<String>::new());
}
