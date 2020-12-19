use crate::calc::pow;
use crate::units::consts::{
    get_base_unit_for, init_aliases, init_units, UnitDimensionExponent, UnitType,
    BASE_UNIT_DIMENSIONS, BASE_UNIT_DIMENSION_COUNT,
};
use crate::units::{Prefix, Unit, UnitPrefixes};
use rust_decimal::Decimal;
use smallvec::alloc::fmt::{Debug, Display, Formatter};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Write;
use std::rc::Rc;
use std::str::FromStr;

fn next(str: &[char]) -> &[char] {
    &str[1..]
}

fn parse_unit(str: &[char]) -> Option<&[char]> {
    let mut i = 0;
    for ch in str {
        if !ch.is_alphanumeric() && *ch != '$' {
            break;
        }
        i += 1;
    }
    Some(&str[0..i])
}

fn skip_whitespaces(str: &[char]) -> &[char] {
    let mut i = 0;
    for ch in str {
        if !ch.is_ascii_whitespace() {
            break;
        }
        i += 1;
    }
    &str[i..]
}

pub struct Units {
    pub prefixes: UnitPrefixes,
    pub units: HashMap<&'static str, Rc<Unit>>,
    pub aliases: HashMap<&'static str, &'static str>,
    pub no_prefix: Rc<Prefix>,
}

impl Units {
    pub fn new() -> Units {
        let (units, prefixes) = init_units();
        Units {
            no_prefix: Rc::new(Prefix::from_decimal(&[], "1", false)),
            units,
            prefixes,
            aliases: init_aliases(),
        }
    }

    pub fn parse(&self, text: &[char]) -> (UnitOutput, usize) {
        let mut output = UnitOutput::new();
        let mut power_multiplier_current: UnitDimensionExponent = 1;

        // Optional number at the start of the string
        let mut last_valid_cursor_pos = 0;
        let mut c = text;

        // Stack to keep track of powerMultipliers applied to each parentheses group
        let mut power_multiplier_stack = vec![];

        // Running product of all elements in powerMultiplierStack
        let mut power_multiplier_stack_product = 1;
        let mut expecting_unit = false;

        'main_loop: loop {
            c = skip_whitespaces(c);

            // Check for and consume opening parentheses, pushing powerMultiplierCurrent to the stack
            // A '(' will always appear directly before a unit.
            while !c.is_empty() && c[0] == '(' {
                power_multiplier_stack.push(power_multiplier_current);
                power_multiplier_stack_product *= power_multiplier_current;
                power_multiplier_current = 1;
                c = next(c);
                c = skip_whitespaces(c);
            }

            let value = parse_char(&mut c, '1');

            c = skip_whitespaces(c);
            if value && !c.is_empty() {
                // handle multiplication or division right after the value, like '1/s'
                if expecting_unit {
                    return (output, last_valid_cursor_pos);
                }
                if parse_char(&mut c, '*') {
                    power_multiplier_current = 1;
                } else if parse_char(&mut c, '/') {
                    power_multiplier_current = -1;
                } else {
                    return (output, last_valid_cursor_pos);
                }
            }

            // Is there something here?
            let u_str = if c.len() > 0 {
                parse_unit(c).unwrap()
            } else {
                // End of input.
                break 'main_loop;
            };

            // Verify the unit exists and get the prefix (if any)
            let res = if let Some(res) = self.find_unit(u_str) {
                c = skip(c, u_str.len());
                res
            } else {
                break 'main_loop;
            };
            if power_multiplier_stack.is_empty() {
                // there is no open parenthesis
                last_valid_cursor_pos = Units::calc_parsed_len(text, c);
            }

            let mut power = if let Some(power) =
                power_multiplier_current.checked_mul(power_multiplier_stack_product)
            {
                power
            } else {
                break 'main_loop;
            };
            // Is there a "^ number"?
            c = skip_whitespaces(c);
            if parse_char(&mut c, '^') {
                c = skip_whitespaces(c);
                let p = parse_number(&mut c);
                if let Some(p) = p {
                    power = if let Some(mul) =
                        i8::try_from(p).ok().and_then(|b| power.checked_mul(b))
                    {
                        mul
                    } else {
                        return (output, 0);
                    };
                } else {
                    // No valid number found for the power!
                    if !output.add_unit(UnitInstance::new(res.0, res.1, power)) {
                        return (output, 0);
                    }
                    break 'main_loop;
                }
            }
            if !output.checked_add_unit(UnitInstance::new(res.0, res.1, power)) {
                return (output, 0);
            }
            // Add the unit to the list

            c = skip_whitespaces(c);

            // Check for and consume closing parentheses, popping from the stack.
            // A ')' will always follow a unit.
            while !c.is_empty() && c[0] == ')' {
                if let Some(a) = power_multiplier_stack.pop() {
                    power_multiplier_stack_product /= a;
                } else {
                    last_valid_cursor_pos = Units::calc_parsed_len(text, c);
                    break 'main_loop;
                }
                c = next(c);
                c = skip_whitespaces(c);
            }

            // it is valid only if there is no open parenthesis
            if power_multiplier_stack.is_empty() {
                last_valid_cursor_pos = Units::calc_parsed_len(text, c);
            }
            // "*" and "/" should mean we are expecting something to come next.
            // Is there a forward slash? If so, negate powerMultiplierCurrent. The next unit or paren group is in the denominator.
            expecting_unit = false;
            if parse_char(&mut c, '*') {
                // explicit multiplication
                power_multiplier_current = 1;
                expecting_unit = true;
            } else if parse_char(&mut c, '/') {
                power_multiplier_current = -1;
                expecting_unit = true;
            } else {
                // implicit multiplication is allowed only inside parenthesis
                let inside_parenthesis = !power_multiplier_stack.is_empty();
                if inside_parenthesis {
                    power_multiplier_current = 1;
                } else {
                    break;
                }
            }
            if !expecting_unit {
                last_valid_cursor_pos = Units::calc_parsed_len(text, c);
            }
        }

        if last_valid_cursor_pos == 0 {
            for unit in output.unit_instances.iter_mut().take(output.unit_count) {
                *unit = None;
            }
            output.unit_count = 0;
        }
        return (output, last_valid_cursor_pos);
    }

    fn calc_parsed_len(text: &[char], current: &[char]) -> usize {
        let mut parsed_len = unsafe { current.as_ptr().offset_from(text.as_ptr()) } as usize;
        // remove spaces
        while text[parsed_len - 1].is_ascii_whitespace() {
            parsed_len -= 1;
        }
        return parsed_len;
    }

    fn find_unit(&self, str: &[char]) -> Option<(Rc<Unit>, Rc<Prefix>)> {
        if str.is_empty() {
            return None;
        }
        // TODO fostos char slice (collect...)
        if let Some(exact_match_unit) = self
            .units
            .get(str.iter().map(|it| *it).collect::<String>().as_str())
        {
            return Some((Rc::clone(exact_match_unit), Rc::clone(&self.no_prefix)));
        }
        fn check(
            this: &Units,
            str: &[char],
            unit: &Rc<Unit>,
            unit_name: &'static str,
        ) -> Option<(Rc<Unit>, Rc<Prefix>)> {
            if unit_name.chars().count() > str.len() {
                return None;
            }
            let str_endswith_unitname = unit_name
                .chars()
                .rev()
                .zip(str.iter().rev())
                .all(|(unit_name_char, actual_char)| unit_name_char == *actual_char);
            // if str.ends_with(unit_name) {
            if str_endswith_unitname {
                let prefix_len = str.len() - unit_name.len();
                if prefix_len == 0 {
                    return Some((Rc::clone(unit), Rc::clone(&this.no_prefix)));
                }
                let prefix_name = &str[0..prefix_len];
                let prefix = Units::find_prefix_for(&(*unit), prefix_name);
                if let Some(prefix) = prefix {
                    return Some((Rc::clone(unit), prefix));
                }
            }
            return None;
        }
        for (unit_name, unit) in &self.units {
            let result = check(self, str, unit, unit_name);
            if result.is_some() {
                return result;
            }
        }
        for (alias, unit_name) in &self.aliases {
            let unit = self.units.get(unit_name).expect(unit_name);
            let result = check(self, str, unit, alias);
            if result.is_some() {
                return result;
            }
        }
        return None;
    }

    pub fn simplify(&self, unit: &UnitOutput) -> Option<UnitOutput> {
        if let Some(base_unit) = get_base_unit_for(self, &unit.dimensions) {
            let dimensions = base_unit.unit.base;
            let mut unit_instances = [None; MAX_UNIT_COUNT];
            unit_instances[0] = Some(UnitInstance {
                unit: base_unit.unit,
                prefix: base_unit.prefix,
                power: 1,
            });
            Some(UnitOutput {
                unit_instances,
                dimensions,
                unit_count: 1,
            })
        } else {
            None
        }
    }

    fn find_prefix_for(unit: &Unit, prefix_name: &[char]) -> Option<Rc<Prefix>> {
        match &unit.prefix_groups {
            (Some(p1), Some(p2)) => p1
                .iter()
                .chain(p2.iter())
                .find(|it| it.name == prefix_name)
                .map(|it| Rc::clone(it)),
            (Some(p1), None) => p1
                .iter()
                .find(|it| it.name == prefix_name)
                .map(|it| Rc::clone(it)),
            (None, None) => None,
            (None, Some(_)) => panic!("Cannot happen"),
        }
    }
}

fn parse_number(text: &mut &[char]) -> Option<isize> {
    let mut tmp: [u8; 32] = [0; 32];
    let mut i = if !text.is_empty() && text[0] == '-' {
        tmp[0] = b'-';
        1
    } else {
        0
    };
    while i < text.len() && text[i].is_ascii_digit() {
        if i >= 32 {
            return None;
        }
        tmp[i] = text[i] as u8;
        i += 1;
    }
    return if i > 0 && tmp[i - 1] != 0 {
        let num = isize::from_str(&unsafe { std::str::from_utf8_unchecked(&tmp[0..i]) }).ok()?;
        *text = &text[i..];
        Some(num)
    } else {
        None
    };
}

fn parse_char(c: &mut &[char], ch: char) -> bool {
    return if c.is_empty() {
        false
    } else {
        let ret = (*c)[0] == ch;
        if ret {
            *c = &c[1..];
        }
        ret
    };
}

fn skip(c: &[char], len: usize) -> &[char] {
    &c[len..]
}

pub const MAX_UNIT_COUNT: usize = 8;

#[derive(Clone, Eq)]
pub struct UnitOutput {
    pub unit_instances: [Option<UnitInstance>; MAX_UNIT_COUNT],
    pub unit_count: usize,
    pub dimensions: [UnitDimensionExponent; BASE_UNIT_DIMENSION_COUNT],
}

impl Debug for UnitOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}]", self.unit_instances)
    }
}

impl Display for UnitOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut nnum = 0;
        let mut nden = 0;
        let mut str_num: SmallVec<[char; 32]> = SmallVec::with_capacity(32);
        let mut str_den: SmallVec<[char; 32]> = SmallVec::with_capacity(32);

        for unit in self.unit_instances.iter().take(self.unit_count) {
            let unit = unit.as_ref().unwrap();
            if unit.power > 0 {
                nnum += 1;
                str_num.push(' ');
                str_num.extend_from_slice(unit.prefix.name);
                str_num.extend_from_slice(unit.unit.name);
                if (unit.power as f64 - 1.0).abs() > 1e-15 {
                    str_num.push('^');
                    str_num.extend(unit.power.to_string().chars());
                }
            } else {
                nden += 1;
            }
        }

        if nden > 0 {
            for unit in self.unit_instances.iter().take(self.unit_count) {
                let unit = unit.as_ref().unwrap();
                if unit.power < 0 {
                    if nnum > 0 {
                        str_den.push(' ');
                        str_den.extend_from_slice(unit.prefix.name);
                        str_den.extend_from_slice(unit.unit.name);
                        if (unit.power as f64 + 1.0).abs() > 1e-15 {
                            str_den.push('^');
                            str_den.extend((-unit.power).to_string().chars());
                        }
                    } else {
                        str_den.push(' ');
                        str_den.extend_from_slice(unit.prefix.name);
                        str_den.extend_from_slice(unit.unit.name);
                        str_den.push('^');
                        str_den.extend(unit.power.to_string().chars());
                    }
                }
            }
        }
        if !str_num.is_empty() {
            let need_paren = nnum > 1 && nden > 0;
            if need_paren {
                f.write_char('(').expect("must work");
            }
            for ch in &str_num[1..] {
                f.write_char(*ch).expect("must work");
            }
            if need_paren {
                f.write_char(')').expect("must work");
            }
        }
        if nnum > 0 && nden > 0 {
            f.write_char(' ').expect("must work");
            f.write_char('/').expect("must work");
            f.write_char(' ').expect("must work");
        }
        if !str_den.is_empty() {
            let need_paren = nnum > 0 && nden > 1;
            if need_paren {
                f.write_char('(').expect("must work");
            }
            for ch in &str_den[1..] {
                f.write_char(*ch).expect("must work");
            }
            if need_paren {
                f.write_char(')').expect("must work");
            }
        }
        Ok(())
    }
}

impl UnitOutput {
    pub fn new() -> UnitOutput {
        UnitOutput {
            unit_instances: [None; MAX_UNIT_COUNT],
            unit_count: 0,
            dimensions: [0; BASE_UNIT_DIMENSION_COUNT],
        }
    }

    pub fn get_unit(&self, i: usize) -> &UnitInstance {
        self.unit_instances[i].as_ref().unwrap()
    }

    pub fn new_inch(units: &Units) -> UnitOutput {
        let mut unit = UnitOutput::new();
        let _ = unit.add_unit(UnitInstance::new(
            Rc::clone(&units.units["in"]),
            Rc::clone(&units.no_prefix),
            1,
        ));
        return unit;
    }

    pub fn new_rad(units: &Units) -> UnitOutput {
        let mut unit = UnitOutput::new();
        let _ = unit.add_unit(UnitInstance::new(
            Rc::clone(&units.units["rad"]),
            Rc::clone(&units.no_prefix),
            1,
        ));
        return unit;
    }

    #[must_use]
    pub fn add_unit(&mut self, unit: UnitInstance) -> bool {
        for i in 0..BASE_UNIT_DIMENSION_COUNT {
            if let Some(num) = unit.unit.base[i].checked_mul(unit.power) {
                if let Some(a) = self.dimensions[i].checked_add(num) {
                    self.dimensions[i] = a;
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
        self.unit_instances[self.unit_count] = Some(unit);
        self.unit_count += 1;
        return true;
    }

    #[must_use]
    pub fn checked_add_unit(&mut self, unit: UnitInstance) -> bool {
        if self.unit_count >= MAX_UNIT_COUNT {
            return false;
        }
        return self.add_unit(unit);
    }

    pub fn is_unitless(&self) -> bool {
        self.dimensions.iter().all(|it| *it == 0)
    }

    pub fn simplify(&self, units: &Units) -> Option<UnitOutput> {
        if let Some(base_unit) = units.simplify(self) {
            // e.g. don't convert from km to m, but convert from kg*m/s^2 to N
            // base_unit.unit_count is always 1
            let base_unit_is_simpler = self.unit_count > 1;
            if base_unit_is_simpler {
                Some(base_unit)
            } else {
                None
            }
        } else {
            let mut proposed_unit_list: [Option<UnitInstance>; MAX_UNIT_COUNT] =
                [None; MAX_UNIT_COUNT];
            let mut proposed_unit_count = 0;
            for i in 0..BASE_UNIT_DIMENSION_COUNT {
                if self.dimensions[i] != 0 {
                    if let Some(u) = get_base_unit_for(units, &BASE_UNIT_DIMENSIONS[i]) {
                        proposed_unit_list[proposed_unit_count] = Some(UnitInstance {
                            unit: u.unit,
                            prefix: u.prefix,
                            power: self.dimensions[i],
                        });
                        proposed_unit_count += 1;
                    } else {
                        return None;
                    }
                }
            }
            // Is the proposed unit list "simpler" than the existing one?
            if proposed_unit_count < self.unit_count {
                Some(UnitOutput {
                    unit_instances: proposed_unit_list,
                    unit_count: proposed_unit_count,
                    ..self.clone()
                })
            } else {
                None
            }
        }
    }

    pub fn is(&self, typ: UnitType) -> bool {
        self.dimensions == BASE_UNIT_DIMENSIONS[typ as usize]
    }
}

impl std::ops::Mul for &UnitOutput {
    type Output = UnitOutput;

    fn mul(self, other: Self) -> Self::Output {
        let mut result = self.clone();

        for (i, (this_dim, other_dim)) in self
            .dimensions
            .iter()
            .zip(other.dimensions.iter())
            .enumerate()
        {
            result.dimensions[i] = this_dim + other_dim;
        }

        for other_unit in other.unit_instances.iter().take(other.unit_count) {
            let other_unit = other_unit.as_ref().unwrap();
            result.unit_instances[result.unit_count] = Some(other_unit.clone());
            result.unit_count += 1;
        }

        return result;
    }
}

impl std::ops::Div for &UnitOutput {
    type Output = UnitOutput;

    fn div(self, other: Self) -> Self::Output {
        let mut result = self.clone();

        for (i, (this_dim, other_dim)) in self
            .dimensions
            .iter()
            .zip(other.dimensions.iter())
            .enumerate()
        {
            result.dimensions[i] = this_dim - other_dim;
        }

        for other_unit in other.unit_instances.iter().take(other.unit_count) {
            let other_unit = other_unit.as_ref().unwrap();
            let mut clone = other_unit.clone();
            clone.power = -clone.power;
            result.unit_instances[result.unit_count] = Some(clone);
            result.unit_count += 1;
        }

        return result;
    }
}

impl PartialEq for UnitOutput {
    fn eq(&self, other: &Self) -> bool {
        // All dimensions must be the same
        for (a, b) in self.dimensions.iter().zip(other.dimensions.iter()) {
            if a != b {
                return false;
            }
        }
        return true;
    }
}

impl UnitOutput {
    pub fn normalize(&self, value: &Decimal) -> Option<Decimal> {
        if self.is_derived() {
            let mut result = value.clone();
            for unit in self.unit_instances.iter().take(self.unit_count) {
                let unit = unit.as_ref().unwrap();
                let base_value = &unit.unit.value;
                let prefix_val = &unit.prefix.value;
                let power = unit.power;

                result = result.checked_mul(&pow(base_value * prefix_val, power as i64)?)?;
            }
            return Some(result);
        } else {
            let instance = self.get_unit(0);
            let base_value = &instance.unit.value;
            let offset = &instance.unit.offset;
            let prefix_val = &instance.prefix.value;

            let a = value + offset;
            let b = base_value * prefix_val;
            return a.checked_mul(&b);
        }
    }

    pub fn from_base_to_this_unit(&self, value: &Decimal) -> Option<Decimal> {
        return if self.is_derived() {
            let mut result = value.clone();
            for unit in self.unit_instances.iter().take(self.unit_count) {
                let unit = unit.as_ref().unwrap();
                let base_value = &unit.unit.value;
                let prefix_val = &unit.prefix.value;
                let power = unit.power;
                let pow = pow(base_value.checked_mul(prefix_val)?, power as i64)?;
                result = result.checked_div(&pow)?;
            }
            Some(result)
        } else {
            // az előző ág az a current numra hivodik meg mivel az a km/h*h unitot számolja
            // ki, ami derived, viszont a /h*h miatt eltünik a m/sbol fakadó
            // pontatlanság és visszakap 120at
            //     ez az ág akkor hivodik, amikor a 120 000.0006.. m-t akarja megkapni mben,
            // mivel a méter már alapban pontatlanul van tárolva, vissza is pontatlant kap
            let instance = self.get_unit(0);
            let borrow = &instance.unit;
            let base_value = &borrow.value;
            let offset = &borrow.offset;
            let borrow_prefix = &instance.prefix;
            let prefix_val = &borrow_prefix.value;

            use rust_decimal::prelude::One;
            if base_value < &Decimal::one() {
                let denom = prefix_val.checked_mul(base_value)?;
                value.checked_div(&denom)?.checked_sub(offset)
            } else {
                let a = value.checked_div(base_value)?;
                a.checked_div(&prefix_val)?.checked_sub(offset)
            }
        };
    }

    pub fn pow(&self, p: i64) -> Option<UnitOutput> {
        let mut result = self.clone();
        let p = i8::try_from(p).ok()?;
        for dim in &mut result.dimensions {
            *dim = dim.checked_mul(p)?;
        }
        for unit in result.unit_instances.iter_mut().take(self.unit_count) {
            let mut unit = unit.as_mut().unwrap();
            unit.power = unit.power.checked_mul(p)?;
        }

        return Some(result);
    }

    pub fn is_derived(&self) -> bool {
        self.unit_count > 1 || (self.unit_count == 1 && self.get_unit(0).power != 1)
    }
}

#[derive(Eq, PartialEq, Clone)]
pub struct UnitInstance {
    pub unit: Rc<Unit>,
    pub prefix: Rc<Prefix>,
    pub power: UnitDimensionExponent,
}

impl UnitInstance {
    pub fn new(unit: Rc<Unit>, prefix: Rc<Prefix>, power: UnitDimensionExponent) -> UnitInstance {
        UnitInstance {
            unit,
            prefix,
            power,
        }
    }
}

impl Debug for UnitInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "'{}{}^{}'",
            self.prefix.name.iter().collect::<String>(),
            self.unit.name.iter().collect::<String>(),
            self.power
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::consts::EMPTY_UNIT_DIMENSIONS;
    use rust_decimal::prelude::*;

    fn parse(str: &str, units: &Units) -> UnitOutput {
        units.parse(&str.chars().collect::<Vec<char>>()).0
    }

    #[test]
    fn should_create_unit_correctly() {
        let units = Units::new();

        let unit1 = parse("cm", &units);
        assert_eq!(&['m'], unit1.get_unit(0).unit.name);

        let unit1 = parse("kg", &units);
        assert_eq!(&['g'], unit1.get_unit(0).unit.name);

        let unit1 = parse("(kg m)/J^2", &units);
        assert_eq!(&['g'], unit1.get_unit(0).unit.name);
        assert_eq!(&['k'], unit1.get_unit(0).prefix.name);
        assert_eq!(&['m'], unit1.get_unit(1).unit.name);
        assert_eq!(&['J'], unit1.get_unit(2).unit.name);
        assert_eq!(-2, unit1.get_unit(2).power);

        let unit1 = parse("(kg m)/s^2", &units);
        assert_eq!(&['g'], unit1.get_unit(0).unit.name);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(unit1.get_unit(0).prefix.name, &['k']);
        assert_eq!(&['m'], unit1.get_unit(1).unit.name);
        assert_eq!(1, unit1.get_unit(1).power);
        assert_eq!(unit1.get_unit(1).prefix.name, &[]);
        assert_eq!(&['s'], unit1.get_unit(2).unit.name);
        assert_eq!(unit1.get_unit(2).prefix.name, &[]);
        assert_eq!(-2, unit1.get_unit(2).power);

        let unit1 = parse("cm/s", &units);
        assert_eq!(&['c'], unit1.get_unit(0).prefix.name);
        assert_eq!(&['m'], unit1.get_unit(0).unit.name);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(&['s'], unit1.get_unit(1).unit.name);
        assert_eq!(-1, unit1.get_unit(1).power);

        let unit1 = parse("ml", &units);
        assert_eq!(&['m'], unit1.get_unit(0).prefix.name);
        assert_eq!(&['l'], unit1.get_unit(0).unit.name);
        assert_eq!(3, unit1.dimensions[1]);
        assert_eq!(1, unit1.get_unit(0).power);

        let unit1 = parse("ml^-1", &units);
        assert_eq!(&['m'], unit1.get_unit(0).prefix.name);
        assert_eq!(&['l'], unit1.get_unit(0).unit.name);
        assert_eq!(-3, unit1.dimensions[1]);
        assert_eq!(-1, unit1.get_unit(0).power);

        let unit1 = parse("Hz", &units);
        assert_eq!(&['H', 'z'], unit1.get_unit(0).unit.name);

        let unit1 = parse("km2", &units);
        assert_eq!(&['m', '2'], unit1.get_unit(0).unit.name);

        let unit1 = parse("km^3", &units);
        assert_eq!(&['m'], unit1.get_unit(0).unit.name);
        assert_eq!(3, unit1.get_unit(0).power);
        assert_eq!(3, unit1.dimensions[1]);
        assert_eq!(&['k'], unit1.get_unit(0).prefix.name);

        let unit1 = parse("km3", &units);
        assert_eq!(&['m', '3'], unit1.get_unit(0).unit.name);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(3, unit1.dimensions[1]);
        assert_eq!(
            Decimal::from_i64(1000000000).unwrap(),
            unit1.get_unit(0).prefix.value
        );
        assert_eq!(&['k'], unit1.get_unit(0).prefix.name);

        // should test whether two units have the same base unit
        assert_eq!(&parse("cm", &units), &parse("m", &units));
        assert_ne!(&parse("cm", &units), &parse("kg", &units));
        assert_eq!(&parse("N", &units), &parse("kg*m / s ^ 2", &units));
        assert_eq!(
            &parse("J / mol*K", &units),
            &parse("ft^3*psi / mol*degF", &units)
        );

        let unit1 = parse("bytes", &units);
        assert_eq!(&['b', 'y', 't', 'e', 's'], unit1.get_unit(0).unit.name);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(unit1.get_unit(0).prefix.name, &[]);

        // Kibi BIT!
        let unit1 = parse("Kib", &units);
        assert_eq!(&['b'], unit1.get_unit(0).unit.name);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(&['K', 'i'], unit1.get_unit(0).prefix.name);

        let unit1 = parse("Kib/s", &units);
        assert_eq!(&['K', 'i'], unit1.get_unit(0).prefix.name);
        assert_eq!(&['b'], unit1.get_unit(0).unit.name);
        assert_eq!(&['s'], unit1.get_unit(1).unit.name);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(-1, unit1.get_unit(1).power);

        let unit1 = parse("b/s", &units);
        assert_eq!(unit1.get_unit(0).prefix.name, &[]);
        assert_eq!(&['b'], unit1.get_unit(0).unit.name);
        assert_eq!(&['s'], unit1.get_unit(1).unit.name);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(-1, unit1.get_unit(1).power);

        let unit1 = parse("kb", &units);
        assert_eq!(unit1.get_unit(0).prefix.name, &['k']);
        assert_eq!(&['b'], unit1.get_unit(0).unit.name);
        assert_eq!(1, unit1.get_unit(0).power);

        let unit1 = parse("cm*s^-2", &units);
        assert_eq!(unit1.get_unit(0).unit.name, &['m']);
        assert_eq!(unit1.get_unit(1).unit.name, &['s']);
        assert_eq!(&['c'], unit1.get_unit(0).prefix.name);
        assert_eq!(-2, unit1.get_unit(1).power);

        let unit1 = parse("kg*m^2 / s^2 / K / mol", &units);
        assert_eq!(unit1.get_unit(0).unit.name, &['g']);
        assert_eq!(unit1.get_unit(1).unit.name, &['m']);
        assert_eq!(unit1.get_unit(2).unit.name, &['s']);
        assert_eq!(unit1.get_unit(3).unit.name, &['K']);
        assert_eq!(unit1.get_unit(4).unit.name, &['m', 'o', 'l']);
        assert_eq!(&['k'], unit1.get_unit(0).prefix.name);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(2, unit1.get_unit(1).power);
        assert_eq!(-2, unit1.get_unit(2).power);
        assert_eq!(-1, unit1.get_unit(3).power);
        assert_eq!(-1, unit1.get_unit(4).power);

        let unit1 = parse("kg*(m^2 / (s^2 / (K^-1 / mol)))", &units);
        assert_eq!(unit1.get_unit(0).unit.name, &['g']);
        assert_eq!(unit1.get_unit(1).unit.name, &['m']);
        assert_eq!(unit1.get_unit(2).unit.name, &['s']);
        assert_eq!(unit1.get_unit(3).unit.name, &['K']);
        assert_eq!(unit1.get_unit(4).unit.name, &['m', 'o', 'l']);
        assert_eq!(&['k'], unit1.get_unit(0).prefix.name);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(2, unit1.get_unit(1).power);
        assert_eq!(-2, unit1.get_unit(2).power);
        assert_eq!(-1, unit1.get_unit(3).power);
        assert_eq!(-1, unit1.get_unit(4).power);

        let unit1 = parse("(m / ( s / ( kg mol ) / ( lbm / h ) K ) )", &units);
        assert_eq!(unit1.get_unit(0).unit.name, &['m']);
        assert_eq!(unit1.get_unit(1).unit.name, &['s']);
        assert_eq!(unit1.get_unit(2).unit.name, &['g']);
        assert_eq!(unit1.get_unit(3).unit.name, &['m', 'o', 'l']);
        assert_eq!(unit1.get_unit(4).unit.name, &['l', 'b', 'm']);
        assert_eq!(unit1.get_unit(5).unit.name, &['h']);
        assert_eq!(unit1.get_unit(6).unit.name, &['K']);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(-1, unit1.get_unit(1).power);
        assert_eq!(1, unit1.get_unit(2).power);
        assert_eq!(1, unit1.get_unit(3).power);
        assert_eq!(1, unit1.get_unit(4).power);
        assert_eq!(-1, unit1.get_unit(5).power);
        assert_eq!(-1, unit1.get_unit(6).power);

        let unit1 = parse("(m/(s/(kg mol)/(lbm/h)K))", &units);
        assert_eq!(unit1.get_unit(0).unit.name, &['m']);
        assert_eq!(unit1.get_unit(1).unit.name, &['s']);
        assert_eq!(unit1.get_unit(2).unit.name, &['g']);
        assert_eq!(unit1.get_unit(3).unit.name, &['m', 'o', 'l']);
        assert_eq!(unit1.get_unit(4).unit.name, &['l', 'b', 'm']);
        assert_eq!(unit1.get_unit(5).unit.name, &['h']);
        assert_eq!(unit1.get_unit(6).unit.name, &['K']);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(-1, unit1.get_unit(1).power);
        assert_eq!(1, unit1.get_unit(2).power);
        assert_eq!(1, unit1.get_unit(3).power);
        assert_eq!(1, unit1.get_unit(4).power);
        assert_eq!(-1, unit1.get_unit(5).power);
        assert_eq!(-1, unit1.get_unit(6).power);

        // should parse units with correct precedence
        let unit1 = parse("m^3 / kg*s^2", &units);
        assert_eq!(unit1.get_unit(0).unit.name, &['m']);
        assert_eq!(unit1.get_unit(1).unit.name, &['g']);
        assert_eq!(unit1.get_unit(2).unit.name, &['s']);
        assert_eq!(3, unit1.get_unit(0).power);
        assert_eq!(-1, unit1.get_unit(1).power);
        assert_eq!(2, unit1.get_unit(2).power);

        let unit1 = parse("m^3 / (kg s^2)", &units);
        assert_eq!(unit1.get_unit(0).unit.name, &['m']);
        assert_eq!(unit1.get_unit(1).unit.name, &['g']);
        assert_eq!(unit1.get_unit(2).unit.name, &['s']);
        assert_eq!(3, unit1.get_unit(0).power);
        assert_eq!(-1, unit1.get_unit(1).power);
        assert_eq!(-2, unit1.get_unit(2).power);
    }

    #[test]
    fn exp_notation() {
        let units = Units::new();

        // exponential notation, binary or hex is not supported in exponents
        let unit1 = parse("kg^1e0 * m^1.0e3 * s^-2.0e0", &units);
        assert_eq!(1, unit1.unit_count);
        assert_eq!(unit1.get_unit(0).prefix.name, &['k']);
        assert_eq!(unit1.get_unit(0).unit.name, &['g']);
        assert_eq!(1, unit1.get_unit(0).power);

        let unit1 = parse("kg^0b01", &units);
        assert_eq!(1, unit1.unit_count);
        assert_eq!(unit1.get_unit(0).prefix.name, &['k']);
        assert_eq!(unit1.get_unit(0).unit.name, &['g']);
        assert_eq!(0, unit1.get_unit(0).power);

        let unit1 = parse("kg^0xFF", &units);
        assert_eq!(1, unit1.unit_count);
        assert_eq!(unit1.get_unit(0).prefix.name, &['k']);
        assert_eq!(unit1.get_unit(0).unit.name, &['g']);
        assert_eq!(0, unit1.get_unit(0).power);
    }

    #[test]
    fn test_prefixes() {
        let units = Units::new();
        //should accept both long and short prefixes
        assert_eq!(parse("ohm", &units).get_unit(0).unit.name, &['o', 'h', 'm']);
        assert_eq!(
            parse("milliohm", &units).get_unit(0).unit.name,
            &['o', 'h', 'm']
        );
        assert_eq!(
            parse("mohm", &units).get_unit(0).unit.name,
            &['o', 'h', 'm']
        );

        assert_eq!(parse("bar", &units).get_unit(0).unit.name, &['b', 'a', 'r']);
        assert_eq!(
            parse("millibar", &units).get_unit(0).unit.name,
            &['b', 'a', 'r']
        );
        assert_eq!(
            parse("mbar", &units).get_unit(0).unit.name,
            &['b', 'a', 'r']
        );
    }

    #[test]
    fn test_plurals() {
        let units = Units::new();

        let unit1 = parse("meters", &units);
        assert_eq!(&['m', 'e', 't', 'e', 'r'], unit1.get_unit(0).unit.name);
        assert_eq!(unit1.get_unit(0).prefix.name, &[]);

        let unit1 = parse("kilometers", &units);
        assert_eq!(&['m', 'e', 't', 'e', 'r'], unit1.get_unit(0).unit.name);
        assert_eq!(unit1.get_unit(0).prefix.name, &['k', 'i', 'l', 'o']);

        let unit1 = parse("inches", &units);
        assert_eq!(&['i', 'n', 'c', 'h'], unit1.get_unit(0).unit.name);
        assert_eq!(unit1.get_unit(0).prefix.name, &[]);
    }

    #[test]
    fn test_units_j_mol_k_parsing() {
        let units = Units::new();

        let unit1 = parse("(J / mol / K)", &units);
        assert_eq!(unit1.get_unit(0).prefix.name, &[]);
        assert_eq!(unit1.get_unit(1).prefix.name, &[]);
        assert_eq!(unit1.get_unit(2).prefix.name, &[]);
        assert_eq!(unit1.get_unit(0).unit.name, &['J']);
        assert_eq!(unit1.get_unit(1).unit.name, &['m', 'o', 'l']);
        assert_eq!(unit1.get_unit(2).unit.name, &['K']);
        let parsed_len = units
            .parse(&"(J / mol / K)".chars().collect::<Vec<char>>())
            .1;
        assert_eq!(parsed_len, "(J / mol / K)".len());

        let unit1 = parse("(J / mol / K) ^ 0", &units);
        assert_eq!(unit1.get_unit(0).prefix.name, &[]);
        assert_eq!(unit1.get_unit(1).prefix.name, &[]);
        assert_eq!(unit1.get_unit(2).prefix.name, &[]);
        assert_eq!(unit1.get_unit(0).unit.name, &['J']);
        assert_eq!(unit1.get_unit(1).unit.name, &['m', 'o', 'l']);
        assert_eq!(unit1.get_unit(2).unit.name, &['K']);

        let parsed_len = units
            .parse(&"(J / mol / K) ^ 0".chars().collect::<Vec<char>>())
            .1;
        // it cannot parse the exponent
        assert_eq!(parsed_len, "(J / mol / K)".len());

        // e.g. if the input is (8.314 J / mol / K)
        let parsed_len = units
            .parse(&"J / mol / K)".chars().collect::<Vec<char>>())
            .1;
        assert_eq!(parsed_len, "J / mol / K".len());
    }

    #[test]
    fn test_cancelling_out() {
        let units = Units::new();

        let unit1 = parse("(km/h) * h", &units);
        assert_eq!(3, unit1.unit_count);
        assert_eq!(unit1.get_unit(0).prefix.name, &['k']);
        assert_eq!(unit1.get_unit(0).unit.name, &['m']);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(unit1.get_unit(1).prefix.name, &[]);
        assert_eq!(unit1.get_unit(1).unit.name, &['h']);
        assert_eq!(-1, unit1.get_unit(1).power);
        assert_eq!(unit1.get_unit(2).prefix.name, &[]);
        assert_eq!(unit1.get_unit(2).unit.name, &['h']);
        assert_eq!(1, unit1.get_unit(2).power);

        let unit1 = parse("km/h*h/h/h", &units);
        assert_eq!(5, unit1.unit_count);

        assert_eq!(unit1.get_unit(0).prefix.name, &['k']);
        assert_eq!(unit1.get_unit(0).unit.name, &['m']);
        assert_eq!(1, unit1.get_unit(0).power);

        assert_eq!(unit1.get_unit(1).prefix.name, &[]);
        assert_eq!(unit1.get_unit(1).unit.name, &['h']);
        assert_eq!(-1, unit1.get_unit(1).power);

        assert_eq!(unit1.get_unit(1).prefix.name, &[]);
        assert_eq!(unit1.get_unit(1).unit.name, &['h']);
        assert_eq!(-1, unit1.get_unit(1).power);

        assert_eq!(unit1.get_unit(1).prefix.name, &[]);
        assert_eq!(unit1.get_unit(1).unit.name, &['h']);
        assert_eq!(-1, unit1.get_unit(1).power);

        assert_eq!(unit1.get_unit(1).prefix.name, &[]);
        assert_eq!(unit1.get_unit(1).unit.name, &['h']);
        assert_eq!(-1, unit1.get_unit(1).power);

        let unit1 = units.parse(&"km/m".chars().collect::<Vec<char>>());
        assert_eq!(EMPTY_UNIT_DIMENSIONS, unit1.0.dimensions);
        assert_eq!(4, unit1.1);
    }

    #[test]
    fn test_is_derive() {
        let units = Units::new();
        assert_eq!(false, parse("kg", &units).is_derived());
        assert_eq!(true, parse("kg/s", &units).is_derived());
        assert_eq!(true, parse("kg^2", &units).is_derived());
        assert_eq!(false, parse("N", &units).is_derived());
        assert_eq!(true, parse("kg*m/s^2", &units).is_derived());
    }

    #[test]
    fn test_value_and_dim() {
        let units = Units::new();
        assert_eq!(parse("s*A", &units), parse("C", &units));
        assert_eq!(parse("W/A", &units), parse("V", &units));
        assert_eq!(parse("V/A", &units), parse("ohm", &units));
        assert_eq!(parse("C/V", &units), parse("F", &units));
        assert_eq!(parse("J/A", &units), parse("Wb", &units));
        assert_eq!(parse("Wb/m^2", &units), parse("T", &units));
        assert_eq!(parse("Wb/A", &units), parse("H", &units));
        assert_eq!(parse("ohm^-1", &units), parse("S", &units));
        assert_eq!(parse("eV", &units), parse("J", &units));
    }

    #[test]
    fn test_angles() {
        let units = Units::new();

        assert_eq!(parse("radian", &units), parse("rad", &units));
        assert_eq!(parse("radians", &units), parse("rad", &units));
        assert_eq!(parse("degree", &units), parse("deg", &units));
        assert_eq!(parse("degrees", &units), parse("deg", &units));
        assert_eq!(parse("gradian", &units), parse("grad", &units));
        assert_eq!(parse("gradians", &units), parse("grad", &units));
    }

    #[test]
    fn test_invalid_power() {
        let units = Units::new();

        let unit1 = parse("s ^^", &units);
        assert_eq!(1, unit1.unit_count);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(1, unit1.dimensions[2]);

        assert_eq!(unit1.get_unit(0).prefix.name, &[]);
        assert_eq!(unit1.get_unit(0).unit.name, &['s']);
        assert_eq!(1, unit1.get_unit(0).power);

        let unit1 = parse("(in*lbg)", &units);
        assert_eq!(0, unit1.unit_count);

        let unit1 = parse("(s ^^)", &units);
        assert_eq!(0, unit1.unit_count);
    }

    #[test]
    fn test_parsing_units_in_denom() {
        let units = Units::new();

        let unit1 = parse("years * 12/year", &units);
        assert_eq!(1, unit1.unit_count);
        assert_eq!(1, unit1.get_unit(0).power);
        assert_eq!(1, unit1.dimensions[2]);

        assert_eq!(unit1.get_unit(0).prefix.name, &[]);
        assert_eq!(unit1.get_unit(0).unit.name, &['y', 'e', 'a', 'r',]);
        assert_eq!(1, unit1.get_unit(0).power);
    }

    #[test]
    fn test_parsing_units_in_denom2() {
        let units = Units::new();

        let unit1 = parse("12/year", &units);
        assert_eq!(0, unit1.unit_count);
    }

    #[test]
    fn test_too_big_exponent() {
        let units = Units::new();

        let unit1 = parse("T^81", &units);
        assert_eq!(0, unit1.unit_count);
    }

    #[test]
    fn test_too_big_exponent2() {
        let units = Units::new();

        let unit1 = parse("T^-81", &units);
        assert_eq!(0, unit1.unit_count);
    }

    #[test]
    fn parsing_bug_fuzz() {
        let units = Units::new();

        let unit1 = parse("K^61595", &units);
        assert_eq!(0, unit1.unit_count);
    }

    #[test]
    fn test_parsing_huge_number_no_panic() {
        let units = Units::new();

        let unit1 = parse("$^917533673846412864165166106750540", &units);
        assert_eq!(unit1.unit_count, 1);
        assert_eq!(unit1.get_unit(0).unit.name, &['$']);
        assert_eq!(unit1.get_unit(0).power, 1);
    }

    #[test]
    fn test_parse_too_many_units() {
        let units = Units::new();

        let unit1 = parse("km*h*s*b*J*A*ft*L*mi", &units);
        assert_eq!(unit1.unit_count, 8);
        assert_eq!(unit1.get_unit(0).unit.name, &['m']);
        assert_eq!(unit1.get_unit(1).unit.name, &['h']);
        assert_eq!(unit1.get_unit(2).unit.name, &['s']);
        assert_eq!(unit1.get_unit(3).unit.name, &['b']);
        assert_eq!(unit1.get_unit(4).unit.name, &['J']);
        assert_eq!(unit1.get_unit(5).unit.name, &['A']);
        assert_eq!(unit1.get_unit(6).unit.name, &['f', 't']);
        assert_eq!(unit1.get_unit(7).unit.name, &['L']);
    }
}
