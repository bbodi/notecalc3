use bigdecimal::*;
use smallvec::SmallVec;
use std::fmt::Write;
use std::ops::BitXor;
use std::ops::Neg;
use std::ops::Not;
use std::str::FromStr;

use crate::matrix::MatrixData;
use crate::token_parser::TokenType::StringLiteral;
use crate::token_parser::{OperatorTokenType, Token, TokenType};
use crate::units::consts::EMPTY_UNIT_DIMENSIONS;
use crate::units::units::{UnitOutput, Units};
use std::collections::HashMap;
use std::io::BufWriter;

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
pub enum CalcResult {
    Number(BigDecimal),
    Percentage(BigDecimal),
    Quantity(BigDecimal, UnitOutput),
    Matrix(MatrixData),
}

impl CalcResult {
    /// creates a cheap CalcResult without memory allocation. Use it only as a temporary value.
    pub fn hack_empty() -> CalcResult {
        CalcResult::Matrix(MatrixData {
            cells: Vec::new(),
            row_count: 0,
            col_count: 0,
        })
    }

    pub fn zero() -> CalcResult {
        CalcResult::Number(BigDecimal::zero())
    }
}

pub struct EvaluationResult {
    pub there_was_unit_conversion: bool,
    pub there_was_operation: bool,
    pub assignment: bool,
    pub result: CalcResult,
}

pub fn evaluate_tokens<'text_ptr>(
    tokens: &mut Vec<TokenType>,
    variables: &[(&'text_ptr [char], CalcResult)],
) -> Result<Option<EvaluationResult>, ()> {
    let mut stack = vec![];
    let mut there_was_unit_conversion = false;
    let mut assignment = false;
    let mut last_success_operation_result_index = None;
    for token in tokens.iter_mut() {
        match &token {
            TokenType::NumberLiteral(num) => stack.push(CalcResult::Number(num.clone())),
            TokenType::Operator(typ) => {
                if *typ == OperatorTokenType::Assign {
                    assignment = true;
                    continue;
                }
                if apply_operation(&mut stack, &typ) == true {
                    if matches!(typ, OperatorTokenType::UnitConverter) {
                        there_was_unit_conversion = true;
                    }
                    if !stack.is_empty() {
                        last_success_operation_result_index = Some(stack.len() - 1);
                    }
                } else {
                    return Err(());
                }
            }
            TokenType::StringLiteral => panic!(),
            TokenType::Variable { var_index } => {
                // TODO clone :(
                stack.push(variables[*var_index].1.clone());
            }
            TokenType::LineReference { var_index } => {
                // TODO clone :(
                stack.push(variables[*var_index].1.clone());
            }
        }
    }
    return match last_success_operation_result_index {
        Some(last_success_operation_index) => {
            // TODO: after shunting yard validation logic, do we need it?
            // e.g. "1+2 some text 3"
            // in this case prefer the result of 1+2 and ignore the number 3
            Ok(Some(EvaluationResult {
                there_was_unit_conversion,
                there_was_operation: last_success_operation_result_index.is_some(),
                assignment,
                result: stack[last_success_operation_index].clone(),
            }))
        }
        None => Ok(stack.pop().map(|it| EvaluationResult {
            there_was_operation: last_success_operation_result_index.is_some(),
            there_was_unit_conversion,
            assignment,
            result: it,
        })),
    };
}

fn apply_operation(stack: &mut Vec<CalcResult>, op: &OperatorTokenType) -> bool {
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
                if let Some(result) = binary_operation(op, lhs, rhs) {
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
        OperatorTokenType::Matrix {
            row_count,
            col_count,
        } => {
            let arg_count = row_count * col_count;
            if stack.len() >= arg_count {
                let matrix_args = stack.drain(stack.len() - arg_count..).collect::<Vec<_>>();
                stack.push(CalcResult::Matrix(MatrixData::new(
                    matrix_args,
                    *row_count,
                    *col_count,
                )));
                true
            } else {
                false
            }
        }
        OperatorTokenType::Fn { arg_count, typ } => typ.execute(*arg_count, stack),
        OperatorTokenType::Semicolon | OperatorTokenType::Comma => {
            // ignore
            true
        }
        OperatorTokenType::Assign => panic!("handled in the main loop above"),
        OperatorTokenType::ParenOpen
        | OperatorTokenType::ParenClose
        | OperatorTokenType::BracketOpen
        | OperatorTokenType::BracketClose => panic!(),
    };
    return succeed;
}

fn unary_operation(op: &OperatorTokenType, top: &CalcResult) -> Option<CalcResult> {
    return match &op {
        OperatorTokenType::UnaryPlus => Some(top.clone()),
        OperatorTokenType::UnaryMinus => unary_minus_op(top),
        OperatorTokenType::Perc => percentage_operator(top),
        OperatorTokenType::Not => binary_complement(top),
        OperatorTokenType::Unit(target_unit) => match top {
            CalcResult::Number(num) => {
                let norm = target_unit.normalize(num);
                if target_unit.dimensions == EMPTY_UNIT_DIMENSIONS {
                    // the units cancelled each other, e.g. km/m
                    Some(CalcResult::Number(norm))
                } else {
                    Some(CalcResult::Quantity(norm, target_unit.clone()))
                }
            }
            _ => None,
        },
        _ => None,
    };
}

fn binary_operation(
    op: &OperatorTokenType,
    lhs: &CalcResult,
    rhs: &CalcResult,
) -> Option<CalcResult> {
    let result = match &op {
        OperatorTokenType::Mult => multiply_op(lhs, rhs),
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
                (CalcResult::Matrix(mat), CalcResult::Quantity(_, target_unit)) => {
                    let cells: Option<Vec<CalcResult>> = mat
                        .cells
                        .iter()
                        .map(|cell| binary_operation(op, cell, rhs))
                        .collect();
                    cells.map(|it| {
                        CalcResult::Matrix(MatrixData::new(it, mat.row_count, mat.col_count))
                    })
                }
                _ => None,
            };
        }
        // todo: ronda h nem a tipusokkal kezelem le hanem panickal a többit
        // , csinálj egy TokenType::BinaryOp::Add
        _ => panic!(),
    };
    result
}

fn percentage_operator(lhs: &CalcResult) -> Option<CalcResult> {
    match lhs {
        CalcResult::Number(lhs) => {
            // 5%
            Some(CalcResult::Percentage(lhs.clone()))
        }
        _ => None,
    }
}

fn binary_complement(lhs: &CalcResult) -> Option<CalcResult> {
    match lhs {
        CalcResult::Number(lhs) => {
            // 0b01 and 0b10
            let lhs = lhs.to_i64()?;
            Some(CalcResult::Number(dec(lhs.not())))
        }
        _ => None,
    }
}

fn binary_xor_op(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
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

fn binary_or_op(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
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

fn binary_shift_right(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
    match (lhs, rhs) {
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            let lhs = lhs.to_i64()?;
            let rhs = rhs.to_u32()?;
            Some(CalcResult::Number(dec(lhs.wrapping_shr(rhs))))
        }
        _ => None,
    }
}

fn binary_shift_left(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
    match (lhs, rhs) {
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            let lhs = lhs.to_i64()?;
            let rhs = rhs.to_u32()?;
            Some(CalcResult::Number(dec(lhs.wrapping_shl(rhs))))
        }
        _ => None,
    }
}

fn binary_and_op(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
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

fn unary_minus_op(lhs: &CalcResult) -> Option<CalcResult> {
    match lhs {
        CalcResult::Number(lhs) => {
            // -12
            Some(CalcResult::Number(lhs.neg()))
        }
        CalcResult::Quantity(lhs, unit) => {
            // -12km
            Some(CalcResult::Quantity(lhs.neg(), unit.clone()))
        }
        CalcResult::Percentage(lhs) => {
            // -50%
            Some(CalcResult::Percentage(lhs.neg()))
        }
        _ => None, // CalcResult::Matrix(mat) => CalcResult::Matrix(mat.neg()),
    }
}

fn pow_op(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
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
        }
        _ => None,
    }
}

pub fn multiply_op(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
    let result = match (lhs, rhs) {
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
        (CalcResult::Number(scalar), CalcResult::Matrix(mat)) => mat.mult_scalar(lhs),
        //////////////
        // 12km * x
        //////////////
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Number(rhs)) => {
            // 2m * 5
            Some(CalcResult::Quantity(lhs * rhs, lhs_unit.clone()))
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 2s * 3s
            let num = lhs * rhs;
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
        (CalcResult::Quantity(..), CalcResult::Matrix(mat)) => mat.mult_scalar(lhs),
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
        (CalcResult::Percentage(..), CalcResult::Matrix(..)) => None,
        //////////////
        // Matrix
        //////////////
        (CalcResult::Matrix(mat), CalcResult::Number(..))
        | (CalcResult::Matrix(mat), CalcResult::Quantity(..))
        | (CalcResult::Matrix(mat), CalcResult::Percentage(..)) => mat.mult_scalar(rhs),
        (CalcResult::Matrix(a), CalcResult::Matrix(b)) => {
            if a.col_count != b.row_count {
                return None;
            }
            let mut result = Vec::with_capacity(a.row_count * b.col_count);
            for row in 0..a.row_count {
                for col in 0..b.col_count {
                    let mut sum = if let Some(r) = multiply_op(a.cell(row, 0), b.cell(0, col)) {
                        r
                    } else {
                        return None;
                    };
                    for i in 1..a.col_count {
                        if let Some(r) = multiply_op(a.cell(row, i), b.cell(i, col)) {
                            if let Some(s) = add_op(&sum, &r) {
                                sum = s;
                            } else {
                                return None;
                            }
                        }
                    }
                    result.push(sum);
                }
            }
            Some(CalcResult::Matrix(MatrixData::new(
                result,
                a.row_count,
                b.col_count,
            )))
        }
    };
    return match result {
        Some(CalcResult::Quantity(num, unit)) if unit.is_unitless() => {
            // some operation cancelled out its units, put a simple number on the stack
            Some(CalcResult::Number(num))
        }
        _ => result,
    };
}

pub fn add_op(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
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
        (CalcResult::Number(lhs), CalcResult::Matrix(..)) => None,
        //////////////
        // 12km + x
        //////////////
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Number(rhs)) => {
            // 2m + 5
            None
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 2s + 3s
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
        (CalcResult::Quantity(..), CalcResult::Matrix(..)) => None,
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
        (CalcResult::Percentage(..), CalcResult::Matrix(..)) => None,
        ///////////
        // Matrix
        //////////
        (CalcResult::Matrix(..), CalcResult::Number(..)) => None,
        (CalcResult::Matrix(..), CalcResult::Quantity(..)) => None,
        (CalcResult::Matrix(..), CalcResult::Percentage(..)) => None,
        (CalcResult::Matrix(lhs), CalcResult::Matrix(rhs)) => {
            if lhs.row_count != rhs.row_count || lhs.col_count != rhs.col_count {
                return None;
            }
            let cells: Option<Vec<CalcResult>> = lhs
                .cells
                .iter()
                .zip(rhs.cells.iter())
                .map(|(a, b)| add_op(a, b))
                .collect();
            cells.map(|it| CalcResult::Matrix(MatrixData::new(it, lhs.row_count, lhs.col_count)))
        }
    }
}

fn sub_op(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
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
        (CalcResult::Number(..), CalcResult::Matrix(..)) => None,
        //////////////
        // 12km - x
        //////////////
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Number(rhs)) => {
            // 2m - 5
            None
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 2s - 3s
            if lhs_unit != rhs_unit {
                None
            } else {
                Some(CalcResult::Quantity(lhs - rhs, lhs_unit.clone()))
            }
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Percentage(rhs)) => {
            // e.g. 2m - 50%
            let x_percent_of_left_hand_side = lhs / dec(100) * rhs;
            Some(CalcResult::Quantity(
                lhs - x_percent_of_left_hand_side,
                lhs_unit.clone(),
            ))
        }
        (CalcResult::Quantity(..), CalcResult::Matrix(..)) => None,
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
        (CalcResult::Percentage(..), CalcResult::Matrix(..)) => None,
        ///////////
        // Matrix
        //////////
        (CalcResult::Matrix(..), CalcResult::Number(..)) => None,
        (CalcResult::Matrix(..), CalcResult::Quantity(..)) => None,
        (CalcResult::Matrix(..), CalcResult::Percentage(..)) => None,
        (CalcResult::Matrix(lhs), CalcResult::Matrix(rhs)) => {
            if lhs.row_count != rhs.row_count || lhs.col_count != rhs.col_count {
                return None;
            }
            let cells: Option<Vec<CalcResult>> = lhs
                .cells
                .iter()
                .zip(rhs.cells.iter())
                .map(|(a, b)| sub_op(a, b))
                .collect();
            cells.map(|it| CalcResult::Matrix(MatrixData::new(it, lhs.row_count, lhs.col_count)))
        }
    }
}

pub fn divide_op(lhs: &CalcResult, rhs: &CalcResult) -> Option<CalcResult> {
    let result = match (lhs, rhs) {
        //////////////
        // 12 / x
        //////////////
        (CalcResult::Number(lhs), CalcResult::Number(rhs)) => {
            // 100 / 2
            if rhs.is_zero() {
                None
            } else {
                Some(CalcResult::Number(lhs / rhs))
            }
        }
        (CalcResult::Number(lhs), CalcResult::Quantity(rhs, unit)) => {
            // 100 / 2km => 100 / (2 km)
            let mut new_unit = unit.pow(-1);

            let denormalized_num = unit.denormalize(rhs);
            if denormalized_num.is_zero() {
                return None;
            }
            let num_part = new_unit.normalize(&(lhs / &denormalized_num));
            Some(CalcResult::Quantity(num_part, new_unit.clone()))
        }
        (CalcResult::Number(lhs), CalcResult::Percentage(rhs)) => {
            if rhs.is_zero() {
                return None;
            }
            // 100 / 50%
            Some(CalcResult::Percentage(lhs / rhs * dec(100)))
        }
        (CalcResult::Number(..), CalcResult::Matrix(..)) => None,
        //////////////
        // 12km / x
        //////////////
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Number(rhs)) => {
            // 2m / 5
            if rhs.is_zero() {
                return None;
            }
            Some(CalcResult::Quantity(lhs / rhs, lhs_unit.clone()))
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Quantity(rhs, rhs_unit)) => {
            // 12 km / 3s
            if rhs.is_zero() {
                return None;
            }
            Some(CalcResult::Quantity(lhs / rhs, lhs_unit / rhs_unit))
        }
        (CalcResult::Quantity(lhs, lhs_unit), CalcResult::Percentage(rhs)) => {
            // 2m / 50%
            None
        }
        (CalcResult::Quantity(..), CalcResult::Matrix(..)) => None,
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
        (CalcResult::Percentage(..), CalcResult::Matrix(..)) => None,
        (CalcResult::Matrix(mat), CalcResult::Number(..))
        | (CalcResult::Matrix(mat), CalcResult::Quantity(..))
        | (CalcResult::Matrix(mat), CalcResult::Percentage(..)) => mat.div_scalar(rhs),
        (CalcResult::Matrix(mat), CalcResult::Matrix(..)) => None,
        _ => None,
    };
    return match result {
        Some(CalcResult::Quantity(num, unit)) if unit.is_unitless() => {
            // some operation cancelled out its units, put a simple number on the stack
            Some(CalcResult::Number(num))
        }
        _ => result,
    };
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
    use crate::units::consts::init_units;
    use crate::ResultFormat;

    use super::*;
    use crate::renderer::render_result;

    static mut DECIMAL_COUNT: usize = 4;

    fn test_tokens(text: &str, expected_tokens: &[Token]) {
        println!("===================================================");
        println!("{}", text);
        let mut units = Units::new();
        let temp = text.chars().collect::<Vec<char>>();
        let mut tokens = vec![];
        let vars = Vec::new();
        let mut shunting_output =
            crate::shunting_yard::tests::do_shunting_yard(&temp, &units, &mut tokens, &vars);
        let mut result_stack = evaluate_tokens(&mut shunting_output, &vars);

        crate::shunting_yard::tests::compare_tokens(expected_tokens, &tokens);
    }

    fn test_vars(vars: &Vec<(&'static [char], CalcResult)>, text: &str, expected: &'static str) {
        dbg!("===========================================================");
        dbg!(text);
        let temp = text.chars().collect::<Vec<char>>();

        let mut units = Units::new();

        let mut tokens = vec![];
        let mut shunting_output =
            crate::shunting_yard::tests::do_shunting_yard(&temp, &units, &mut tokens, vars);

        let result = evaluate_tokens(&mut shunting_output, vars);

        if let Err(..) = &result {
            assert_eq!("Err", expected);
        } else if let Ok(Some(EvaluationResult {
            there_was_unit_conversion,
            there_was_operation,
            assignment: _assignment,
            result: CalcResult::Quantity(num, unit),
        })) = &result
        {
            assert_eq!(
                expected,
                render_result(
                    &units,
                    &result.as_ref().unwrap().as_ref().unwrap().result,
                    &ResultFormat::Dec,
                    *there_was_unit_conversion,
                    unsafe { DECIMAL_COUNT }
                )
            );
        } else if let Ok(..) = &result {
            assert_eq!(
                result
                    .unwrap()
                    .map(|it| render_result(
                        &units,
                        &it.result,
                        &ResultFormat::Dec,
                        false,
                        unsafe { DECIMAL_COUNT }
                    ))
                    .unwrap_or(" ".to_string()),
                expected,
            );
        }
    }

    fn test(text: &str, expected: &'static str) {
        test_vars(&Vec::new(), text, expected);
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
            "0.66666666666666666666666666666666666666666666666667 m^-1",
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

        test("30/200%", "15%");
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

        test("3000/50ml", "60 ml^-1");
        test("(3000/50)ml", "60 ml");
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

        // invalid
        test("[[2 * 1]]", "[2]");
        test("[[2 * 3, 4]]", "[6, 4]");
        test("[[2 * 1, 3], [4, 5]]", "[4, 5]");
    }

    #[test]
    fn calc_simplify_units() {
        // simplify from base to derived units if possible
        test("3 kg * m * 1 s^-2", "3 N");

        test(
            "128PiB / 30Mb/s",
            "38430716586.666666666666666666666666666666666666667 s",
        );
    }

    #[test]
    fn unit_calcs() {
        test("50km + 50mm", "50.00005 km");
        test("50km - 50mm", "49.99995 km");
        test("5kg * 5g", "0.025 kg^2");
        test("5km * 5mm", "25 m^2");
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
        test("60 km/h*h/h/h * 1", "0.004629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629630740740740740740740740740740740740740740740740740740740740740740740740740740740740740740740740740740829629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629629632 m / s^2");
        // it is a very important test, if it gets converted wrongly
        // then 60 km/h is converted to m/s, which is 16.6666...7 m/s,
        // and it causes inaccuracies
        test("60km/h * 2h", "120000 m");
        test("60km/h * 2h to km", "120 km");
        test("1s * 2s^-1", "2");
        test("2s * 3(s^-1)", "6");
        test("2s * 3(1/s)", "6");
    }

    #[test]
    fn test_calc_inside_matrix() {
        test("[2 * 1]", "[2]");
        test("[2 * 1, 3]", "[2, 3]");
        test("[2 * 1, 3, 4, 5, 6]", "[2, 3, 4, 5, 6]");

        test("[2+3]", "[5]");
        test("[2+3, 4 - 1, 5*2, 6/3, 2^4]", "[5, 3, 10, 2, 16]");

        test("[2 * 1]", "[2]");
        test("[2 * 3; 4]", "[6; 4]");
        test("[2 * 1, 3; 4, 5]", "[2, 3; 4, 5]");
    }

    #[test]
    fn test_matrix_addition() {
        test("[2] + [3]", "[5]");
        test("[2, 3] + [4, 5]", "[6, 8]");
        test("[2, 3, 4] + [5, 6, 7]", "[7, 9, 11]");
        test("[2; 3] + [4; 5]", "[6; 8]");
        test(
            "[2, 3, 4; 5, 6, 7] + [8, 9, 10; 11, 12, 13]",
            "[10, 12, 14; 16, 18, 20]",
        );

        test("2 km + [3]", "Err");
        test("[2 km] + [3]", "Err");
    }

    #[test]
    fn test_matrix_sub() {
        test("[2] - [3]", "[-1]");
        test("[2, 3] - [4, 5]", "[-2, -2]");
        test("[2, 3, 4] - [5, 6, 7]", "[-3, -3, -3]");
        test("[4; 5] - [2; 3]", "[2; 2]");

        test("[2 km] - [3]", "Err");
    }

    #[test]
    fn test_matrix_scalar_mult() {
        test("3 * [2]", "[6]");
        test("[2] * 6", "[12]");

        test("2 * [2, 3]", "[4, 6]");
        test("2 * [2, 3, 4]", "[4, 6, 8]");
        test("2 * [2; 3]", "[4; 6]");
        test("2 * [2, 3; 4, 5]", "[4, 6; 8, 10]");
        test("[2, 3; 4, 5] * 2", "[4, 6; 8, 10]");

        test("2km * [2]", "[4 km]");
    }

    #[test]
    fn div_by_zero() {
        test("1 / 0", "Err");
        test("1kg / 0", "Err");
        test("1m / 0s", "Err");
        test("1% / 0", "Err");
        test("10 / 0%", "Err");
    }

    #[test]
    fn test_matrix_scalar_div() {
        test("3 / [2]", "Err");
        test("[6] / 2", "[3]");

        test("[6, 10] / 2", "[3, 5]");
        test("[2, 3, 4] / 2", "[1, 1.5, 2]");
        test("[2; 3] / 2", "[1; 1.5]");
        test("[2, 3; 4, 5] / 2", "[1, 1.5; 2, 2.5]");

        test("[100g] / 2g", "[50]");
    }

    #[test]
    fn test_matrix_matrix_mult() {
        test("[3] * [2]", "[6]");
        test("[2;3] * [4, 5]", "[8, 10; 12, 15]");

        test(
            "[1,2,3,4; 5,6,7,8; 9,10,11,12; 13,14,15,16] * [30;40;50;60]",
            "[500; 1220; 1940; 2660]",
        );

        test(
            "[2,3,4,5] * [2,3,4,5; 6,7,8,9; 10,11,12,13; 14,15,16,17]",
            "[132, 146, 160, 174]",
        );
        test("[3m] * [2cm]", "[0.06 m^2]");

        test("[2,3] * [4]", "Err");
    }

    #[test]
    fn matrix_unit() {
        test("[2cm,3mm; 4m,5km] to m", "[0.02 m, 0.003 m; 4 m, 5000 m]");
    }

    #[test]
    fn kcal_unit() {
        test("1 cal to J", "4.1868 J");
        test("3kcal to J", "12560.4 J");
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

    #[test]
    fn test_variables() {
        let mut vars = Vec::new();
        vars.push((
            &['v', 'a', 'r'][..],
            CalcResult::Number(BigDecimal::from_str("12").unwrap()),
        ));
        test_vars(&vars, "var * 2", "24");
        test_vars(&vars, "var - var", "0");
    }

    #[test]
    fn test_unit_cancelling() {
        test("1 km / 50m", "20");

        test_tokens("1 km/m", &[num(1), str(" "), unit("km / m")]);
        test("1 km/m", "1000");
        test("1 m/km", "0.001");
        test("140k h/ month", "191.6495550992470910335386721423682409308692676249144421629021218343600273785078713210130047912388774992");

        test("1 m*km", "1000 m^2");
    }

    #[test]
    fn test_unit_money() {
        test_tokens("10 $/month", &[num(10), str(" "), unit("$ / month")]);
        test("1 $/month", "1 $ / month");
        test("140k $ / month * 3 years", "5040000 $");
    }

    #[test]
    fn test_func_nth() {
        test("nth([5, 6, 7], 0)", "5");
        test("nth([5, 6, 7], 1)", "6");
        test("nth([5, 6, 7], 2)", "7");
    }

    #[test]
    fn test_func_sum() {
        test("sum([5, 6, 7])", "18");
    }

    #[test]
    fn test_func_transpose() {
        test("transpose([5, 6, 7])", "[5; 6; 7]");
        test("transpose([1, 2; 3, 4])", "[1, 3; 2, 4]");
        test("transpose([1, 2; 3, 4; 5, 6])", "[1, 3, 5; 2, 4, 6]");
    }

    #[test]
    fn test_func_pi() {
        test("pi()", "3.1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679821480865132823066470938446095505822317253594081284811174502841027019385211055596446229489549303819644288109756659334461284756482337867831652712019091456485669234603486104543266482133936072602491412737245870066063155881748815209209628292540917153643678925903600113305305488204665213841469519415116094330572703657595919530921861173819326117931051185480744623799627495673518857527248912279381830119491298336733624406566430860213949463952247371907021798609437027705392171762931767523846748184676694051320005681271452635608277857713427577896091736371787214684409012249534301465495853710507922796892589235420199561121290219608640344181598136297747713099605187072113499999983729780499510597317328160963185950244594553469083026425223082533446850352619311881710100031378387528865875332083814206171776691473035982534904287554687311595628638823537875937519577818577805321712268066130019278766111959092164201989");
        test("pi(1)", "Err");
    }

    #[test]
    fn test_single_brackets() {
        test("[", " ");
        test("]", " ");
        test("(", " ");
        test(")", " ");
        test("=", " ");
    }
}
