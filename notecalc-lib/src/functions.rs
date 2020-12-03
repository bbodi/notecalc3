use crate::calc::{add_op, CalcResult, CalcResultType};
use crate::token_parser::Token;
use rust_decimal::prelude::*;
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(PartialEq, Eq, Clone, Copy, Debug, EnumIter)]
pub enum FnType {
    Sin,
    Cos,
    Nth,
    Sum,
    Transpose,
    Pi,
    Ceil,
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
            FnType::Sin => &['s', 'i', 'n'],
            FnType::Cos => &['c', 'o', 's'],
            FnType::Nth => &['n', 't', 'h'],
            FnType::Sum => &['s', 'u', 'm'],
            FnType::Transpose => &['t', 'r', 'a', 'n', 's', 'p', 'o', 's', 'e'],
            FnType::Pi => &['p', 'i'],
            FnType::Ceil => &['c', 'e', 'i', 'l'],
        }
    }

    #[inline]
    pub fn execute<'text_ptr>(
        &self,
        arg_count: usize,
        stack: &mut Vec<CalcResult>,
        fn_token_index: usize,
        tokens: &mut [Token<'text_ptr>],
    ) -> bool {
        match self {
            FnType::Nth => fn_nth(arg_count, stack, tokens, fn_token_index),
            FnType::Sum => fn_sum(arg_count, stack),
            FnType::Transpose => fn_transpose(arg_count, stack),
            FnType::Pi => fn_pi(arg_count, stack, fn_token_index),
            FnType::Sin => true,
            FnType::Cos => true,
            FnType::Ceil => fn_ceil(arg_count, stack, tokens, fn_token_index),
        }
    }
}

fn fn_pi(arg_count: usize, stack: &mut Vec<CalcResult>, token_index: usize) -> bool {
    if arg_count != 0 {
        return false;
    }

    stack.push(CalcResult::new(CalcResultType::Number(
        Decimal::from_str("3.1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679821480865132823066470938446095505822317253594081284811174502841027019385211055596446229489549303819644288109756659334461284756482337867831652712019091456485669234603486104543266482133936072602491412737245870066063155881748815209209628292540917153643678925903600113305305488204665213841469519415116094330572703657595919530921861173819326117931051185480744623799627495673518857527248912279381830119491298336733624406566430860213949463952247371907021798609437027705392171762931767523846748184676694051320005681271452635608277857713427577896091736371787214684409012249534301465495853710507922796892589235420199561121290219608640344181598136297747713099605187072113499999983729780499510597317328160963185950244594553469083026425223082533446850352619311881710100031378387528865875332083814206171776691473035982534904287554687311595628638823537875937519577818577805321712268066130019278766111959092164201989").unwrap()
    ), token_index));

    true
}

fn fn_ceil<'text_ptr>(
    arg_count: usize,
    stack: &mut Vec<CalcResult>,
    tokens: &mut [Token<'text_ptr>],
    fn_token_index: usize,
) -> bool {
    if arg_count < 1 || stack.len() < 1 {
        Token::set_token_error_flag_by_index(fn_token_index, tokens);
        false
    } else {
        let param = &stack[stack.len() - 1];
        match &param.typ {
            CalcResultType::Number(num) => {
                let result = num.ceil();
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
