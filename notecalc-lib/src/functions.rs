use crate::calc::{add_op, CalcResult, CalcResultType};
use crate::token_parser::{Token, DECIMAL_E, DECIMAL_PI};
use crate::units::consts::UnitType;
use crate::units::units::{UnitOutput, Units};
use rust_decimal::prelude::*;
use std::ops::Neg;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(PartialEq, Eq, Clone, Copy, Debug, EnumIter)]
pub enum FnType {
    Nth,
    Sum,
    Transpose,
    Pi,
    E,
    Ceil,
    Ln,
    Lg,
    Log,
    Abs,
    Sin,
    Asin,
    Cos,
    Acos,
    Tan,
    Atan,
}

impl FnType {
    #[inline]
    pub fn value_of(ptr: &[char]) -> Option<FnType> {
        for fn_type in FnType::iter() {
            if ptr == fn_type.name() {
                return Some(fn_type);
            }
        }
        return None;
    }

    #[inline]
    pub fn name(&self) -> &'static [char] {
        match self {
            FnType::Abs => &['a', 'b', 's'],
            FnType::Sin => &['s', 'i', 'n'],
            FnType::Cos => &['c', 'o', 's'],
            FnType::Asin => &['a', 's', 'i', 'n'],
            FnType::Acos => &['a', 'c', 'o', 's'],
            FnType::Tan => &['t', 'a', 'n'],
            FnType::Atan => &['a', 't', 'a', 'n'],
            FnType::Nth => &['n', 't', 'h'],
            FnType::Sum => &['s', 'u', 'm'],
            FnType::Transpose => &['t', 'r', 'a', 'n', 's', 'p', 'o', 's', 'e'],
            FnType::Pi => &['p', 'i'],
            FnType::E => &['e'],
            FnType::Ceil => &['c', 'e', 'i', 'l'],
            FnType::Ln => &['l', 'n'],
            FnType::Lg => &['l', 'g'],
            FnType::Log => &['l', 'o', 'g'],
        }
    }

    #[inline]
    pub fn execute<'text_ptr>(
        &self,
        arg_count: usize,
        stack: &mut Vec<CalcResult>,
        fn_token_index: usize,
        tokens: &mut [Token<'text_ptr>],
        units: &Units,
    ) -> bool {
        match self {
            FnType::Abs => {
                fn_single_param_decimal(arg_count, stack, tokens, fn_token_index, Decimal::abs)
            }
            FnType::Nth => fn_nth(arg_count, stack, tokens, fn_token_index),
            FnType::Sum => fn_sum(arg_count, stack),
            FnType::Transpose => fn_transpose(arg_count, stack),
            FnType::Pi => fn_const(arg_count, stack, fn_token_index, tokens, DECIMAL_PI),
            FnType::E => fn_const(arg_count, stack, fn_token_index, tokens, DECIMAL_E),
            FnType::Sin => fn_f64_rad_to_num(arg_count, stack, tokens, fn_token_index, f64::sin),
            FnType::Asin => {
                fn_f64_num_to_rad(arg_count, stack, tokens, fn_token_index, f64::asin, units)
            }
            FnType::Cos => fn_f64_rad_to_num(arg_count, stack, tokens, fn_token_index, f64::cos),
            FnType::Acos => {
                fn_f64_num_to_rad(arg_count, stack, tokens, fn_token_index, f64::acos, units)
            }
            FnType::Tan => fn_f64_rad_to_num(arg_count, stack, tokens, fn_token_index, f64::tan),
            FnType::Atan => {
                fn_f64_num_to_rad(arg_count, stack, tokens, fn_token_index, f64::atan, units)
            }
            FnType::Ceil => {
                fn_single_param_decimal(arg_count, stack, tokens, fn_token_index, Decimal::ceil)
            }
            FnType::Ln => fn_single_param_f64(arg_count, stack, tokens, fn_token_index, f64::ln),
            FnType::Lg => fn_single_param_f64(arg_count, stack, tokens, fn_token_index, f64::log2),
            FnType::Log => fn_double_param_f64(arg_count, stack, tokens, fn_token_index, |a, b| {
                f64::log(b, a)
            }),
        }
    }
}

fn fn_const<'text_ptr>(
    arg_count: usize,
    stack: &mut Vec<CalcResult>,
    token_index: usize,
    tokens: &mut [Token<'text_ptr>],
    const_value: Decimal,
) -> bool {
    if arg_count != 0 {
        for i in 0..arg_count.min(stack.len()) {
            stack[stack.len() - 1 - i].set_token_error_flag(tokens);
        }
        return false;
    }

    stack.push(CalcResult::new(
        CalcResultType::Number(const_value),
        token_index,
    ));

    true
}

fn fn_single_param_f64<'text_ptr, F>(
    arg_count: usize,
    stack: &mut Vec<CalcResult>,
    tokens: &mut [Token<'text_ptr>],
    fn_token_index: usize,
    action: F,
) -> bool
where
    F: Fn(f64) -> f64,
{
    let arg_count = arg_count.min(stack.len());
    if arg_count == 0 {
        Token::set_token_error_flag_by_index(fn_token_index, tokens);
        false
    } else if arg_count > 1 {
        for i in 0..arg_count {
            stack[stack.len() - 1 - i].set_token_error_flag(tokens);
        }
        false
    } else {
        let param = &stack[stack.len() - 1];
        match &param.typ {
            CalcResultType::Number(num) => {
                if let Some(result) = num
                    .to_f64()
                    .map(|numf| action(numf))
                    .and_then(|ln_result| Decimal::from_f64(ln_result))
                {
                    let token_index = param.get_index_into_tokens();
                    stack.pop();
                    stack.push(CalcResult::new(CalcResultType::Number(result), token_index));
                    true
                } else {
                    param.set_token_error_flag(tokens);
                    false
                }
            }
            _ => {
                param.set_token_error_flag(tokens);
                false
            }
        }
    }
}

fn fn_double_param_f64<'text_ptr, F>(
    arg_count: usize,
    stack: &mut Vec<CalcResult>,
    tokens: &mut [Token<'text_ptr>],
    fn_token_index: usize,
    action: F,
) -> bool
where
    F: Fn(f64, f64) -> f64,
{
    let arg_count = arg_count.min(stack.len());
    if arg_count < 2 {
        Token::set_token_error_flag_by_index(fn_token_index, tokens);
        false
    } else if arg_count > 2 {
        for i in 2..arg_count {
            stack[stack.len() - 1 - i].set_token_error_flag(tokens);
        }
        false
    } else {
        let first_param = &stack[stack.len() - 2];
        let second_param = &stack[stack.len() - 1];
        match (&first_param.typ, &second_param.typ) {
            (CalcResultType::Number(p1_dec), CalcResultType::Number(p2_dec)) => {
                let p1_f64 = p1_dec.to_f64();
                let p2_f64 = p2_dec.to_f64();
                if p1_f64.is_some() && p2_f64.is_some() {
                    if let Some(result) =
                        Decimal::from_f64(action(p1_f64.unwrap(), p2_f64.unwrap()))
                    {
                        let token_index = first_param.get_index_into_tokens();
                        stack.pop();
                        stack.pop();
                        stack.push(CalcResult::new(CalcResultType::Number(result), token_index));
                        true
                    } else {
                        Token::set_token_error_flag_by_index(fn_token_index, tokens);
                        false
                    }
                } else {
                    if p1_f64.is_none() {
                        stack[first_param.get_index_into_tokens()].set_token_error_flag(tokens);
                    }
                    if p2_f64.is_none() {
                        stack[second_param.get_index_into_tokens()].set_token_error_flag(tokens);
                    }
                    false
                }
            }
            _ => {
                stack[first_param.get_index_into_tokens()].set_token_error_flag(tokens);
                stack[second_param.get_index_into_tokens()].set_token_error_flag(tokens);
                false
            }
        }
    }
}

fn fn_f64_rad_to_num<'text_ptr, F>(
    arg_count: usize,
    stack: &mut Vec<CalcResult>,
    tokens: &mut [Token<'text_ptr>],
    fn_token_index: usize,
    action: F,
) -> bool
where
    F: Fn(f64) -> f64,
{
    let arg_count = arg_count.min(stack.len());
    if arg_count < 1 {
        Token::set_token_error_flag_by_index(fn_token_index, tokens);
        false
    } else if arg_count > 1 {
        for i in 1..arg_count {
            stack[stack.len() - 1 - i].set_token_error_flag(tokens);
        }
        false
    } else {
        let param = &stack[stack.len() - 1];
        match &param.typ {
            CalcResultType::Quantity(num, unit) if unit.is(UnitType::Angle) => {
                let rad = num; // the base unit is rad, so num is already in radian
                if let Some(result) = rad
                    .to_f64()
                    .map(|it| action(it))
                    .and_then(|it| Decimal::from_f64(it))
                {
                    let token_index = param.get_index_into_tokens();
                    stack.pop();
                    stack.push(CalcResult::new(CalcResultType::Number(result), token_index));
                    true
                } else {
                    param.set_token_error_flag(tokens);
                    false
                }
            }
            _ => {
                dbg!(param);
                dbg!(param.get_index_into_tokens());
                param.set_token_error_flag(tokens);
                false
            }
        }
    }
}

fn fn_f64_num_to_rad<'text_ptr, F>(
    arg_count: usize,
    stack: &mut Vec<CalcResult>,
    tokens: &mut [Token<'text_ptr>],
    fn_token_index: usize,
    action: F,
    units: &Units,
) -> bool
where
    F: Fn(f64) -> f64,
{
    if arg_count < 1 {
        Token::set_token_error_flag_by_index(fn_token_index, tokens);
        false
    } else if arg_count > 1 {
        for i in 1..arg_count {
            stack[stack.len() - 1 - i].set_token_error_flag(tokens);
        }
        false
    } else {
        let param = &stack[stack.len() - 1];
        match &param.typ {
            CalcResultType::Number(num) => {
                if num > &Decimal::one() || num < &Decimal::one().neg() {
                    param.set_token_error_flag(tokens);
                    return false;
                }
                if let Some(result) = num
                    .to_f64()
                    .map(|it| action(it))
                    .and_then(|it| Decimal::from_f64(it))
                {
                    let token_index = param.get_index_into_tokens();
                    stack.pop();
                    stack.push(CalcResult::new(
                        CalcResultType::Quantity(result, UnitOutput::new_rad(units)),
                        token_index,
                    ));
                    true
                } else {
                    param.set_token_error_flag(tokens);
                    false
                }
            }
            _ => {
                param.set_token_error_flag(tokens);
                false
            }
        }
    }
}

fn fn_single_param_decimal<'text_ptr, F>(
    arg_count: usize,
    stack: &mut Vec<CalcResult>,
    tokens: &mut [Token<'text_ptr>],
    fn_token_index: usize,
    action: F,
) -> bool
where
    F: Fn(&Decimal) -> Decimal,
{
    if arg_count != 1 || stack.len() < 1 {
        Token::set_token_error_flag_by_index(fn_token_index, tokens);
        false
    } else {
        let param = &stack[stack.len() - 1];
        match &param.typ {
            CalcResultType::Number(num) => {
                let result = action(num);
                let token_index = param.get_index_into_tokens();
                stack.pop();
                stack.push(CalcResult::new(CalcResultType::Number(result), token_index));
                true
            }
            _ => {
                param.set_token_error_flag(tokens);
                false
            }
        }
    }
}

fn fn_nth<'text_ptr>(
    arg_count: usize,
    stack: &mut Vec<CalcResult>,
    tokens: &mut [Token<'text_ptr>],
    fn_token_index: usize,
) -> bool {
    if arg_count < 2 || stack.len() < 2 {
        Token::set_token_error_flag_by_index(fn_token_index, tokens);
        false
    } else {
        let index_token = &stack[stack.len() - 1];
        let mat_token = &stack[stack.len() - 2];
        match (&index_token.typ, &mat_token.typ) {
            (CalcResultType::Number(n), CalcResultType::Matrix(mat)) => {
                if let Some(index) = n.to_u32() {
                    if mat.col_count < (index + 1) as usize {
                        index_token.set_token_error_flag(tokens);
                        false
                    } else {
                        let result = mat.cell(0, index as usize).clone();
                        stack.truncate(stack.len() - 2);
                        stack.push(result);
                        true
                    }
                } else {
                    index_token.set_token_error_flag(tokens);
                    false
                }
            }
            (CalcResultType::Number(_), _) => {
                mat_token.set_token_error_flag(tokens);
                false
            }
            (_, CalcResultType::Matrix(_)) => {
                index_token.set_token_error_flag(tokens);
                false
            }
            _ => {
                index_token.set_token_error_flag(tokens);
                mat_token.set_token_error_flag(tokens);
                false
            }
        }
    }
}

fn fn_sum(arg_count: usize, stack: &mut Vec<CalcResult>) -> bool {
    if arg_count < 1 {
        false
    } else {
        let param = &stack[stack.len() - 1];
        match &param.typ {
            CalcResultType::Matrix(mat) => {
                let mut sum = mat.cells[0].clone();
                for cell in mat.cells.iter().skip(1) {
                    if let Some(result) = add_op(&sum, cell) {
                        sum = result;
                    } else {
                        return false;
                    }
                }
                stack.truncate(stack.len() - 1);
                stack.push(sum);
                true
            }
            _ => false,
        }
    }
}

fn fn_transpose(arg_count: usize, stack: &mut Vec<CalcResult>) -> bool {
    if arg_count < 1 {
        false
    } else {
        let param = &stack[stack.len() - 1];
        let index_into_tokens = param.get_index_into_tokens();
        if let Some(transposed) = match &param.typ {
            CalcResultType::Matrix(mat) => {
                let t = CalcResultType::Matrix(mat.transposed());
                Some(t)
            }
            _ => None,
        } {
            stack.truncate(stack.len() - 1);
            stack.push(CalcResult::new(transposed, index_into_tokens));
            true
        } else {
            false
        }
    }
}
