use crate::build::{Sketch, both, boxed, either, input, int, null, one, two, yes};
use earnest_ir::{Expr, Math, Op, Order, SeqOp, Shelf, Sweep};
use earnest_tla::{Files, Invariants};
use earnest_value::{Domain, Value};

fn counter() -> Sketch {
    let mut sketch = Sketch::new("Counter");
    let count = sketch.number("count", 0, 3, 0);
    sketch.action(
        "inc",
        two(Op::Order(Order::Less), &count, int(3)),
        &[(&count, two(Op::Math(Math::Add), &count, int(1)))],
    );
    sketch.rule(
        "bounded",
        two(Op::Order(Order::LessOrEqual), &count, int(3)),
    );
    sketch.end("full", two(Op::Equal, &count, int(3)));
    sketch
}

#[test]
fn a_counter_from_zero_to_three_is_this_module_and_this_configuration() {
    let expected = Files {
        name: "Counter".to_owned(),
        module: "\
---- MODULE Counter ----
EXTENDS Integers, Sequences, FiniteSets, TLC

VARIABLES count

vars == <<count>>

InRange ==
    /\\ (count \\in 0..3)

Init ==
    /\\ count = 0

Action_inc ==
    IF InRange /\\ (count < 3)
    THEN /\\ count' = (count + 1)
    ELSE FALSE

End_full ==
    (count = 3)

Stop ==
    IF (~InRange) \\/ End_full
    THEN UNCHANGED vars
    ELSE FALSE

Next ==
    \\/ Action_inc
    \\/ Stop

Rule_bounded ==
    InRange => (count <= 3)

====
"
        .to_owned(),
        config: "\
INIT Init
NEXT Next
CHECK_DEADLOCK FALSE
INVARIANT Rule_bounded
"
        .to_owned(),
        rules: vec![Invariants {
            rule: "bounded".to_owned(),
            holds: "Rule_bounded".to_owned(),
            judged: None,
        }],
    };
    assert_eq!(counter().written(), expected);
}

#[test]
fn a_queue_of_two_letters_is_this_module() {
    let mut sketch = Sketch::new("Mailbox");
    let letters = sketch.one_of(&["Bill", "Card"]);
    let card = sketch.text("Card");
    let inbox = sketch.field(
        "inbox",
        Domain::Seq {
            item: boxed(letters.clone()),
            max: 2,
        },
        vec![Value::seq(Vec::new())],
    );
    let size = one(Op::Seq(SeqOp::Size), &inbox);
    let full = two(Op::Order(Order::GreaterOrEqual), &size, int(2));
    let empty = two(Op::Equal, &size, int(0));
    sketch.action_with(
        "deliver",
        vec![input("letter", 0, letters)],
        one(Op::Not, &full),
        &[(&inbox, two(Op::Seq(SeqOp::Push), &inbox, Expr::Local(0)))],
    );
    sketch.action(
        "read",
        one(Op::Not, &empty),
        &[(&inbox, one(Op::Seq(SeqOp::Shift), &inbox))],
    );
    let a_card = Expr::Sweep {
        sweep: Sweep::Some,
        shelf: Shelf::Seq,
        slot: 0,
        source: Box::new(inbox.clone()),
        body: Box::new(two(Op::Equal, &Expr::Local(0), Expr::Const(card))),
    };
    sketch.rule(
        "bills",
        either(two(Op::Order(Order::Less), &size, int(2)), a_card),
    );
    let files = sketch.written();
    let expected = "\
---- MODULE Mailbox ----
EXTENDS Integers, Sequences, FiniteSets, TLC

VARIABLES inbox

vars == <<inbox>>

SeqShift(seq) == IF Len(seq) = 0 THEN seq ELSE Tail(seq)
SeqItems(seq) == {seq[index] : index \\in DOMAIN seq}

InRange ==
    /\\ ((Len(inbox) <= 2) /\\ (\\A index \\in DOMAIN inbox : (inbox[index] \\in {\"Bill\", \"Card\"})))

Init ==
    /\\ inbox = <<>>

Action_deliver ==
    \\E letter \\in {\"Bill\", \"Card\"} :
        IF InRange /\\ (~(Len(inbox) >= 2))
        THEN /\\ inbox' = Append(inbox, letter)
        ELSE FALSE

Action_read ==
    IF InRange /\\ (~(Len(inbox) = 0))
    THEN /\\ inbox' = SeqShift(inbox)
    ELSE FALSE

Stop ==
    IF (~InRange)
    THEN UNCHANGED vars
    ELSE FALSE

Next ==
    \\/ Action_deliver
    \\/ Action_read
    \\/ Stop

Rule_bills ==
    InRange => ((Len(inbox) < 2) \\/ (\\E item \\in SeqItems(inbox) : (item = \"Card\")))

====
";
    assert_eq!(files.module, expected);
    assert_eq!(
        files.config,
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\nINVARIANT Rule_bills\n"
    );
}

#[test]
fn a_model_with_an_optional_record_is_this_module() {
    let mut sketch = Sketch::new("Basket");
    let qty = sketch.sym("qty");
    let item = sketch.record(&[("qty", Domain::Int { lo: 1, hi: 2 })]);
    let chosen = sketch.field(
        "chosen",
        Domain::Optional(boxed(item)),
        vec![Value::record(vec![(qty, Value::Int(1))])],
    );
    let total = sketch.number("total", 0, 2, 0);
    sketch.action(
        "clear",
        two(Op::Differ, &chosen, null()),
        &[(&chosen, null())],
    );
    sketch.action(
        "price",
        two(Op::Equal, &total, int(0)),
        &[(&total, Expr::Get(Box::new(one(Op::Unwrap, &chosen)), qty))],
    );
    sketch.rule("capped", two(Op::Order(Order::LessOrEqual), &total, int(2)));
    sketch.end("settled", two(Op::Equal, &chosen, null()));
    let expected = "\
---- MODULE Basket ----
EXTENDS Integers, Sequences, FiniteSets, TLC

VARIABLES chosen, total

vars == <<chosen, total>>

InRange ==
    /\\ ((chosen = <<>>) \\/ (chosen[1].qty \\in 1..2))
    /\\ (total \\in 0..2)

Init ==
    /\\ chosen = <<[qty |-> 1]>>
    /\\ total = 0

Action_clear ==
    IF InRange /\\ (chosen # <<>>)
    THEN /\\ chosen' = <<>>
         /\\ UNCHANGED <<total>>
    ELSE FALSE

Action_price ==
    IF InRange /\\ (total = 0) /\\ (chosen # <<>>)
    THEN /\\ total' = chosen[1].qty
         /\\ UNCHANGED <<chosen>>
    ELSE FALSE

End_settled ==
    (chosen = <<>>)

Stop ==
    IF (~InRange) \\/ End_settled
    THEN UNCHANGED vars
    ELSE FALSE

Next ==
    \\/ Action_clear
    \\/ Action_price
    \\/ Stop

Rule_capped ==
    InRange => (total <= 2)

====
";
    assert_eq!(sketch.written().module, expected);
}

#[test]
fn a_model_with_two_ends_and_a_stuck_state_is_this_module() {
    let mut sketch = Sketch::new("Download");
    let phases = sketch.one_of(&["Idle", "Loading", "Done", "Failed"]);
    let [idle, loading, done, failed] =
        ["Idle", "Loading", "Done", "Failed"].map(|phase| Expr::Const(sketch.text(phase)));
    let Expr::Const(start) = idle.clone() else {
        panic!("a phase is a constant");
    };
    let phase = sketch.field("phase", phases, vec![start]);
    let saved = sketch.field("saved", Domain::Bool, vec![Value::Bool(false)]);
    sketch.action(
        "start",
        two(Op::Equal, &phase, idle),
        &[(&phase, loading.clone())],
    );
    sketch.action(
        "finish",
        two(Op::Equal, &phase, loading.clone()),
        &[(&phase, done.clone()), (&saved, yes())],
    );
    sketch.action(
        "fail",
        two(Op::Equal, &phase, loading),
        &[(&phase, failed.clone())],
    );
    sketch.rule(
        "whole",
        either(two(Op::Differ, &phase, done.clone()), saved.clone()),
    );
    sketch.end("done", two(Op::Equal, &phase, done));
    sketch.end(
        "gaveUp",
        both(two(Op::Equal, &phase, failed), one(Op::Not, &saved)),
    );
    let files = sketch.written();
    let expected = "\
---- MODULE Download ----
EXTENDS Integers, Sequences, FiniteSets, TLC

VARIABLES phase, saved

vars == <<phase, saved>>

InRange ==
    /\\ (phase \\in {\"Idle\", \"Loading\", \"Done\", \"Failed\"})
    /\\ (saved \\in BOOLEAN)

Init ==
    /\\ phase = \"Idle\"
    /\\ saved = FALSE

Action_start ==
    IF InRange /\\ (phase = \"Idle\")
    THEN /\\ phase' = \"Loading\"
         /\\ UNCHANGED <<saved>>
    ELSE FALSE

Action_finish ==
    IF InRange /\\ (phase = \"Loading\")
    THEN /\\ phase' = \"Done\"
         /\\ saved' = TRUE
    ELSE FALSE

Action_fail ==
    IF InRange /\\ (phase = \"Loading\")
    THEN /\\ phase' = \"Failed\"
         /\\ UNCHANGED <<saved>>
    ELSE FALSE

End_done ==
    (phase = \"Done\")

End_gaveUp ==
    ((phase = \"Failed\") /\\ (~saved))

Stop ==
    IF (~InRange) \\/ End_done \\/ End_gaveUp
    THEN UNCHANGED vars
    ELSE FALSE

Next ==
    \\/ Action_start
    \\/ Action_finish
    \\/ Action_fail
    \\/ Stop

Rule_whole ==
    InRange => ((phase # \"Done\") \\/ saved)

====
";
    assert_eq!(files.module, expected);
    assert_eq!(
        files.config,
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\nINVARIANT Rule_whole\n"
    );
}

#[test]
fn an_action_that_can_always_happen_is_guarded_only_by_the_range() {
    let mut sketch = Sketch::new("Tally");
    let count = sketch.number("count", 0, 3, 0);
    sketch.action(
        "add",
        yes(),
        &[(&count, two(Op::Math(Math::Add), &count, int(1)))],
    );
    let module = sketch.written().module;
    let expected = "\
Action_add ==
    IF InRange
    THEN /\\ count' = (count + 1)
    ELSE FALSE
";
    assert!(module.contains(expected), "{module}");
}

#[test]
fn an_action_with_two_inputs_tries_every_pair_of_their_values() {
    let mut sketch = Sketch::new("Grid");
    let x = sketch.number("x", 0, 1, 0);
    let lit = sketch.field("lit", Domain::Bool, vec![Value::Bool(false)]);
    sketch.action_with(
        "set",
        vec![
            input("to", 0, Domain::Int { lo: 0, hi: 1 }),
            input("on", 1, Domain::Bool),
        ],
        two(Op::Differ, &x, Expr::Local(0)),
        &[(&x, Expr::Local(0)), (&lit, Expr::Local(1))],
    );
    let module = sketch.written().module;
    let expected = "\
Action_set ==
    \\E to \\in {0, 1}, on \\in {FALSE, TRUE} :
        IF InRange /\\ (x # to)
        THEN /\\ x' = to
             /\\ lit' = on
        ELSE FALSE
";
    assert!(module.contains(expected), "{module}");
}

#[test]
fn an_update_past_the_last_field_is_left_out_as_the_engine_ignores_it() {
    let mut model = counter().model();
    model.actions[0].updates.push(Some(int(9).into()));
    let module = earnest_tla::write(&model).module;
    assert!(
        module.contains("    THEN /\\ count' = (count + 1)\n    ELSE FALSE\n"),
        "{module}"
    );
}

#[test]
fn a_model_without_fields_rules_or_ends_still_has_one_state_to_stand_in() {
    let mut sketch = Sketch::new("Empty");
    sketch.action("wait", yes(), &[]);
    let expected = Files {
        name: "Empty".to_owned(),
        module: "\
---- MODULE Empty ----
EXTENDS Integers, Sequences, FiniteSets, TLC

VARIABLES nothing

vars == <<nothing>>

InRange ==
    TRUE

Init ==
    /\\ nothing = 0

Action_wait ==
    IF InRange
    THEN /\\ UNCHANGED <<nothing>>
    ELSE FALSE

Stop ==
    IF (~InRange)
    THEN UNCHANGED vars
    ELSE FALSE

Next ==
    \\/ Action_wait
    \\/ Stop

====
"
        .to_owned(),
        config: "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n".to_owned(),
        rules: Vec::new(),
    };
    assert_eq!(sketch.written(), expected);
}

#[test]
fn a_model_without_actions_can_only_stop() {
    let mut sketch = Sketch::new("Still");
    sketch.number("n", 0, 1, 0);
    let module = sketch.written().module;
    assert!(module.contains("Next ==\n    \\/ Stop\n"), "{module}");
}

#[test]
fn the_configuration_names_one_invariant_per_rule_in_order() {
    let mut sketch = counter();
    sketch.rule("even", yes());
    assert_eq!(
        sketch.written().config,
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\nINVARIANT Rule_bounded\nINVARIANT Rule_even\n"
    );
}
