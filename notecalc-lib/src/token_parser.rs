use crate::units::units::{UnitOutput, Units};
use bigdecimal::{BigDecimal, Num};
use std::ops::Neg;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum TokenType<'units> {
    // UnitOfMeasure(&'text_pr [char], UnitOutput<'units>),
    StringLiteral,
    Variable,
    NumberLiteral(BigDecimal),
    Operator(OperatorTokenType<'units>),
}

#[derive(Debug, Clone)]
pub struct Token<'text_ptr, 'units> {
    pub ptr: &'text_ptr [char],
    pub typ: TokenType<'units>,
}

impl<'text_ptr, 'units> Token<'text_ptr, 'units> {
    pub fn is_number(&self) -> bool {
        matches!(self.typ, TokenType::NumberLiteral(..))
    }

    pub fn is_string(&self) -> bool {
        matches!(self.typ, TokenType::StringLiteral)
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum OperatorTokenType<'units> {
    Comma,
    Add,
    UnaryPlus,
    Sub,
    UnaryMinus,
    Mult,
    Div,
    Perc,
    And,
    Or,
    Xor,
    Not,
    Pow,
    ParenOpen,
    ParenClose,
    BracketOpen,
    BracketClose,
    ShiftLeft,
    ShiftRight,
    Assign,
    UnitConverter,
    Matrix { arg_count: usize },
    Unit(UnitOutput<'units>),
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum Assoc {
    Left,
    Right,
}

impl<'a> OperatorTokenType<'a> {
    pub fn precedence(&self) -> usize {
        match self {
            OperatorTokenType::Add => 2,
            OperatorTokenType::UnaryPlus => 4,
            OperatorTokenType::Sub => 2,
            OperatorTokenType::UnaryMinus => 4,
            OperatorTokenType::Mult => 3,
            OperatorTokenType::Div => 3,
            OperatorTokenType::Perc => 6,
            OperatorTokenType::And => 0,
            OperatorTokenType::Or => 0,
            OperatorTokenType::Xor => 0,
            OperatorTokenType::Not => 0,
            OperatorTokenType::Pow => 5,
            OperatorTokenType::ParenOpen => 0,
            OperatorTokenType::ParenClose => 0,
            OperatorTokenType::ShiftLeft => 0,
            OperatorTokenType::ShiftRight => 0,
            OperatorTokenType::Assign => 0,
            OperatorTokenType::UnitConverter => 0,
            OperatorTokenType::Unit(_) => 4,
            OperatorTokenType::Comma => 0,
            OperatorTokenType::BracketOpen => 0,
            OperatorTokenType::BracketClose => 0,
            OperatorTokenType::Matrix { .. } => 0,
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
            OperatorTokenType::And => Assoc::Left,
            OperatorTokenType::Or => Assoc::Left,
            OperatorTokenType::Xor => Assoc::Left,
            OperatorTokenType::Not => Assoc::Left,
            OperatorTokenType::Pow => Assoc::Right,
            OperatorTokenType::ParenOpen => Assoc::Left,
            OperatorTokenType::ShiftLeft => Assoc::Left,
            OperatorTokenType::ShiftRight => Assoc::Left,
            OperatorTokenType::Assign => Assoc::Left,
            OperatorTokenType::UnitConverter => Assoc::Left,
            OperatorTokenType::Unit(_) => Assoc::Left,
            // Right, so 1 comma won't replace an other on the operator stack
            OperatorTokenType::Comma => Assoc::Right,
            OperatorTokenType::BracketOpen => Assoc::Left,
            OperatorTokenType::BracketClose => Assoc::Left,
            OperatorTokenType::Matrix { .. } => Assoc::Left,
        }
    }
}

pub struct TokenParser {}

impl TokenParser {
    pub fn parse_line<'text_ptr, 'units>(
        line: &'text_ptr [char],
        variable_names: &[String],
        function_names: &[&str],
        // dst: &'text_ptr mut Vec<Token<'text_ptr, 'units>>,
        dst: &mut Vec<Token<'text_ptr, 'units>>,
        units: &'units Units,
    ) {
        // let text = text.trim();
        // sort them
        // val sortedVariableNames = variableNames.sortedByDescending { it.length }
        // val sortedFunctionNames = functionNames.sortedByDescending { it.length }
        let mut index = 0;
        let mut can_be_unit = false;
        while index < line.len() {
            if let Some(token) = TokenParser::try_extract_unit(&line[index..], units, can_be_unit)
                .or_else(|| {
                    TokenParser::try_extract_operator(&line[index..]).or_else(|| {
                        TokenParser::try_extract_number_literal(&line[index..])
                            .map(|it| Token {
                                typ: TokenType::NumberLiteral(it.0),
                                ptr: it.1,
                            })
                            .or_else(|| {
                                TokenParser::try_extract_variable_name(
                                    &line[index..],
                                    variable_names,
                                )
                                .or_else(|| TokenParser::try_extract_string_literal(&line[index..]))
                            })
                    })
                })
            {
                match &token.typ {
                    // Token::UnitOfMeasure(ptr, ..) => {
                    //     can_be_unit = false;
                    //     index += ptr.len()
                    // }
                    TokenType::StringLiteral => {
                        if token.ptr[0].is_ascii_whitespace() {
                            // keep can_be_unit as it was
                        } else {
                            can_be_unit = false;
                        }
                        index += token.ptr.len()
                    }
                    TokenType::NumberLiteral(..) => {
                        can_be_unit = true;
                        index += token.ptr.len()
                    }
                    TokenType::Operator(typ) => {
                        can_be_unit = matches!(typ, OperatorTokenType::ParenClose)
                            || matches!(typ, OperatorTokenType::UnitConverter);
                        index += token.ptr.len()
                    }
                    TokenType::Variable => {
                        can_be_unit = true;
                        index += token.ptr.len()
                    }
                }
                dst.push(token);
            } else {
                break;
            }
        }
    }

    pub fn try_extract_number_literal<'text_ptr>(
        str: &'text_ptr [char],
    ) -> Option<(BigDecimal, &'text_ptr [char])> {
        let mut number_str = [b'0'; 32];
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

        if str[i..].starts_with(&['0', 'b']) {
            i += 2;
            let mut end_index_before_last_whitespace = i;
            while i < str.len() {
                if str[i] == '0' || str[i] == '1' {
                    end_index_before_last_whitespace = i + 1;
                    number_str[number_str_index] = str[i] as u8;
                    number_str_index += 1;
                } else if str[i].is_ascii_whitespace() {
                    // allowed
                } else {
                    break;
                }
                i += 1;
            }
            i = end_index_before_last_whitespace;
            if i > 2 {
                // bigdecimal cannot parse binary, that's why the explicit i64 type
                let num: i64 = Num::from_str_radix(
                    &unsafe { std::str::from_utf8_unchecked(&number_str[0..number_str_index]) },
                    2,
                )
                .ok()?;
                Some((num.into(), &str[0..i]))
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
                        .map(|it| {
                            it.is_ascii_hexdigit()
                                || it.is_ascii_whitespace()
                                || !it.is_alphabetic()
                        })
                        .unwrap_or(true)
                {
                    end_index_before_last_whitespace = i + 1;
                    number_str[number_str_index] = str[i] as u8;
                    number_str_index += 1;
                } else if str[i].is_ascii_whitespace() {
                    // allowed
                } else {
                    break;
                }
                i += 1;
            }
            i = end_index_before_last_whitespace;
            if i > 2 {
                // bigdecimal cannot parse hex, that's why the explicit i64 type
                let num: i64 = Num::from_str_radix(
                    &unsafe { std::str::from_utf8_unchecked(&number_str[0..number_str_index]) },
                    16,
                )
                .ok()?;
                Some((num.into(), &str[0..i]))
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
                let num = BigDecimal::from_str(&unsafe {
                    std::str::from_utf8_unchecked(&number_str[0..number_str_index])
                })
                .ok()?;
                Some((
                    multiplier
                        .map(|it| BigDecimal::from(it) * &num)
                        .unwrap_or(num),
                    &str[0..i],
                ))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn try_extract_unit<'text_ptr, 'unit>(
        str: &'text_ptr [char],
        unit: &'unit Units,
        can_be_unit: bool,
    ) -> Option<Token<'text_ptr, 'unit>> {
        if !can_be_unit || str[0].is_ascii_whitespace() {
            return None;
        }
        let (unit, parsed_len) = unit.parse(str);
        return if parsed_len == 0 {
            None
        } else {
            // remove trailing spaces
            let mut i = parsed_len;
            while i > 0 && str[i - 1].is_ascii_whitespace() {
                i -= 1;
            }
            Some(Token {
                typ: TokenType::Operator(OperatorTokenType::Unit(unit)),
                ptr: &str[0..i],
            })
        };
    }

    fn try_extract_variable_name<'text_ptr, 'units>(
        str: &'text_ptr [char],
        variable_names: &[String],
    ) -> Option<Token<'text_ptr, 'units>> {
        'asd: for var_name in variable_names {
            for (i, ch) in var_name.chars().enumerate() {
                if i >= str.len() || str[i] != ch {
                    continue 'asd;
                }
            }
            return Some(Token {
                typ: TokenType::Variable,
                ptr: &str[0..var_name.chars().count()],
            });
        }
        return None;
    }

    fn try_extract_string_literal<'text_ptr, 'unit>(
        str: &'text_ptr [char],
    ) -> Option<Token<'text_ptr, 'unit>> {
        let mut i = 0;
        for ch in str {
            if "=%/+-*^()[]".chars().any(|it| it == *ch) || ch.is_ascii_whitespace() {
                break;
            }
            i += 1;
        }
        if i > 0 {
            // alphabetical literal
            return Some(Token {
                typ: TokenType::StringLiteral,
                ptr: &str[0..i],
            });
        } else {
            for ch in &str[0..] {
                if !ch.is_ascii_whitespace() {
                    break;
                }
                i += 1;
            }
            return if i > 0 {
                // whitespace
                Some(Token {
                    typ: TokenType::StringLiteral,
                    ptr: &str[0..i],
                })
            } else {
                None
            };
        }
    }

    fn try_extract_operator<'text_ptr, 'unit>(
        str: &'text_ptr [char],
    ) -> Option<Token<'text_ptr, 'unit>> {
        fn op<'text_ptr, 'unit>(
            typ: OperatorTokenType<'unit>,
            str: &'text_ptr [char],
            len: usize,
        ) -> Option<Token<'text_ptr, 'unit>> {
            return Some(Token {
                typ: TokenType::Operator(typ),
                ptr: &str[0..len],
            });
        }
        match str[0] {
            '=' => op(OperatorTokenType::Assign, str, 1),
            '+' => op(OperatorTokenType::Add, str, 1),
            '-' => op(OperatorTokenType::Sub, str, 1),
            '*' => op(OperatorTokenType::Mult, str, 1),
            '/' => op(OperatorTokenType::Div, str, 1),
            '%' => op(OperatorTokenType::Perc, str, 1),
            '^' => op(OperatorTokenType::Pow, str, 1),
            '(' => op(OperatorTokenType::ParenOpen, str, 1),
            ')' => op(OperatorTokenType::ParenClose, str, 1),
            '[' => op(OperatorTokenType::BracketOpen, str, 1),
            ']' => op(OperatorTokenType::BracketClose, str, 1),
            ',' => op(OperatorTokenType::Comma, str, 1),
            _ => {
                if str.starts_with(&['t', 'o', ' ']) {
                    op(OperatorTokenType::UnitConverter, str, 2)
                } else if str.starts_with(&['A', 'N', 'D'])
                    && str.get(3).map(|it| !it.is_alphabetic()).unwrap_or(true)
                {
                    // TODO unit test "0xff and(12)"
                    op(OperatorTokenType::And, str, 3)
                } else if str.starts_with(&['O', 'R'])
                    && str.get(2).map(|it| !it.is_alphabetic()).unwrap_or(true)
                {
                    op(OperatorTokenType::Or, str, 2)
                } else if str.starts_with(&['N', 'O', 'T', '(']) {
                    op(OperatorTokenType::Not, str, 3)
                // '(' will be parsed separately as an operator
                } else if str.starts_with(&['X', 'O', 'R'])
                    && str.get(3).map(|it| !it.is_alphabetic()).unwrap_or(true)
                {
                    op(OperatorTokenType::Xor, str, 3)
                } else if str.starts_with(&['<', '<']) {
                    op(OperatorTokenType::ShiftLeft, str, 2)
                } else if str.starts_with(&['>', '>']) {
                    op(OperatorTokenType::ShiftRight, str, 2)
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Eq, PartialEq)]
enum ValidationTokenType {
    Nothing,
    Expr,
    Op,
}

fn handle_unary_shit() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shunting_yard::tests::*;
    use crate::units::consts::{create_prefixes, init_units};
    use crate::units::units::Units;

    #[test]
    fn test_number_parsing() {
        fn test_parse(str: &str, expected_value: u64) {
            let mut vec = vec![];
            let temp = str.chars().collect::<Vec<_>>();
            let prefixes = create_prefixes();
            let units = Units::new(&prefixes);
            TokenParser::parse_line(&temp, &[], &[], &mut vec, &units);
            match vec.get(0) {
                Some(Token {
                    ptr,
                    typ: TokenType::NumberLiteral(num),
                }) => {
                    assert_eq!(*num, expected_value.into());
                }
                _ => panic!("'{}' failed", str),
            }
            println!("{} OK", str);
        }

        fn test_parse_f(str: &str, expected_value: f64) {
            let mut vec = vec![];
            let temp = str.chars().collect::<Vec<_>>();
            let prefixes = create_prefixes();
            let units = Units::new(&prefixes);
            TokenParser::parse_line(&temp, &[], &[], &mut vec, &units);
            match vec.get(0) {
                Some(Token {
                    ptr,
                    typ: TokenType::NumberLiteral(num),
                }) => {
                    assert_eq!(BigDecimal::from(expected_value), *num);
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
        test_parse("0xAB Cd e    f", 11_259_375);

        test_parse("1", 1);
        test_parse("123456", 123456);
        test_parse("12 34 5        6", 123456);
        test_parse_f("123.456", 123.456);

        test_parse_f("0.1", 0.1);
        test_parse_f(".1", 0.1);
        test_parse_f(".1.", 0.1);
        test_parse_f("123.456.", 123.456);
        // it means 2 numbers, 123.456 and 0.3
        test_parse_f("123.456.3", 123.456);
    }

    fn test(text: &str, expected_tokens: &[Token]) {
        println!("{}", text);
        let mut vec = vec![];
        let temp = text.chars().collect::<Vec<_>>();
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);
        TokenParser::parse_line(&temp, &[], &[], &mut vec, &units);
        assert_eq!(
            expected_tokens.len(),
            vec.len(),
            "actual tokens:\n {:?}",
            vec.iter()
                .map(|it| format!("{:?}\n", it))
                .collect::<Vec<_>>()
                .join(" -----> ")
        );
        for (actual_token, expected_token) in vec.iter().zip(expected_tokens.iter()) {
            match (&expected_token.typ, &actual_token.typ) {
                (TokenType::NumberLiteral(expected_num), TokenType::NumberLiteral(actual_num)) => {
                    assert_eq!(expected_num, actual_num)
                }
                (
                    TokenType::Operator(OperatorTokenType::Unit(_)),
                    TokenType::Operator(OperatorTokenType::Unit(_)),
                ) => {
                    //     expected_op is an &str
                    let str_slice = unsafe { std::mem::transmute::<_, &str>(expected_token.ptr) };
                    let expected_chars = str_slice.chars().collect::<Vec<char>>();
                    assert_eq!(expected_chars.as_slice(), actual_token.ptr)
                }
                (TokenType::Operator(etyp), TokenType::Operator(atyp)) => assert_eq!(etyp, atyp),
                (TokenType::StringLiteral, TokenType::StringLiteral) => {
                    // expected_op is an &str
                    let str_slice = unsafe { std::mem::transmute::<_, &str>(expected_token.ptr) };
                    let expected_chars = str_slice.chars().collect::<Vec<char>>();
                    assert_eq!(expected_chars.as_slice(), actual_token.ptr)
                }
                // (Token::UnitOfMeasure(expected_op, ..), Token::UnitOfMeasure(actual_op, ..)) => {

                // }
                _ => panic!(
                    "'{}', {:?} != {:?}, actual tokens:\n {:?}",
                    text,
                    expected_token,
                    actual_token,
                    vec.iter()
                        .map(|it| format!("{:?}\n", it))
                        .collect::<Vec<_>>()
                        .join(" -----> ")
                ),
            }
        }
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
                unit("kg"),
                str(" "),
                str("alma"),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                num(300),
                str(" "),
                unit("kg"),
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
            &[num(1), op(OperatorTokenType::Div), num(2), unit("s")],
        );
        test(
            "0xFF AND 0b11",
            &[
                num(0xFF),
                str(" "),
                op(OperatorTokenType::And),
                str(" "),
                num(0b11),
            ],
        );

        test(
            "0xFF AND",
            &[num(0xff), str(" "), op(OperatorTokenType::And)],
        );
        test("0xFF OR", &[num(0xff), str(" "), op(OperatorTokenType::Or)]);
        test(
            "0xFF XOR",
            &[num(0xff), str(" "), op(OperatorTokenType::Xor)],
        );

        test(
            "((0b00101 AND 0xFF) XOR 0xFF00) << 16 >> 16  NOT(0xFF)",
            &[
                op(OperatorTokenType::ParenOpen),
                op(OperatorTokenType::ParenOpen),
                num(0b00101),
                str(" "),
                op(OperatorTokenType::And),
                str(" "),
                num(0xFF),
                op(OperatorTokenType::ParenClose),
                str(" "),
                op(OperatorTokenType::Xor),
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
                str("  "),
                op(OperatorTokenType::Not),
                op(OperatorTokenType::ParenOpen),
                num(0xFF),
                op(OperatorTokenType::ParenClose),
            ],
        );
        test(
            "10km/h * 45min to m",
            &[
                num(10),
                unit("km/h"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(45),
                unit("min"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("m"),
            ],
        );

        test(
            "45min to m",
            &[
                num(45),
                unit("min"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("m"),
            ],
        );

        test(
            "10(km/h)^2 * 45min to m",
            &[
                num(10),
                unit("(km/h)"),
                op(OperatorTokenType::Pow),
                num(2),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(45),
                unit("min"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("m"),
            ],
        );

        test("1 (m*kg)/(s^2)", &[num(1), str(" "), unit("(m*kg)/(s^2)")]);

        // explicit multiplication is mandatory before units
        test(
            "2m^4kg/s^3",
            &[
                num(2),
                unit("m^4"),
                str("kg"),
                op(OperatorTokenType::Div),
                str("s"),
                op(OperatorTokenType::Pow),
                num(3),
            ],
        );

        // test("5kg*m/s^2", "5 (kg m) / s^2")

        test("2m^2*kg/s^2", &[num(2), unit("m^2*kg/s^2")]);
        test("2(m^2)*kg/s^2", &[num(2), unit("(m^2)*kg/s^2")]);

        // but it is allowed if they parenthesis are around
        test("2(m^2 kg)/s^2", &[num(2), unit("(m^2 kg)/s^2")]);

        test(
            "2/3m",
            &[num(2), op(OperatorTokenType::Div), num(3), unit("m")],
        );

        test(
            "3 s^-1 * 4 s",
            &[
                num(3),
                str(" "),
                unit("s^-1"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(4),
                str(" "),
                unit("s"),
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
                unit("km/h"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(45),
                unit("s"),
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
                unit("J / mol / K"),
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
        test("1.23e50", &[numf(1.23e50f64)]);
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
        // TODO: wait for variables
        // test(
        //     "b = b0 + 100",
        //     &[
        //         str("b"),
        //         str(" "),
        //         // TODO: when we have variable names, it will be op not string
        //         // op(OperatorTokenType::Assign),
        //         str("="),
        //         str(" "),
        //         str("b0"),
        //         str(" "),
        //         // todo same
        //         // op(OperatorTokenType::Add),
        //         str("+"),
        //         str(" "),
        //         num(100),
        //     ],
        // );
    }
}