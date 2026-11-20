use earnest::model::{Model, View, model, require, view};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Lamp {
    #[range(0, 2)]
    #[initial(0)]
    level: u8,
    #[initial(false)]
    on: bool,
}

#[model]
impl Lamp {
    #[action("the lamp brightens")]
    fn brighten(&self) -> Option<Self> {
        require!(self.level < 2);
        Some(Self {
            level: self.level + 1,
            ..self.clone()
        })
    }

    #[action("the lamp is switched")]
    fn toggle(&self) -> Self {
        Self {
            on: !self.on,
            ..self.clone()
        }
    }

    #[rule("The lamp is never at the top while off")]
    fn lit(&self) -> bool {
        self.level < 2 || self.on
    }
}

#[view(
    title = "Lamp",
    step(brighten = "brighter", toggle = "click"),
    colors(end = "seagreen")
)]
impl Lamp {
    #[state]
    fn state(&self) -> String {
        format!("level {}, {{{}}}", self.level, self.on)
    }

    #[group]
    fn group(&self) -> String {
        self.on.to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Bare {
    #[initial(true)]
    open: bool,
}

#[model]
impl Bare {
    #[rule("The door is open")]
    fn open(&self) -> bool {
        self.open
    }
}

#[view]
impl Bare {}

#[test]
fn a_view_gives_the_labels_the_chart_shows() {
    let lamp = Lamp { level: 1, on: true };
    assert_eq!(lamp.state_label().as_deref(), Some("level 1, {true}"));
    assert_eq!(lamp.group_label().as_deref(), Some("true"));
    assert_eq!(Lamp::title(), Some("Lamp"));
    assert_eq!(
        Lamp::step_labels(),
        [("brighten", "brighter"), ("toggle", "click")]
    );
    assert_eq!(Lamp::colors(), [("end", "seagreen")]);
}

#[test]
fn a_view_with_nothing_in_it_labels_nothing() {
    let bare = Bare { open: true };
    assert_eq!(bare.state_label(), None);
    assert_eq!(bare.group_label(), None);
    assert_eq!(Bare::title(), None);
    assert!(Bare::step_labels().is_empty());
    assert!(Bare::colors().is_empty());
    assert!(bare.open());
}

#[test]
fn an_action_that_nothing_can_stop_returns_the_state_and_is_always_a_step() {
    let start = Lamp {
        level: 2,
        on: false,
    };
    assert_eq!(start.toggle(), Lamp { level: 2, on: true });
    let ids: Vec<&str> = earnest::model::Behavior::steps(&start)
        .iter()
        .map(|step| step.action)
        .collect();
    assert_eq!(ids, ["toggle"], "brighten is stopped at the top");
}
