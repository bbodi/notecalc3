use crate::calc::{add_op, CalcResult, CalcResultType, EvalErr};
use crate::token_parser::{DECIMAL_E, DECIMAL_PI};
use crate::units::consts::UnitType;
use crate::units::units::{UnitOutput, Units};
use rust_decimal::prelude::*;
use std::ops::Neg;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(PartialEq, Eq, Clone, Copy, Debug, EnumIter)]
pub enum FnType {
    UserDefined(usize),
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
            FnType::UserDefined(_) => &[],
        }
    }

    #[inline]
    pub fn execute<'text_ptr>(
        &self,
        arg_count: usize,
        stack: &mut Vec<CalcResult>,
        fn_token_index: usize,
        units: &Units,
    ) -> Result<(), EvalErr> {
        match self {
            FnType::Abs => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_single_param_decimal(stack, Decimal::abs)
            }),
            FnType::Nth => arg_count_limited_fn(2, arg_count, stack, fn_token_index, fn_nth),
            FnType::Sum => arg_count_limited_fn(1, arg_count, stack, fn_token_index, fn_sum),
            FnType::Transpose => {
                arg_count_limited_fn(1, arg_count, stack, fn_token_index, fn_transpose)
            }
            FnType::Pi => arg_count_limited_fn(0, arg_count, stack, fn_token_index, |stack| {
                fn_const(stack, fn_token_index, DECIMAL_PI)
            }),
            FnType::E => arg_count_limited_fn(0, arg_count, stack, fn_token_index, |stack| {
                fn_const(stack, fn_token_index, DECIMAL_E)
            }),
            FnType::Sin => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_f64_rad_to_num(stack, units, f64::sin)
            }),
            FnType::Asin => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_f64_num_to_rad(stack, f64::asin, units)
            }),
            FnType::Cos => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_f64_rad_to_num(stack, units, f64::cos)
            }),
            FnType::Acos => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_f64_num_to_rad(stack, f64::acos, units)
            }),
            FnType::Tan => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_f64_rad_to_num(stack, units, f64::tan)
            }),
            FnType::Atan => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_f64_num_to_rad(stack, f64::atan, units)
            }),
            FnType::Ceil => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_single_param_decimal(stack, Decimal::ceil)
            }),

            FnType::Ln => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_single_param_f64(stack, f64::ln)
            }),
            FnType::Lg => arg_count_limited_fn(1, arg_count, stack, fn_token_index, |stack| {
                fn_single_param_f64(stack, f64::log2)
            }),
            FnType::Log => arg_count_limited_fn(2, arg_count, stack, fn_token_index, |stack| {
                fn_double_param_f64(stack, fn_token_index, |a, b| f64::log(b, a))
            }),
            FnType::UserDefined(_i) => {
                panic!("User fn is handled manually")
            }
        }
    }
}

fn fn_const<'text_ptr>(
    stack: &mut Vec<CalcResult>,
    token_index: usize,
    const_value: Decimal,
) -> Result<(), EvalErr> {
    stack.push(CalcResult::new(
        CalcResultType::Number(const_value),
        token_index,
    ));

    Ok(())
}

fn arg_count_limited_fn<F>(
    expected_arg_count: usize,
    arg_count: usize,
    stack: &mut Vec<CalcResult>,
    fn_token_index: usize,
    action: F,
) -> Result<(), EvalErr>
where
    F: Fn(&mut Vec<CalcResult>) -> Result<(), EvalErr>,
{
    let arg_count = arg_count.min(stack.len());
    return if expected_arg_count != arg_count {
        if arg_count > 0 {
            Err(EvalErr::new(
                "Illegal argument".to_owned(),
                stack[stack.len() - 1].get_index_into_tokens(),
            ))
        } else {
            Err(EvalErr::new("Illegal argument".to_owned(), fn_token_index))
        }
    } else {
        action(stack)
    };
}

fn fn_single_param_f64<'text_ptr, F>(stack: &mut Vec<CalcResult>, action: F) -> Result<(), EvalErr>
where
    F: Fn(f64) -> f64,
{
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
                Ok(())
            } else {
                Err(EvalErr::new2(
                    "Number cannot be represented as f64".to_owned(),
                    param,
                ))
            }
        }
        _ => Err(EvalErr::new2(
            "Only numbers are supported currently".to_owned(),
            param,
        )),
    }
}

fn fn_double_param_f64<'text_ptr, F>(
    stack: &mut Vec<CalcResult>,
    fn_token_index: usize,
    action: F,
) -> Result<(), EvalErr>
where
    F: Fn(f64, f64) -> f64,
{
    let first_param = &stack[stack.len() - 2];
    let second_param = &stack[stack.len() - 1];
    match (&first_param.typ, &second_param.typ) {
        (CalcResultType::Number(p1_dec), CalcResultType::Number(p2_dec)) => {
            let p1_f64 = p1_dec.to_f64();
            let p2_f64 = p2_dec.to_f64();
            if p1_f64.is_some() && p2_f64.is_some() {
                if let Some(result) = Decimal::from_f64(action(p1_f64.unwrap(), p2_f64.unwrap())) {
                    let token_index = first_param.get_index_into_tokens();
                    stack.pop();
                    stack.pop();
                    stack.push(CalcResult::new(CalcResultType::Number(result), token_index));
                    Ok(())
                } else {
                    Err(EvalErr::new(
                        "The result cannot be converted from f64 to Decimal".to_owned(),
                        fn_token_index,
                    ))
                }
            } else if p1_f64.is_none() {
                // TODO: format!
                Err(EvalErr::new2(
                    "The first arg could not be represented as f64".to_owned(),
                    first_param,
                ))
            } else {
                // TODO: format!
                Err(EvalErr::new2(
                    "The second arg could not be represented as f64".to_owned(),
                    second_param,
                ))
            }
        }
        _ => Err(EvalErr::new3(
            "Only numbers are supported currently".to_owned(),
            fn_token_index,
            first_param,
            second_param,
        )),
    }
}

fn fn_f64_rad_to_num<'text_ptr, F>(
    stack: &mut Vec<CalcResult>,
    units: &Units,
    action: F,
) -> Result<(), EvalErr>
where
    F: Fn(f64) -> f64,
{
    let param = &stack[stack.len() - 1];
    match &param.typ {
        CalcResultType::Quantity(num, unit) if unit.is(UnitType::Angle) => {
            // TODO make it const
            let rad_unit = UnitOutput::new_rad(units);
            let rad = UnitOutput::convert(unit, &rad_unit, num)
                .ok_or(EvalErr::new2("Could not convert to rad".to_owned(), param))?;
            if let Some(result) = rad
                .to_f64()
                .map(|it| action(it))
                .and_then(|it| Decimal::from_f64(it))
            {
                let token_index = param.get_index_into_tokens();
                stack.pop();
                stack.push(CalcResult::new(CalcResultType::Number(result), token_index));
                Ok(())
            } else {
                Err(EvalErr::new2(
                    "Param or result could not be represented as f64".to_owned(),
                    param,
                ))
            }
        }
        _ => Err(EvalErr::new2(
            "Only numbers are supported currently".to_owned(),
            param,
        )),
    }
}

fn fn_f64_num_to_rad<'text_ptr, F>(
    stack: &mut Vec<CalcResult>,
    action: F,
    units: &Units,
) -> Result<(), EvalErr>
where
    F: Fn(f64) -> f64,
{
    let param = &stack[stack.len() - 1];
    match &param.typ {
        CalcResultType::Number(num) => {
            if num > &Decimal::one() || num < &Decimal::one().neg() {
                return Err(EvalErr::new2("".to_owned(), param));
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
                Ok(())
            } else {
                Err(EvalErr::new2(
                    "Param or result could not be represented as f64".to_owned(),
                    param,
                ))
            }
        }
        _ => Err(EvalErr::new2(
            "Only numbers are supported currently".to_owned(),
            param,
        )),
    }
}

fn fn_single_param_decimal<'text_ptr, F>(
    stack: &mut Vec<CalcResult>,
    action: F,
) -> Result<(), EvalErr>
where
    F: Fn(&Decimal) -> Decimal,
{
    let param = &stack[stack.len() - 1];
    match &param.typ {
        CalcResultType::Number(num) => {
            let result = action(num);
            let token_index = param.get_index_into_tokens();
            stack.pop();
            stack.push(CalcResult::new(CalcResultType::Number(result), token_index));
            Ok(())
        }
        _ => Err(EvalErr::new2(
            "Only numbers are supported currently".to_owned(),
            param,
        )),
    }
}

fn fn_nth<'text_ptr>(stack: &mut Vec<CalcResult>) -> Result<(), EvalErr> {
    let index_token = &stack[stack.len() - 1];
    let mat_token = &stack[stack.len() - 2];
    match (&index_token.typ, &mat_token.typ) {
        (CalcResultType::Number(n), CalcResultType::Matrix(mat)) => {
            if let Some(index) = n.to_u32() {
                if mat.col_count < (index + 1) as usize {
                    Err(EvalErr::new(
                        "Index is out of range".to_owned(),
                        index_token.get_index_into_tokens(),
                    ))
                } else {
                    let result = mat.cell(0, index as usize).clone();
                    stack.truncate(stack.len() - 2);
                    stack.push(result);
                    Ok(())
                }
            } else {
                Err(EvalErr::new(
                    "Index must be zero or a positive integer".to_owned(),
                    index_token.get_index_into_tokens(),
                ))
            }
        }
        (CalcResultType::Number(_), _) => Err(EvalErr::new(
            "Second param must be a Matrix".to_owned(),
            mat_token.get_index_into_tokens(),
        )),
        (_, CalcResultType::Matrix(_)) => Err(EvalErr::new(
            "First param must be zero or a positive integer".to_owned(),
            index_token.get_index_into_tokens(),
        )),
        _ => Err(EvalErr::new(
            "First param must be zero or a positive integer and Second param must be a Matrix"
                .to_owned(),
            index_token.get_index_into_tokens(),
        )),
    }
}

fn fn_sum(stack: &mut Vec<CalcResult>) -> Result<(), EvalErr> {
    let param = &stack[stack.len() - 1];
    match &param.typ {
        CalcResultType::Matrix(mat) => {
            let mut sum = mat.cells[0].clone();
            for cell in mat.cells.iter().skip(1) {
                if let Some(result) = add_op(&sum, cell) {
                    sum = result;
                } else {
                    return Err(EvalErr::new(
                        format!("'{:?}' + '{:?}' failed", &sum, &cell),
                        cell.get_index_into_tokens(),
                    ));
                }
            }
            stack.truncate(stack.len() - 1);
            stack.push(sum);
            Ok(())
        }
        _ => Err(EvalErr::new2("Param must be a matrix".to_owned(), param)),
    }
}

fn fn_transpose(stack: &mut Vec<CalcResult>) -> Result<(), EvalErr> {
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
        Ok(())
    } else {
        Err(EvalErr::new2("Param must be a matrix".to_owned(), param))
    }
}
