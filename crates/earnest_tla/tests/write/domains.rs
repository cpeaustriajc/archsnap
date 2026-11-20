use crate::build::{SMALL, Sketch, body_of, boxed, helpers_of, map_value, set_value};
use earnest_value::{Domain, Value};

fn declared(name: &str, domain: Domain, starts: Vec<Value>) -> (String, String) {
    let mut sketch = Sketch::new("Declared");
    sketch.field(name, domain, starts);
    lines_of(sketch)
}

fn lines_of(sketch: Sketch) -> (String, String) {
    let module = sketch.written().module;
    let range = body_of(&module, "InRange");
    let starts = body_of(&module, "Init");
    let ([in_range], [start]) = (range.as_slice(), starts.as_slice()) else {
        panic!("one field gives one line each, not {range:?} and {starts:?}");
    };
    (in_range.clone(), start.clone())
}

#[test]
fn a_boolean_is_in_the_set_of_booleans_and_starts_at_each_listed_value() {
    let both = vec![Value::Bool(false), Value::Bool(true)];
    assert_eq!(
        declared("lit", Domain::Bool, both),
        (
            "/\\ (lit \\in BOOLEAN)".to_owned(),
            "/\\ lit \\in {FALSE, TRUE}".to_owned()
        )
    );
}

#[test]
fn a_number_is_in_its_range_and_a_single_start_is_an_equality() {
    assert_eq!(
        declared("level", Domain::Int { lo: -1, hi: 3 }, vec![Value::Int(0)]),
        (
            "/\\ (level \\in (-1)..3)".to_owned(),
            "/\\ level = 0".to_owned()
        )
    );
}

#[test]
fn a_field_with_no_start_value_has_no_start_state() {
    assert_eq!(
        declared("level", SMALL, Vec::new()),
        (
            "/\\ (level \\in 0..2)".to_owned(),
            "/\\ level \\in {}".to_owned()
        )
    );
}

#[test]
fn an_enum_is_in_the_set_of_its_texts() {
    let mut sketch = Sketch::new("Declared");
    let doors = sketch.one_of(&["open", "closed"]);
    let start = sketch.text("closed");
    sketch.field("door", doors, vec![start]);
    assert_eq!(
        lines_of(sketch),
        (
            "/\\ (door \\in {\"open\", \"closed\"})".to_owned(),
            "/\\ door = \"closed\"".to_owned()
        )
    );
}

#[test]
fn an_optional_is_the_empty_tuple_or_a_tuple_of_a_declared_value() {
    let mut sketch = Sketch::new("Declared");
    let owners = Domain::Optional(boxed(sketch.one_of(&["Ada", "Bo"])));
    let starts = owners.values();
    sketch.field("owner", owners, starts);
    assert_eq!(
        lines_of(sketch),
        (
            "/\\ ((owner = <<>>) \\/ (owner[1] \\in {\"Ada\", \"Bo\"}))".to_owned(),
            "/\\ owner \\in {<<>>, <<\"Ada\">>, <<\"Bo\">>}".to_owned()
        )
    );
}

#[test]
fn a_record_is_in_range_field_by_field() {
    let mut sketch = Sketch::new("Declared");
    let seat = sketch.record(&[
        ("row", Domain::Int { lo: 1, hi: 2 }),
        ("taken", Domain::Optional(boxed(Domain::Bool))),
    ]);
    let row = sketch.sym("row");
    let taken = sketch.sym("taken");
    let starts = vec![
        Value::record(vec![(row, Value::Int(1)), (taken, Value::Null)]),
        Value::record(vec![(row, Value::Int(2)), (taken, Value::Bool(true))]),
    ];
    sketch.field("seat", seat, starts);
    assert_eq!(
        lines_of(sketch),
        (
            "/\\ ((seat.row \\in 1..2) /\\ ((seat.taken = <<>>) \\/ (seat.taken[1] \\in BOOLEAN)))"
                .to_owned(),
            "/\\ seat \\in {[row |-> 1, taken |-> <<>>], [row |-> 2, taken |-> <<TRUE>>]}"
                .to_owned()
        )
    );
}

#[test]
fn a_set_is_in_range_when_each_item_is() {
    assert_eq!(
        declared(
            "seen",
            Domain::Set(boxed(SMALL)),
            vec![set_value(Vec::new())]
        ),
        (
            "/\\ (\\A item \\in seen : (item \\in 0..2))".to_owned(),
            "/\\ seen = {}".to_owned()
        )
    );
    let some = set_value(vec![Value::Int(2), Value::Int(0)]);
    assert_eq!(
        declared("seen", Domain::Set(boxed(SMALL)), vec![some]).1,
        "/\\ seen = {0, 2}"
    );
}

#[test]
fn a_map_is_in_range_when_each_key_and_its_value_are() {
    let mut sketch = Sketch::new("Declared");
    let items = sketch.one_of(&["Pen", "Ink"]);
    let pen = sketch.text("Pen");
    sketch.field(
        "stock",
        Domain::Map(boxed(items.clone()), boxed(SMALL)),
        vec![map_value(Vec::new())],
    );
    sketch.field(
        "held",
        Domain::Map(boxed(items), boxed(SMALL)),
        vec![map_value(vec![(pen, Value::Int(1))])],
    );
    let module = sketch.written().module;
    assert_eq!(
        body_of(&module, "InRange"),
        [
            "/\\ (\\A key \\in DOMAIN stock : ((key \\in {\"Pen\", \"Ink\"}) /\\ \
             (stock[key] \\in 0..2)))",
            "/\\ (\\A key_2 \\in DOMAIN held : ((key_2 \\in {\"Pen\", \"Ink\"}) /\\ \
             (held[key_2] \\in 0..2)))",
        ]
    );
    assert_eq!(
        body_of(&module, "Init"),
        [
            "/\\ stock = EmptyMap",
            "/\\ held = MapSet(EmptyMap, \"Pen\", 1)"
        ]
    );
    assert_eq!(
        helpers_of(&module),
        [
            "EmptyMap == [key \\in {} |-> key]",
            "MapSet(map, key, value) == [other \\in (DOMAIN map) \\cup {key} |-> \
             IF other = key THEN value ELSE map[other]]",
        ]
    );
}

#[test]
fn a_sequence_is_in_range_when_it_is_short_enough_and_each_item_is() {
    let mut sketch = Sketch::new("Declared");
    let stops = sketch.one_of(&["North", "South"]);
    let trail = Domain::Seq {
        item: boxed(stops),
        max: 2,
    };
    let starts = vec![
        Value::seq(Vec::new()),
        Value::seq(sketch.texts(&["South", "North"])),
    ];
    sketch.field("trail", trail, starts);
    assert_eq!(
        lines_of(sketch),
        (
            "/\\ ((Len(trail) <= 2) /\\ (\\A index \\in DOMAIN trail : \
             (trail[index] \\in {\"North\", \"South\"})))"
                .to_owned(),
            "/\\ trail \\in {<<>>, <<\"South\", \"North\">>}".to_owned()
        )
    );
}

#[test]
fn a_collection_in_a_collection_gives_each_level_its_own_name() {
    let nest = Domain::Set(boxed(Domain::Set(boxed(SMALL))));
    let start = set_value(vec![set_value(vec![Value::Int(1)])]);
    assert_eq!(
        declared("nest", nest, vec![start]),
        (
            "/\\ (\\A item \\in nest : (\\A item_2 \\in item : (item_2 \\in 0..2)))".to_owned(),
            "/\\ nest = {{1}}".to_owned()
        )
    );
}

#[test]
fn an_optional_sequence_of_optional_items_keeps_absent_and_empty_apart() {
    let rows = Domain::Optional(boxed(Domain::Seq {
        item: boxed(Domain::Optional(boxed(SMALL))),
        max: 1,
    }));
    let starts = vec![
        Value::Null,
        Value::seq(Vec::new()),
        Value::seq(vec![Value::Null]),
        Value::seq(vec![Value::Int(1)]),
    ];
    assert_eq!(
        declared("rows", rows, starts),
        (
            "/\\ ((rows = <<>>) \\/ ((Len(rows[1]) <= 1) /\\ (\\A index \\in DOMAIN rows[1] : \
             ((rows[1][index] = <<>>) \\/ (rows[1][index][1] \\in 0..2)))))"
                .to_owned(),
            "/\\ rows \\in {<<>>, << <<>> >>, << << <<>> >> >>, << << <<1>> >> >>}".to_owned()
        )
    );
}

#[test]
fn a_record_without_fields_is_always_in_range() {
    assert_eq!(
        declared(
            "unit",
            Domain::Record(Vec::new()),
            vec![Value::record(Vec::new())]
        ),
        ("/\\ TRUE".to_owned(), "/\\ unit = <<>>".to_owned())
    );
}

#[test]
fn every_field_gets_a_line_in_declaration_order() {
    let mut sketch = Sketch::new("Declared");
    sketch.field("lit", Domain::Bool, vec![Value::Bool(false)]);
    sketch.number("level", 0, 2, 1);
    let module = sketch.written().module;
    assert_eq!(
        body_of(&module, "InRange"),
        ["/\\ (lit \\in BOOLEAN)", "/\\ (level \\in 0..2)"]
    );
    assert_eq!(
        body_of(&module, "Init"),
        ["/\\ lit = FALSE", "/\\ level = 1"]
    );
}
