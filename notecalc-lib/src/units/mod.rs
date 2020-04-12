// Credits to https://mathjs.org, most of this code based on their implementation

use bigdecimal::*;
use smallvec::alloc::fmt::Formatter;
use std::str::FromStr;

pub mod consts;
pub mod units;

const BASE_DIMENSION_COUNT: usize = 9;
const BASE_DIMENSIONS: [&str; BASE_DIMENSION_COUNT] = [
    "MASS",
    "LENGTH",
    "TIME",
    "CURRENT",
    "TEMPERATURE",
    "LUMINOUS_INTENSITY",
    "AMOUNT_OF_SUBSTANCE",
    "ANGLE",
    "BIT",
];

#[derive(Eq, PartialEq, Clone)]
pub struct Unit<'a> {
    name: &'static [char],
    base: [isize; BASE_DIMENSION_COUNT],
    // e.g. prefix_groups: (Some(&prefixes.short), Some(&prefixes.long)),
    prefix_groups: (Option<&'a [Prefix]>, Option<&'a [Prefix]>),
    value: BigDecimal,
    offset: BigDecimal,
}

impl<'a> std::fmt::Debug for Unit<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Unit({:?}, value: {:?}, offset: {:?})",
            self.name, self.value, self.offset
        )
    }
}

pub struct UnitPrefixes {
    short: [Prefix; 20],
    long: [Prefix; 20],
    squared: [Prefix; 20],
    cubic: [Prefix; 20],
    binary_short_si: [Prefix; 8],
    binary_short_iec: [Prefix; 8],
    binary_long_si: [Prefix; 8],
    binary_long_iec: [Prefix; 8],
    btu: [Prefix; 1],
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Prefix {
    name: &'static [char],
    value: BigDecimal,
    scientific: bool,
}

impl Prefix {
    pub fn new(name: &'static [char], num: &str, scientific: bool) -> Prefix {
        Prefix {
            name,
            value: BigDecimal::from_str(num).unwrap(),
            scientific,
        }
    }
}
