use earnest::model::{Domain, Model, model, require, view};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Cta {
    Add,
    Adding,
    InCart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Focus {
    Add,
    InCart,
    Lost,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Cart {
    #[one_of("results", "shortlist")]
    surface: &'static str,
    #[initial(Cta::Add)]
    cta: Cta,
    #[initial(Focus::Add)]
    focus: Focus,
}

#[model]
impl Cart {
    #[action("the buyer presses Add")]
    fn press(&self) -> Option<Self> {
        require!(self.cta == Cta::Add);
        Some(Self {
            cta: Cta::Adding,
            ..self.clone()
        })
    }

    #[action("the add lands")]
    fn land(&self) -> Option<Self> {
        require!(self.cta == Cta::Adding);
        Some(Self {
            cta: Cta::InCart,
            focus: if self.surface == "results" {
                Focus::InCart
            } else {
                Focus::Lost
            },
            ..self.clone()
        })
    }

    #[rule("After an add lands, focus is on In cart")]
    fn focus_lands(&self) -> bool {
        self.cta != Cta::InCart || self.focus == Focus::InCart
    }

    #[end("The item is in the cart")]
    fn in_cart(&self) -> bool {
        self.cta == Cta::InCart
    }
}

#[view(
    title = "Add to cart focus",
    step(press = "press Add"),
    colors(broken = "#c62828")
)]
impl Cart {
    #[state]
    fn state(&self) -> String {
        format!("{:?}, focus on {:?}", self.cta, self.focus)
    }

    #[group]
    fn group(&self) -> String {
        self.surface.to_owned()
    }
}
