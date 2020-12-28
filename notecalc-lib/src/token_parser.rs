use crate::functions::FnType;
use crate::units::units::{UnitOutput, Units};
use crate::{Variables, SUM_VARIABLE_INDEX};
use bumpalo::Bump;
use rust_decimal::prelude::*;
use std::str::FromStr;
use tracy_client::*;

// TODO
#[allow(variant_size_differences)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TokenType {
    StringLiteral,
    Header,
    // index to the variable vec
    Variable { var_index: usize },
    LineReference { var_index: usize },
    NumberLiteral(Decimal),
    Operator(OperatorTokenType),
    Unit(UnitTokenType, UnitOutput),
    NumberErr,
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub ptr: &'a [char],
    pub typ: TokenType,
    pub has_error: bool,
}

// dbg!(Decimal::from_str("2.7182818284590452353602874713527")
//     .unwrap()
//     .unpack());

/// 3.1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679821480865132823066470938446095505822317253594081284811174502841027019385211055596446229489549303819644288109756659334461284756482337867831652712019091456485669234603486104543266482133936072602491412737245870066063155881748815209209628292540917153643678925903600113305305488204665213841469519415116094330572703657595919530921861173819326117931051185480744623799627495673518857527248912279381830119491298336733624406566430860213949463952247371907021798609437027705392171762931767523846748184676694051320005681271452635608277857713427577896091736371787214684409012249534301465495853710507922796892589235420199561121290219608640344181598136297747713099605187072113499999983729780499510597317328160963185950244594553469083026425223082533446850352619311881710100031378387528865875332083814206171776691473035982534904287554687311595628638823537875937519577818577805321712268066130019278766111959092164201989
pub const DECIMAL_PI: Decimal = Decimal::from_parts(1102470953, 185874565, 1703060790, false, 28);

/// 2.7182818284590452353602874713527
pub const DECIMAL_E: Decimal = Decimal::from_parts(2239425882, 3958169141, 1473583531, false, 28);

impl<'text_ptr> Token<'text_ptr> {
    pub fn is_number(&self) -> bool {
        matches!(self.typ, TokenType::NumberLiteral(..))
    }

    pub fn is_string(&self) -> bool {
        matches!(self.typ, TokenType::StringLiteral)
    }

    pub fn has_error(&self) -> bool {
        self.has_error
    }

    pub fn set_token_error_flag_by_index(index: usize, tokens: &mut [Token]) {
        // TODO I could not reproduce it but it happened runtime, so I use 'get_mut'
        // later when those indices will be used correctly (now they are just dummy values lot of times),
        // we can use direct indexing
        if let Some(t) = tokens.get_mut(index) {
            t.has_error = true
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum OperatorTokenType {
    Comma,
    Add,
    UnaryPlus,
    Sub,
    UnaryMinus,
    Mult,
    Div,
    Perc,
    BinAnd,
    BinOr,
    BinXor,
    BinNot,
    Pow,
    ParenOpen,
    ParenClose,
    BracketOpen,
    Semicolon,
    BracketClose,
    ShiftLeft,
    ShiftRight,
    Assign,
    UnitConverter,
    Matrix { row_count: usize, col_count: usize },
    Fn { arg_count: usize, typ: FnType },
    PercentageIs,
    // 41 is 17% on what
    Percentage_Find_Base_From_Result_Increase_X,
    // what plus 17% is 41
    Percentage_Find_Base_From_X_Icrease_Result,
    // 17% on what is 41
    Percentage_Find_Base_From_Icrease_X_Result,
    // 41 is what % on 35
    Percentage_Find_Incr_Rate_From_Result_X_Base,
    // 41 is 17% off what
    Percentage_Find_Base_From_Result_Decrease_X,
    // what minus 17% is 41
    Percentage_Find_Base_From_X_Decrease_Result,
    // 17% off what is 41
    Percentage_Find_Base_From_Decrease_X_Result,
    // 35 is what % off 41
    Percentage_Find_Decr_Rate_From_Result_X_Base,

    // 20 is what percent of 60
    Percentage_Find_Rate_From_Result_Base,
    // 5 is 25% of what
    Percentage_Find_Base_From_Result_Rate,
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum Assoc {
    Left,
    Right,
}

pub const APPLY_UNIT_OP_PREC: usize = 5;

impl OperatorTokenType {
    pub fn precedence(&self) -> usize {
        match self {
            OperatorTokenType::Add => 2,
            OperatorTokenType::UnaryPlus => 4,
            OperatorTokenType::Sub => 2,
            OperatorTokenType::UnaryMinus => 4,
            OperatorTokenType::Mult => 3,
            OperatorTokenType::Div => 3,
            OperatorTokenType::Perc => 6,
            OperatorTokenType::BinAnd => 0,
            OperatorTokenType::BinOr => 0,
            OperatorTokenType::BinXor => 0,
            OperatorTokenType::BinNot => 4,
            OperatorTokenType::Pow => APPLY_UNIT_OP_PREC + 1,
            OperatorTokenType::ParenOpen => 0,
            OperatorTokenType::ParenClose => 0,
            OperatorTokenType::ShiftLeft => 0,
            OperatorTokenType::ShiftRight => 0,
            OperatorTokenType::Assign => 0,
            OperatorTokenType::UnitConverter => 0,
            OperatorTokenType::Semicolon | OperatorTokenType::Comma => 0,
            OperatorTokenType::BracketOpen => 0,
            OperatorTokenType::BracketClose => 0,
            OperatorTokenType::Matrix { .. } => 0,
            OperatorTokenType::Fn { .. } => 0,
            OperatorTokenType::PercentageIs => 20,
            OperatorTokenType::Percentage_Find_Base_From_Result_Increase_X => 10,
            OperatorTokenType::Percentage_Find_Base_From_X_Icrease_Result => 10,
            OperatorTokenType::Percentage_Find_Base_From_Icrease_X_Result => 10,
            OperatorTokenType::Percentage_Find_Incr_Rate_From_Result_X_Base => 10,
            OperatorTokenType::Percentage_Find_Base_From_Result_Decrease_X => 10,
            OperatorTokenType::Percentage_Find_Base_From_X_Decrease_Result => 10,
            OperatorTokenType::Percentage_Find_Base_From_Decrease_X_Result => 10,
            OperatorTokenType::Percentage_Find_Decr_Rate_From_Result_X_Base => 10,
            OperatorTokenType::Percentage_Find_Rate_From_Result_Base => 10,
            OperatorTokenType::Percentage_Find_Base_From_Result_Rate => 10,
        }
    }

    pub fn assoc(&self) -> Assoc {
        match self {
            OperatorTokenType::ParenClose => Assoc::Left,
            OperatorTokenType::Add => Assoc::Left,
            OperatorTokenType::UnaryPlus => Assoc::Left,
            OperatorTokenType::Sub => Assoc::Left,
            OperatorTokenType::UnaryMinus => Assoc::Left,
            OperatorTokenType::Mult => Assoc::Left,
            OperatorTokenType::Div => Assoc::Left,
            OperatorTokenType::Perc => Assoc::Left,
            OperatorTokenType::BinAnd => Assoc::Left,
            OperatorTokenType::BinOr => Assoc::Left,
            OperatorTokenType::BinXor => Assoc::Left,
            OperatorTokenType::BinNot => Assoc::Left,
            OperatorTokenType::Pow => Assoc::Right,
            OperatorTokenType::ParenOpen => Assoc::Left,
            OperatorTokenType::ShiftLeft => Assoc::Left,
            OperatorTokenType::ShiftRight => Assoc::Left,
            OperatorTokenType::Assign => Assoc::Left,
            OperatorTokenType::UnitConverter => Assoc::Left,
            // Right, so 1 comma won't replace an other on the operator stack
            OperatorTokenType::Semicolon | OperatorTokenType::Comma => Assoc::Right,
            OperatorTokenType::BracketOpen => Assoc::Left,
            OperatorTokenType::BracketClose => Assoc::Left,
            OperatorTokenType::Matrix { .. } => Assoc::Left,
            OperatorTokenType::Fn { .. } => Assoc::Left,
            OperatorTokenType::PercentageIs => Assoc::Left,
            OperatorTokenType::Percentage_Find_Base_From_Result_Increase_X => Assoc::Left,
            OperatorTokenType::Percentage_Find_Base_From_X_Icrease_Result => Assoc::Left,
            OperatorTokenType::Percentage_Find_Base_From_Icrease_X_Result => Assoc::Left,
            OperatorTokenType::Percentage_Find_Incr_Rate_From_Result_X_Base => Assoc::Left,
            OperatorTokenType::Percentage_Find_Base_From_Result_Decrease_X => Assoc::Left,
            OperatorTokenType::Percentage_Find_Base_From_X_Decrease_Result => Assoc::Left,
            OperatorTokenType::Percentage_Find_Base_From_Decrease_X_Result => Assoc::Left,
            OperatorTokenType::Percentage_Find_Decr_Rate_From_Result_X_Base => Assoc::Left,
            OperatorTokenType::Percentage_Find_Rate_From_Result_Base => Assoc::Left,
            OperatorTokenType::Percentage_Find_Base_From_Result_Rate => Assoc::Left,
        }
    }
}

pub struct TokenParser {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitTokenType {
    ApplyToPrevToken,
    StandInItself,
}

impl TokenParser {
    pub fn parse_line<'text_ptr>(
        line: &[char],
        variable_names: &Variables,
        dst: &mut Vec<Token<'text_ptr>>,
        units: &Units,
        line_index: usize,
        allocator: &'text_ptr Bump,
    ) {
        let _span = Span::new("parse_line", "parse_line", file!(), line!(), 100);
        let mut index = 0;
        let mut can_be_unit = None;
        let mut can_be_unit_converter = false;
        if line.starts_with(&['#']) {
            dst.push(Token {
                ptr: allocator.alloc_slice_fill_iter(line.iter().map(|it| *it)),
                typ: TokenType::Header,
                has_error: false,
            });
            return;
        }
        while index < line.len() {
            let parse_result = TokenParser::try_extract_comment(&line[index..], allocator)
                .or_else(|| {
                    let prev_was_lineref = dst
                        .last()
                        .map(|token| matches!(token.typ, TokenType::LineReference{..}))
                        .unwrap_or(false);
                    TokenParser::try_extract_variable_name(
                        &line[index..],
                        variable_names,
                        line_index,
                        allocator,
                        prev_was_lineref,
                    )
                })
                .or_else(|| {
                    TokenParser::try_extract_unit(
                        &line[index..],
                        units,
                        can_be_unit,
                        can_be_unit_converter,
                        allocator,
                    )
                    .or_else(|| {
                        TokenParser::try_extract_operator(
                            &line[index..],
                            allocator,
                            can_be_unit_converter,
                        )
                        .or_else(|| {
                            TokenParser::try_extract_number_literal(&line[index..], allocator)
                                .or_else(|| {
                                    TokenParser::try_extract_string_literal(
                                        &line[index..],
                                        allocator,
                                    )
                                })
                        })
                    })
                });

            if let Some(token) = parse_result {
                match &token.typ {
                    TokenType::Header => {
                        // the functions already returned in this case
                        panic!();
                    }
                    TokenType::StringLiteral => {
                        if token.ptr[0].is_ascii_whitespace() {
                            // keep can_be_unit as it was
                        } else {
                            can_be_unit = None;
                            can_be_unit_converter = false;
                        }
                    }
                    TokenType::NumberLiteral(..) | TokenType::NumberErr => {
                        can_be_unit = Some(UnitTokenType::ApplyToPrevToken);
                        can_be_unit_converter = false;
                    }
                    TokenType::Unit(..) => {
                        can_be_unit = Some(UnitTokenType::StandInItself);
                        can_be_unit_converter = true;
                    }
                    TokenType::Operator(typ) => {
                        match typ {
                            OperatorTokenType::ParenClose => {
                                // keep can_be_unit as it was
                            }
                            OperatorTokenType::BracketClose => {
                                // keep can_be_unit as it was
                            }
                            OperatorTokenType::UnitConverter => {
                                can_be_unit = Some(UnitTokenType::StandInItself);
                                can_be_unit_converter = false;
                            }
                            OperatorTokenType::Div => {
                                can_be_unit = Some(UnitTokenType::StandInItself)
                            }
                            _ => {
                                can_be_unit = None;
                                can_be_unit_converter = false;
                            }
                        }
                    }
                    TokenType::Variable { .. } | TokenType::LineReference { .. } => {
                        can_be_unit = Some(UnitTokenType::ApplyToPrevToken);
                        can_be_unit_converter = true;
                    }
                }
                index += token.ptr.len();
                dst.push(token);
            } else {
                break;
            }
        }
    }

    pub fn try_extract_number_literal<'text_ptr>(
        str: &[char],
        allocator: &'text_ptr Bump,
    ) -> Option<Token<'text_ptr>> {
        let _span = Span::new(
            "try_extract_number_literal",
            "try_extract_number_literal",
            file!(),
            line!(),
            100,
        );
        let mut number_str = [b'0'; 256];
        let mut number_str_index = 0;
        let mut i = 0;
        // unary minus is parsed as part of the number only if
        // it is right before the number
        if str[0] == '-'
            && str
                .get(1)
                .map(|it| !it.is_ascii_whitespace())
                .unwrap_or(false)
        {
            number_str[0] = b'-';
            number_str_index = 1;
            i = 1;
        };

        // TODO: make it a builtin variable?
        if str[0] == 'π' {
            return Some(Token {
                typ: TokenType::NumberLiteral(DECIMAL_PI),
                // ptr: &str[0..i],
                ptr: allocator.alloc_slice_fill_iter(str.iter().map(|it| *it).take(1)),
                has_error: false,
            });
        }

        let result = if str[i..].starts_with(&['0', 'b']) {
            i += 2;
            let mut end_index_before_last_whitespace = i;
            while i < str.len() {
                if str[i] == '0' || str[i] == '1' {
                    end_index_before_last_whitespace = i + 1;
                    number_str[number_str_index] = str[i] as u8;
                    number_str_index += 1;
                } else if str[i].is_ascii_whitespace() || str[i] == '_' {
                    // allowed
                } else {
                    break;
                }
                i += 1;
            }
            i = end_index_before_last_whitespace;
            if i > 2 {
                // Decimal cannot parse binary, that's why the explicit i64 type
                let num: i64 = i64::from_str_radix(
                    &unsafe { std::str::from_utf8_unchecked(&number_str[0..number_str_index]) },
                    2,
                )
                .ok()?;
                Some(Token {
                    typ: TokenType::NumberLiteral(num.into()),
                    // ptr: &str[0..i],
                    ptr: allocator.alloc_slice_fill_iter(str.iter().map(|it| *it).take(i)),
                    has_error: false,
                })
            } else {
                None
            }
        } else if str[i..].starts_with(&['0', 'x']) {
            i += 2;
            let mut end_index_before_last_whitespace = i;
            while i < str.len() {
                if str[i].is_ascii_hexdigit()
                    && str
                        .get(i + 1)
                        .map(|it| it.is_ascii_hexdigit() || *it == '_' || !it.is_alphabetic())
                        .unwrap_or(true)
                {
                    end_index_before_last_whitespace = i + 1;
                    number_str[number_str_index] = str[i] as u8;
                    number_str_index += 1;
                } else if str[i] == '_' {
                    // allowed
                } else {
                    break;
                }
                i += 1;
            }
            i = end_index_before_last_whitespace;
            if i > 2 {
                // Decimal cannot parse hex, that's why the explicit i64 type
                let num: u64 = u64::from_str_radix(
                    &unsafe { std::str::from_utf8_unchecked(&number_str[0..number_str_index]) },
                    16,
                )
                .ok()?;
                Some(Token {
                    typ: TokenType::NumberLiteral(num.into()),
                    // ptr: &str[0..i],
                    ptr: allocator.alloc_slice_fill_iter(str.iter().map(|it| *it).take(i)),
                    has_error: false,
                })
            } else {
                None
            }
        } else if str
            .get(0)
            .map(|it| it.is_ascii_digit() || *it == '.' || *it == '-')
            .unwrap_or(false)
        {
            let mut decimal_point_count = 0;
            let mut digit_count = 0;
            let mut e_count = 0;
            let mut end_index_before_last_whitespace = 0;
            let mut e_neg = false;
            let mut e_already_added = false;
            let mut multiplier = None;

            while i < str.len() {
                if str[i] == '.' && decimal_point_count < 1 && e_count < 1 {
                    decimal_point_count += 1;
                    end_index_before_last_whitespace = i + 1;
                    number_str[number_str_index] = str[i] as u8;
                    number_str_index += 1;
                } else if str[i] == '-' && e_count == 1 {
                    if e_neg || e_already_added {
                        break;
                    }
                    e_neg = true;
                } else if str[i] == 'e' && e_count < 1 && !str[i - 1].is_ascii_whitespace() {
                    // cannot have whitespace before 'e'
                    e_count += 1;
                } else if str[i] == 'k'
                    && e_count < 1
                    && !str[i - 1].is_ascii_whitespace()
                    && str.get(i + 1).map(|it| !it.is_alphabetic()).unwrap_or(true)
                {
                    multiplier = Some(1_000);
                    end_index_before_last_whitespace = i + 1;
                    break;
                } else if str[i] == 'M'
                    && e_count < 1
                    && !str[i - 1].is_ascii_whitespace()
                    && str.get(i + 1).map(|it| !it.is_alphabetic()).unwrap_or(true)
                {
                    multiplier = Some(1_000_000);
                    end_index_before_last_whitespace = i + 1;
                    break;
                } else if str[i].is_ascii_digit() {
                    if e_count > 0 && !e_already_added {
                        number_str[number_str_index] = 'e' as u8;
                        number_str_index += 1;
                        if e_neg {
                            number_str[number_str_index] = '-' as u8;
                            number_str_index += 1;
                        }
                        number_str[number_str_index] = str[i] as u8;
                        number_str_index += 1;
                        end_index_before_last_whitespace = i + 1;
                        e_already_added = true;
                    } else {
                        digit_count += 1;
                        end_index_before_last_whitespace = i + 1;
                        number_str[number_str_index] = str[i] as u8;
                        number_str_index += 1;
                    }
                } else if str[i].is_ascii_whitespace() {
                    // allowed
                } else {
                    break;
                }
                i += 1;
            }
            i = end_index_before_last_whitespace;
            if digit_count > 0 {
                let num = if e_already_added {
                    Decimal::from_scientific(&unsafe {
                        std::str::from_utf8_unchecked(&number_str[0..number_str_index])
                    })
                } else {
                    Decimal::from_str(&unsafe {
                        std::str::from_utf8_unchecked(&number_str[0..number_str_index])
                    })
                };
                if let Ok(num) = num {
                    if let Some(multiplier) = multiplier {
                        if let Some(result) = Decimal::from(multiplier).checked_mul(&num) {
                            Some(Token {
                                typ: TokenType::NumberLiteral(result),
                                ptr: allocator
                                    .alloc_slice_fill_iter(str.iter().map(|it| *it).take(i)),
                                has_error: false,
                            })
                        } else {
                            Some(Token {
                                typ: TokenType::NumberErr,
                                ptr: allocator
                                    .alloc_slice_fill_iter(str.iter().map(|it| *it).take(i)),
                                has_error: true,
                            })
                        }
                    } else {
                        Some(Token {
                            typ: TokenType::NumberLiteral(num),
                            ptr: allocator.alloc_slice_fill_iter(str.iter().map(|it| *it).take(i)),
                            has_error: false,
                        })
                    }
                } else {
                    Some(Token {
                        typ: TokenType::NumberErr,
                        // ptr: &str[0..i],
                        ptr: allocator.alloc_slice_fill_iter(str.iter().map(|it| *it).take(i)),
                        has_error: true,
                    })
                }
            } else {
                None
            }
        } else {
            None
        };
        return result;
    }

    fn try_extract_unit<'text_ptr>(
        str: &[char],
        unit: &Units,
        can_be_unit: Option<UnitTokenType>,
        can_be_unit_converter: bool,
        allocator: &'text_ptr Bump,
    ) -> Option<Token<'text_ptr>> {
        let _span = Span::new(
            "try_extract_unit",
            "try_extract_unit",
            file!(),
            line!(),
            100,
        );
        if can_be_unit.is_none() || str[0].is_ascii_whitespace() {
            return None;
        }
        let (unit, parsed_len) = unit.parse(str);
        let result = if parsed_len == 0 {
            None
        } else {
            // remove trailing spaces
            let mut i = parsed_len;
            while i > 0 && str[i - 1].is_ascii_whitespace() {
                i -= 1;
            }
            let ptr = allocator.alloc_slice_fill_iter(str.iter().map(|it| *it).take(i));
            if ptr == &['i', 'n'] && can_be_unit_converter {
                // 'in' is always a UnitConverter, the shunting_yard will convert it to
                // ApplyUnit or String if necessary.
                // It is required "&[1] in", here, we don't know yet if 'in' is a converter or a unit
                // for the referenced value
                // However with 'can_be_unit_converter' we try to reduce its occurance
                // as much as possible
                Some(Token {
                    typ: TokenType::Operator(OperatorTokenType::UnitConverter),
                    ptr,
                    has_error: false,
                })
            } else {
                if let Some(can_be_unit) = can_be_unit {
                    Some(Token {
                        typ: TokenType::Unit(can_be_unit, unit),
                        ptr,
                        has_error: false,
                    })
                } else {
                    panic!("impossible")
                }
            }
        };
        return result;
    }

    fn try_extract_comment<'text_ptr>(
        line: &[char],
        allocator: &'text_ptr Bump,
    ) -> Option<Token<'text_ptr>> {
        return if line.starts_with(&['/', '/']) {
            Some(Token {
                typ: TokenType::StringLiteral,
                ptr: allocator.alloc_slice_fill_iter(line.iter().map(|it| *it)),
                has_error: false,
            })
        } else {
            None
        };
    }

    fn try_extract_variable_name<'text_ptr>(
        line: &[char],
        vars: &Variables,
        row_index: usize,
        allocator: &'text_ptr Bump,
        prev_was_lineref: bool,
    ) -> Option<Token<'text_ptr>> {
        let _span = Span::new(
            "try_extract_variable_name",
            "try_extract_variable_name",
            file!(),
            line!(),
            100,
        );
        if line.starts_with(&['s', 'u', 'm']) && line.get(3).map(|it| *it == ' ').unwrap_or(true) {
            return Some(Token {
                typ: TokenType::Variable {
                    var_index: SUM_VARIABLE_INDEX,
                },
                ptr: allocator.alloc_slice_fill_iter(line.iter().map(|it| *it).take(3)),
                has_error: false,
            });
        }
        let mut longest_match_index = 0;
        let mut longest_match = 0;
        'asd: for (var_index, var) in vars[0..row_index].iter().enumerate().rev() {
            if var.is_none() {
                continue;
            }
            let var = var.as_ref().unwrap();
            for (i, ch) in var.name.iter().enumerate() {
                if i >= line.len() || line[i] != *ch {
                    continue 'asd;
                }
            }
            // if the next char is '(', it can't be a var name
            if line
                .get(var.name.len())
                .map(|it| *it == '(')
                .unwrap_or(false)
            {
                continue 'asd;
            }
            // only full match allowed e.g. if there is variable 'b', it should not match "b0" as 'b' and '0'
            let not_full_match = line
                .get(var.name.len())
                .map(|it| it.is_alphanumeric())
                .unwrap_or(false);
            if not_full_match {
                continue 'asd;
            }
            if var.name.len() > longest_match {
                longest_match = var.name.len();
                longest_match_index = var_index;
            }
        }
        let result = if longest_match > 0 {
            let is_line_ref = longest_match > 2 && line[0] == '&' && line[1] == '[';
            let typ = if is_line_ref {
                if prev_was_lineref {
                    return None;
                } else {
                    TokenType::LineReference {
                        var_index: longest_match_index,
                    }
                }
            } else {
                TokenType::Variable {
                    var_index: longest_match_index,
                }
            };
            Some(Token {
                typ,
                ptr: allocator.alloc_slice_fill_iter(line.iter().map(|it| *it).take(longest_match)),
                has_error: false,
            })
        } else {
            None
        };
        return result;
    }

    fn try_extract_string_literal<'text_ptr>(
        str: &[char],
        allocator: &'text_ptr Bump,
    ) -> Option<Token<'text_ptr>> {
        let _span = Span::new(
            "try_extract_string_literal",
            "try_extract_string_literal",
            file!(),
            line!(),
            100,
        );
        let mut i = 0;
        for ch in str {
            if "=%/+-*^()[]".chars().any(|it| it == *ch) || ch.is_ascii_whitespace() {
                break;
            }
            // it means somwewhere we passed an invalid slice
            debug_assert!(*ch as u8 != 0);
            i += 1;
        }
        let result = if i > 0 {
            // alphabetical literal
            Some(Token {
                typ: TokenType::StringLiteral,
                ptr: allocator.alloc_slice_fill_iter(str.iter().map(|it| *it).take(i)),
                // ptr: &str[0..i],
                has_error: false,
            })
        } else {
            for ch in &str[0..] {
                if !ch.is_ascii_whitespace() {
                    break;
                }
                i += 1;
            }
            if i > 0 {
                // whitespace
                Some(Token {
                    typ: TokenType::StringLiteral,
                    // ptr: &str[0..i],
                    ptr: allocator.alloc_slice_fill_iter(str.iter().map(|it| *it).take(i)),
                    has_error: false,
                })
            } else {
                None
            }
        };
        return result;
    }

    fn try_extract_operator<'text_ptr>(
        str: &[char],
        allocator: &'text_ptr Bump,
        can_be_unit_converter: bool,
    ) -> Option<Token<'text_ptr>> {
        let _span = Span::new(
            "try_extract_operator",
            "try_extract_operator",
            file!(),
            line!(),
            100,
        );
        fn op<'text_ptr>(
            typ: OperatorTokenType,
            str: &[char],
            len: usize,
            allocator: &'text_ptr Bump,
        ) -> Option<Token<'text_ptr>> {
            return Some(Token {
                typ: TokenType::Operator(typ),
                // ptr: &str[0..len],
                ptr: allocator.alloc_slice_fill_iter(str.iter().map(|it| *it).take(len)),
                has_error: false,
            });
        }
        fn cmp<'text_ptr>(str: &[char], expected: &[char]) -> bool {
            return str.starts_with(expected)
                && str
                    .get(expected.len())
                    .map(|it| !it.is_alphabetic())
                    .unwrap_or(true);
        }
        let result = match str[0] {
            '=' => op(OperatorTokenType::Assign, str, 1, allocator),
            '+' => op(OperatorTokenType::Add, str, 1, allocator),
            '-' => op(OperatorTokenType::Sub, str, 1, allocator),
            '*' => op(OperatorTokenType::Mult, str, 1, allocator),
            '/' => op(OperatorTokenType::Div, str, 1, allocator),
            '%' => op(OperatorTokenType::Perc, str, 1, allocator),
            '^' => op(OperatorTokenType::Pow, str, 1, allocator),
            '(' => op(OperatorTokenType::ParenOpen, str, 1, allocator),
            ')' => op(OperatorTokenType::ParenClose, str, 1, allocator),
            '[' => op(OperatorTokenType::BracketOpen, str, 1, allocator),
            ']' => op(OperatorTokenType::BracketClose, str, 1, allocator),
            ',' => op(OperatorTokenType::Comma, str, 1, allocator),
            ';' => op(OperatorTokenType::Semicolon, str, 1, allocator),
            _ => {
                if str.starts_with(&['i', 'n', ' ']) && can_be_unit_converter {
                    op(OperatorTokenType::UnitConverter, str, 2, allocator)
                } else if cmp(
                    str,
                    &['i', 's', ' ', 'w', 'h', 'a', 't', ' ', '%', ' ', 'o', 'n'],
                ) {
                    op(
                        OperatorTokenType::Percentage_Find_Incr_Rate_From_Result_X_Base,
                        str,
                        12,
                        allocator,
                    )
                } else if cmp(
                    str,
                    &[
                        'i', 's', ' ', 'w', 'h', 'a', 't', ' ', '%', ' ', 'o', 'f', 'f',
                    ],
                ) {
                    op(
                        OperatorTokenType::Percentage_Find_Decr_Rate_From_Result_X_Base,
                        str,
                        13,
                        allocator,
                    )
                } else if cmp(str, &['o', 'n', ' ', 'w', 'h', 'a', 't', ' ', 'i', 's']) {
                    op(
                        OperatorTokenType::Percentage_Find_Base_From_Icrease_X_Result,
                        str,
                        10,
                        allocator,
                    )
                } else if cmp(
                    str,
                    &['o', 'f', 'f', ' ', 'w', 'h', 'a', 't', ' ', 'i', 's'],
                ) {
                    op(
                        OperatorTokenType::Percentage_Find_Base_From_Decrease_X_Result,
                        str,
                        11,
                        allocator,
                    )
                } else if cmp(str, &['o', 'n', ' ', 'w', 'h', 'a', 't']) {
                    op(
                        OperatorTokenType::Percentage_Find_Base_From_Result_Increase_X,
                        str,
                        7,
                        allocator,
                    )
                } else if cmp(str, &['o', 'f', 'f', ' ', 'w', 'h', 'a', 't']) {
                    op(
                        OperatorTokenType::Percentage_Find_Base_From_Result_Decrease_X,
                        str,
                        8,
                        allocator,
                    )
                } else if cmp(str, &['w', 'h', 'a', 't', ' ', 'p', 'l', 'u', 's']) {
                    op(
                        OperatorTokenType::Percentage_Find_Base_From_X_Icrease_Result,
                        str,
                        9,
                        allocator,
                    )
                } else if cmp(str, &['w', 'h', 'a', 't', ' ', 'm', 'i', 'n', 'u', 's']) {
                    op(
                        OperatorTokenType::Percentage_Find_Base_From_X_Decrease_Result,
                        str,
                        10,
                        allocator,
                    )
                } else if cmp(str, &['o', 'f', ' ', 'w', 'h', 'a', 't']) {
                    op(
                        OperatorTokenType::Percentage_Find_Base_From_Result_Rate,
                        str,
                        7,
                        allocator,
                    )
                } else if cmp(
                    str,
                    &[
                        'i', 's', ' ', 'w', 'h', 'a', 't', ' ', 'p', 'e', 'r', 'c', 'e', 'n', 't',
                        ' ', 'o', 'f',
                    ],
                ) {
                    op(
                        OperatorTokenType::Percentage_Find_Rate_From_Result_Base,
                        str,
                        18,
                        allocator,
                    )
                } else if cmp(str, &['i', 's']) {
                    op(OperatorTokenType::PercentageIs, str, 2, allocator)
                } else if cmp(str, &['A', 'N', 'D']) {
                    // TODO unit test "0xff and(12)"
                    op(OperatorTokenType::BinAnd, str, 3, allocator)
                } else if cmp(str, &['O', 'R']) {
                    op(OperatorTokenType::BinOr, str, 2, allocator)
                } else if cmp(str, &['N', 'O', 'T', '(']) {
                    op(OperatorTokenType::BinNot, str, 3, allocator)
                // '(' will be parsed separately as an operator
                } else if cmp(str, &['X', 'O', 'R']) {
                    op(OperatorTokenType::BinXor, str, 3, allocator)
                } else if str.starts_with(&['<', '<']) {
                    op(OperatorTokenType::ShiftLeft, str, 2, allocator)
                } else if str.starts_with(&['>', '>']) {
                    op(OperatorTokenType::ShiftRight, str, 2, allocator)
                } else {
                    None
                }
            }
        };
        return result;
    }
}

#[allow(dead_code)]
#[allow(unused_variables)]
pub fn debug_print(str: &str) {
    if true {
        return;
    }
    #[cfg(debug_assertions)]
    {
        println!("{}", str);
    }
}

#[allow(dead_code)]
#[allow(unused_variables)]
pub fn pad_rust_is_shit(error: &mut String, str: &str, len: usize) {
    #[cfg(debug_assertions)]
    {
        error.push_str(str);
        if str.len() < len {
            for _ in 0..len - str.len() {
                error.push(' ');
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::borrow_checker_fighter::create_vars;
    use crate::calc::{CalcResult, CalcResultType};
    use crate::shunting_yard::tests::*;
    use crate::units::units::Units;
    use crate::{Variable, MAX_LINE_COUNT};

    #[test]
    fn test_number_parsing() {
        fn test_parse(str: &str, expected_value: u64) {
            let mut vec = vec![];
            let temp = str.chars().collect::<Vec<_>>();
            let units = Units::new();
            let arena = Bump::new();
            TokenParser::parse_line(&temp, &create_vars(), &mut vec, &units, 0, &arena);
            match vec.get(0) {
                Some(Token {
                    ptr: _,
                    typ: TokenType::NumberLiteral(num),
                    has_error: _,
                }) => {
                    assert_eq!(*num, expected_value.into());
                }
                _ => panic!("'{}' failed", str),
            }
            println!("{} OK", str);
        }

        fn test_parse_f(str: &str, expected_value: &str) {
            let mut vec = vec![];
            let temp = str.chars().collect::<Vec<_>>();
            let units = Units::new();
            let arena = Bump::new();
            TokenParser::parse_line(&temp, &create_vars(), &mut vec, &units, 0, &arena);
            match vec.get(0) {
                Some(Token {
                    ptr: _,
                    typ: TokenType::NumberLiteral(num),
                    has_error: _,
                }) => {
                    assert_eq!(Decimal::from_str(expected_value).expect("must"), *num);
                }
                _ => panic!("'{}' failed", str),
            }
            println!("{} OK", str);
        }

        test_parse("0b1", 1);
        test_parse("0b0101", 5);
        test_parse("0b0101 1010", 90);
        test_parse("0b0101 101     1", 91);

        test_parse("0x1", 1);
        test_parse("0xAB_Cd_e____f", 11_259_375);

        test_parse("1", 1);
        test_parse("123456", 123456);
        test_parse("12 34 5        6", 123456);
        test_parse_f("123.456", "123.456");

        test_parse_f("0.1", "0.1");
        test_parse_f(".1", "0.1");
        test_parse_f(".1.", "0.1");
        test_parse_f("123.456.", "123.456");
        // it means 2 numbers, 123.456 and 0.3
        test_parse_f("123.456.3", "123.456");
    }

    fn test_vars(var_names: &[&'static [char]], text: &str, expected_tokens: &[Token]) {
        let var_names: Vec<Option<Variable>> = (0..MAX_LINE_COUNT + 1)
            .into_iter()
            .map(|index| {
                if let Some(var_name) = var_names.get(index) {
                    Some(Variable {
                        name: Box::from(*var_name),
                        value: Ok(CalcResult::new(CalcResultType::Number(Decimal::zero()), 0)),
                    })
                } else {
                    None
                }
            })
            .collect();
        println!("{}", text);
        let mut vec = vec![];
        let temp = text.chars().collect::<Vec<_>>();
        let units = Units::new();
        let arena = Bump::new();
        // line index is 10 so the search for the variable does not stop at 0
        TokenParser::parse_line(&temp, &var_names, &mut vec, &units, 10, &arena);

        let mut differences = Vec::with_capacity(vec.len().max(expected_tokens.len()));
        for (error_index, (actual_token, expected_token)) in
            vec.iter().zip(expected_tokens.iter()).enumerate()
        {
            match (&expected_token.typ, &actual_token.typ) {
                (TokenType::NumberLiteral(expected_num), TokenType::NumberLiteral(actual_num)) => {
                    if expected_num != actual_num {
                        differences.push(error_index);
                    }
                }
                (TokenType::NumberErr, _) => {
                    if actual_token.typ != expected_token.typ {
                        differences.push(error_index);
                    }
                }
                (TokenType::Operator(etyp), TokenType::Operator(atyp)) => assert_eq!(atyp, etyp),
                (
                    TokenType::Unit(UnitTokenType::StandInItself, _),
                    TokenType::Unit(UnitTokenType::StandInItself, _),
                )
                | (
                    TokenType::Unit(UnitTokenType::ApplyToPrevToken, _),
                    TokenType::Unit(UnitTokenType::ApplyToPrevToken, _),
                )
                | (TokenType::StringLiteral, TokenType::StringLiteral)
                | (TokenType::Header, TokenType::Header)
                | (TokenType::Variable { .. }, TokenType::Variable { .. })
                | (TokenType::Unit(_, _), TokenType::Unit(_, _))
                | (TokenType::LineReference { .. }, TokenType::LineReference { .. }) => {
                    if actual_token.ptr != expected_token.ptr {
                        differences.push(error_index);
                    }
                }
                _ => {
                    differences.push(error_index);
                }
            }
        }
        if !differences.is_empty() || expected_tokens.len() != vec.len() {
            print_tokens_compare_error_and_panic(
                &format!("Text: {}\nActual Tokens", text),
                &vec,
                &expected_tokens,
                &differences,
            );
        }
    }

    pub fn print_tokens_compare_error_and_panic(
        name: &str,
        actual_tokens: &[Token],
        expected_tokens: &[Token],
        error_indices: &[usize],
    ) -> ! {
        fn println_token(error: &mut String, token: &Token) {
            pad_rust_is_shit(error, &format!("{:?}", token.typ), 35);
            pad_rust_is_shit(error, &format!("{:?}", token.ptr), 50);
            error.push_str(&format!("{}", token.has_error));
            error.push('\n');
        }
        // Rust's format! macro escapes newline characters...
        let mut error = String::with_capacity(200);
        error.push_str(name);
        error.push('\n');

        for (i, actual_token) in actual_tokens.iter().enumerate() {
            if error_indices.contains(&i) {
                error.push_str(&format!(
                    "{}. ERR --------------------------------------------------------------------------------------------------------------",
                    i
                ));
                error.push_str("\nactual:   ");
                println_token(&mut error, actual_token);
                error.push_str("expected: ");
                println_token(&mut error, &expected_tokens[i]);
                error.push_str(
                    "---------------------------------------------------------------------------------------------------------------------\n",
                );
            } else {
                error.push_str(&format!("{}. ok           ", i));
                println_token(&mut error, actual_token);
                if let Some(expected_token) = expected_tokens.get(i) {
                    error.push_str(&format!("{}. expected     ", i));
                    println_token(&mut error, expected_token);
                } else {
                    error.push_str(&format!("{}. expected     NOTHING\n", i));
                }
            }
        }
        for (i, expected_token) in expected_tokens.iter().skip(actual_tokens.len()).enumerate() {
            error.push_str(&format!(
                "{}. actual       NOTHING\n",
                actual_tokens.len() + i
            ));
            error.push_str(&format!("{}. expected     ", actual_tokens.len() + i));
            println_token(&mut error, expected_token);
        }

        panic!(error);
    }

    fn test(text: &str, expected_tokens: &[Token]) {
        test_vars(&[], text, expected_tokens);
    }

    #[test]
    fn test_numbers_plus_operators_parsing() {
        test("0ba", &[str("0ba")]);
        test("2", &[num(2)]);
        test("-2", &[op(OperatorTokenType::Sub), num(2)]);
        test(".2", &[numf(0.2)]);
        test("2.", &[numf(2.)]);
        test(".2.", &[numf(0.2), str(".")]);
        test(".2.0", &[numf(0.2), numf(0.0)]);

        test(
            "2^-2",
            &[
                num(2),
                op(OperatorTokenType::Pow),
                op(OperatorTokenType::Sub),
                num(2),
            ],
        );

        test(
            "text with space at end ",
            &[
                str("text"),
                str(" "),
                str("with"),
                str(" "),
                str("space"),
                str(" "),
                str("at"),
                str(" "),
                str("end"),
                str(" "),
            ],
        );

        test("1+2.0", &[num(1), op(OperatorTokenType::Add), numf(2.0)]);
        test(
            "1 + 2.0",
            &[
                num(1),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                numf(2.0),
            ],
        );
        test(
            "1.2 + 2.0",
            &[
                numf(1.2),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                numf(2.0),
            ],
        );

        test("-3", &[op(OperatorTokenType::Sub), num(3)]);
        test("- 3", &[op(OperatorTokenType::Sub), str(" "), num(3)]);
        test("-0xFF", &[op(OperatorTokenType::Sub), num(255)]);
        test("-0b110011", &[op(OperatorTokenType::Sub), num(51)]);

        test(
            "-1 + -2",
            &[
                op(OperatorTokenType::Sub),
                num(1),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                op(OperatorTokenType::Sub),
                num(2),
            ],
        );

        test(
            "-(1) - -(2)",
            &[
                op(OperatorTokenType::Sub),
                op(OperatorTokenType::ParenOpen),
                num(1),
                op(OperatorTokenType::ParenClose),
                str(" "),
                op(OperatorTokenType::Sub),
                str(" "),
                op(OperatorTokenType::Sub),
                op(OperatorTokenType::ParenOpen),
                num(2),
                op(OperatorTokenType::ParenClose),
            ],
        );

        test(
            "-1 - -2",
            &[
                op(OperatorTokenType::Sub),
                num(1),
                str(" "),
                op(OperatorTokenType::Sub),
                str(" "),
                op(OperatorTokenType::Sub),
                num(2),
            ],
        );

        test(
            "200kg alma + 300 kg banán",
            &[
                num(200),
                apply_to_prev_token_unit("kg"),
                str(" "),
                str("alma"),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                num(300),
                str(" "),
                apply_to_prev_token_unit("kg"),
                str(" "),
                str("banán"),
            ],
        );
        test(
            "(1 alma + 4 körte) * 3 ember",
            &[
                op(OperatorTokenType::ParenOpen),
                num(1),
                str(" "),
                str("alma"),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                num(4),
                str(" "),
                str("körte"),
                op(OperatorTokenType::ParenClose),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(3),
                str(" "),
                str("ember"),
            ],
        );

        test(
            "1/2s",
            &[
                num(1),
                op(OperatorTokenType::Div),
                num(2),
                apply_to_prev_token_unit("s"),
            ],
        );
        test(
            "0xFF AND 0b11",
            &[
                num(0xFF),
                str(" "),
                op(OperatorTokenType::BinAnd),
                str(" "),
                num(0b11),
            ],
        );

        test(
            "0xFF AND",
            &[num(0xff), str(" "), op(OperatorTokenType::BinAnd)],
        );
        test(
            "0xFF OR",
            &[num(0xff), str(" "), op(OperatorTokenType::BinOr)],
        );
        test(
            "0xFF XOR",
            &[num(0xff), str(" "), op(OperatorTokenType::BinXor)],
        );

        test(
            "((0b00101 AND 0xFF) XOR 0xFF00) << 16 >> 16",
            &[
                op(OperatorTokenType::ParenOpen),
                op(OperatorTokenType::ParenOpen),
                num(0b00101),
                str(" "),
                op(OperatorTokenType::BinAnd),
                str(" "),
                num(0xFF),
                op(OperatorTokenType::ParenClose),
                str(" "),
                op(OperatorTokenType::BinXor),
                str(" "),
                num(0xFF00),
                op(OperatorTokenType::ParenClose),
                str(" "),
                op(OperatorTokenType::ShiftLeft),
                str(" "),
                num(16),
                str(" "),
                op(OperatorTokenType::ShiftRight),
                str(" "),
                num(16),
            ],
        );
        test(
            "NOT(0xFF)",
            &[
                op(OperatorTokenType::BinNot),
                op(OperatorTokenType::ParenOpen),
                num(0xFF),
                op(OperatorTokenType::ParenClose),
            ],
        );
        test(
            "((0b00101 AND 0xFF) XOR 0xFF00) << 16 >> 16 AND NOT(0xFF)",
            &[
                op(OperatorTokenType::ParenOpen),
                op(OperatorTokenType::ParenOpen),
                num(0b00101),
                str(" "),
                op(OperatorTokenType::BinAnd),
                str(" "),
                num(0xFF),
                op(OperatorTokenType::ParenClose),
                str(" "),
                op(OperatorTokenType::BinXor),
                str(" "),
                num(0xFF00),
                op(OperatorTokenType::ParenClose),
                str(" "),
                op(OperatorTokenType::ShiftLeft),
                str(" "),
                num(16),
                str(" "),
                op(OperatorTokenType::ShiftRight),
                str(" "),
                num(16),
                str(" "),
                op(OperatorTokenType::BinAnd),
                str(" "),
                op(OperatorTokenType::BinNot),
                op(OperatorTokenType::ParenOpen),
                num(0xFF),
                op(OperatorTokenType::ParenClose),
            ],
        );
        test(
            "10km/h * 45min in m",
            &[
                num(10),
                apply_to_prev_token_unit("km/h"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(45),
                apply_to_prev_token_unit("min"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("m"),
            ],
        );

        test(
            "45min in m",
            &[
                num(45),
                apply_to_prev_token_unit("min"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("m"),
            ],
        );

        test(
            "10(km/h)^2 * 45min in m",
            &[
                num(10),
                apply_to_prev_token_unit("(km/h)"),
                op(OperatorTokenType::Pow),
                num(2),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(45),
                apply_to_prev_token_unit("min"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("m"),
            ],
        );

        test(
            "1 (m*kg)/(s^2)",
            &[num(1), str(" "), apply_to_prev_token_unit("(m*kg)/(s^2)")],
        );

        // test("5kg*m/s^2", "5 (kg m) / s^2")

        test(
            "2m^2*kg/s^2",
            &[num(2), apply_to_prev_token_unit("m^2*kg/s^2")],
        );
        test(
            "2(m^2)*kg/s^2",
            &[num(2), apply_to_prev_token_unit("(m^2)*kg/s^2")],
        );

        // but it is allowed if they parenthesis are around
        test(
            "2(m^2 kg)/s^2",
            &[num(2), apply_to_prev_token_unit("(m^2 kg)/s^2")],
        );

        test(
            "2/3m",
            &[
                num(2),
                op(OperatorTokenType::Div),
                num(3),
                apply_to_prev_token_unit("m"),
            ],
        );

        test(
            "3 s^-1 * 4 s",
            &[
                num(3),
                str(" "),
                apply_to_prev_token_unit("s^-1"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(4),
                str(" "),
                apply_to_prev_token_unit("s"),
            ],
        );
    }

    #[test]
    fn test_explicit_multipl_is_mandatory_before_units() {
        test(
            "2m^4kg/s^3",
            &[
                num(2),
                apply_to_prev_token_unit("m^4"),
                unit("kg/s^3"), // here, it must be unit for now, calc or shunting can recognize that it is illegal and transform it to string
                                //op(OperatorTokenType::Div),
                                //unit("s^3"),
            ],
        );
        // this is the accepted form
        test(
            "2m^4*kg/s^3",
            &[num(2), apply_to_prev_token_unit("m^4*kg/s^3")],
        );
    }

    #[test]
    fn test_parsing_units_in_denom() {
        test(
            "30 years * 12/year",
            &[
                num(30),
                str(" "),
                apply_to_prev_token_unit("years"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(12),
                op(OperatorTokenType::Div),
                unit("year"),
            ],
        );
    }

    #[test]
    fn test_longer_texts() {
        test(
            "15 asd 75-15",
            &[
                num(15),
                str(" "),
                str("asd"),
                str(" "),
                num(75),
                op(OperatorTokenType::Sub),
                num(15),
            ],
        );

        test(
            "12km/h * 45s ^^",
            &[
                num(12),
                apply_to_prev_token_unit("km/h"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(45),
                apply_to_prev_token_unit("s"),
                str(" "),
                op(OperatorTokenType::Pow),
                op(OperatorTokenType::Pow),
            ],
        );
    }

    #[test]
    fn test_j_mol_k_parsing() {
        test(
            "(8.314 J / mol / K) ^ 0",
            &[
                op(OperatorTokenType::ParenOpen),
                numf(8.314),
                str(" "),
                apply_to_prev_token_unit("J / mol / K"),
                op(OperatorTokenType::ParenClose),
                str(" "),
                op(OperatorTokenType::Pow),
                str(" "),
                num(0),
            ],
        );
    }

    #[test]
    fn matrix_parsing() {
        // there are no empty matrices
        test(
            "[]",
            &[
                op(OperatorTokenType::BracketOpen),
                op(OperatorTokenType::BracketClose),
            ],
        );
        test(
            "[1]",
            &[
                op(OperatorTokenType::BracketOpen),
                num(1),
                op(OperatorTokenType::BracketClose),
            ],
        );

        test(
            "[1, 2]",
            &[
                op(OperatorTokenType::BracketOpen),
                num(1),
                op(OperatorTokenType::Comma),
                str(" "),
                num(2),
                op(OperatorTokenType::BracketClose),
            ],
        );

        test(
            "[1, 2; 3, 4]",
            &[
                op(OperatorTokenType::BracketOpen),
                num(1),
                op(OperatorTokenType::Comma),
                str(" "),
                num(2),
                op(OperatorTokenType::Semicolon),
                str(" "),
                num(3),
                op(OperatorTokenType::Comma),
                str(" "),
                num(4),
                op(OperatorTokenType::BracketClose),
            ],
        );

        // it becomes invalid during validation
        test(
            "[[1, 2], [3, 4]]",
            &[
                op(OperatorTokenType::BracketOpen),
                op(OperatorTokenType::BracketOpen),
                num(1),
                op(OperatorTokenType::Comma),
                str(" "),
                num(2),
                op(OperatorTokenType::BracketClose),
                op(OperatorTokenType::Comma),
                str(" "),
                op(OperatorTokenType::BracketOpen),
                num(3),
                op(OperatorTokenType::Comma),
                str(" "),
                num(4),
                op(OperatorTokenType::BracketClose),
                op(OperatorTokenType::BracketClose),
            ],
        );

        test(
            "[1, asda]",
            &[
                op(OperatorTokenType::BracketOpen),
                num(1),
                op(OperatorTokenType::Comma),
                str(" "),
                str("asda"),
                op(OperatorTokenType::BracketClose),
            ],
        );
    }

    #[test]
    fn exponential_notation() {
        test("2.3e-4", &[numf(2.3e-4f64)]);
        test("1.23e18", &[numf(1.23e18f64)]);

        // TODO rust_decimal's range is too small for this :(
        // test("1.23e50", &[numf(1.23e50f64)]);

        test("3 e", &[num(3), str(" "), str("e")]);
        test("3e", &[num(3), str("e")]);
        test("33e", &[num(33), str("e")]);
        test("3e3", &[num(3000)]);
        test(
            "3e--3",
            &[
                num(3),
                str("e"),
                op(OperatorTokenType::Sub),
                op(OperatorTokenType::Sub),
                num(3),
            ],
        );

        test("3e-3-", &[numf(3e-3f64), op(OperatorTokenType::Sub)]);
        // TODO: parse sign together with digits
        test(
            "-3e-3-",
            &[
                op(OperatorTokenType::Sub),
                numf(3e-3f64),
                op(OperatorTokenType::Sub),
            ],
        );
        // exp, binary and hex is not allowed in unit exponents
        // test(
        //     "3 kg^1.0e0 * m^1.0e0 * s^-2e0",
        //     // &[num(3), str(" "), unit("kg^1.0e0 * m^1.0e0 * s^-2e0")],
        // );

        // invalid input tests
        test("2.3e4e5", &[num(23000), str("e5")]);
        test("2.3e4.0e5", &[num(23000), numf(0e5f64)]);
    }

    #[test]
    fn test_dont_count_zeroes() {
        test("1k", &[num(1_000)]);
        test("2k", &[num(2_000)]);
        test("1k ", &[num(1_000), str(" ")]);
        test("2k ", &[num(2_000), str(" ")]);
        test("3k-2k", &[num(3000), op(OperatorTokenType::Sub), num(2000)]);
        test(
            "3k - 2k",
            &[
                num(3000),
                str(" "),
                op(OperatorTokenType::Sub),
                str(" "),
                num(2000),
            ],
        );

        test("1M", &[num(1_000_000)]);
        test("2M", &[num(2_000_000)]);
        test(
            "3M-2M",
            &[num(3_000_000), op(OperatorTokenType::Sub), num(2_000_000)],
        );

        test(
            "3M+1k",
            &[num(3_000_000), op(OperatorTokenType::Add), num(1_000)],
        );

        // missing digit
        test(
            "3M+k",
            &[num(3_000_000), op(OperatorTokenType::Add), str("k")],
        );
        test("2kalap", &[num(2), str("kalap")]);
    }

    #[test]
    fn test_that_strings_are_parsed_fully_so_b0_is_not_equal_to_b_and_0() {
        test_vars(
            &[&['b'], &['b', '0']],
            "b0 + 100",
            &[
                var("b0"),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                num(100),
            ],
        );

        test_vars(
            &[&['b'], &['b', '0']],
            "b = b0 + 100",
            &[
                var("b"),
                str(" "),
                op(OperatorTokenType::Assign),
                str(" "),
                var("b0"),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                num(100),
            ],
        );

        test_vars(
            &[&['b']],
            "1 + b(2)",
            &[
                num(1),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                str("b"),
                op(OperatorTokenType::ParenOpen),
                num(2),
                op(OperatorTokenType::ParenClose),
            ],
        );
    }

    #[test]
    fn test_variables() {
        test_vars(
            &[&['1', '2', ' ', 'a', 'l', 'm', 'a']],
            "3 + 12 alma",
            &[
                num(3),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                var("12 alma"),
            ],
        );

        test_vars(
            &[],
            "12 = 13",
            &[
                num(12),
                str(" "),
                op(OperatorTokenType::Assign),
                str(" "),
                num(13),
            ],
        );

        test_vars(
            &[&['v', 'a', 'r', '(', '1', '2', '*', '4', ')']],
            "13 * var(12*4)",
            &[
                num(13),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                var("var(12*4)"),
            ],
        );

        test_vars(
            &[&['&', '[', '1', ']']],
            "3 + &[1]",
            &[
                num(3),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                line_ref("&[1]"),
            ],
        );
    }

    #[test]
    fn test_line_ref_parsing() {
        test_vars(
            &[&['&', '[', '2', '1', ']']],
            "3 years * &[21]",
            &[
                num(3),
                str(" "),
                apply_to_prev_token_unit("years"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                line_ref("&[21]"),
            ],
        );

        // line refs requires space between them for readabality and avoiding confusion
        test_vars(
            &[&['&', '[', '2', '1', ']']],
            "3 years * &[21]&[21]",
            &[
                num(3),
                str(" "),
                apply_to_prev_token_unit("years"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                line_ref("&[21]"),
                str("&"),
                op(OperatorTokenType::BracketOpen),
                num(21),
                op(OperatorTokenType::BracketClose),
            ],
        );
    }

    #[test]
    fn test_unit_cancelling() {
        test(
            "1 km/m",
            &[num(1), str(" "), apply_to_prev_token_unit("km/m")],
        );
    }

    #[test]
    fn test_unit_parsing_latin_chars() {
        test("1 hónap", &[num(1), str(" "), str("hónap")]);
    }

    #[test]
    fn test_unit_in_denominator_tokens2() {
        test(
            "1/12/year",
            &[
                num(1),
                op(OperatorTokenType::Div),
                num(12),
                op(OperatorTokenType::Div),
                unit("year"),
            ],
        );
    }

    #[test]
    fn test_unit_in_denominator_tokens_with_parens() {
        test(
            "(12/year)",
            &[
                op(OperatorTokenType::ParenOpen),
                num(12),
                op(OperatorTokenType::Div),
                unit("year"),
                op(OperatorTokenType::ParenClose),
            ],
        );
    }

    #[test]
    fn test_fn_parsing() {
        test(
            "sin(60 degree)",
            &[
                str("sin"),
                op(OperatorTokenType::ParenOpen),
                num(60),
                str(" "),
                apply_to_prev_token_unit("degree"),
                op(OperatorTokenType::ParenClose),
            ],
        );
        test(
            "nth([5,6,7],1)",
            &[
                str("nth"),
                op(OperatorTokenType::ParenOpen),
                op(OperatorTokenType::BracketOpen),
                num(5),
                op(OperatorTokenType::Comma),
                num(6),
                op(OperatorTokenType::Comma),
                num(7),
                op(OperatorTokenType::BracketClose),
                op(OperatorTokenType::Comma),
                num(1),
                op(OperatorTokenType::ParenClose),
            ],
        );
    }

    #[test]
    fn test_multiple_equal_signs() {
        test(
            "z=1=2",
            &[
                str("z"),
                op(OperatorTokenType::Assign),
                num(1),
                op(OperatorTokenType::Assign),
                num(2),
            ],
        );
    }

    #[test]
    fn test_huge_number_no_panic() {
        test("017327229991661686687892454247286090975M", &[num_err()]);
    }

    #[test]
    fn test_huge_unit_exponent_no_panic() {
        test(
            "3T^81",
            &[num(3), str("T"), op(OperatorTokenType::Pow), num(81)],
        );
    }

    #[test]
    fn parsing_too_big_unit2() {
        test(
            "6K^61595",
            &[num(6), str("K"), op(OperatorTokenType::Pow), num(61595)],
        );
    }

    #[test]
    fn test_fuzzing_issue_1() {
        test(
            "90-J7qt799/9b^72u5KYD76O26w6^4f2z",
            &[
                num(90),
                op(OperatorTokenType::Sub),
                str("J7qt799"),
                op(OperatorTokenType::Div),
                num(9),
                apply_to_prev_token_unit("b^72"),
                str("u5KYD76O26w6"),
                op(OperatorTokenType::Pow),
                num(4),
                str("f2z"),
            ],
        );
    }

    #[test]
    fn test_huge_unit_number_no_panic() {
        test(
            "11822$^917533673846412864165166106750540",
            &[
                num(11822),
                apply_to_prev_token_unit("$"),
                op(OperatorTokenType::Pow),
                num_err(),
            ],
        );
    }

    #[test]
    fn test_parsing_too_big_unit_exponent() {
        test(
            "2S^42/T",
            &[
                num(2),
                str("S"),
                op(OperatorTokenType::Pow),
                num(42),
                op(OperatorTokenType::Div),
                unit("T"),
            ],
        );
    }

    #[test]
    fn test_comments() {
        test("//", &[str("//")]);
        test("//a", &[str("//a")]);
        test("// a", &[str("// a")]);

        test("// 1", &[str("// 1")]);
        test("// 1+2", &[str("// 1+2")]);

        test("a// 1+2", &[str("a"), str("// 1+2")]);

        test("1// 1+2", &[num(1), str("// 1+2")]);
        test(
            "1+2// 1+2",
            &[num(1), op(OperatorTokenType::Add), num(2), str("// 1+2")],
        );
    }

    #[test]
    fn test_header() {
        test("#", &[header("#")]);
        test("#a", &[header("#a")]);
        test("# a", &[header("# a")]);
        test("# 12 + 3", &[header("# 12 + 3")]);
        test("a#", &[str("a#")]);
        test(" #", &[str(" "), str("#")]);
        test(" #a", &[str(" "), str("#a")]);
        test(" # a", &[str(" "), str("#"), str(" "), str("a")]);
    }

    #[test]
    fn test_spaces_are_not_allowed_in_hex() {
        // e.g. 0xFF B
        // is it 0xFFB or 0xFF byte?
        test("0xAA BB", &[num(0xAA), str(" "), str("BB")]);
        test(
            "0xAA B",
            &[num(0xAA), str(" "), apply_to_prev_token_unit("B")],
        );
        test("0xAABB", &[num(0xAABB)]);
    }

    #[test]
    fn test_underscore_is_allowed_in_hex() {
        test("0xAA_B", &[num(0xAAB)]);
        test("0xAA_BB", &[num(0xAABB)]);
        test("0xA_A_B", &[num(0xAAB)]);
        test("0x_AAB_", &[num(0xAAB), str("_")]);
        test("0x_A_A_B_", &[num(0xAAB), str("_")]);
        test(
            "0xAA_B B",
            &[num(0xAAB), str(" "), apply_to_prev_token_unit("B")],
        );
    }

    #[test]
    fn test_underscore_is_allowed_in_bin() {
        test("0b11_0", &[num(0b110)]);
        test("0b11_00", &[num(0b1100)]);
        test("0b1_1_0", &[num(0b110)]);
        test("0b_110_", &[num(0b110), str("_")]);
        test("0b_1_1_0_", &[num(0b110), str("_")]);
        test("0b11_0 0", &[num(0b1100)]);
    }

    #[test]
    fn test_parsing_u64_hex() {
        test("0xFFFFFFFFFFFFFFFF", &[num(0xFFFFFFFFFFFFFFFF)]);
    }

    #[test]
    fn test_parsing_u64_dec() {
        test(
            "18 446 744 073 709 551 615",
            &[num(18_446_744_073_709_551_615)],
        );
    }

    #[test]
    fn test_unit_after_lineref_is_allowed() {
        test_vars(
            &[&['&', '[', '1', ']']],
            "&[1] m",
            &[line_ref("&[1]"), str(" "), apply_to_prev_token_unit("m")],
        );
    }

    #[test]
    fn test_unit_after_var_is_allowed() {
        test_vars(
            &[&['v', 'a', 'r']],
            "var m",
            &[var("var"), str(" "), apply_to_prev_token_unit("m")],
        );
    }

    #[test]
    fn not_in_must_be_str_if_we_are_sure_it_cant_be_unit() {
        test(
            "12 m in",
            &[
                num(12),
                str(" "),
                apply_to_prev_token_unit("m"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
            ],
        );
    }

    #[test]
    fn test_parse_ininin() {
        test(
            "12 in in in",
            &[
                num(12),
                str(" "),
                apply_to_prev_token_unit("in"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("in"),
            ],
        );
    }

    #[test]
    fn test_parse_longer_texts2() {
        test(
            "transfer of around 1.587GB in about / 3 seconds",
            &[
                str("transfer"),
                str(" "),
                str("of"),
                str(" "),
                str("around"),
                str(" "),
                numf(1.587),
                apply_to_prev_token_unit("GB"),
                str(" "),
                // TODO: currently it is allowed it here, shunting will demote it to String
                op(OperatorTokenType::UnitConverter),
                str(" "),
                str("about"),
                str(" "),
                op(OperatorTokenType::Div),
                str(" "),
                num(3),
                str(" "),
                apply_to_prev_token_unit("seconds"),
            ],
        );
    }

    #[test]
    fn test_parse_num_perc_on_what_is() {
        test(
            "41 is 17% on what",
            &[
                num(41),
                str(" "),
                op(OperatorTokenType::PercentageIs),
                str(" "),
                num(17),
                op(OperatorTokenType::Perc),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Base_From_Result_Increase_X),
            ],
        );
    }

    #[test]
    fn test_parse_num_perc_on_what_is_paren() {
        test(
            "(41 is 17% on what)",
            &[
                op(OperatorTokenType::ParenOpen),
                num(41),
                str(" "),
                op(OperatorTokenType::PercentageIs),
                str(" "),
                num(17),
                op(OperatorTokenType::Perc),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Base_From_Result_Increase_X),
                op(OperatorTokenType::ParenClose),
            ],
        );
    }

    #[test]
    fn test_parse_percentage_what_plus() {
        test(
            "what plus 17% is 41",
            &[
                op(OperatorTokenType::Percentage_Find_Base_From_X_Icrease_Result),
                str(" "),
                num(17),
                op(OperatorTokenType::Perc),
                str(" "),
                op(OperatorTokenType::PercentageIs),
                str(" "),
                num(41),
            ],
        );
    }

    #[test]
    fn test_parse_percentage_on_what_is() {
        test(
            "17% on what is 41",
            &[
                num(17),
                op(OperatorTokenType::Perc),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Base_From_Icrease_X_Result),
                str(" "),
                num(41),
            ],
        );
    }

    #[test]
    fn test_parse_num_what_perc_on_num() {
        test(
            "41 is what % on 35",
            &[
                num(41),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Incr_Rate_From_Result_X_Base),
                str(" "),
                num(35),
            ],
        );
    }

    #[test]
    fn test_parse_num_perc_off_what_is() {
        test(
            "41 is 17% off what",
            &[
                num(41),
                str(" "),
                op(OperatorTokenType::PercentageIs),
                str(" "),
                num(17),
                op(OperatorTokenType::Perc),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Base_From_Result_Decrease_X),
            ],
        );
    }

    #[test]
    fn test_parse_num_perc_off_what_is_paren() {
        test(
            "(41 is 17% off what)",
            &[
                op(OperatorTokenType::ParenOpen),
                num(41),
                str(" "),
                op(OperatorTokenType::PercentageIs),
                str(" "),
                num(17),
                op(OperatorTokenType::Perc),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Base_From_Result_Decrease_X),
                op(OperatorTokenType::ParenClose),
            ],
        );
    }

    #[test]
    fn test_parse_percentage_what_minus() {
        test(
            "what minus 17% is 41",
            &[
                op(OperatorTokenType::Percentage_Find_Base_From_X_Decrease_Result),
                str(" "),
                num(17),
                op(OperatorTokenType::Perc),
                str(" "),
                op(OperatorTokenType::PercentageIs),
                str(" "),
                num(41),
            ],
        );
    }

    #[test]
    fn test_parse_percentage_off_what_is() {
        test(
            "17% off what is 41",
            &[
                num(17),
                op(OperatorTokenType::Perc),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Base_From_Decrease_X_Result),
                str(" "),
                num(41),
            ],
        );
    }

    #[test]
    fn test_parse_num_what_perc_off_num() {
        test(
            "41 is what % off 35",
            &[
                num(41),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Decr_Rate_From_Result_X_Base),
                str(" "),
                num(35),
            ],
        );
    }

    #[test]
    fn test_parse_percentage_find_rate_from_result_base() {
        test(
            "20 is what percent of 60",
            &[
                num(20),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Rate_From_Result_Base),
                str(" "),
                num(60),
            ],
        );
    }

    #[test]
    fn test_parse_percentage_find_base_from_result_rate() {
        test(
            "5 is 25% of what",
            &[
                num(5),
                str(" "),
                op(OperatorTokenType::PercentageIs),
                str(" "),
                num(25),
                op(OperatorTokenType::Perc),
                str(" "),
                op(OperatorTokenType::Percentage_Find_Base_From_Result_Rate),
            ],
        );
    }

    #[test]
    fn test_unit_conversion_26() {
        test(
            "(256byte * 120) in MiB",
            &[
                op(OperatorTokenType::ParenOpen),
                num(256),
                apply_to_prev_token_unit("byte"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(120),
                op(OperatorTokenType::ParenClose),
                str(" "),
                apply_to_prev_token_unit("in"),
                str(" "),
                unit("MiB"),
            ],
        );
    }

    #[test]
    fn test_fuzz_bug_201220() {
        test(
            ")5)t[Mr/(K)",
            &[
                op(OperatorTokenType::ParenClose),
                num(5),
                op(OperatorTokenType::ParenClose),
                apply_to_prev_token_unit("t"),
                op(OperatorTokenType::BracketOpen),
                str("Mr"),
                op(OperatorTokenType::Div),
                unit("(K)"),
            ],
        );
    }
}
