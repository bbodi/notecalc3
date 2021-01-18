use crate::calc::{CalcResult, CalcResultType};
use crate::units::units::{UnitOutput, Units};
use crate::{ResultFormat, ResultLengths};
use byteorder::WriteBytesExt;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::io::Cursor;
use tinyvec::ArrayVec;

pub fn render_result(
    units: &Units,
    result: &CalcResult,
    format: &ResultFormat,
    there_was_unit_conversion: bool,
    decimal_count: Option<usize>,
    use_grouping: bool,
) -> String {
    let mut c = Cursor::new(Vec::with_capacity(64));
    render_result_into(
        units,
        result,
        format,
        there_was_unit_conversion,
        &mut c,
        decimal_count,
        use_grouping,
    );
    return unsafe { String::from_utf8_unchecked(c.into_inner()) };
}

pub fn render_result_into(
    units: &Units,
    result: &CalcResult,
    format: &ResultFormat,
    there_was_unit_conversion: bool,
    f: &mut impl std::io::Write,
    decimal_count: Option<usize>,
    use_grouping: bool,
) -> ResultLengths {
    match &result.typ {
        CalcResultType::Quantity(num, unit) => {
            if *format != ResultFormat::Dec {
                f.write_u8(b'E').expect("");
                f.write_u8(b'r').expect("");
                f.write_u8(b'r').expect("");
                return ResultLengths {
                    int_part_len: 3,
                    frac_part_len: 0,
                    unit_part_len: 0,
                };
            }
            let final_unit_and_coeff = if there_was_unit_conversion {
                None
            } else {
                if let Some(new_unit) = unit.simplify(units) {
                    if let Some(coeff) = new_unit.get_unit_coeff() {
                        if let Some(orig_coeff) = unit.get_unit_coeff() {
                            if let Some(rust_is_a_joke) = orig_coeff.checked_div(&coeff) {
                                Some((new_unit, rust_is_a_joke))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            fn rust_is_a_joke_lang(
                num: &Decimal,
                unit: &UnitOutput,
                f: &mut impl std::io::Write,
                decimal_count: Option<usize>,
                use_grouping: bool,
            ) -> ResultLengths {
                if unit.unit_count == 0 {
                    num_to_string(f, &num, &ResultFormat::Dec, decimal_count, use_grouping)
                } else {
                    let mut lens =
                        num_to_string(f, &num, &ResultFormat::Dec, decimal_count, use_grouping);
                    f.write_u8(b' ').expect("");
                    // TODO:mem to_string -> into(buf)
                    // implement a into(std::io:Write) method for UnitOutput
                    if unit.unit_count == 1 && unit.get_unit(0).power == -1 {
                        let unit = &unit.get_unit(0);
                        f.write_u8(b'/').expect("");
                        f.write_u8(b' ').expect("");
                        for ch in unit.prefix.name {
                            f.write_u8(*ch as u8).expect("");
                        }
                        for ch in unit.unit.name {
                            f.write_u8(*ch as u8).expect("");
                        }
                        lens.unit_part_len += 2 + unit.prefix.name.len() + unit.unit.name.len();
                    } else {
                        for ch in unit.to_string().as_bytes() {
                            f.write_u8(*ch).expect("");
                            lens.unit_part_len += 1;
                        }
                    }
                    lens
                }
            }
            return if let Some((final_unit, coeff)) = final_unit_and_coeff {
                if let Some(rust_is_a_joke) = num.checked_mul(&coeff) {
                    rust_is_a_joke_lang(
                        &rust_is_a_joke,
                        &final_unit,
                        f,
                        decimal_count,
                        use_grouping,
                    )
                } else {
                    rust_is_a_joke_lang(num, unit, f, decimal_count, use_grouping)
                }
            } else {
                // rust is a joke
                rust_is_a_joke_lang(num, unit, f, decimal_count, use_grouping)
            };
        }
        CalcResultType::Unit(unit) => {
            // TODO:mem to_string -> into(buf)
            // implement a into(std::io:Write) method for UnitOutput
            let mut len = 0;
            for ch in unit.to_string().as_bytes() {
                f.write_u8(*ch).expect("");
                len += 1;
            }
            ResultLengths {
                int_part_len: 0,
                frac_part_len: 0,
                unit_part_len: len,
            }
        }
        CalcResultType::Number(num) => {
            // TODO optimize
            num_to_string(f, num, format, decimal_count, use_grouping)
        }
        CalcResultType::Percentage(num) => {
            if *format != ResultFormat::Dec {
                f.write_u8(b'E').expect("");
                f.write_u8(b'r').expect("");
                f.write_u8(b'r').expect("");
                return ResultLengths {
                    int_part_len: 3,
                    frac_part_len: 0,
                    unit_part_len: 0,
                };
            } else {
                let mut lens =
                    num_to_string(f, num, &ResultFormat::Dec, decimal_count, use_grouping);
                f.write_u8(b' ').expect("");
                f.write_u8(b'%').expect("");
                lens.unit_part_len += 1;
                lens
            }
        }
        CalcResultType::Matrix(mat) => {
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
                    render_result_into(units, cell, format, false, f, decimal_count, use_grouping);
                }
            }
            f.write_u8(b']').expect("");
            ResultLengths {
                int_part_len: 0,
                frac_part_len: 0,
                unit_part_len: 0,
            }
        }
    }
}

fn num_to_string(
    f: &mut impl std::io::Write,
    num: &Decimal,
    format: &ResultFormat,
    decimal_count: Option<usize>,
    use_grouping: bool,
) -> ResultLengths {
    let is_int = num.trunc() == *num;
    let num_a = if *format != ResultFormat::Dec && is_int {
        Some(num.clone())
    } else if let Some(decimal_count) = decimal_count {
        let mut result = num.clone();
        result.rescale(decimal_count as u32);
        Some(result.normalize())
    } else {
        let with_scale_0 = num.trunc();
        if *num == with_scale_0 {
            Some(with_scale_0)
        } else {
            None
        }
    };
    let num = num_a.as_ref().unwrap_or(num);

    return if *format == ResultFormat::Bin || *format == ResultFormat::Hex {
        let rust_is_shit = if is_int { Some(true) } else { None };
        if let Some(n) =
            rust_is_shit.and_then(|_| num.to_u64().or_else(|| num.to_i64().map(|it| it as u64)))
        {
            let ss = if *format == ResultFormat::Bin {
                format!("{:b}", n)
            } else {
                format!("{:X}", n)
            };
            ResultLengths {
                int_part_len: apply_grouping(
                    f,
                    &ss,
                    if use_grouping {
                        if *format == ResultFormat::Bin {
                            8
                        } else {
                            2
                        }
                    } else {
                        std::i32::MAX as usize
                    },
                ),
                frac_part_len: 0,
                unit_part_len: 0,
            }
        } else {
            f.write_u8(b'E').expect("");
            f.write_u8(b'r').expect("");
            f.write_u8(b'r').expect("");
            ResultLengths {
                int_part_len: 3,
                frac_part_len: 0,
                unit_part_len: 0,
            }
        }
    } else {
        // TODO to_string opt
        let string = if num.scale() == 0 {
            num.to_string()
        } else {
            if let Some(without_repeating_fract) = remove_repeatings(num) {
                without_repeating_fract.to_string()
            } else {
                num.to_string()
            }
        };

        if let Some(pos) = string.bytes().position(|it| it == b'.') {
            let (int_part, fract_part) = string.split_at(pos);
            let int_len = apply_grouping(
                f,
                &int_part,
                if use_grouping {
                    3
                } else {
                    std::i32::MAX as usize
                },
            );
            for ch in fract_part.as_bytes() {
                f.write_u8(*ch).expect("");
            }
            ResultLengths {
                int_part_len: int_len,
                frac_part_len: fract_part.len(),
                unit_part_len: 0,
            }
        } else {
            ResultLengths {
                int_part_len: apply_grouping(
                    f,
                    &string,
                    if use_grouping {
                        3
                    } else {
                        std::i32::MAX as usize
                    },
                ),
                frac_part_len: 0,
                unit_part_len: 0,
            }
        }
    };
}

fn remove_repeatings(num: &Decimal) -> Option<Decimal> {
    let string = num.to_string();
    if let Some(pos) = string.bytes().position(|it| it == b'.') {
        let (_int_part, fract_part) = string.split_at(pos);
        // TODO HACKY way for determining unrepresentable numbers
        // if all the fractional digit except the last two consist the same number, reduce them
        if (fract_part.len() - 1) > 4 {
            let frac_buf = fract_part.as_bytes();
            let mut checking_digit = *frac_buf.last().expect("must");
            let mut i = fract_part.len() - 2;
            let mut count = 1;
            while i > 0 {
                if frac_buf[i] != checking_digit {
                    if i as isize > fract_part.len() as isize - 7 {
                        // the last 5 digits can be different
                        checking_digit = frac_buf[i];
                        count = 0;
                    } else {
                        break;
                    }
                }
                count += 1;
                i -= 1;
            }
            if count > 15 || (count == (fract_part.len() - 1) && count > 5) {
                let mut clone = num.clone();
                clone.rescale(if i > 0 { i + 1 } else { 4 } as u32);
                return Some(clone);
            }
        }
    }
    return None;
}

fn apply_grouping(f: &mut impl std::io::Write, ss: &str, group_size: usize) -> usize {
    // TODO isnt it too much/is it enough?
    let mut buf: ArrayVec<[u8; 128]> = ArrayVec::new();
    for ch in ss.as_bytes() {
        buf.push(*ch);
    }
    let buff = &mut buf[0..ss.len()];
    buff.reverse();
    let mut len = 0;
    for (i, group) in buff.chunks(group_size).rev().enumerate() {
        if i > 0 {
            f.write_u8(b' ').expect("");
            len += 1;
        }
        for ch in group.iter().rev() {
            f.write_u8(*ch).expect("");
            len += 1;
        }
    }
    return len;
}

pub fn get_int_frac_part_len(cell_str: &str) -> ResultLengths {
    let mut int_part_len = 0;
    let mut frac_part_len = 0;
    let mut unit_part_len = 0;
    let mut was_point = false;
    let mut was_space = false;
    let mut only_digits_or_space_so_far = true;
    for ch in cell_str.as_bytes() {
        if *ch == b'.' {
            was_point = true;
            only_digits_or_space_so_far = false;
        } else if *ch == b' ' {
            was_space = true;
        }
        if was_space {
            if only_digits_or_space_so_far && ch.is_ascii_digit() {
                // this space was just a thousand separator
                int_part_len += 1;
                if unit_part_len > 0 {
                    // 2 000, that space was registered as unit, so add it to int_part
                    int_part_len += 1;
                }
                unit_part_len = 0;
            } else {
                if only_digits_or_space_so_far && !ch.is_ascii_whitespace() {
                    only_digits_or_space_so_far = false;
                }
                unit_part_len += 1;
            }
        } else if was_point {
            frac_part_len += 1;
        } else {
            int_part_len += 1;
        }
    }
    return ResultLengths {
        int_part_len,
        frac_part_len,
        unit_part_len,
    };
}

// TODO: really hack and ugly and slow
// pub fn strip_trailing_zeroes(num: &BigDecimal) -> BigDecimal {
//     let (_, mut scale) = num.as_bigint_and_exponent();
//     let mut result = num.clone();
//     loop {
//         if scale == 0 {
//             break;
//         }
//         let scaled = result.with_scale(scale - 1);
//         if &scaled == num {
//             result = scaled;
//         } else {
//             break;
//         }
//         scale -= 1;
//     }
//     return result;
// }
