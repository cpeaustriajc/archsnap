use earnest::model::{Domain, Model, model, require};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Mode {
    Slow,
    Fast,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Line {
    #[range(0, 2)]
    qty: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Basket {
    #[range(1, 3)]
    #[initial(None)]
    last: Option<u8>,
    #[initial(None)]
    mode: Option<Mode>,
    #[range(0, 9)]
    #[initial(0)]
    total: u8,
    #[range(0, 9)]
    #[initial(None)]
    peak: Option<u8>,
    #[initial(None)]
    line: Option<Line>,
}

#[model]
impl Basket {
    fn bonus(&self) -> u8 {
        if let Some(x) = self.last { x * 2 } else { 1 }
    }

    fn quantity(&self) -> u8 {
        self.line.clone().map_or(0, |line| line.qty)
    }

    #[action("a number is put in")]
    fn put(&self, #[range(1, 3)] n: u8) -> Self {
        Self {
            last: Some(n),
            ..self.clone()
        }
    }

    #[action("a mode is chosen")]
    fn choose(&self, mode: Mode) -> Self {
        Self {
            mode: Some(mode),
            ..self.clone()
        }
    }

    #[action("a line of the basket is made")]
    fn line_up(&self, #[range(0, 2)] qty: u8) -> Self {
        Self {
            line: Some(Line { qty }),
            ..self.clone()
        }
    }

    #[action("the last number is added to the total")]
    fn add(&self) -> Option<Self> {
        let x = self.last?;
        require!(self.total + x <= 9);
        Some(Self {
            total: self.total + x,
            last: None,
            ..self.clone()
        })
    }

    #[action("the peak is raised")]
    fn raise(&self) -> Option<Self> {
        require!(self.total > 0 && self.peak.unwrap_or(0) < 5);
        Some(Self {
            peak: Some(self.peak.map_or(self.total, |top| top + 1)),
            ..self.clone()
        })
    }

    #[action("the chosen mode is shown")]
    fn show(&self) -> Option<Self> {
        require!(self.total > 1);
        if let Some(mode) = self.mode {
            Some(Self {
                last: Some(if mode == Mode::Fast { 3 } else { 1 }),
                mode: None,
                ..self.clone()
            })
        } else {
            Some(Self {
                total: 0,
                ..self.clone()
            })
        }
    }

    #[action("the line is emptied")]
    fn empty(&self) -> Option<Self> {
        let line = self.line.clone()?;
        require!(line.qty > 0);
        Some(Self {
            line: Some(Line { qty: line.qty - 1 }),
            ..self.clone()
        })
    }

    #[rule("The last number weighs less than six")]
    fn light(&self) -> bool {
        self.last.map_or(0, |x| x * 2) < 6
    }

    #[rule("The total stays below eight")]
    fn small(&self) -> bool {
        self.total < 8
    }

    #[rule("The bonus is never four")]
    fn bonus_never_four(&self) -> bool {
        self.bonus() != 4
    }

    #[rule("A peak is never above the total plus two")]
    fn low_peak(&self) -> bool {
        self.peak.unwrap_or(0) <= self.total + 2
    }

    #[rule("A fast mode comes with a total of eight or less")]
    fn fast_is_small(&self) -> bool {
        self.mode != Some(Mode::Fast) || self.total <= 8
    }

    #[rule("A mode that is chosen is on the list")]
    fn listed(&self) -> bool {
        self.mode.is_none() || self.mode == Some(Mode::Slow) || self.mode == Some(Mode::Fast)
    }

    #[rule("The line holds two or fewer")]
    fn short_line(&self) -> bool {
        self.quantity() <= 2
    }
}
