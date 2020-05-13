// Credits to https://mathjs.org, most of this code based on their implementation

use crate::units::consts::BASE_UNIT_DIMENSION_COUNT;
use bigdecimal::*;
use smallvec::alloc::fmt::Formatter;
use std::cell::RefCell;
use std::str::FromStr;

pub mod consts;
pub mod units;

#[derive(Eq, PartialEq, Clone)]
pub struct Unit {
    name: &'static [char],
    base: [i8; BASE_UNIT_DIMENSION_COUNT],
    // e.g. prefix_groups: (Some(&prefixes.short), Some(&prefixes.long)),
    prefix_groups: (
        Option<RefCell<Box<Vec<RefCell<Prefix>>>>>,
        Option<RefCell<Box<Vec<RefCell<Prefix>>>>>,
    ),
    value: BigDecimal,
    offset: BigDecimal,
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Unit({:?}, value: {:?}, offset: {:?})",
            self.name, self.value, self.offset
        )
    }
}

pub struct UnitPrefixes {
    short: RefCell<Box<Vec<RefCell<Prefix>>>>,
    long: RefCell<Box<Vec<RefCell<Prefix>>>>,
    squared: RefCell<Box<Vec<RefCell<Prefix>>>>,
    cubic: RefCell<Box<Vec<RefCell<Prefix>>>>,
    binary_short_si: RefCell<Box<Vec<RefCell<Prefix>>>>,
    binary_short_iec: RefCell<Box<Vec<RefCell<Prefix>>>>,
    binary_long_si: RefCell<Box<Vec<RefCell<Prefix>>>>,
    binary_long_iec: RefCell<Box<Vec<RefCell<Prefix>>>>,
    btu: RefCell<Box<Vec<RefCell<Prefix>>>>,
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
