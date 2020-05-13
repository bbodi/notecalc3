use crate::calc::CalcResult;
use crate::units::units::Units;
use crate::ResultFormat;
use bigdecimal::{BigDecimal, ToPrimitive};
use byteorder::WriteBytesExt;
use std::io::Cursor;

pub fn render_result(
    units: &Units,
    result: &CalcResult,
    format: &ResultFormat,
    there_was_unit_conversion: bool,
    decimal_count: usize,
) -> String {
    let mut c = Cursor::new(Vec::with_capacity(64));
    render_result_into(
        units,
        result,
        format,
        there_was_unit_conversion,
        &mut c,
        decimal_count,
    );
    return unsafe { String::from_utf8_unchecked(c.into_inner()) };
}

pub fn render_result_into(
    units: &Units,
    result: &CalcResult,
    format: &ResultFormat,
    there_was_unit_conversion: bool,
    f: &mut impl std::io::Write,
    decimal_count: usize,
) {
    match &result {
        CalcResult::Quantity(num, unit) => {
            let final_unit = if there_was_unit_conversion {
                None
            } else {
                unit.simplify(units)
            };
            let unit = final_unit.as_ref().unwrap_or(unit);
            if unit.units.is_empty() {
                num_to_string(f, &num, &ResultFormat::Dec, decimal_count);
            } else {
                num_to_string(f, &unit.denormalize(num), &ResultFormat::Dec, decimal_count);
                f.write_u8(b' ').expect("");
                // TODO to_string -> into(buf)
                for ch in unit.to_string().as_bytes() {
                    f.write_u8(*ch).expect("");
                }
            }
        }
        CalcResult::Number(num) => {
            // TODO optimize
            num_to_string(f, num, format, decimal_count);
        }
        CalcResult::Percentage(num) => {
            num_to_string(f, num, &ResultFormat::Dec, decimal_count);
            f.write_u8(b'%').expect("");
        }
        CalcResult::Matrix(mat) => {
            f.write_u8(b'[').expect("");
            for row_i in 0..mat.row_count {
                if row_i > 0 {
                    f.write_u8(b';').expect("");
                    f.write_u8(b' ').expect("");
                }
                for col_i in 0..mat.col_count {
                    if col_i > 0 {
                        f.write_u8(b',').expect("");
                        f.write_u8(b' ').expect("");
                    }
                    let cell = &mat.cells[row_i * mat.col_count + col_i];
                    render_result_into(units, cell, format, false, f, decimal_count);
                }
            }
            f.write_u8(b']').expect("");
        }
    }
}

fn num_to_string(
    f: &mut impl std::io::Write,
    num: &BigDecimal,
    format: &ResultFormat,
    _decimal_count: usize,
) {
    let num = if *format != ResultFormat::Dec && num.is_integer() {
        num.with_scale(0)
    } else {
        strip_trailing_zeroes(num)
    };
    // let num = strip_trailing_zeroes(&num.with_scale(decimal_count as i64));
    // let num = num.with_scale(decimal_count as i64);

    if *format == ResultFormat::Bin || *format == ResultFormat::Hex {
        if let Some(n) = num.to_i64() {
            let mut ss = if *format == ResultFormat::Bin {
                format!("{:b}", n)
            } else {
                format!("{:X}", n)
            };
            let s = unsafe { ss.as_bytes_mut() };
            s.reverse();
            let group_size = if *format == ResultFormat::Bin { 8 } else { 2 };
            for (i, group) in s.chunks(group_size).rev().enumerate() {
                if i > 0 {
                    f.write_u8(b' ').expect("");
                }
                for ch in group.iter().rev() {
                    f.write_u8(*ch).expect("");
                }
            }
        } else {
            // TODO to_string opt
            for ch in num.to_string().as_bytes() {
                f.write_u8(*ch).expect("");
            }
        }
    } else {
        // TODO to_string opt
        for ch in num.to_string().as_bytes() {
            f.write_u8(*ch).expect("");
        }
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
