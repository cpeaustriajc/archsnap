use earnest::model::{Model, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Grid100k {
    #[range(0, 9)]
    #[initial(0)]
    a: u8,
    #[range(0, 9)]
    #[initial(0)]
    b: u8,
    #[range(0, 9)]
    #[initial(0)]
    c: u8,
    #[range(0, 9)]
    #[initial(0)]
    d: u8,
    #[range(0, 9)]
    #[initial(0)]
    e: u8,
}

#[model]
impl Grid100k {
    #[action("wheel a turns")]
    fn turn_a(&self) -> Option<Self> {
        require!(self.a < 9);
        Some(Self { a: self.a + 1, ..self.clone() })
    }

    #[action("wheel b turns")]
    fn turn_b(&self) -> Option<Self> {
        require!(self.b < 9);
        Some(Self { b: self.b + 1, ..self.clone() })
    }

    #[action("wheel c turns")]
    fn turn_c(&self) -> Option<Self> {
        require!(self.c < 9);
        Some(Self { c: self.c + 1, ..self.clone() })
    }

    #[action("wheel d turns")]
    fn turn_d(&self) -> Option<Self> {
        require!(self.d < 9);
        Some(Self { d: self.d + 1, ..self.clone() })
    }

    #[action("wheel e turns")]
    fn turn_e(&self) -> Option<Self> {
        require!(self.e < 9);
        Some(Self { e: self.e + 1, ..self.clone() })
    }

    #[rule("Every wheel stays on its dial")]
    fn inside(&self) -> bool {
        self.a <= 9 && self.b <= 9 && self.c <= 9 && self.d <= 9 && self.e <= 9
    }

    #[end("Every wheel is at its last notch")]
    fn stopped(&self) -> bool {
        self.a == 9 && self.b == 9 && self.c == 9 && self.d == 9 && self.e == 9
    }
}
