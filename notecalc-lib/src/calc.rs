use std::fmt::Write;
use std::ops::BitXor;
use std::ops::Neg;
use std::ops::Not;

use bigdecimal::*;
use smallvec::alloc::fmt::{Error, Formatter};
use smallvec::SmallVec;

use crate::matrix::MatrixData;
use crate::token_parser::{OperatorTokenType, Token, TokenType};
use crate::units::units::{UnitOutput, Units};

// it is limited by bigdecimal crate :(
// TODO: download and mofiy the crate
// modositsd ugy, h a beégetett 100 precision legyen akár 1000,
// de adjon vissza valmai jelzést, ha egy irracionális számmal van dolgunk (azaz elértük
// a limitet, és akkor arra a számolásra limitáljuk a precisiont
// A megoldás a problémára az volt, h jelenleg 100 precisionnel számol a rendszer
// de a számokat ahol használjuk 50 precisionnel használjuk, igy a számolási hibák eltünnek
// az 50 precision miatt, mivel azok "mögötte" lesznek (tehát az 50. tizedesjegy után)
pub const MAX_PRECISION: u64 = 50;

#[derive(Debug, Clone)]
pub enum CalcResult<'units> {
    Number(BigDecimal),
    Percentage(BigDecimal),
    Quantity(BigDecimal, UnitOutput<'units>),
    Matrix(MatrixData<'units>),
}

impl<'a> std::fmt::Display for CalcResult<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fn num_to_string(f: &mut Formatter<'_>, num: &BigDecimal) -> std::fmt::Result {
            let num = if num.is_integer() {
                num.with_scale(0)
            } else {
                strip_trailing_zeroes(num)
            };

            f.write_str(&num.to_string())
        }
        match self {
            CalcResult::Number(num) => {
                // TODO optimize
                num_to_string(f, num)?;
            }
            CalcResult::Percentage(num) => {
                num_to_string(f, num)?;
                f.write_char('%')?;
            }
            CalcResult::Quantity(num, unit) => {
                num_to_string(f, dbg!(&unit.denormalize(num)))?;
                f.write_char(' ')?;
                f.write_str(&unit.to_string())?;
            }
            CalcResult::Matrix(mat) => {
                if !mat.is_vector() {
                    f.write_char('[')?;
                }
                for (col_i, cols) in mat.cols.iter().enumerate() {
                    if col_i > 0 {
                        f.write_char(',')?;
                        f.write_char(' ')?;
                    }
                    f.write_char('[')?;
                    for (row_i, cell) in cols.iter().enumerate() {
                        if row_i > 0 {
                            f.write_char(',')?;
                            f.write_char(' ')?;
                        }
                        f.write_str(&cell.to_string())?;
                    }
                    f.write_char(']')?;
                }
                if !mat.is_vector() {
                    f.write_char(']')?;
                }
            }
        }
        Ok(())
    }
}

pub fn evaluate_tokens<'text_ptr, 'units>(
    tokens: &mut Vec<TokenType<'units>>,
    units: &'units Units,
) -> Option<(CalcResult<'units>, bool)> {
    let mut stack = vec![];
    let mut there_was_unit_conversion = false;
    let mut last_success_operation_result_index = None;
    dbg!(&tokens);
    for (i, token) in tokens.iter_mut().enumerate() {
        match &token {
            TokenType::NumberLiteral(num) => stack.push(CalcResult::Number(num.clone())),
            TokenType::Operator(typ) => {
                if !stack.is_empty() {
                    if apply_operation(&mut stack, &typ, units) == true {
                        if matches!(typ, OperatorTokenType::UnitConverter) {
                            there_was_unit_conversion = true;
                        }
                        dbg!(&stack);
                        last_success_operation_result_index = dbg!(Some(stack.len() - 1));
                    } else {
                        // the operation failed, it is not an operator but a string ?
                    }
                }
            }
            TokenType::StringLiteral => panic!(),
            TokenType::Variable => {}
        }
    }
    return match last_success_operation_result_index {
        Some(last_success_operation_index) => {
            // e.g. "1+2 some text 3"
            // in this case prefer the result of 1+2 and ignore the number 3
            Some((
                stack[last_success_operation_index].clone(),
                there_was_unit_conversion,
            ))
        }
        None => stack.pop().map(|it| (it, there_was_unit_conversion)),
    };
}

fn apply_operation<'text_ptr, 'units>(
    stack: &mut Vec<CalcResult<'units>>,
    op: &OperatorTokenType<'units>,
    units: &'units Units,
) -> bool {
    let succeed = match &op {
        OperatorTokenType::Mult
        | OperatorTokenType::Div
        | OperatorTokenType::Add
        | OperatorTokenType::Sub
        | OperatorTokenType::And
        | OperatorTokenType::Or
        | OperatorTokenType::Xor
        | OperatorTokenType::Pow
        | OperatorTokenType::ShiftLeft
        | OperatorTokenType::ShiftRight
        | OperatorTokenType::UnitConverter => {
            if stack.len() > 1 {
                let (lhs, rhs) = (&stack[stack.len() - 2], &stack[stack.len() - 1]);
                if let Some(result) = binary_operation(op, lhs, rhs, units) {
                    stack.truncate(stack.len() - 2);
                    stack.push(result);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
        OperatorTokenType::UnaryMinus
        | OperatorTokenType::UnaryPlus
        | OperatorTokenType::Perc
        | OperatorTokenType::Not
        | OperatorTokenType::Unit(_) => {
            let maybe_top = stack.last();
            if let Some(result) = maybe_top.and_then(|top| unary_operation(&op, top)) {
                stack.pop();
                stack.push(result);
                true
            } else if let OperatorTokenType::Unit(target_unit) = &op {
                // it is the unit operand for "in" conversion
                // e.g. "3m in cm",
                // put the unit name into the stack, the next operator is probably an 'in'
                stack.push(CalcResult::Quantity(
                    BigDecimal::zero(),
                    target_unit.clone(),
                ));
                true
            } else {
                false
            }
        }
        OperatorTokenType::Matrix { arg_count } => {
            if stack.len() >= *arg_count {
                let matrix_args = stack.drain(stack.len() - *arg_count..).collect::<Vec<_>>();
                stack.push(CalcResult::Matrix(MatrixData::new(matrix_args)));
                true
            } else {
                false
            }
        }
        OperatorTokenType::Comma => {
            // ignore
            true
        }
        OperatorTokenType::Assign => todo!(),
        OperatorTokenType::ParenOpen
        | OperatorTokenType::ParenClose
        | OperatorTokenType::BracketOpen
        | OperatorTokenType::BracketClose => {
            dbg!(op);
            panic!();
        }
    };
    return succeed;
}

fn unary_operation<'text_ptr, 'units>(
    op: &OperatorTokenType<'units>,
    top: &CalcResult<'units>,
) -> Option<CalcResult<'units>> {
    return match &op {
        OperatorTokenType::UnaryPlus => Some(top.clone()),
        OperatorTokenType::UnaryMinus => Some(unary_minus_op(top)),
        OperatorTokenType::Perc => percentage_operator(top),
        OperatorTokenType::Not => binary_complement(top),
        OperatorTokenType::Unit(target_unit) => match top {
            CalcResult::Number(num) => Some(CalcResult::Quantity(
                dbg!(target_unit.normalize(dbg!(num))),
                target_unit.clone(),
            )),
            _ => None,
        },
        _ => panic!(),
    };
}

fn binary_operation<'text_ptr, 'units>(
    op: &OperatorTokenType<'units>,
    lhs: &CalcResult<'units>,
    rhs: &CalcResult<'units>,
    units: &'units Units<'units>,
) -> Option<CalcResult<'units>> {
    let result = match &op {
        OperatorTokenType::Mult => multiply_op(lhs, rhs, units),
        OperatorTokenType::Div => divide_op(lhs, rhs),
        OperatorTokenType::Add => add_op(lhs, rhs),
        OperatorTokenType::Sub => sub_op(lhs, rhs),
        OperatorTokenType::And => binary_and_op(lhs, rhs),
        OperatorTokenType::Or => binary_or_op(lhs, rhs),
        OperatorTokenType::Xor => binary_xor_op(lhs, rhs),
        OperatorTokenType::Pow => pow_op(lhs, rhs),
        OperatorTokenType::ShiftLeft => binary_shift_left(lhs, rhs),
        OperatorTokenType::ShiftRight => binary_shift_right(lhs, rhs),
        OperatorTokenType::UnitConverter => {
            return match (lhs, rhs) {
                (
                    CalcResult::Quantity(lhs_num, source_unit),
                    CalcResult::Quantity(_, target_unit),
                ) => {
                    if source_unit == target_unit {
                        Some(CalcResult::Quantity(lhs_num.clone(), target_unit.clone()))
                    } else {
                        // incompatible units, obvious error
                        // return something so the top 2 elements will be removed from stack
                        // we might return an error?
                        Some(CalcResult::Number(BigDecimal::zero()))
                    }
                }
                _ => None,
            };
        }
        // todo: ronda h nem a tipusokkal kezelem le hanem panickal a többit
        // , csinálj egy TokenType::BinaryOp::Add
        _ => panic!(),
    };
    return match result {
        Some(CalcResult::Quantity(num, unit)) if unit.is_unitless() => {
            // some operation cancelled out its units, put a simple number on the stack
            Some(CalcResult::Number(num))
        }
        _ => result,
    };
}

fn percentage_operator<'a>(lhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match lhs {
        CalcResult::Number(lhs) => {
            // 5%
            Some(CalcResult::Percentage(lhs.clone()))
        }
        _ => None,
    }
}

fn binary_complement<'a>(lhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match lhs {
        CalcResult::Number(lhs) => {
            // 0b01 and 0b10
            let lhs = lhs.to_i64()?;
            Some(CalcResult::Number(dec(lhs.not())))
        }
        _ => None,
    }
}

fn binary_xor_op<'a>(lhs: &CalcResult<'a>, rhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match (lhs, rhs) {
        //////////////
        // 12 and x
        //////////////
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            // 0b01 and 0b10
            let lhs = lhs.to_i64()?;
            let rhs = rhs.to_i64()?;
            Some(CalcResult::Number(dec(lhs.bitxor(rhs))))
        }
        _ => None,
    }
}

fn binary_or_op<'a>(lhs: &CalcResult<'a>, rhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match (lhs, rhs) {
        //////////////
        // 12 and x
        //////////////
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            // 0b01 and 0b10
            let lhs = lhs.to_i64()?;
            let rhs = rhs.to_i64()?;
            Some(CalcResult::Number(dec(lhs | rhs)))
        }
        _ => None,
    }
}

fn binary_shift_right<'a>(lhs: &CalcResult, rhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match (lhs, rhs) {
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            let lhs = lhs.to_i64()?;
            let rhs = rhs.to_u32()?;
            Some(CalcResult::Number(dec(lhs.wrapping_shr(rhs))))
        }
        _ => None,
    }
}

fn binary_shift_left<'a>(lhs: &CalcResult, rhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match (lhs, rhs) {
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            let lhs = lhs.to_i64()?;
            let rhs = rhs.to_u32()?;
            Some(CalcResult::Number(dec(lhs.wrapping_shl(rhs))))
        }
        _ => None,
    }
}

fn binary_and_op<'a>(lhs: &CalcResult, rhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match (lhs, rhs) {
        //////////////
        // 12 and x
        //////////////
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            // 0b01 and 0b10
            let lhs = lhs.to_i64()?;
            let rhs = rhs.to_i64()?;
            Some(CalcResult::Number(dec(lhs & rhs)))
        }
        _ => None,
    }
}

fn unary_minus_op<'a>(lhs: &CalcResult<'a>) -> CalcResult<'a> {
    match lhs {
        CalcResult::Number(lhs) => {
            // -12
            CalcResult::Number(lhs.neg())
        }
        CalcResult::Quantity(lhs, unit) => {
            // -12km
            CalcResult::Quantity(lhs.neg(), unit.clone())
        }
        CalcResult::Percentage(lhs) => {
            // -50%
            CalcResult::Percentage(lhs.neg())
        }
        _ => todo!(), // CalcResult::Matrix(mat) => CalcResult::Matrix(mat.neg()),
    }
}

fn pow_op<'a>(lhs: &CalcResult<'a>, rhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match (lhs, rhs) {
        //////////////
        // 1^x
        //////////////
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            // 2^3
            rhs.to_i64()
                .map(|rhs| CalcResult::Number(pow(lhs.clone(), rhs)))
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Number(rhs)) => {
            let p = rhs.to_i64()?;
            let num_powered = pow(lhs.clone(), p);
            let unit_powered = lhs_unit.pow(p);
            Some(CalcResult::Quantity(num_powered, unit_powered))
            // TODO 1 s * 2 s^-1
        }
        _ => None,
    }
}

fn multiply_op<'units>(
    lhs: &CalcResult<'units>,
    rhs: &CalcResult<'units>,
    units: &'units Units<'units>,
) -> Option<CalcResult<'units>> {
    dbg!(lhs);
    dbg!(rhs);
    match (lhs, rhs) {
        //////////////
        // 12 * x
        //////////////
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            // 12 * 2
            Some(CalcResult::Number(lhs * rhs))
        }
        (CalcResult::Number(lhs), CalcResult::Quantity(rhs, unit)) => {
            // 12 * 2km
            Some(CalcResult::Quantity(lhs * rhs, unit.clone()))
        }
        (CalcResult::Number(lhs), CalcResult::Percentage(rhs)) => {
            // 100 * 50%
            Some(CalcResult::Number(percentage_of(rhs, lhs)))
        }
        //////////////
        // 12km * x
        //////////////
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Number(rhs)) => {
            // 2m * 5
            Some(CalcResult::Quantity(lhs * rhs, lhs_unit.clone()))
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 2s * 3s
            // TODO pls 2s * 3(1/s), az sima szám lesz
            let num = dbg!(dbg!(lhs) * dbg!(rhs));
            let new_unit = lhs_unit * rhs_unit;
            Some(CalcResult::Quantity(num, new_unit))
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Percentage(rhs)) => {
            // e.g. 2m * 50%
            Some(CalcResult::Quantity(
                percentage_of(rhs, lhs),
                lhs_unit.clone(),
            ))
        }
        //////////////
        // 12% * x
        //////////////
        (CalcResult::Percentage(lhs), CalcResult::Number(rhs)) => {
            // 5% * 10
            Some(CalcResult::Number(percentage_of(lhs, rhs)))
        }
        (CalcResult::Percentage(lhs), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 5% * 10km
            Some(CalcResult::Quantity(
                percentage_of(lhs, rhs),
                rhs_unit.clone(),
            ))
        }
        (CalcResult::Percentage(lhs), CalcResult::Percentage(rhs)) => {
            // 50% * 50%
            Some(CalcResult::Percentage((lhs / dec(100)) * (rhs / dec(100))))
        }
        _ => todo!(),
    }
}

fn add_op<'a>(lhs: &CalcResult<'a>, rhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match (lhs, rhs) {
        //////////////
        // 12 + x
        //////////////
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            // 12 + 3
            Some(CalcResult::Number(lhs + rhs))
        }
        (CalcResult::Number(_lhs), CalcResult::Quantity(_rhs, _unit)) => {
            // 12 + 3 km
            None
        }
        (CalcResult::Number(lhs), CalcResult::Percentage(rhs)) => {
            // 100 + 50%
            let x_percent_of_left_hand_side = lhs / &dec(100) * rhs;
            Some(CalcResult::Number(lhs + x_percent_of_left_hand_side))
        }
        //////////////
        // 12km + x
        //////////////
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Number(rhs)) => {
            // 2m + 5
            None
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 2s + 3s
            // TODO
            if lhs_unit != rhs_unit {
                None
            } else {
                Some(CalcResult::Quantity(lhs + rhs, lhs_unit.clone()))
            }
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Percentage(rhs)) => {
            // e.g. 2m + 50%
            let x_percent_of_left_hand_side = lhs / dec(100) * rhs;
            Some(CalcResult::Quantity(
                lhs + x_percent_of_left_hand_side,
                lhs_unit.clone(),
            ))
        }
        //////////////
        // 12% + x
        //////////////
        (CalcResult::Percentage(lhs), CalcResult::Number(rhs)) => {
            // 5% + 10
            None
        }
        (CalcResult::Percentage(lhs), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 5% + 10km
            None
        }
        (CalcResult::Percentage(lhs), CalcResult::Percentage(rhs)) => {
            // 50% + 50%
            Some(CalcResult::Percentage(lhs + rhs))
        }
        _ => todo!(),
    }
}

fn sub_op<'a>(lhs: &CalcResult<'a>, rhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match (lhs, rhs) {
        //////////////
        // 12 - x
        //////////////
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            // 12 - 3
            Some(CalcResult::Number(lhs - rhs))
        }
        (CalcResult::Number(lhs), CalcResult::Quantity(rhs, unit)) => {
            // 12 - 3 km
            None
        }
        (CalcResult::Number(lhs), CalcResult::Percentage(rhs)) => {
            // 100 - 50%
            let x_percent_of_left_hand_side = lhs / dec(100) * rhs;
            Some(CalcResult::Number(lhs - x_percent_of_left_hand_side))
        }
        //////////////
        // 12km - x
        //////////////
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Number(rhs)) => {
            // 2m - 5
            None
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Quantity(rhs, unit)) => {
            // 2s - 3s
            // TODO
            None
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Percentage(rhs)) => {
            // e.g. 2m - 50%
            let x_percent_of_left_hand_side = lhs / dec(100) * rhs;
            Some(CalcResult::Quantity(
                lhs - x_percent_of_left_hand_side,
                lhs_unit.clone(),
            ))
        }
        //////////////
        // 12% - x
        //////////////
        (CalcResult::Percentage(lhs), CalcResult::Number(rhs)) => {
            // 5% - 10
            None
        }
        (CalcResult::Percentage(lhs), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 5% - 10km
            None
        }
        (CalcResult::Percentage(lhs), CalcResult::Percentage(rhs)) => {
            // 50% - 50%
            Some(CalcResult::Percentage(lhs - rhs))
        }
        _ => todo!(),
    }
}

fn divide_op<'a>(lhs: &CalcResult<'a>, rhs: &CalcResult<'a>) -> Option<CalcResult<'a>> {
    match (lhs, rhs) {
        //////////////
        // 12 / x
        //////////////
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            // 100 / 2
            Some(CalcResult::Number(lhs / rhs))
        }
        (CalcResult::Number(lhs), CalcResult::Quantity(rhs, unit)) => {
            // 100 / 2km => (100 / 2) km
            // 100 / 2km => (100 / 2) km
            let mut new_unit = unit.pow(-1);

            let denormalized_num = unit.denormalize(rhs);
            let num_part = new_unit.normalize(&(lhs / &denormalized_num));
            Some(CalcResult::Quantity(num_part, new_unit.clone()))
        }
        (CalcResult::Number(lhs), CalcResult::Percentage(rhs)) => {
            // 100 / 50%
            Some(CalcResult::Number(lhs / rhs * dec(100)))
        }
        //////////////
        // 12km / x
        //////////////
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Number(rhs)) => {
            // 2m * 5
            Some(CalcResult::Quantity(lhs / rhs, lhs_unit.clone()))
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 12 km / 3s
            Some(CalcResult::Quantity(lhs / rhs, lhs_unit / rhs_unit))
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Percentage(rhs)) => {
            // 2m / 50%
            None
        }
        //////////////
        // 12% / x
        //////////////
        (CalcResult::Percentage(lhs), CalcResult::Number(rhs)) => {
            // 5% / 10
            None
        }
        (CalcResult::Percentage(lhs), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 5% / 10km
            None
        }
        (CalcResult::Percentage(lhs), CalcResult::Percentage(rhs)) => {
            // 50% / 50%
            None
        }
        _ => todo!(),
    }
}

pub fn pow(this: BigDecimal, mut exp: i64) -> BigDecimal {
    let mut base = this.clone();
    let mut acc = BigDecimal::one();
    let neg = exp < 0;

    exp = exp.abs();

    while exp > 1 {
        if (exp & 1) == 1 {
            acc *= &base;
        }
        exp /= 2;
        base = base.square();
    }

    if exp == 1 {
        acc *= &base;
    }

    if neg {
        BigDecimal::one() / acc
    } else {
        acc
    }
}

// TODO: really hack and ugly and slow
pub fn strip_trailing_zeroes(num: &BigDecimal) -> BigDecimal {
    let (_, mut scale) = num.as_bigint_and_exponent();
    let mut result = num.clone();
    loop {
        if scale == 0 {
            break;
        }
        let scaled = result.with_scale(scale - 1);
        if &scaled == num {
            result = scaled;
        } else {
            break;
        }
        scale -= 1;
    }
    return result;
}

pub fn dec(num: i64) -> BigDecimal {
    BigDecimal::from_i64(num).unwrap()
}

fn percentage_of(this: &BigDecimal, base: &BigDecimal) -> BigDecimal {
    base / &dec(100) * this
}

fn top_as_number(stack: &Vec<CalcResult>) -> Option<BigDecimal> {
    let top_of_stack_num = match stack.last() {
        Some(CalcResult::Number(num)) => Some(num.clone()),
        _ => None,
    };
    return top_of_stack_num;
}

#[cfg(test)]
mod tests {
    use crate::shunting_yard::tests::*;
    use crate::shunting_yard::ShuntingYard;
    use crate::token_parser::TokenParser;
    use crate::units;
    use crate::units::consts::{create_prefixes, init_units};

    use super::*;

    fn test_tokens(text: &str, expected_tokens: &[Token]) {
        println!("===================================================");
        println!("{}", text);
        let prefixes = create_prefixes();
        let mut units = Units::new(&prefixes);
        units.units = init_units(&units.prefixes);
        let temp = text.chars().collect::<Vec<char>>();
        let mut tokens = vec![];
        let mut shunting_output =
            crate::shunting_yard::tests::do_shunting_yard(&temp, &units, &mut tokens);
        let mut result_stack = evaluate_tokens(&mut shunting_output, &units);

        crate::shunting_yard::tests::compare_tokens(expected_tokens, &tokens);
    }

    fn test(text: &str, expected: &'static str) {
        dbg!("===========================================================");
        dbg!(text);
        let temp = text.chars().collect::<Vec<char>>();

        let prefixes = create_prefixes();
        let mut units = Units::new(&prefixes);
        units.units = init_units(&units.prefixes);

        let mut tokens = vec![];
        let mut shunting_output =
            crate::shunting_yard::tests::do_shunting_yard(&temp, &units, &mut tokens);
        let result = evaluate_tokens(&mut shunting_output, &units);
        dbg!(&result);
        if let Some((CalcResult::Quantity(num, unit), there_was_unit_conversion)) = result {
            if there_was_unit_conversion {
                assert_eq!(expected, CalcResult::Quantity(num, unit).to_string());
            } else {
                let maybe_simpler = unit.simplify(&units, &num);
                if let Some(simpler) = maybe_simpler {
                    assert_eq!(expected, CalcResult::Quantity(num, simpler).to_string());
                } else {
                    dbg!((&num, &unit));
                    assert_eq!(expected, CalcResult::Quantity(num, unit).to_string());
                }
            }
        } else {
            assert_eq!(
                expected,
                result.map(|it| it.0.to_string()).unwrap_or(" ".to_string())
            );
        }
    }

    #[test]
    fn calc_tests() {
        test("2^-2", "0.25");
        test("5km + 5cm", "5.00005 km");
        test("5kg*m / 1s^2", "5 N");
        test("0.000001 km2 to m2", "1 m2");
        test("0.000000001 km3 to m3", "1 m3");

        test("0.000000002 km^3 to m^3", "2 m^3");
        test("0.000000002 km3 to m^3", "2 m^3");

        test("2 - -1", "3");

        test("24 bla + 0", "24");

        // should skip automatic simplification if created directly in the constructor
        test("9.81 kg*m/s^2 * 1", "9.81 N");

        // should test whether two units are equal
        test("100 cm to m", "1 m");
        test("5000 cm to m", "50 m");

        test("100 ft * lbf to (in*lbf)", "1200 in lbf");
        test("100 N to kg*m / s ^ 2", "100 (kg m) / s^2");
        test("100 cm to m", "1 m");
        test("100 Hz to 1/s", "100 s^-1");

        test("1 ft * lbf * 2 rad", "2 ft lbf rad");
        test("1 ft * lbf * 2 rad to in*lbf*rad", "24 in lbf rad");
        test(
            "(2/3)m",
            "0.66666666666666666666666666666666666666666666666667 m",
        );
        test(
            "2/3m",
            "0.66666666666666666666666666666666666666666666666667 m",
        );

        test("123 N to (kg m)/s^2", "123 (kg m) / s^2");

        test(
            "1 km / 3000000 mm",
            "0.3333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333",
        );

        test("5kg * 1", "5 kg");
        test("5 kg * 1", "5 kg");
        test(" 5 kg  * 1", "5 kg");
        test("-5kg  * 1", "-5 kg");
        test("+5kg  * 1", "5 kg");
        test(".5kg  * 1", "0.5 kg");
        test("-5mg to kg", "-0.000005 kg");
        test("5.2mg * 1", "5.2 mg");

        test("981 cm/s^2 to m/s^2", "9.81 m / s^2");
        test("5exabytes to bytes", "5000000000000000000 bytes");
        test(
            "8.314 kg*(m^2 / (s^2 / (K^-1 / mol))) * 1",
            "8.314 (kg m^2) / (s^2 K mol)",
        );

        // TODO mindig a rövid formábanm kellene kiirni
        test("9.81 meters/second^2 * 1", "9.81 meter / second^2");
        test("10 decades to decade", "10 decade");
        test("10 centuries to century", "10 century");
        test("10 millennia to millennium", "10 millennium");

        test("(10 + 20)km", "30 km");
    }

    #[test]
    fn calc_exp_test() {
        // exp, binary and hex does not work with units
        // test("5e3kg", "5000 kg");
        // test("3 kg^1.0e0 * m^1.0e0 * s^-2e0", "3 (kg m) / s^2");

        test("2.3e-4 + 0", "0.00023");
        test(
            "1.23e50 + 0",
            "123000000000000000000000000000000000000000000000000",
        );
        test("3 e + 0", "3");
        test("3e + 0", "3");
        test("33e + 0", "33");
        test("3e3 + 0", "3000");

        // it interprets it as 3 - (-3)
        test("3e--3", "6");

        // invalid input tests
        test("2.3e4e5 + 0", "23000");
    }

    #[test]
    fn test_percentages() {
        test("200 km/h * 10%", "20 km / h");
        test("200 km/h * 0%", "0 km / h");
        test("200 km/h + 10%", "220 km / h");
        test("200 km/h - 10%", "180 km / h");
        test("200 km/h + 0%", "200 km / h");
        test("200 km/h - 0%", "200 km / h");

        test("0 + 10%", "0");
        test("200 - 10%", "180");
        test("200 - 0%", "200");
        test("0 - 10%", "0");
        test("200 * 10%", "20");
        test("200 * 0%", "0");
        test("10% * 200", "20");
        test("0% * 200", "0");
        test("(10 + 20)%", "30%");
    }

    #[test]
    fn test_longer_texts() {
        test("I traveled 13km at a rate / 40km/h to min", "19.5 min");
        test(
            "I traveled 24 miles and rode my bike  / 2 hours",
            "12 mile / hour",
        );
        test(
            "Now let's say you rode your bike at a rate of 10 miles/h for * 4 h to mile",
            "40 mile",
        );
        test(
            "Now let's say you rode your bike at a rate of 10 miles/h for * 4 h",
            "64373.76 m",
        );
        test(
            "transfer of around 1.587GB in about / 3 seconds",
            "0.529 GB / second",
        );
        test(
            " is a unit but should not be handled here so... 37.5MB*1 of DNA information in it.",
            "37.5 MB",
        );
    }

    #[test]
    fn test_result_heuristics() {
        // 2 numbers but no oepration, select none
        test("2.3e4.0e5", "23000");

        // ignore "15" and return with the last successful operation
        test("75-15 euróból kell adózni mert 15 EUR adómentes", "60");

        test("15 EUR adómentes azaz 75-15 euróból kell adózni", "60");
    }

    #[test]
    fn test_dont_count_zeroes() {
        test("1k * 1", "1000");
        test("2k * 1", "2000");
        test("3k - 2k", "1000");

        test("1k*1", "1000");
        test("2k*1", "2000");
        test("3k-2k", "1000");

        test("1M * 1", "1000000");
        test("2M * 1", "2000000");
        test("3M - 2M", "1000000");

        test("3M + 1k", "3001000");
        test("3M * 2k", "6000000000");
        // missing digit
        test("3M + k", "3000000");

        test("2kalap * 1", "2");
    }

    #[test]
    fn test_quant_vs_non_quant() {
        // test("12 km/h * 5 ", "60 km / h");
        // test("200kg alma + 300 kg banán ", "500 kg");
        // test("(1 alma + 4 körte) * 3 ember", "15");

        test("3000/50ml", "60 ml");
        test("3000/(50ml)", "60 ml^-1");
        test("1/(2km/h)", "0.5 h / km");
    }

    #[test]
    fn tests_for_invalid_input() {
        test("3", "3");
        test("3e-3-", "0.003");

        test_tokens(
            "[2, asda]",
            &[
                str("["),
                str("2"),
                str(","),
                str(" "),
                str("asda"),
                str("]"),
            ],
        );
        test("[2, asda]", " ");

        test(
            "2+3 - this minus sign is part of the text, should not affect the result",
            "5",
        );

        test_tokens(
            "1szer sem jött el + *megjegyzés 2 éve...",
            &[
                num(1),
                str("szer"),
                str(" "),
                str("sem"),
                str(" "),
                str("jött"),
                str(" "),
                str("el"),
                str(" "),
                str("+"),
                str(" "),
                str("*"),
                str("megjegyzés"),
                str(" "),
                str("2"),
                str(" "),
                str("éve..."),
            ],
        );
        test("1szer sem jött el + *megjegyzés 2 éve...", "1");
        //
        // // TODO these should be errors, because easily identifiable
        // // there is a typo in lbg, so the "in..." part is not evaulated
        // test("100 ft * lbf to (in*lbg)", " ");
        // test("100 ft * lbf * 1 to (in*lbg)", "100 ft lbf");
        // // wrong type
        // test("100 Hz to s", "0");

        test("12m/h * 45s ^^", "0.15 m");
        test("12km/h * 45s ^^", "150 m");
        test_tokens(
            "12km/h * 45s ^^",
            &[
                num(12),
                unit("km / h"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(45),
                unit("s"),
                str(" "),
                str("^"),
                str("^"),
            ],
        );

        // there are no empty vectors

        // matrix
        test_tokens("[]", &[str("["), str("]")]); // there are no empty vectors
        test_tokens(
            "1 + [2,]",
            &[
                num(1),
                str(" "),
                str("+"),
                str(" "),
                str("["),
                str("2"),
                str(","),
                str("]"),
            ],
        );
        test("1 + [2,]", "1");

        // multiply operator must be explicit, "5" is ignored here
        test("5(1+2)", "3");
    }

    #[test]
    fn calc_simplify_units() {
        // simplify from base to derived units if possible
        test("3 kg * m * 1 s^-2", "3 N");
    }

    #[test]
    fn test_invalid_input() {
        // missing multiplication sign
        // test("8.314 kg (m^2 / (s^2 / (K^-1 / mol)))", "");
        // TODO: egyelőre hagyjuk hibásan, meglátjuk ha a syntax hilight segit
        // e észrevenni a parser hibákat
    }

    #[test]
    fn test_calc_angles() {
        test("1 radian to rad", "1 rad");
        test(
            "1 deg to rad",
            "0.017453292519943295769236907684886127111111111111111 rad",
        );
    }

    #[test]
    fn test_cancelling_out() {
        test("40 m * 40 N / 40 J", "40");
        test("3 (s^-1) * 4 s", "12");
        test("(8.314 J / mol / K) ^ 0", "1");
        test("60 minute / 1 s", "3600");
        test("60 km/h*h/h/h * 1", "0.004629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629630740740740740740740740740740740740740740740740740740740740740740740740740740740740740740740740740740829629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629632 kg / s^2");
        // it is a very important test, if it gets converted wrongly
        // then 60 km/h is converted to m/s, which is 16.6666...7 m/s,
        // and it causes inaccuracies
        test("60km/h * 2h", "120000 m");
        test("60km/h * 2h to km", "120 km");
    }

    #[test]
    fn test_calc_matrix() {
        test("[2 * 1]", "[2]");
        test("[2 * 1, 3]", "[2, 3]");
        test("[2 * 1, 3, 4, 5, 6]", "[2, 3, 4, 5, 6]");

        test("[2+3]", "[5]");
        test("[2+3, 4 - 1, 5*2, 6/3, 2^4]", "[5, 3, 10, 2, 16]");

        test("[[2 * 1]]", "[[2]]");
        test("[[2 * 1, 3]]", "[[2, 3]]");
        test("[[2 * 1, 3], [4, 5]]", "[[2, 3], [4, 5]]");
    }

    #[test]
    fn test_eval_failure_changes_token_type() {
        test_tokens(
            "1 - not_variable",
            &[num(1), str(" "), str("-"), str(" "), str("not_variable")],
        );
    }

    #[test]
    fn test_matrix_wont_take_operands_from_outside_its_scope() {
        test("1 + [2, asda]", "1");
    }

    #[test]
    fn test_binary_ops() {
        test("0xFF AND 0b111", "7");

        test_tokens(
            "0xFF AND(0b11 OR 0b1111)",
            &[
                num(0xff),
                str(" "),
                op(OperatorTokenType::And),
                op(OperatorTokenType::ParenOpen),
                num(0b11),
                str(" "),
                op(OperatorTokenType::Or),
                str(" "),
                num(0b1111),
                op(OperatorTokenType::ParenClose),
            ],
        );

        test("0xFF AND(0b11 OR 0b1111)", "15");
    }

    #[test]
    fn test_unfinished_operators() {
        test_tokens(
            "0xFF AND 0b11 AND",
            &[
                num(0xff),
                str(" "),
                op(OperatorTokenType::And),
                str(" "),
                num(0b11),
                str(" "),
                str("AND"),
            ],
        );
    }

    #[test]
    fn test_binary() {
        ///// binary
        // Kibi BIT!
        test("1 Kib to bits", "1024 bits");
        test("1 Kib to bytes", "128 bytes");
        test("1 Kib/s to b/s", "1024 b / s");

        test("1kb to bytes", "125 bytes");
    }
}
