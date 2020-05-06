use crate::calc::{pow, MAX_PRECISION};
use crate::renderer::strip_trailing_zeroes;
use crate::units::consts::{
    get_base_unit_for, init_aliases, init_units, BASE_UNIT_DIMENSIONS, BASE_UNIT_DIMENSION_COUNT,
};
use crate::units::{Prefix, Unit, UnitPrefixes};
use bigdecimal::BigDecimal;
use smallvec::alloc::fmt::{Debug, Display, Formatter};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::fmt::Write;
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

pub struct Units<'a> {
    pub prefixes: &'a UnitPrefixes,
    pub units: HashMap<&'static str, Unit<'a>>,
    pub aliases: HashMap<&'static str, &'static str>,
    pub no_prefix: Prefix,
}

impl<'a> Units<'a> {
    pub fn new(prefixes: &'a UnitPrefixes) -> Units<'a> {
        Units {
            no_prefix: Prefix::new(&[], "1", false),
            prefixes,
            units: init_units(&prefixes),
            aliases: init_aliases(),
        }
    }

    pub fn parse(&self, text: &[char]) -> (UnitOutput, usize) {
        let mut output = UnitOutput::new();
        let mut power_multiplier_current: isize = 1;
        let mut expecting_unit = false;

        // Optional number at the start of the string
        let mut last_valid_cursor_pos = 0;
        let mut c = text;

        // Stack to keep track of powerMultipliers applied to each parentheses group
        let mut power_multiplier_stack = vec![];

        // Running product of all elements in powerMultiplierStack
        let mut power_multiplier_stack_product = 1;

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

            let value = parse_number(&mut c);

            c = skip_whitespaces(c);
            if value.is_some() && !c.is_empty() {
                // handle multiplication or division right after the value, like '1/s'
                if parse_char(&mut c, '*') {
                    power_multiplier_current = 1;
                } else if parse_char(&mut c, '/') {
                    power_multiplier_current = -1;
                } else {
                    return (output, last_valid_cursor_pos);
                }
                expecting_unit = true;
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

            let mut power = power_multiplier_current * power_multiplier_stack_product;
            // Is there a "^ number"?
            c = skip_whitespaces(c);
            if parse_char(&mut c, '^') {
                c = skip_whitespaces(c);
                let p = parse_number(&mut c);
                if let Some(p) = p {
                    power *= p as isize;
                } else {
                    // No valid number found for the power!
                    output.add_unit(UnitInstance::new(res.0, res.1, power));
                    break 'main_loop;
                }
            }
            output.add_unit(UnitInstance::new(res.0, res.1, power));
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
            output.units.clear();
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

    fn find_unit<'units>(&'units self, str: &[char]) -> Option<(&'units Unit, &'units Prefix)> {
        if str.is_empty() {
            return None;
        }
        // TODO fostos char slice (collect...)
        if let Some(exact_match_unit) = self
            .units
            .get(str.iter().map(|it| *it).collect::<String>().as_str())
        {
            return Some((exact_match_unit, &self.no_prefix));
        }
        fn check<'units>(
            this: &'units Units,
            str: &[char],
            unit: &'units Unit<'units>,
            unit_name: &'static str,
        ) -> Option<(&'units Unit<'units>, &'units Prefix)> {
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
                    return Some((unit, &this.no_prefix));
                }
                let prefix_name = &str[0..prefix_len];
                let prefix = Units::find_prefix_for(unit, prefix_name);
                if let Some(prefix) = prefix {
                    return Some((unit, prefix));
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
            let unit = &self.units.get(unit_name).expect(unit_name);
            let result = check(self, str, unit, alias);
            if result.is_some() {
                return result;
            }
        }
        return None;
    }

    pub fn simplify(&self, unit: &UnitOutput) -> Option<UnitOutput> {
        if let Some(base_unit) = get_base_unit_for(self, &unit.dimensions) {
            Some(UnitOutput {
                units: vec![UnitInstance {
                    unit: base_unit.unit,
                    prefix: base_unit.prefix,
                    power: 1,
                }],
                dimensions: base_unit.unit.base,
            })
        } else {
            None
        }
    }

    fn find_prefix_for<'b>(unit: &'b Unit, prefix_name: &[char]) -> Option<&'b Prefix> {
        match unit.prefix_groups {
            (Some(p1), Some(p2)) => p1.iter().chain(p2).find(|it| it.name == prefix_name),
            (Some(p1), None) => p1.iter().find(|it| it.name == prefix_name),
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

#[derive(Clone, Eq)]
pub struct UnitOutput<'a> {
    // TOOD: replace it with a fixed array Some None?
    pub units: Vec<UnitInstance<'a>>,
    pub dimensions: [isize; BASE_UNIT_DIMENSION_COUNT],
}

impl<'a> Debug for UnitOutput<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}]", self.units)
    }
}

impl<'a> Display for UnitOutput<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut nnum = 0;
        let mut nden = 0;
        let mut str_num: SmallVec<[char; 32]> = SmallVec::with_capacity(32);
        let mut str_den: SmallVec<[char; 32]> = SmallVec::with_capacity(32);

        // find prefix here
        // eltárolni nem hülyeség a prefixet mert lehet h abban akarja
        // látni alapból amivel számol, csak ne használjuk számolásra

        for unit in self.units.iter() {
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
            for unit in self.units.iter() {
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

impl<'a> UnitOutput<'a> {
    pub fn new() -> UnitOutput<'a> {
        UnitOutput {
            units: vec![],
            dimensions: [0; BASE_UNIT_DIMENSION_COUNT],
        }
    }

    pub fn add_unit(&mut self, unit: UnitInstance<'a>) {
        for i in 0..BASE_UNIT_DIMENSION_COUNT {
            self.dimensions[i] += unit.unit.base[i] * unit.power;
        }
        self.units.push(unit);
    }

    pub fn is_unitless(&self) -> bool {
        self.dimensions.iter().all(|it| *it == 0)
    }

    pub fn simplify(&self, units: &'a Units) -> Option<UnitOutput<'a>> {
        if let Some(base_unit) = dbg!(units.simplify(self)) {
            // e.g. don't convert from km to m, but convert from kg*m/s^2 to N
            // base_unit.units.len() is always 1
            let base_unit_is_simpler = self.units.len() > 1;
            if base_unit_is_simpler {
                Some(base_unit)
            } else {
                None
            }
        } else {
            let mut proposed_unit_list: SmallVec<[UnitInstance; 8]> = SmallVec::with_capacity(8);
            for i in 0..BASE_UNIT_DIMENSION_COUNT {
                if self.dimensions[i] != 0 {
                    if let Some(u) = get_base_unit_for(units, &BASE_UNIT_DIMENSIONS[i]) {
                        proposed_unit_list.push(UnitInstance {
                            unit: u.unit,
                            prefix: u.prefix,
                            power: self.dimensions[i],
                        });
                    } else {
                        return None;
                    }
                }
            }
            // Is the proposed unit list "simpler" than the existing one?
            if proposed_unit_list.len() < self.units.len() {
                Some(UnitOutput {
                    units: proposed_unit_list.to_vec(),
                    ..self.clone()
                })
            } else {
                None
            }
        }
    }
}

impl<'a> std::ops::Mul for &UnitOutput<'a> {
    type Output = UnitOutput<'a>;

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

        for other_unit in &other.units {
            result.units.push(other_unit.clone());
        }

        return result;
    }
}

impl<'a> std::ops::Div for &UnitOutput<'a> {
    type Output = UnitOutput<'a>;

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

        for other_unit in &other.units {
            let mut clone = other_unit.clone();
            clone.power = -clone.power;
            result.units.push(clone);
        }

        return result;
    }
}

impl<'a> PartialEq for UnitOutput<'a> {
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

impl<'a> UnitOutput<'a> {
    pub fn normalize(&self, value: &BigDecimal) -> BigDecimal {
        if self.is_derived() {
            let mut result = value.clone();
            for unit in &self.units {
                dbg!(&unit);
                let base_value = &unit.unit.value;
                let prefix_val = &unit.prefix.value;
                let power = unit.power;
                result = dbg!(result) * dbg!(pow(base_value * prefix_val, power as i64));
            }
            return result;
        } else {
            let base_value = &self.units[0].unit.value;
            let offset = &self.units[0].unit.offset;
            let prefix_val = &self.units[0].prefix.value;

            return (value + offset) * (base_value * prefix_val);
        }
    }

    pub fn denormalize(&self, value: &BigDecimal) -> BigDecimal {
        return if self.is_derived() {
            let mut result = value.clone();
            for unit in &self.units {
                let base_value = &unit.unit.value;
                let prefix_val = &unit.prefix.value;
                let power = unit.power;
                let pow = pow(base_value * prefix_val, power as i64);
                result = dbg!(result / dbg!(pow));
            }
            result
        } else {
            // az előző ág az a current numra hivodik meg mivel az a km/h*h unitot számolja
            // ki, ami derived, viszont a /h*h miatt eltünik a m/sbol fakadó
            // pontatlanság és visszakap 120at
            //     ez az ág akkor hivodik, amikor a 120 000.0006.. m-t akarja megkapni mben,
            // mivel a méter már alapban pontatlanul van téárolva, vissza is pontatlant kap
            let base_value = dbg!(&self.units[0].unit.value);
            let offset = dbg!(&self.units[0].unit.offset);
            let prefix_val = dbg!(&self.units[0].prefix.value);

            dbg!(((value / base_value) / prefix_val) - offset).with_prec(MAX_PRECISION)
        };
    }

    pub fn pow(&self, p: i64) -> UnitOutput<'a> {
        let mut result = self.clone();
        for dim in &mut result.dimensions {
            *dim *= p as isize;
        }
        for unit in &mut result.units {
            unit.power *= p as isize;
        }

        return result;
    }

    pub fn is_derived(&self) -> bool {
        self.units.len() > 1 || (self.units.len() == 1 && self.units[0].power > 1)
    }
}

#[derive(Eq, PartialEq, Clone)]
pub struct UnitInstance<'a> {
    pub unit: &'a Unit<'a>,
    pub prefix: &'a Prefix,
    pub power: isize,
}

impl<'a> UnitInstance<'a> {
    pub fn new(unit: &'a Unit, prefix: &'a Prefix, power: isize) -> UnitInstance<'a> {
        UnitInstance {
            unit,
            prefix,
            power,
        }
    }
}

impl<'a> Debug for UnitInstance<'a> {
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
    use crate::units::consts::{create_prefixes, EMPTY_UNIT_DIMENSIONS};
    use bigdecimal::*;

    fn parse<'a>(str: &'a str, units: &'a Units) -> UnitOutput<'a> {
        units.parse(&str.chars().collect::<Vec<char>>()).0
    }

    #[test]
    fn should_create_unit_correctly() {
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);

        let unit1 = parse("cm", &units);
        assert_eq!(&['m'], unit1.units[0].unit.name);

        let unit1 = parse("kg", &units);
        assert_eq!(&['g'], unit1.units[0].unit.name);

        let unit1 = parse("(kg m)/J^2", &units);
        assert_eq!(&['g'], unit1.units[0].unit.name);
        assert_eq!(&['k'], unit1.units[0].prefix.name);
        assert_eq!(&['m'], unit1.units[1].unit.name);
        assert_eq!(&['J'], unit1.units[2].unit.name);
        assert_eq!(-2, unit1.units[2].power);

        let unit1 = parse("(kg m)/s^2", &units);
        assert_eq!(&['g'], unit1.units[0].unit.name);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(unit1.units[0].prefix.name, &['k']);
        assert_eq!(&['m'], unit1.units[1].unit.name);
        assert_eq!(1, unit1.units[1].power);
        assert_eq!(unit1.units[1].prefix.name, &[]);
        assert_eq!(&['s'], unit1.units[2].unit.name);
        assert_eq!(unit1.units[2].prefix.name, &[]);
        assert_eq!(-2, unit1.units[2].power);

        let unit1 = parse("cm/s", &units);
        assert_eq!(&['c'], unit1.units[0].prefix.name);
        assert_eq!(&['m'], unit1.units[0].unit.name);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(&['s'], unit1.units[1].unit.name);
        assert_eq!(-1, unit1.units[1].power);

        let unit1 = parse("ml", &units);
        assert_eq!(&['m'], unit1.units[0].prefix.name);
        assert_eq!(&['l'], unit1.units[0].unit.name);
        assert_eq!(3, unit1.dimensions[1]);
        assert_eq!(1, unit1.units[0].power);

        let unit1 = parse("ml^-1", &units);
        assert_eq!(&['m'], unit1.units[0].prefix.name);
        assert_eq!(&['l'], unit1.units[0].unit.name);
        assert_eq!(-3, unit1.dimensions[1]);
        assert_eq!(-1, unit1.units[0].power);

        let unit1 = parse("Hz", &units);
        assert_eq!(&['H', 'z'], unit1.units[0].unit.name);

        let unit1 = parse("km2", &units);
        assert_eq!(&['m', '2'], unit1.units[0].unit.name);

        let unit1 = parse("km^3", &units);
        assert_eq!(&['m'], unit1.units[0].unit.name);
        assert_eq!(3, unit1.units[0].power);
        assert_eq!(3, unit1.dimensions[1]);
        assert_eq!(&['k'], unit1.units[0].prefix.name);

        let unit1 = parse("km3", &units);
        assert_eq!(&['m', '3'], unit1.units[0].unit.name);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(3, unit1.dimensions[1]);
        assert_eq!(
            BigDecimal::from_i64(1000000000).unwrap(),
            unit1.units[0].prefix.value
        );
        assert_eq!(&['k'], unit1.units[0].prefix.name);

        // should test whether two units have the same base unit
        assert_eq!(&parse("cm", &units), &parse("m", &units));
        assert_ne!(&parse("cm", &units), &parse("kg", &units));
        assert_eq!(&parse("N", &units), &parse("kg*m / s ^ 2", &units));
        assert_eq!(
            &parse("J / mol*K", &units),
            &parse("ft^3*psi / mol*degF", &units)
        );

        let unit1 = parse("bytes", &units);
        assert_eq!(&['b', 'y', 't', 'e', 's'], unit1.units[0].unit.name);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(unit1.units[0].prefix.name, &[]);

        // Kibi BIT!
        let unit1 = parse("Kib", &units);
        assert_eq!(&['b'], unit1.units[0].unit.name);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(&['K', 'i'], unit1.units[0].prefix.name);

        let unit1 = parse("Kib/s", &units);
        assert_eq!(&['K', 'i'], unit1.units[0].prefix.name);
        assert_eq!(&['b'], unit1.units[0].unit.name);
        assert_eq!(&['s'], unit1.units[1].unit.name);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(-1, unit1.units[1].power);

        let unit1 = parse("b/s", &units);
        assert_eq!(unit1.units[0].prefix.name, &[]);
        assert_eq!(&['b'], unit1.units[0].unit.name);
        assert_eq!(&['s'], unit1.units[1].unit.name);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(-1, unit1.units[1].power);

        let unit1 = parse("kb", &units);
        assert_eq!(unit1.units[0].prefix.name, &['k']);
        assert_eq!(&['b'], unit1.units[0].unit.name);
        assert_eq!(1, unit1.units[0].power);

        let unit1 = parse("cm*s^-2", &units);
        assert_eq!(unit1.units[0].unit.name, &['m']);
        assert_eq!(unit1.units[1].unit.name, &['s']);
        assert_eq!(&['c'], unit1.units[0].prefix.name);
        assert_eq!(-2, unit1.units[1].power);

        let unit1 = parse("kg*m^2 / s^2 / K / mol", &units);
        assert_eq!(unit1.units[0].unit.name, &['g']);
        assert_eq!(unit1.units[1].unit.name, &['m']);
        assert_eq!(unit1.units[2].unit.name, &['s']);
        assert_eq!(unit1.units[3].unit.name, &['K']);
        assert_eq!(unit1.units[4].unit.name, &['m', 'o', 'l']);
        assert_eq!(&['k'], unit1.units[0].prefix.name);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(2, unit1.units[1].power);
        assert_eq!(-2, unit1.units[2].power);
        assert_eq!(-1, unit1.units[3].power);
        assert_eq!(-1, unit1.units[4].power);

        let unit1 = parse("kg*(m^2 / (s^2 / (K^-1 / mol)))", &units);
        assert_eq!(unit1.units[0].unit.name, &['g']);
        assert_eq!(unit1.units[1].unit.name, &['m']);
        assert_eq!(unit1.units[2].unit.name, &['s']);
        assert_eq!(unit1.units[3].unit.name, &['K']);
        assert_eq!(unit1.units[4].unit.name, &['m', 'o', 'l']);
        assert_eq!(&['k'], unit1.units[0].prefix.name);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(2, unit1.units[1].power);
        assert_eq!(-2, unit1.units[2].power);
        assert_eq!(-1, unit1.units[3].power);
        assert_eq!(-1, unit1.units[4].power);

        let unit1 = parse("(m / ( s / ( kg mol ) / ( lbm / h ) K ) )", &units);
        assert_eq!(unit1.units[0].unit.name, &['m']);
        assert_eq!(unit1.units[1].unit.name, &['s']);
        assert_eq!(unit1.units[2].unit.name, &['g']);
        assert_eq!(unit1.units[3].unit.name, &['m', 'o', 'l']);
        assert_eq!(unit1.units[4].unit.name, &['l', 'b', 'm']);
        assert_eq!(unit1.units[5].unit.name, &['h']);
        assert_eq!(unit1.units[6].unit.name, &['K']);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(-1, unit1.units[1].power);
        assert_eq!(1, unit1.units[2].power);
        assert_eq!(1, unit1.units[3].power);
        assert_eq!(1, unit1.units[4].power);
        assert_eq!(-1, unit1.units[5].power);
        assert_eq!(-1, unit1.units[6].power);

        let unit1 = parse("(m/(s/(kg mol)/(lbm/h)K))", &units);
        assert_eq!(unit1.units[0].unit.name, &['m']);
        assert_eq!(unit1.units[1].unit.name, &['s']);
        assert_eq!(unit1.units[2].unit.name, &['g']);
        assert_eq!(unit1.units[3].unit.name, &['m', 'o', 'l']);
        assert_eq!(unit1.units[4].unit.name, &['l', 'b', 'm']);
        assert_eq!(unit1.units[5].unit.name, &['h']);
        assert_eq!(unit1.units[6].unit.name, &['K']);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(-1, unit1.units[1].power);
        assert_eq!(1, unit1.units[2].power);
        assert_eq!(1, unit1.units[3].power);
        assert_eq!(1, unit1.units[4].power);
        assert_eq!(-1, unit1.units[5].power);
        assert_eq!(-1, unit1.units[6].power);

        // should parse units with correct precedence
        let unit1 = parse("m^3 / kg*s^2", &units);
        assert_eq!(unit1.units[0].unit.name, &['m']);
        assert_eq!(unit1.units[1].unit.name, &['g']);
        assert_eq!(unit1.units[2].unit.name, &['s']);
        assert_eq!(3, unit1.units[0].power);
        assert_eq!(-1, unit1.units[1].power);
        assert_eq!(2, unit1.units[2].power);

        let unit1 = parse("m^3 / (kg s^2)", &units);
        assert_eq!(unit1.units[0].unit.name, &['m']);
        assert_eq!(unit1.units[1].unit.name, &['g']);
        assert_eq!(unit1.units[2].unit.name, &['s']);
        assert_eq!(3, unit1.units[0].power);
        assert_eq!(-1, unit1.units[1].power);
        assert_eq!(-2, unit1.units[2].power);
    }

    #[test]
    fn exp_notation() {
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);

        // exponential notation, binary or hex is not supported in exponents
        let unit1 = parse("kg^1e0 * m^1.0e3 * s^-2.0e0", &units);
        assert_eq!(1, unit1.units.len());
        assert_eq!(unit1.units[0].prefix.name, &['k']);
        assert_eq!(unit1.units[0].unit.name, &['g']);
        assert_eq!(1, unit1.units[0].power);

        let unit1 = parse("kg^0b01", &units);
        assert_eq!(1, unit1.units.len());
        assert_eq!(unit1.units[0].prefix.name, &['k']);
        assert_eq!(unit1.units[0].unit.name, &['g']);
        assert_eq!(0, unit1.units[0].power);

        let unit1 = parse("kg^0xFF", &units);
        assert_eq!(1, unit1.units.len());
        assert_eq!(unit1.units[0].prefix.name, &['k']);
        assert_eq!(unit1.units[0].unit.name, &['g']);
        assert_eq!(0, unit1.units[0].power);
    }

    #[test]
    fn test_prefixes() {
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);
        //should accept both long and short prefixes
        assert_eq!(parse("ohm", &units).units[0].unit.name, &['o', 'h', 'm']);
        assert_eq!(
            parse("milliohm", &units).units[0].unit.name,
            &['o', 'h', 'm']
        );
        assert_eq!(parse("mohm", &units).units[0].unit.name, &['o', 'h', 'm']);

        assert_eq!(parse("bar", &units).units[0].unit.name, &['b', 'a', 'r']);
        assert_eq!(
            parse("millibar", &units).units[0].unit.name,
            &['b', 'a', 'r']
        );
        assert_eq!(parse("mbar", &units).units[0].unit.name, &['b', 'a', 'r']);
    }

    #[test]
    fn test_plurals() {
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);

        let unit1 = parse("meters", &units);
        assert_eq!(&['m', 'e', 't', 'e', 'r'], unit1.units[0].unit.name);
        assert_eq!(unit1.units[0].prefix.name, &[]);

        let unit1 = parse("kilometers", &units);
        assert_eq!(&['m', 'e', 't', 'e', 'r'], unit1.units[0].unit.name);
        assert_eq!(unit1.units[0].prefix.name, &['k', 'i', 'l', 'o']);

        let unit1 = parse("inches", &units);
        assert_eq!(&['i', 'n', 'c', 'h'], unit1.units[0].unit.name);
        assert_eq!(unit1.units[0].prefix.name, &[]);
    }

    #[test]
    fn test_units_j_mol_k_parsing() {
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);

        let unit1 = parse("(J / mol / K)", &units);
        assert_eq!(unit1.units[0].prefix.name, &[]);
        assert_eq!(unit1.units[1].prefix.name, &[]);
        assert_eq!(unit1.units[2].prefix.name, &[]);
        assert_eq!(unit1.units[0].unit.name, &['J']);
        assert_eq!(unit1.units[1].unit.name, &['m', 'o', 'l']);
        assert_eq!(unit1.units[2].unit.name, &['K']);
        let parsed_len = units
            .parse(&"(J / mol / K)".chars().collect::<Vec<char>>())
            .1;
        assert_eq!(parsed_len, "(J / mol / K)".len());

        let unit1 = parse("(J / mol / K) ^ 0", &units);
        assert_eq!(unit1.units[0].prefix.name, &[]);
        assert_eq!(unit1.units[1].prefix.name, &[]);
        assert_eq!(unit1.units[2].prefix.name, &[]);
        assert_eq!(unit1.units[0].unit.name, &['J']);
        assert_eq!(unit1.units[1].unit.name, &['m', 'o', 'l']);
        assert_eq!(unit1.units[2].unit.name, &['K']);

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
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);

        let unit1 = parse("(km/h) * h", &units);
        assert_eq!(3, unit1.units.len());
        assert_eq!(unit1.units[0].prefix.name, &['k']);
        assert_eq!(unit1.units[0].unit.name, &['m']);
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(unit1.units[1].prefix.name, &[]);
        assert_eq!(unit1.units[1].unit.name, &['h']);
        assert_eq!(-1, unit1.units[1].power);
        assert_eq!(unit1.units[2].prefix.name, &[]);
        assert_eq!(unit1.units[2].unit.name, &['h']);
        assert_eq!(1, unit1.units[2].power);

        let unit1 = parse("km/h*h/h/h", &units);
        assert_eq!(5, unit1.units.len());

        assert_eq!(unit1.units[0].prefix.name, &['k']);
        assert_eq!(unit1.units[0].unit.name, &['m']);
        assert_eq!(1, unit1.units[0].power);

        assert_eq!(unit1.units[1].prefix.name, &[]);
        assert_eq!(unit1.units[1].unit.name, &['h']);
        assert_eq!(-1, unit1.units[1].power);

        assert_eq!(unit1.units[1].prefix.name, &[]);
        assert_eq!(unit1.units[1].unit.name, &['h']);
        assert_eq!(-1, unit1.units[1].power);

        assert_eq!(unit1.units[1].prefix.name, &[]);
        assert_eq!(unit1.units[1].unit.name, &['h']);
        assert_eq!(-1, unit1.units[1].power);

        assert_eq!(unit1.units[1].prefix.name, &[]);
        assert_eq!(unit1.units[1].unit.name, &['h']);
        assert_eq!(-1, unit1.units[1].power);

        let unit1 = units.parse(&"km/m".chars().collect::<Vec<char>>());
        assert_eq!(EMPTY_UNIT_DIMENSIONS, unit1.0.dimensions);
        assert_eq!(4, unit1.1);
    }

    #[test]
    fn test_is_derive() {
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);
        assert_eq!(false, parse("kg", &units).is_derived());
        assert_eq!(true, parse("kg/s", &units).is_derived());
        assert_eq!(true, parse("kg^2", &units).is_derived());
        assert_eq!(false, parse("N", &units).is_derived());
        assert_eq!(true, parse("kg*m/s^2", &units).is_derived());
    }

    #[test]
    fn test_value_and_dim() {
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);
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
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);

        assert_eq!(parse("radian", &units), parse("rad", &units));
        assert_eq!(parse("radians", &units), parse("rad", &units));
        assert_eq!(parse("degree", &units), parse("deg", &units));
        assert_eq!(parse("degrees", &units), parse("deg", &units));
        assert_eq!(parse("gradian", &units), parse("grad", &units));
        assert_eq!(parse("gradians", &units), parse("grad", &units));
    }

    #[test]
    fn test_invalid_power() {
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);

        let unit1 = parse("s ^^", &units);
        assert_eq!(1, unit1.units.len());
        assert_eq!(1, unit1.units[0].power);
        assert_eq!(1, unit1.dimensions[2]);

        assert_eq!(unit1.units[0].prefix.name, &[]);
        assert_eq!(unit1.units[0].unit.name, &['s']);
        assert_eq!(1, unit1.units[0].power);

        let unit1 = parse("(in*lbg)", &units);
        assert_eq!(0, unit1.units.len());

        let unit1 = parse("(s ^^)", &units);
        assert_eq!(0, unit1.units.len());
    }
}
