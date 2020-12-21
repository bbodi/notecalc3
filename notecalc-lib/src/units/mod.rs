// Credits to https://mathjs.org, most of this code based on their implementation

use crate::units::consts::BASE_UNIT_DIMENSION_COUNT;
use rust_decimal::prelude::*;
use smallvec::alloc::fmt::Formatter;
use std::rc::Rc;

pub mod consts;
pub mod units;

#[derive(Eq, PartialEq, Clone)]
pub struct Unit {
    pub name: &'static [char],
    pub base: [i8; BASE_UNIT_DIMENSION_COUNT],
    // e.g. prefix_groups: (Some(&prefixes.short), Some(&prefixes.long)),
    pub prefix_groups: (
        Option<Rc<Box<Vec<Rc<Prefix>>>>>,
        Option<Rc<Box<Vec<Rc<Prefix>>>>>,
    ),
    pub value: Decimal,
    pub offset: Decimal,
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
    short: Rc<Box<Vec<Rc<Prefix>>>>,
    long: Rc<Box<Vec<Rc<Prefix>>>>,
    squared: Rc<Box<Vec<Rc<Prefix>>>>,
    cubic: Rc<Box<Vec<Rc<Prefix>>>>,
    binary_short_si: Rc<Box<Vec<Rc<Prefix>>>>,
    binary_short_iec: Rc<Box<Vec<Rc<Prefix>>>>,
    binary_long_si: Rc<Box<Vec<Rc<Prefix>>>>,
    binary_long_iec: Rc<Box<Vec<Rc<Prefix>>>>,
    btu: Rc<Box<Vec<Rc<Prefix>>>>,
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Prefix {
    pub name: &'static [char],
    pub value: Decimal,
    pub scientific: bool,
}

impl Prefix {
    pub fn from_scientific(name: &'static [char], num: &str, scientific: bool) -> Prefix {
        Prefix {
            name,
            value: Decimal::from_scientific(num).unwrap(),
            scientific,
        }
    }

    pub fn from_decimal(name: &'static [char], num: &str, scientific: bool) -> Prefix {
        Prefix {
            name,
            value: Decimal::from_str(num).unwrap(),
            scientific,
        }
    }
}
