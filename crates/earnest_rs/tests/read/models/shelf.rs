use earnest::model::{Domain, Model, Seq, model, require};

const TOP: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Cover {
    Soft,
    Hard,
}

fn pick(_first: Cover, second: Cover) -> Cover {
    second
}

fn double(number: u8) -> u8 {
    number + number
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Shelf {
    #[range(0, TOP)]
    #[initial(0)]
    count: u8,
    books: Seq<Cover, 2>,
}

#[model]
impl Shelf {
    fn pick(&self, second: Cover) -> Cover {
        if self.books.is_empty() {
            second
        } else {
            Cover::Hard
        }
    }

    fn below(&self, limit: u8) -> bool {
        self.count < limit
    }

    #[action("a book is shelved")]
    fn shelve(&self, cover: Cover) -> Option<Self> {
        require!(self.below(TOP) && self.books.size() < 2);
        let step: u8 = 1_u8;
        Some(Self {
            count: self.count + step,
            books: self.books.push(cover),
        })
    }

    #[rule("The function gives back its second argument")]
    fn outside(&self) -> bool {
        pick(Cover::Hard, Cover::Soft) == Cover::Soft && self.count <= TOP
    }

    #[rule("The method gives back its argument while the shelf is empty")]
    fn inside(&self) -> bool {
        self.pick(Cover::Soft) == Cover::Soft
    }

    #[rule("The count is below twice one")]
    fn low(&self) -> bool {
        self.below(double(1))
    }

    #[rule("One fewer than the books is none")]
    fn spare(&self) -> bool {
        self.books.size() - 1 == 0
    }

    #[end("The shelf is full")]
    fn full(&self) -> bool {
        self.books.size() == 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_method_and_the_function_of_one_name_are_two_functions() {
        let empty = Shelf::starts().remove(0);
        assert!(empty.outside(), "the function gives its second argument");
        assert!(empty.inside(), "the method: the shelf is empty");
        let one = empty.shelve(Cover::Soft).unwrap();
        assert!(one.outside(), "the function does not read the shelf");
        assert!(!one.inside(), "the method: the shelf holds a book");
        assert!(one.low(), "one is below two");
        let two = one.shelve(Cover::Hard).unwrap();
        assert!(!two.low(), "two is not below two");
        assert!(!two.spare(), "two books less one is one");
        assert!(two.full() && two.shelve(Cover::Soft).is_none(), "the end");
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "attempt to subtract with overflow")]
    fn one_fewer_than_no_books_stops_a_test_build() {
        let empty = Shelf::starts().remove(0);
        assert!(empty.spare(), "never reached: the subtraction panics");
    }
}
