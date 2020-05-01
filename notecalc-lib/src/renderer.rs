use crate::calc::CalcResult;
use crate::units::units::Units;
use crate::ResultFormat;
use bigdecimal::{BigDecimal, ToPrimitive};
use std::fmt::Write;

pub fn render_result(
    units: &Units,
    result: &CalcResult,
    format: &ResultFormat,
    there_was_unit_conversion: bool,
) -> String {
    let mut f = String::new();
    render_result_into(units, result, format, there_was_unit_conversion, &mut f);
    return f;
}

fn render_result_into(
    units: &Units,
    result: &CalcResult,
    format: &ResultFormat,
    there_was_unit_conversion: bool,
    f: &mut impl Write,
) {
    match &result {
        CalcResult::Quantity(num, unit) => {
            let final_unit = if there_was_unit_conversion {
                None
            } else {
                unit.simplify(units, &num)
            };
            let unit = final_unit.as_ref().unwrap_or(unit);
            if unit.units.is_empty() {
                num_to_string(f, &num, &ResultFormat::Dec);
            } else {
                num_to_string(f, &unit.denormalize(num), &ResultFormat::Dec);
                f.write_char(' ');
                f.write_str(&unit.to_string());
            }
        }
        CalcResult::Number(num) => {
            // TODO optimize
            num_to_string(f, num, format);
        }
        CalcResult::Percentage(num) => {
            num_to_string(f, num, &ResultFormat::Dec);
            f.write_char('%');
        }
        CalcResult::Matrix(mat) => {
            f.write_char('[');
            for row_i in 0..mat.row_count {
                if row_i > 0 {
                    f.write_char(';');
                    f.write_char(' ');
                }
                for col_i in 0..mat.col_count {
                    if col_i > 0 {
                        f.write_char(',');
                        f.write_char(' ');
                    }
                    let cell = &mat.cells[row_i * mat.col_count + col_i];
                    render_result_into(units, cell, format, false, f);
                }
            }
            f.write_char(']');
        }
    }
}

fn num_to_string(f: &mut impl Write, num: &BigDecimal, format: &ResultFormat) {
    let num = if num.is_integer() {
        num.with_scale(0)
    } else {
        strip_trailing_zeroes(num)
    };

    if *format == ResultFormat::Bin || *format == ResultFormat::Hex {
        if let Some(n) = num.to_i64() {
            let mut ss = if *format == ResultFormat::Bin {
                format!("{:b}", n)
            } else {
                format!("{:X}", n)
            };
            let mut s = unsafe { ss.as_bytes_mut() };
            s.reverse();
            let group_size = if *format == ResultFormat::Bin { 8 } else { 2 };
            for group in s.chunks(group_size).rev() {
                for ch in group.iter().rev() {
                    f.write_char(*ch as char);
                }
                f.write_char(' ');
            }
        } else {
            f.write_str(&num.to_string());
        }
    } else {
        f.write_str(&num.to_string());
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
