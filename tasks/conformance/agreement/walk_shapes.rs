use earnest::model::{Domain, Map, Model, Seq, Set, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Tone {
    Low,
    Bright,
    Mid,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Spot {
    #[range(0, 2)]
    zebra: u8,
    #[range(0, 2)]
    apple: u8,
    tone: Tone,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Shapes {
    #[range(0, 1)]
    #[initial(0)]
    n: u8,
    #[range(0, 3)]
    #[initial(Set::of([3, 1, 2]))]
    nums: Set<u8>,
    #[initial(Set::of([true, false]))]
    flags: Set<bool>,
    #[initial(Set::of([
        Spot { zebra: 0, apple: 2, tone: Tone::Low },
        Spot { zebra: 2, apple: 1, tone: Tone::Mid },
        Spot { zebra: 1, apple: 1, tone: Tone::Bright },
        Spot { zebra: 1, apple: 1, tone: Tone::Low },
    ]))]
    spots: Set<Spot>,
    #[range(0, 3)]
    #[initial(Set::of([Set::of([2]), Set::of([1, 3]), Set::of([1, 2]), Set::of([])]))]
    groups: Set<Set<u8>>,
    #[initial(Set::of([
        Seq::of([Tone::Mid]),
        Seq::of([Tone::Low, Tone::Mid]),
        Seq::of([Tone::Low]),
        Seq::of([]),
    ]))]
    lines: Set<Seq<Tone, 2>>,
    #[range(0, 3)]
    #[initial(Set::of([
        Map::of([(Tone::Mid, 1)]),
        Map::of([(Tone::Bright, 2)]),
        Map::of([(Tone::Bright, 1), (Tone::Mid, 1)]),
        Map::of([(Tone::Bright, 1)]),
    ]))]
    maps: Set<Map<Tone, u8>>,
    #[initial(Map::of([
        (Spot { zebra: 2, apple: 0, tone: Tone::Low }, Tone::Low),
        (Spot { zebra: 0, apple: 2, tone: Tone::Low }, Tone::Mid),
        (Spot { zebra: 0, apple: 2, tone: Tone::Bright }, Tone::Bright),
    ]))]
    grid: Map<Spot, Tone>,
}

#[model]
impl Shapes {
    #[action("the shapes are opened")]
    fn open(&self) -> Option<Self> {
        require!(self.n == 0);
        Some(Self {
            n: 1,
            ..self.clone()
        })
    }

    #[rule("The shapes are open")]
    fn shown(&self) -> bool {
        self.n == 1
    }

    #[rule("The least number is walked first")]
    fn nums_ascend(&self) -> bool {
        self.nums.some(|x| x == 1 || 6 / self.n > 1)
    }

    #[rule("The greatest number is walked first")]
    fn nums_do_not_descend(&self) -> bool {
        self.nums.some(|x| x == 3 || 6 / self.n > 1)
    }

    #[rule("False is walked first")]
    fn false_first(&self) -> bool {
        self.flags.some(|x| !x || 6 / self.n > 1)
    }

    #[rule("The record with the least apple, then zebra, then tone is walked first")]
    fn spots_by_names(&self) -> bool {
        self.spots
            .some(|x| (x.apple == 1 && x.zebra == 1 && x.tone == Tone::Bright) || 6 / self.n > 1)
    }

    #[rule("The record with the least zebra is walked first")]
    fn spots_not_in_declared_order(&self) -> bool {
        self.spots.some(|x| x.zebra == 0 || 6 / self.n > 1)
    }

    #[rule("The empty group is walked first")]
    fn empty_group_first(&self) -> bool {
        self.groups.some(|x| x.is_empty() || 6 / self.n > 1)
    }

    #[rule("A group is walked before every group that starts with it")]
    fn prefix_first(&self) -> bool {
        self.groups
            .every(|x| x.is_empty() || x.has(1) || x.has(2) || 6 / self.n > 1)
    }

    #[rule("The empty line is walked first")]
    fn short_line_first(&self) -> bool {
        self.lines.some(|x| x.is_empty() || 6 / self.n > 1)
    }

    #[rule("The line of Low alone is walked before the line of Mid alone")]
    fn low_line_before_mid(&self) -> bool {
        self.lines.some(|x| x.first() == Some(Tone::Mid) || 6 / self.n > 1)
            && self.lines.some(|x| x.first() == Some(Tone::Low) || 6 / self.n > 1)
    }

    #[rule("The map of Bright to 1 alone is walked first")]
    fn bright_map_first(&self) -> bool {
        self.maps
            .some(|x| (x.size() == 1 && x.get(Tone::Bright) == Some(1)) || 6 / self.n > 1)
    }

    #[end("The shapes are open")]
    fn done(&self) -> bool {
        self.n == 1
    }
}
