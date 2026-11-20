use earnest::model::{Behavior, Domain, Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Parcel {
    #[one_of(2, 4)]
    weight: u8,
    #[one_of("air", "sea")]
    route: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Shipment {
    #[one_of(1, 5, 9)]
    size: u8,
    #[one_of("Kim", "Lee")]
    #[initial(None)]
    carrier: Option<&'static str>,
    #[one_of(10, 20)]
    #[initial(20)]
    fee: u16,
    parcel: Parcel,
}

#[model]
impl Shipment {
    #[action("the shipper picks a size")]
    fn resize(&self, #[one_of(1, 5, 9)] to: u8) -> Option<Self> {
        require!(self.size != to);
        Some(Self {
            size: to,
            ..self.clone()
        })
    }

    #[action("a carrier takes the shipment")]
    fn assign(&self, #[one_of("Kim", "Lee")] who: &'static str) -> Option<Self> {
        require!(self.carrier.is_none());
        Some(Self {
            carrier: Some(who),
            ..self.clone()
        })
    }

    #[rule("A shipment has a size")]
    fn sized(&self) -> bool {
        self.size > 0
    }
}

fn shipment(size: u8, carrier: Option<&'static str>, route: &'static str) -> Shipment {
    Shipment {
        size,
        carrier,
        fee: 20,
        parcel: Parcel { weight: 2, route },
    }
}

#[test]
fn a_listed_number_and_a_listed_text_start_at_every_value_in_the_order_listed() {
    let starts = Shipment::starts();
    assert_eq!(starts.len(), 12, "3 sizes times 4 parcels");
    assert_eq!(
        starts[0],
        shipment(1, None, "air"),
        "the first of each list"
    );
    let sizes: Vec<u8> = starts.iter().map(|start| start.size).collect();
    assert_eq!(
        sizes,
        [1, 1, 1, 1, 5, 5, 5, 5, 9, 9, 9, 9],
        "the first field is the slowest"
    );
}

#[test]
fn a_listed_value_in_a_record_is_listed_in_the_records_domain() {
    let parcels: Vec<(u8, &str)> = Parcel::values()
        .into_iter()
        .map(|parcel| (parcel.weight, parcel.route))
        .collect();
    assert_eq!(
        parcels,
        [(2, "air"), (2, "sea"), (4, "air"), (4, "sea")],
        "each weight with each route"
    );
}

#[test]
fn an_initial_value_still_wins_over_the_list() {
    assert!(
        Shipment::starts().iter().all(|start| start.fee == 20),
        "the fee starts at its initial value"
    );
}

#[test]
fn an_input_that_lists_values_is_tried_with_each_of_them() {
    let start = shipment(1, None, "air");
    let steps = start.steps();
    let tried: Vec<(&str, Vec<(&str, String)>)> = steps
        .iter()
        .map(|step| (step.action, step.input.clone()))
        .collect();
    assert_eq!(
        tried,
        [
            ("resize", vec![("to", "5".to_owned())]),
            ("resize", vec![("to", "9".to_owned())]),
            ("assign", vec![("who", "\"Kim\"".to_owned())]),
            ("assign", vec![("who", "\"Lee\"".to_owned())]),
        ],
        "the guard drops the size already held; a text is printed with its quotes"
    );
    assert_eq!(
        steps[2].state,
        shipment(1, Some("Kim"), "air"),
        "the text reaches the state"
    );
}
