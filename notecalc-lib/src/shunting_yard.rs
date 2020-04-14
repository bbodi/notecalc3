use crate::token_parser::{Assoc, OperatorTokenType, Token, TokenType};
use bigdecimal::*;
use std::ops::Neg;

#[derive(Eq, PartialEq, Debug)]
enum ValidationTokenType {
    Nothing,
    Expr,
    Op,
}

#[derive(Eq, PartialEq, Debug)]
struct ValidationState {
    expect_expression: bool,
    had_operator: bool,
    open_brackets: usize,
    open_parens: usize,
    prev_token_type: ValidationTokenType,
    tmp_output_stack_start_index: usize,
    tmp_input_token_start_index: usize,
    neg: bool,
}

impl ValidationState {
    fn new_from_index(output_stack_index: usize, token_index: isize) -> ValidationState {
        ValidationState {
            expect_expression: true,
            open_brackets: 0,
            open_parens: 0,
            prev_token_type: ValidationTokenType::Nothing,
            tmp_output_stack_start_index: output_stack_index,
            tmp_input_token_start_index: token_index as usize,
            had_operator: false,
            neg: false,
        }
    }

    fn can_be_valid_closing_token(&self) -> bool {
        self.open_brackets == 0 && self.open_parens == 0 && self.had_operator
    }
}

pub struct ShuntingYard {}

impl ShuntingYard {
    pub fn shunting_yard<'text_ptr, 'units>(
        tokens: &mut Vec<Token<'text_ptr, 'units>>,
        function_names: &[String],
        output_stack: &mut Vec<TokenType<'units>>,
    ) {
        // TODO: into iter!!!
        // TODO extract out so no alloc SmallVec?
        let mut operator_stack: Vec<OperatorTokenType> = vec![];

        let mut last_valid_range = None;
        let mut v = ValidationState::new_from_index(0, 0);
        let mut input_index: isize = -1;

        let mut matrix_start_text_pos = None;

        dbg!(&tokens);
        while input_index + 1 < tokens.len() as isize {
            input_index += 1; // it is here so it is incremented always when "continue"
            let input_token = &tokens[input_index as usize];
            dbg!(&input_token);
            match &input_token.typ {
                TokenType::StringLiteral => {
                    // it ignores strings
                    if false {
                        // if function_names.iter().any(|it| it == str) {
                        //  ShuntingYardStacks((operatorStack + Token.Operator("fun " + inputToken.str)), output + inputToken)
                    } else {
                        // ignore it
                        // output.push(input_token.clone());
                    }
                }
                TokenType::Operator(op) => match op {
                    OperatorTokenType::ParenOpen => {
                        operator_stack.push(op.clone());
                        v.open_parens += 1;
                        v.prev_token_type = ValidationTokenType::Nothing;
                    }
                    OperatorTokenType::ParenClose => {
                        if v.expect_expression || v.open_parens == 0 {
                            dbg!("error1");
                            operator_stack.clear();
                            ShuntingYard::set_tokens_to_string(tokens, input_index, &v);
                            v = ValidationState::new_from_index(output_stack.len(), input_index);
                            continue;
                        } else {
                            v.expect_expression = false;
                            v.open_parens -= 1;
                            v.prev_token_type = ValidationTokenType::Expr;
                        }
                        let _ = ShuntingYard::send_anything_until_opening_bracket(
                            &mut operator_stack,
                            output_stack,
                            &OperatorTokenType::ParenOpen,
                            0,
                        );
                        if v.can_be_valid_closing_token() {
                            dbg!("close");
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            last_valid_range =
                                Some((v.tmp_output_stack_start_index, output_stack.len() - 1));
                        }
                    }
                    OperatorTokenType::BracketOpen => {
                        v.open_brackets += 1;
                        v.prev_token_type = ValidationTokenType::Nothing;
                        operator_stack.push(op.clone());
                        if matrix_start_text_pos.is_none() {
                            matrix_start_text_pos = Some(input_token.ptr);
                        }
                    }
                    OperatorTokenType::BracketClose => {
                        if v.expect_expression || v.open_brackets <= 0 {
                            dbg!("error2");
                            operator_stack.clear();
                            ShuntingYard::set_tokens_to_string(tokens, input_index, &v);
                            v = ValidationState::new_from_index(output_stack.len(), input_index);
                            continue;
                        } else {
                            v.expect_expression = false;
                            v.open_brackets -= 1;
                            v.prev_token_type = ValidationTokenType::Expr;
                        }
                        // todo számmal térjen vissza ne tokentypeal..
                        if let Some(comma_count) = ShuntingYard::send_anything_until_opening_bracket(
                            &mut operator_stack,
                            output_stack,
                            &OperatorTokenType::BracketOpen,
                            0,
                        ) {
                            let matrix_token_type =
                                TokenType::Operator(OperatorTokenType::Matrix {
                                    arg_count: comma_count + 1,
                                });
                            output_stack.push(matrix_token_type.clone());

                            if v.can_be_valid_closing_token() {
                                dbg!("close");
                                // Matrix will point to the whole output,
                                // so we can highlight only the first/last char if we want
                                // (the brackets)
                                unsafe {
                                    let char_count = input_token
                                        .ptr
                                        .as_ptr()
                                        .offset_from(matrix_start_text_pos.unwrap().as_ptr())
                                        as usize;
                                    tokens.insert(
                                        0,
                                        Token {
                                            ptr: std::mem::transmute(
                                                std::ptr::slice_from_raw_parts(
                                                    matrix_start_text_pos.unwrap().as_ptr(),
                                                    char_count + 1,
                                                ),
                                            ),
                                            typ: matrix_token_type.clone(),
                                        },
                                    );
                                    matrix_start_text_pos = None;
                                }
                                ShuntingYard::send_everything_to_output(
                                    &mut operator_stack,
                                    output_stack,
                                );
                                last_valid_range =
                                    Some((v.tmp_output_stack_start_index, output_stack.len() - 1));
                            }
                        } else {
                            panic!()
                        }
                    }
                    OperatorTokenType::Sub => {
                        if v.prev_token_type == ValidationTokenType::Nothing
                            || v.prev_token_type == ValidationTokenType::Op
                        {
                            // it is a unary op

                            if !v.expect_expression {
                                dbg!("error3");
                                operator_stack.clear();
                                ShuntingYard::set_tokens_to_string(tokens, input_index, &v);
                                v = ValidationState::new_from_index(
                                    output_stack.len(),
                                    input_index,
                                );
                                continue;
                            } else if ShuntingYard::get_next_nonstring_token(
                                tokens,
                                input_index as usize + 1,
                            )
                            .map(|it| it.0.is_number())
                            .unwrap_or(false)
                            {
                                v.neg = true;
                            } else {
                                // process it as a unary op
                                operator_stack.push(OperatorTokenType::UnaryMinus);
                            }
                        } else {
                            ShuntingYard::operator_rule(op, &mut operator_stack, output_stack);
                            operator_stack.push(op.clone());
                            v.expect_expression = true;
                            v.had_operator = true;
                            v.prev_token_type = ValidationTokenType::Op;
                        }
                    }
                    OperatorTokenType::Add => {
                        if v.prev_token_type == ValidationTokenType::Nothing
                            || v.prev_token_type == ValidationTokenType::Op
                        {
                            // it is a unary op

                            if !v.expect_expression {
                                dbg!("error4");
                                operator_stack.clear();
                                ShuntingYard::set_tokens_to_string(tokens, input_index, &v);
                                v = ValidationState::new_from_index(
                                    output_stack.len(),
                                    input_index,
                                );
                                continue;
                            } else if ShuntingYard::get_next_nonstring_token(
                                tokens,
                                input_index as usize + 1,
                            )
                            .map(|it| it.0.is_number())
                            .unwrap_or(false)
                            {
                                v.neg = false;
                            }
                        } else {
                            ShuntingYard::operator_rule(op, &mut operator_stack, output_stack);
                            operator_stack.push(op.clone());
                            v.expect_expression = true;
                            v.had_operator = true;
                            v.prev_token_type = ValidationTokenType::Op;
                        }
                    }
                    OperatorTokenType::Assign => {
                        // the left side of the '=' might be a variable name like 'km' or 'm'
                        // don't put '=' on the stack and remove the lhs of '=' from it
                        output_stack.pop();
                    }
                    OperatorTokenType::Comma => {
                        if v.open_brackets == 0 && v.open_parens == 0 {
                            dbg!("error5");
                            operator_stack.clear();
                            ShuntingYard::set_tokens_to_string(tokens, input_index, &v);
                            v = ValidationState::new_from_index(output_stack.len(), input_index);
                            continue;
                        }
                        v.prev_token_type = ValidationTokenType::Nothing;
                        v.expect_expression = true;
                        ShuntingYard::operator_rule(op, &mut operator_stack, output_stack);
                        operator_stack.push(op.clone());
                    }
                    OperatorTokenType::Perc | OperatorTokenType::Unit(_) => {
                        ShuntingYard::send_to_output(op.clone(), output_stack);
                        v.prev_token_type = ValidationTokenType::Expr;
                        if v.can_be_valid_closing_token() {
                            dbg!("close");
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            last_valid_range =
                                Some((v.tmp_output_stack_start_index, output_stack.len() - 1));
                        }
                    }
                    OperatorTokenType::UnitConverter => {
                        // "in" must be the last operator, only a unit can follow it
                        // so clear the operator stack, push the next unit onto the output
                        // push the unit onto the output, and close it
                        if let Some((
                            Token {
                                typ: TokenType::Operator(OperatorTokenType::Unit(unit)),
                                ..
                            },
                            offset,
                        )) =
                            ShuntingYard::get_next_nonstring_token(tokens, input_index as usize + 1)
                        {
                            v.had_operator = true;
                            v.expect_expression = false;
                            v.prev_token_type = ValidationTokenType::Op;

                            if v.can_be_valid_closing_token() {
                                dbg!("close1");
                                ShuntingYard::send_everything_to_output(
                                    &mut operator_stack,
                                    output_stack,
                                );
                                output_stack.push(TokenType::Operator(OperatorTokenType::Unit(
                                    unit.clone(),
                                )));
                                ShuntingYard::send_to_output(op.clone(), output_stack);
                                last_valid_range =
                                    Some((v.tmp_output_stack_start_index, output_stack.len() - 1));
                            }
                            input_index += 1 + offset as isize;
                        } else {
                            // it is not an "in" operator but a string literal
                        }
                    }
                    OperatorTokenType::UnaryPlus | OperatorTokenType::UnaryMinus => {
                        panic!("Token parser does not generate unary operators");
                    }
                    _ => {
                        if v.expect_expression {
                            dbg!("error6");
                            dbg!(&v);
                            operator_stack.clear();
                            ShuntingYard::set_tokens_to_string(tokens, input_index, &v);
                            v = ValidationState::new_from_index(output_stack.len(), input_index);
                            continue;
                        }
                        v.had_operator = true;
                        v.expect_expression = true;
                        v.prev_token_type = ValidationTokenType::Op;
                        ShuntingYard::operator_rule(op, &mut operator_stack, output_stack);
                        operator_stack.push(op.clone());
                    }
                },
                TokenType::NumberLiteral(num) => {
                    let num = num.clone();
                    if !v.expect_expression {
                        dbg!("error");
                        dbg!(&operator_stack);
                        dbg!(&output_stack);
                        operator_stack.clear();
                        ShuntingYard::set_tokens_to_string(tokens, input_index, &v);
                        v = ValidationState::new_from_index(output_stack.len(), input_index);
                    }
                    // TODO nézd meg muszáj e klnozni, ne me tudja ez a fv átvenni az ownershipet
                    // a input_tokens felett, vagy az outputban nem e lehetnek pointerek
                    output_stack.push(TokenType::NumberLiteral(if v.neg {
                        (&num).neg()
                    } else {
                        num
                    }));
                    v.neg = false;
                    if v.can_be_valid_closing_token() {
                        if let Some((next_token, offset)) =
                            ShuntingYard::get_next_nonstring_token(tokens, input_index as usize + 1)
                        {
                            if let TokenType::Operator(OperatorTokenType::Unit(unit)) =
                                &next_token.typ
                            {
                                // if the next token is unit, push it to the stack immediately, and
                                // skip the next iteration
                                output_stack.push(TokenType::Operator(OperatorTokenType::Unit(
                                    unit.clone(),
                                )));
                                input_index += 1 + offset as isize;
                            } else if let TokenType::Operator(OperatorTokenType::Perc) =
                                next_token.typ
                            {
                                // if the next token is '%', push it to the stack immediately, and
                                // skip the next iteration
                                output_stack.push(TokenType::Operator(OperatorTokenType::Perc));
                                input_index += 1 + offset as isize;
                            }
                        }

                        dbg!("close2");
                        dbg!(&operator_stack);
                        dbg!(&output_stack);
                        ShuntingYard::send_everything_to_output(&mut operator_stack, output_stack);
                        last_valid_range =
                            Some((v.tmp_output_stack_start_index, output_stack.len() - 1));
                    }
                    v.prev_token_type = ValidationTokenType::Expr;
                    v.expect_expression = false;
                }
                // Token::UnitOfMeasure(str, unit) => {
                // output.push(input_token.clone());
                // }
                TokenType::Variable => {
                    if !v.expect_expression {
                        dbg!("error8");
                        operator_stack.clear();
                        ShuntingYard::set_tokens_to_string(tokens, input_index, &v);
                        v = ValidationState::new_from_index(output_stack.len(), input_index);
                        continue;
                    }
                    output_stack.push(input_token.typ.clone());
                }
            }
        }

        for op in operator_stack.iter().rev() {
            ShuntingYard::send_to_output(op.clone(), output_stack);
        }

        // keep only the valid interval
        if let Some((last_valid_start_index, last_valid_end_index)) = last_valid_range {
            output_stack.drain(last_valid_end_index + 1..);
            output_stack.drain(0..last_valid_start_index);
        } else if input_index > 0 {
            output_stack.clear();
            ShuntingYard::set_tokens_to_string(tokens, input_index, &v);
        }
    }

    fn set_tokens_to_string<'text_ptr, 'units>(
        tokens: &mut Vec<Token<'text_ptr, 'units>>,
        input_index: isize,
        v: &ValidationState,
    ) {
        for token in tokens[v.tmp_input_token_start_index..=input_index as usize].iter_mut() {
            token.typ = TokenType::StringLiteral
        }
    }

    fn get_next_nonstring_token<'a, 'text_ptr, 'units>(
        tokens: &'a Vec<Token<'text_ptr, 'units>>,
        i: usize,
    ) -> Option<(&'a Token<'text_ptr, 'units>, usize)> {
        let mut offset = 0;
        while i + offset < tokens.len() {
            if !tokens[i + offset].is_string() {
                return Some((&tokens[i + offset], offset));
            }
            offset += 1;
        }
        return None;
    }

    fn operator_rule<'text_ptr, 'units>(
        incoming_op: &OperatorTokenType<'units>,
        operator_stack: &mut Vec<OperatorTokenType<'units>>,
        output: &mut Vec<TokenType<'units>>,
    ) {
        if operator_stack.is_empty() {
            return;
        }
        let top_of_stack = operator_stack[operator_stack.len() - 1].clone();

        if matches!(top_of_stack, OperatorTokenType::ParenOpen)
            || matches!(top_of_stack, OperatorTokenType::ParenClose)
            || matches!(top_of_stack, OperatorTokenType::BracketOpen)
            || matches!(top_of_stack, OperatorTokenType::BracketClose)
        {
            return;
        }
        let incoming_op_precedence = incoming_op.precedence();
        let top_of_stack_precedence = top_of_stack.precedence();
        let assoc = incoming_op.assoc();
        let incoming_prec_left_assoc_and_equal =
            assoc == Assoc::Left && incoming_op_precedence == top_of_stack_precedence;
        if incoming_op_precedence < top_of_stack_precedence || incoming_prec_left_assoc_and_equal {
            operator_stack.pop();
            ShuntingYard::send_to_output(top_of_stack, output);
            ShuntingYard::operator_rule(incoming_op, operator_stack, output);
        // } else if matches!(top_of_stack.typ, OperatorTokenType::In) {
        //     // 'in' has a lowest precedence to avoid writing a lot of parenthesis,
        //     // but because of that it would be put at the very end of the output stack.
        //     // This code puts it into the output
        //     ShuntingYard::put_operator_on_the_stack(top_of_stack, output);
        //     operator_stack.pop();
        } else {
            // do nothing
        }
    }

    fn send_everything_to_output<'units>(
        operator_stack: &mut Vec<OperatorTokenType<'units>>,
        output_stack: &mut Vec<TokenType<'units>>,
    ) {
        for op in operator_stack.drain(..).rev() {
            ShuntingYard::send_to_output(op, output_stack);
        }
    }

    fn send_anything_until_opening_bracket<'units>(
        operator_stack: &mut Vec<OperatorTokenType<'units>>,
        output: &mut Vec<TokenType<'units>>,
        open_paren_type: &OperatorTokenType,
        mut arg_count: usize,
    ) -> Option<usize> {
        if operator_stack.is_empty() {
            return None;
        }
        let top_of_op_stack = operator_stack.pop().unwrap();
        if &top_of_op_stack == open_paren_type {
            return Some(arg_count);
        } else if top_of_op_stack == OperatorTokenType::Comma {
            arg_count += 1;
        } else {
            ShuntingYard::send_to_output(top_of_op_stack, output);
        }
        return ShuntingYard::send_anything_until_opening_bracket(
            operator_stack,
            output,
            open_paren_type,
            arg_count,
        );
    }

    fn send_to_output<'text_ptr, 'units>(
        operator: OperatorTokenType<'units>,
        output: &mut Vec<TokenType<'units>>,
    ) {
        // TODO these should be enums
        match operator {
            OperatorTokenType::Perc
            | OperatorTokenType::Add
            | OperatorTokenType::Sub
            | OperatorTokenType::UnitConverter
            | OperatorTokenType::UnaryPlus
            | OperatorTokenType::UnaryMinus => output.push(TokenType::Operator(operator)),
            OperatorTokenType::Pow => output.push(TokenType::Operator(operator)),
            OperatorTokenType::Mult => output.push(TokenType::Operator(operator)),
            OperatorTokenType::Div => output.push(TokenType::Operator(operator)),
            _ => output.push(TokenType::Operator(operator)),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::token_parser::TokenParser;
    use crate::units::consts::{create_prefixes, init_units};
    use crate::units::units::{UnitOutput, Units};
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    pub fn num<'text_ptr, 'units>(n: i64) -> Token<'text_ptr, 'units> {
        Token {
            ptr: &[],
            typ: TokenType::NumberLiteral(n.into()),
        }
    }

    pub fn op<'text_ptr, 'units>(op_repr: OperatorTokenType<'units>) -> Token<'text_ptr, 'units> {
        Token {
            ptr: &[],
            typ: TokenType::Operator(op_repr),
        }
    }
    pub fn str<'text_ptr, 'units>(op_repr: &'static str) -> Token<'text_ptr, 'units> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::StringLiteral,
        }
    }

    pub fn unit<'text_ptr, 'units>(op_repr: &'static str) -> Token<'text_ptr, 'units> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::Operator(OperatorTokenType::Unit(UnitOutput::new())),
        }
    }

    pub fn var<'text_ptr, 'units>(op_repr: &'static str) -> Token<'text_ptr, 'units> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::Variable,
        }
    }
    pub fn numf<'text_ptr, 'units>(n: f64) -> Token<'text_ptr, 'units> {
        Token {
            ptr: &[],
            typ: TokenType::NumberLiteral(BigDecimal::from_f64(n).unwrap()),
        }
    }

    pub fn compare_tokens(expected_tokens: &[Token], actual_tokens: &[Token]) {
        assert_eq!(
            actual_tokens.len(),
            expected_tokens.len(),
            "actual tokens: {:?}",
            &actual_tokens
        );
        for (actual_token, expected_token) in actual_tokens.iter().zip(expected_tokens.iter()) {
            match (&expected_token.typ, &actual_token.typ) {
                (TokenType::NumberLiteral(expected_num), TokenType::NumberLiteral(actual_num)) => {
                    assert_eq!(
                        expected_num, actual_num,
                        "actual tokens: {:?}",
                        &actual_tokens
                    )
                }
                (TokenType::Operator(expected_op), TokenType::Operator(actual_op)) => {
                    match (expected_op, actual_op) {
                        (OperatorTokenType::Unit(_), OperatorTokenType::Unit(actual_unit)) => {
                            //     expected_op is an &str
                            let str_slice =
                                unsafe { std::mem::transmute::<_, &str>(expected_token.ptr) };
                            assert_eq!(&actual_unit.to_string(), str_slice)
                        }
                        _ => assert_eq!(
                            expected_op, actual_op,
                            "actual tokens: {:?}",
                            &actual_tokens
                        ),
                    }
                }
                (TokenType::StringLiteral, TokenType::StringLiteral) => {
                    // expected_op is an &str
                    let str_slice = unsafe { std::mem::transmute::<_, &str>(expected_token.ptr) };
                    let expected_chars = str_slice.chars().collect::<Vec<char>>();
                    // in shunting yard, we don't care about whitespaces, they are tested in token_parser
                    let trimmed_actual: Vec<char> = actual_token
                        .ptr
                        .iter()
                        .collect::<String>()
                        .chars()
                        .collect();
                    assert_eq!(
                        &trimmed_actual, &expected_chars,
                        "actual tokens: {:?}",
                        &actual_tokens
                    )
                }
                (TokenType::Variable, TokenType::Variable) => {
                    // expected_op is an &str
                    let str_slice = unsafe { std::mem::transmute::<_, &str>(expected_token.ptr) };
                    let expected_chars = str_slice.chars().collect::<Vec<char>>();
                    // in shunting yard, we don't care about whitespaces, they are tested in token_parser
                    let trimmed_actual: Vec<char> = actual_token
                        .ptr
                        .iter()
                        .collect::<String>()
                        .trim()
                        .chars()
                        .collect();
                    assert_eq!(
                        &expected_chars, &trimmed_actual,
                        "actual tokens: {:?}",
                        &actual_tokens
                    )
                }
                _ => panic!(
                    "{:?} != {:?}, actual tokens: {:?}",
                    expected_token, actual_token, &actual_tokens
                ),
            }
        }
    }

    pub fn do_shunting_yard<'text_ptr, 'units>(
        text: &'text_ptr [char],
        units: &'units Units,
        tokens: &mut Vec<Token<'text_ptr, 'units>>,
    ) -> Vec<TokenType<'units>> {
        let mut output = vec![];
        let vars = vec![String::from("var0"), String::from("var1")];
        TokenParser::parse_line(&text, &vars, &[], tokens, &units);
        ShuntingYard::shunting_yard(tokens, &[], &mut output);
        return output;
    }

    fn test_output(text: &str, expected_tokens: &[Token]) {
        println!("===================================================");
        println!("{}", text);
        let temp = text.chars().collect::<Vec<char>>();
        let prefixes = create_prefixes();
        let mut units = Units::new(&prefixes);
        units.units = init_units(&units.prefixes);
        let mut tokens = vec![];
        let output = do_shunting_yard(&temp, &units, &mut tokens);
        dbg!(&output);
        compare_tokens(
            expected_tokens,
            output
                .iter()
                .map(|it| Token {
                    ptr: &[],
                    typ: it.clone(),
                })
                .collect::<Vec<_>>()
                .as_slice(),
        );
    }

    fn test_tokens(text: &str, expected_tokens: &[Token]) {
        println!("===================================================");
        println!("{}", text);
        let temp = text.chars().collect::<Vec<char>>();
        let prefixes = create_prefixes();
        let units = Units::new(&prefixes);
        let mut tokens = vec![];
        let output = do_shunting_yard(&temp, &units, &mut tokens);
        compare_tokens(expected_tokens, &tokens);
    }

    #[test]
    fn test1() {
        test_output(
            "1/2s",
            &[num(1), num(2), unit("s"), op(OperatorTokenType::Div)],
        );

        test_output(
            "30% - 10%",
            &[
                num(30),
                op(OperatorTokenType::Perc),
                num(10),
                op(OperatorTokenType::Perc),
                op(OperatorTokenType::Sub),
            ],
        );

        test_output(
            "10km/h * 45min",
            &[
                num(10),
                unit("km / h"),
                num(45),
                unit("min"),
                op(OperatorTokenType::Mult),
            ],
        );

        test_output(
            "10km/h * 45min * 12 km",
            &[
                num(10),
                unit("km / h"),
                num(45),
                unit("min"),
                op(OperatorTokenType::Mult),
                num(12),
                unit("km"),
                op(OperatorTokenType::Mult),
            ],
        );

        test_output(
            "10km/h * 45min * 12 km to h",
            &[
                num(10),
                unit("km / h"),
                num(45),
                unit("min"),
                op(OperatorTokenType::Mult),
                num(12),
                unit("km"),
                op(OperatorTokenType::Mult),
                unit("h"),
                op(OperatorTokenType::UnitConverter),
            ],
        );

        test_output(
            "space separated numbers 10 000 000 + 1 234",
            &[num(10000000), num(1234), op(OperatorTokenType::Add)],
        );

        test_output(
            "1 * (2+3)",
            &[
                num(1),
                num(2),
                num(3),
                op(OperatorTokenType::Add),
                op(OperatorTokenType::Mult),
            ],
        );
    }

    #[test]
    fn test_shunting_matrices() {
        test_output(
            "[2] + 1",
            &[
                num(2),
                op(OperatorTokenType::Matrix { arg_count: 1 }),
                num(1),
                op(OperatorTokenType::Add),
            ],
        );
        test_output(
            "[2, 3] + 1",
            &[
                num(2),
                num(3),
                op(OperatorTokenType::Matrix { arg_count: 2 }),
                num(1),
                op(OperatorTokenType::Add),
            ],
        );

        test_output(
            "[[2, 3, 4], [5, 6, 7]] + 1",
            &[
                num(2),
                num(3),
                num(4),
                op(OperatorTokenType::Matrix { arg_count: 3 }),
                num(5),
                num(6),
                num(7),
                op(OperatorTokenType::Matrix { arg_count: 3 }),
                op(OperatorTokenType::Matrix { arg_count: 2 }),
                num(1),
                op(OperatorTokenType::Add),
            ],
        );

        test_output(
            "[[2 + 3, 4 * 5], [ 6 / 7, 8^9]]",
            &[
                num(2),
                num(3),
                op(OperatorTokenType::Add),
                num(4),
                num(5),
                op(OperatorTokenType::Mult),
                op(OperatorTokenType::Matrix { arg_count: 2 }),
                num(6),
                num(7),
                op(OperatorTokenType::Div),
                num(8),
                num(9),
                op(OperatorTokenType::Pow),
                op(OperatorTokenType::Matrix { arg_count: 2 }),
                op(OperatorTokenType::Matrix { arg_count: 2 }),
            ],
        );

        test_output("1 + [2,]", &[]);
        test_output(
            "1 + [2,] 3*4",
            &[num(3), num(4), op(OperatorTokenType::Mult)],
        );

        // test(
        //     "1 +* 2",
        //     &[
        //         num(1),
        //         num(2),
        //         op(OperatorTokenType::Mult),
        //         op(OperatorTokenType::Add),
        //     ],
        // );
    }

    #[test]
    fn unary_operators() {
        test_output("1-2", &[num(1), num(2), op(OperatorTokenType::Sub)]);
        test_output("-1 + -2", &[num(-1), num(-2), op(OperatorTokenType::Add)]);
        test_output("-1+-2", &[num(-1), num(-2), op(OperatorTokenType::Add)]);
        test_output("-1 - -2", &[num(-1), num(-2), op(OperatorTokenType::Sub)]);
        test_output("-1--2", &[num(-1), num(-2), op(OperatorTokenType::Sub)]);
        test_output("+1-+2", &[num(1), num(2), op(OperatorTokenType::Sub)]);
        test_output("+1++2", &[num(1), num(2), op(OperatorTokenType::Add)]);
        test_output("2^-2", &[num(2), num(-2), op(OperatorTokenType::Pow)]);

        test_output(
            "-(1) - -(2)",
            &[
                num(1),
                op(OperatorTokenType::UnaryMinus),
                num(2),
                op(OperatorTokenType::UnaryMinus),
                op(OperatorTokenType::Sub),
            ],
        );
    }

    #[test]
    fn test2() {
        // // TODO
        // // test(
        // //     "var1 = var0 + 100",
        // //     &[var("var0"), num(100), op(OperatorTokenType::Add)],
        // // );
        //

        test_output("", &[]);

        test_output(
            "2m/3m",
            &[
                num(2),
                unit("m"),
                num(3),
                unit("m"),
                op(OperatorTokenType::Div),
            ],
        );

        test_output(
            "2/3m",
            &[num(2), num(3), unit("m"), op(OperatorTokenType::Div)],
        );

        test_output(
            "5km + 5cm",
            &[
                num(5),
                unit("km"),
                num(5),
                unit("cm"),
                op(OperatorTokenType::Add),
            ],
        );

        test_output(
            "100 ft * lbf to (in*lbf)",
            &[
                num(100),
                unit("ft lbf"),
                unit("in lbf"),
                op(OperatorTokenType::UnitConverter),
            ],
        );

        test_tokens(
            "100 ft * lbf to (in*lbf)",
            &[
                num(100),
                str(" "),
                unit("ft lbf"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("in lbf"),
            ],
        );
        // typo: the text contain 'lbG' and not lbF
        test_output(
            "100 ft * lbf to (in*lbg)",
            // &[num(100), unit("ft * lbf"), op(OperatorTokenType::In)],
            &[], // invalid, no vlaue
        );
        test_tokens(
            "100 ft * lbf to (in*lbg)",
            &[
                str("100"),
                str(" "),
                str("ft * lbf"),
                str(" "),
                str("to"),
                str(" "),
                str("("),
                str("in"),
                str("*"),
                str("lbg"),
                str(")"),
            ],
        );

        // typo: the text contain 'lbG' and not lbF
        test_output(
            "100 ft * lbf to (in*lbg) 1 + 100",
            &[num(1), num(100), op(OperatorTokenType::Add)],
        );
        test_tokens(
            "100 ft * lbf to (in*lbg) 1 + 100",
            &[
                str("100"),
                str(" "),
                str("ft * lbf"),
                str(" "),
                str("to"),
                str(" "),
                str("("),
                str("in"),
                str("*"),
                str("lbg"),
                str(")"),
                str(" "),
                num(1),
                str(" "),
                op(OperatorTokenType::Add),
                str(" "),
                num(100),
            ],
        );

        test_output("1szer sem jött el + *megjegyzés 2 éve...", &[]);
        test_tokens(
            "1szer sem jött el + *megjegyzés 2 éve...",
            &[
                str("1"),
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

        test_output(
            "1+4szer sem jött el + *megjegyzés 2 éve...",
            &[num(1), num(4), op(OperatorTokenType::Add)],
        );
        test_output(
            "75-15 euróból kell adózni mert 15 EUR adómentes",
            &[num(75), num(15), op(OperatorTokenType::Sub)],
        );
        test_output(
            "15 EUR adómentes azaz 75-15 euróból kell adózni",
            &[num(75), num(15), op(OperatorTokenType::Sub)],
        );
    }
}
