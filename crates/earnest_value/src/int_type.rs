/// The largest whole number a JavaScript number holds exactly, 2^53 - 1. A number of a
/// TypeScript model file is a JavaScript number, so it reaches this far in either direction and
/// no further.
pub const MOST_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// An integer type of Rust, which every number in a Rust model file has.
///
/// A [`Value`](crate::Value) holds a number in 64 bits with a sign, so the bounds given here
/// stop at the bounds of `i64`: `u64`, `u128`, `usize`, `i128` and `isize` reach as far as
/// `i64` does on the side where they are wider. `usize` and `isize` are taken as 64 bits wide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum IntType {
    /// `u8`: 0 to 255.
    U8,
    /// `u16`: 0 to 65535.
    U16,
    /// `u32`: 0 to 4294967295.
    U32,
    /// `u64`, here 0 to the largest `i64`.
    U64,
    /// `u128`, here 0 to the largest `i64`.
    U128,
    /// `usize`, here 0 to the largest `i64`. A size and a position have this type.
    Usize,
    /// `i8`: -128 to 127.
    I8,
    /// `i16`: -32768 to 32767.
    I16,
    /// `i32`: -2147483648 to 2147483647. A number nothing gives another type has this one.
    I32,
    /// `i64`.
    I64,
    /// `i128`, here as wide as `i64`.
    I128,
    /// `isize`, here as wide as `i64`.
    Isize,
}

const EVERY_TYPE: [IntType; 12] = [
    IntType::U8,
    IntType::U16,
    IntType::U32,
    IntType::U64,
    IntType::U128,
    IntType::Usize,
    IntType::I8,
    IntType::I16,
    IntType::I32,
    IntType::I64,
    IntType::I128,
    IntType::Isize,
];

impl IntType {
    /// The type Rust writes as `name`, such as `"u8"`. `None` for any other text.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        EVERY_TYPE.into_iter().find(|each| each.name() == name)
    }

    /// The type as Rust writes it: `u8`, `usize`, `i32`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::Usize => "usize",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::Isize => "isize",
        }
    }

    /// The smallest number of the type that a value can hold: 0 for a type without a sign.
    #[must_use]
    pub fn lowest(self) -> i64 {
        match self {
            Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::U128 | Self::Usize => 0,
            Self::I8 => i64::from(i8::MIN),
            Self::I16 => i64::from(i16::MIN),
            Self::I32 => i64::from(i32::MIN),
            Self::I64 | Self::I128 | Self::Isize => i64::MIN,
        }
    }

    /// The largest number of the type that a value can hold.
    #[must_use]
    pub fn highest(self) -> i64 {
        match self {
            Self::U8 => i64::from(u8::MAX),
            Self::U16 => i64::from(u16::MAX),
            Self::U32 => i64::from(u32::MAX),
            Self::I8 => i64::from(i8::MAX),
            Self::I16 => i64::from(i16::MAX),
            Self::I32 => i64::from(i32::MAX),
            Self::U64 | Self::U128 | Self::Usize | Self::I64 | Self::I128 | Self::Isize => i64::MAX,
        }
    }

    /// Whether `number` is a value of the type, from [`lowest`](Self::lowest) to
    /// [`highest`](Self::highest), both included.
    #[must_use]
    pub fn holds(self, number: i64) -> bool {
        self.lowest() <= number && number <= self.highest()
    }
}

#[cfg(test)]
mod tests {
    use super::{EVERY_TYPE, IntType};

    #[test]
    fn every_type_is_found_by_the_name_rust_gives_it() {
        for each in EVERY_TYPE {
            assert_eq!(IntType::named(each.name()), Some(each), "{}", each.name());
        }
        assert_eq!(IntType::named("u8"), Some(IntType::U8), "u8");
        assert_eq!(IntType::named("isize"), Some(IntType::Isize), "isize");
        assert_eq!(IntType::named("f32"), None, "a float is no integer type");
        assert_eq!(IntType::named("U8"), None, "the name is lower case");
    }

    #[test]
    fn a_type_without_a_sign_starts_at_zero() {
        for each in [
            IntType::U8,
            IntType::U16,
            IntType::U32,
            IntType::U64,
            IntType::U128,
            IntType::Usize,
        ] {
            assert_eq!(each.lowest(), 0, "{}", each.name());
            assert!(!each.holds(-1), "{} has no -1", each.name());
            assert!(each.holds(0), "{} has 0", each.name());
        }
    }

    #[test]
    fn a_narrow_type_ends_where_rust_ends_it() {
        assert_eq!((IntType::U8.lowest(), IntType::U8.highest()), (0, 255));
        assert_eq!((IntType::U16.lowest(), IntType::U16.highest()), (0, 65_535));
        assert_eq!(IntType::U32.highest(), 4_294_967_295);
        assert_eq!((IntType::I8.lowest(), IntType::I8.highest()), (-128, 127));
        assert_eq!(
            (IntType::I16.lowest(), IntType::I16.highest()),
            (-32_768, 32_767)
        );
        assert_eq!(
            (IntType::I32.lowest(), IntType::I32.highest()),
            (-2_147_483_648, 2_147_483_647)
        );
        assert!(IntType::U8.holds(255) && !IntType::U8.holds(256), "u8");
        assert!(IntType::I8.holds(-128) && !IntType::I8.holds(-129), "i8");
        assert!(IntType::I8.holds(127) && !IntType::I8.holds(128), "i8");
    }

    #[test]
    fn a_wide_type_reaches_as_far_as_a_value_can_hold() {
        for each in [IntType::U64, IntType::U128, IntType::Usize] {
            assert_eq!(each.highest(), i64::MAX, "{}", each.name());
            assert!(each.holds(i64::MAX), "{}", each.name());
        }
        for each in [IntType::I64, IntType::I128, IntType::Isize] {
            assert_eq!(
                (each.lowest(), each.highest()),
                (i64::MIN, i64::MAX),
                "{}",
                each.name()
            );
        }
    }
}
