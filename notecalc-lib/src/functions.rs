use crate::calc::{add_op, CalcResult};
use bigdecimal::{BigDecimal, ToPrimitive};
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
        }
    }

    #[inline]
    pub fn execute(&self, arg_count: usize, stack: &mut Vec<CalcResult>) -> bool {
        match self {
            FnType::Nth => fn_nth(arg_count, stack),
            FnType::Sum => fn_sum(arg_count, stack),
            FnType::Transpose => fn_transpose(arg_count, stack),
            FnType::Pi => fn_pi(arg_count, stack),
            FnType::Sin => true,
            FnType::Cos => true,
        }
    }
}

fn fn_pi(arg_count: usize, stack: &mut Vec<CalcResult>) -> bool {
    if arg_count != 0 {
        return false;
    }

    stack.push(CalcResult::Number(
        BigDecimal::from_str("3.1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679821480865132823066470938446095505822317253594081284811174502841027019385211055596446229489549303819644288109756659334461284756482337867831652712019091456485669234603486104543266482133936072602491412737245870066063155881748815209209628292540917153643678925903600113305305488204665213841469519415116094330572703657595919530921861173819326117931051185480744623799627495673518857527248912279381830119491298336733624406566430860213949463952247371907021798609437027705392171762931767523846748184676694051320005681271452635608277857713427577896091736371787214684409012249534301465495853710507922796892589235420199561121290219608640344181598136297747713099605187072113499999983729780499510597317328160963185950244594553469083026425223082533446850352619311881710100031378387528865875332083814206171776691473035982534904287554687311595628638823537875937519577818577805321712268066130019278766111959092164201989").unwrap()
    ));
    true
}

fn fn_nth(arg_count: usize, stack: &mut Vec<CalcResult>) -> bool {
    if arg_count < 2 {
        false
    } else {
        let index = &stack[stack.len() - 1];
        let mat = &stack[stack.len() - 2];
        match (index, mat) {
            (CalcResult::Number(n), CalcResult::Matrix(mat)) => {
                if let Some(index) = n.to_u32() {
                    if mat.col_count < (index + 1) as usize {
                        false
                    } else {
                        let result = mat.cell(0, index as usize).clone();
                        stack.truncate(stack.len() - 2);
                        stack.push(result);
                        true
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

fn fn_sum(arg_count: usize, stack: &mut Vec<CalcResult>) -> bool {
    if arg_count < 1 {
        false
    } else {
        let param = &stack[stack.len() - 1];
        match param {
            CalcResult::Matrix(mat) => {
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
        match param {
            CalcResult::Matrix(mat) => {
                let t = CalcResult::Matrix(mat.transposed());
                stack.truncate(stack.len() - 1);
                stack.push(t);
                true
            }
            _ => false,
        }
    }
}
