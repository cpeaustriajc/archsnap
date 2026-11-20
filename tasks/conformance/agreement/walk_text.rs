use earnest::model::{Domain, Map, Model, Set, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Fruit {
    Pear,
    Apple,
    Fig,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Shelf {
    #[range(0, 1)]
    #[initial(0)]
    n: u8,
    #[initial(Set::of([Fruit::Pear, Fruit::Fig, Fruit::Apple]))]
    fruit: Set<Fruit>,
    #[one_of("b", "a", "B")]
    #[initial(Set::of(["b", "a", "B"]))]
    names: Set<&'static str>,
    #[range(0, 3)]
    #[initial(Map::of([(Fruit::Pear, 1), (Fruit::Fig, 2), (Fruit::Apple, 3)]))]
    stock: Map<Fruit, u8>,
    #[one_of("b", "a", "B")]
    #[initial(Set::of([Some("b"), None, Some("a")]))]
    maybe: Set<Option<&'static str>>,
}

#[model]
impl Shelf {
    #[action("the shelf is opened")]
    fn open(&self) -> Option<Self> {
        require!(self.n == 0);
        Some(Self {
            n: 1,
            ..self.clone()
        })
    }

    #[rule("The shelf is open")]
    fn shown(&self) -> bool {
        self.n == 1
    }

    #[rule("Apple is the first fruit walked")]
    fn apple_first(&self) -> bool {
        self.fruit.some(|x| x == Fruit::Apple || 6 / self.n > 1)
    }

    #[rule("Pear is the first fruit walked")]
    fn pear_first(&self) -> bool {
        self.fruit.some(|x| x == Fruit::Pear || 6 / self.n > 1)
    }

    #[rule("The capital B is the first name walked")]
    fn upper_first(&self) -> bool {
        self.names.some(|x| x == "B" || 6 / self.n > 1)
    }

    #[rule("The small b is the first name walked")]
    fn lower_first(&self) -> bool {
        self.names.some(|x| x == "b" || 6 / self.n > 1)
    }

    #[rule("The absent name is walked first")]
    fn null_first(&self) -> bool {
        self.maybe.some(|x| x.is_none() || 6 / self.n > 1)
    }

    #[rule("The small a is walked before the small b")]
    fn a_first_of_some(&self) -> bool {
        self.maybe
            .every(|x| x.is_none() || x == Some("a") || 6 / self.n > 1)
    }

    #[end("The shelf is open")]
    fn done(&self) -> bool {
        self.n == 1
    }
}
