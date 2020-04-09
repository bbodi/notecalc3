use crate::token_parser::{Assoc, NumberToken, OperatorToken, OperatorTokenType, Token};
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
    tmp_start_index: usize,
    neg: bool,
}

impl ValidationState {
    fn new_from_index(output_stack_index: usize) -> ValidationState {
        ValidationState {
            expect_expression: true,
            open_brackets: 0,
            open_parens: 0,
            prev_token_type: ValidationTokenType::Nothing,
            tmp_start_index: output_stack_index,
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
        mut tokens: Vec<Token<'text_ptr, 'units>>,
        // tokens: &Vec<Token<'text_ptr, 'units>>,
        function_names: &[String],
        output_stack: &mut Vec<Token<'text_ptr, 'units>>,
    ) {
        tokens.drain_filter(|t| match t {
            Token::StringLiteral(_) => true,
            _ => false,
        });
        // TODO: into iter!!!
        dbg!(&tokens);
        // TODO extract out so no alloc
        let mut operator_stack: Vec<OperatorToken> = vec![];

        let mut last_valid_range = None;
        let mut v = ValidationState::new_from_index(0);
        let mut input_index: isize = -1;
        while input_index + 1 < tokens.len() as isize {
            input_index += 1; // it is here so it is incremented always when "continue"
            let input_token = &tokens[input_index as usize];
            dbg!(&input_token);
            match input_token {
                Token::Operator(op) => match op.typ {
                    OperatorTokenType::ParenOpen => {
                        operator_stack.push(op.clone());
                        v.open_parens += 1;
                        v.prev_token_type = ValidationTokenType::Nothing;
                    }
                    OperatorTokenType::ParenClose => {
                        if v.expect_expression || v.open_parens == 0 {
                            dbg!("error1");

                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            v = ValidationState::new_from_index(output_stack.len());
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
                            dbg!(&operator_stack);
                            dbg!(&output_stack);
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            last_valid_range = Some((v.tmp_start_index, output_stack.len() - 1));
                        }
                    }
                    OperatorTokenType::BracketOpen => {
                        v.open_brackets += 1;
                        v.prev_token_type = ValidationTokenType::Nothing;
                        operator_stack.push(op.clone());
                    }
                    OperatorTokenType::BracketClose => {
                        if v.expect_expression || v.open_brackets <= 0 {
                            dbg!("error2");
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            v = ValidationState::new_from_index(output_stack.len());
                            continue;
                        } else {
                            v.expect_expression = false;
                            v.open_brackets -= 1;
                            v.prev_token_type = ValidationTokenType::Expr;
                        }
                        if let Some((opening_bracket, comma_count)) =
                            ShuntingYard::send_anything_until_opening_bracket(
                                &mut operator_stack,
                                output_stack,
                                &OperatorTokenType::BracketOpen,
                                0,
                            )
                        {
                            // Matrix will point to the whole output,
                            // so we can highlight only the first/last char if we want
                            // (the brackets)
                            unsafe {
                                let char_count =
                                    op.ptr.as_ptr().offset_from(opening_bracket.ptr.as_ptr())
                                        as usize;
                                output_stack.push(Token::Operator(OperatorToken {
                                    typ: OperatorTokenType::Matrix {
                                        arg_count: comma_count + 1,
                                    },
                                    ptr: std::mem::transmute(std::ptr::slice_from_raw_parts(
                                        opening_bracket.ptr.as_ptr(),
                                        char_count + 1,
                                    )),
                                }))
                            }
                            if v.can_be_valid_closing_token() {
                                dbg!("close");
                                ShuntingYard::send_everything_to_output(
                                    &mut operator_stack,
                                    output_stack,
                                );
                                last_valid_range =
                                    Some((v.tmp_start_index, output_stack.len() - 1));
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
                                ShuntingYard::send_everything_to_output(
                                    &mut operator_stack,
                                    output_stack,
                                );
                                v = ValidationState::new_from_index(output_stack.len());
                                continue;
                            } else if let Some(Token::NumberLiteral(NumberToken { .. })) =
                                tokens.get(input_index as usize + 1)
                            {
                                v.neg = true;
                            } else {
                                // process it as a unary op
                                operator_stack.push(OperatorToken::new(
                                    OperatorTokenType::UnaryMinus,
                                    op.ptr,
                                ));
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
                                ShuntingYard::send_everything_to_output(
                                    &mut operator_stack,
                                    output_stack,
                                );
                                v = ValidationState::new_from_index(output_stack.len());
                                continue;
                            } else if let Some(Token::NumberLiteral(NumberToken { .. })) =
                                tokens.get(input_index as usize + 1)
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
                        // ShuntingYard::handle_unary_operator(
                        //     op,
                        //     OperatorTokenType::UnaryPlus,
                        //     last_token,
                        //     &mut operator_stack,
                        //     output,
                        // );
                    }
                    OperatorTokenType::Assign => {
                        // the left side of the '=' might be a variable name like 'km' or 'm'
                        // don't put '=' on the stack and remove the lhs of '=' from it
                        output_stack.pop();
                    }
                    OperatorTokenType::Comma => {
                        if v.open_brackets == 0 && v.open_parens == 0 {
                            dbg!("error5");
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            v = ValidationState::new_from_index(output_stack.len());
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
                            last_valid_range = Some((v.tmp_start_index, output_stack.len() - 1));
                        }
                    }
                    OperatorTokenType::UnitConverter => {
                        // "in" must be the last operator, only a unit can follow it
                        // so clear the operator stack, push the next unit onto the output
                        // push the unit onto the output, and close it
                        ShuntingYard::send_everything_to_output(&mut operator_stack, output_stack);
                        if let Some(Token::Operator(OperatorToken {
                            typ: OperatorTokenType::Unit(unit),
                            ptr,
                        })) = tokens.get(input_index as usize + 1)
                        {
                            v.had_operator = true;
                            v.expect_expression = false;
                            v.prev_token_type = ValidationTokenType::Op;
                            output_stack.push(Token::Operator(OperatorToken {
                                typ: OperatorTokenType::Unit(unit.clone()),
                                ptr,
                            }));
                            input_index += 1;
                            ShuntingYard::send_to_output(op.clone(), output_stack);
                            if v.can_be_valid_closing_token() {
                                dbg!("close1");
                                ShuntingYard::send_everything_to_output(
                                    &mut operator_stack,
                                    output_stack,
                                );
                                last_valid_range =
                                    Some((v.tmp_start_index, output_stack.len() - 1));
                            }
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
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            v = ValidationState::new_from_index(output_stack.len());
                            continue;
                        }
                        v.had_operator = true;
                        v.expect_expression = true;
                        v.prev_token_type = ValidationTokenType::Op;
                        ShuntingYard::operator_rule(op, &mut operator_stack, output_stack);
                        operator_stack.push(op.clone());
                    }
                },
                Token::NumberLiteral(num) => {
                    if !v.expect_expression {
                        dbg!("error");
                        dbg!(&operator_stack);
                        ShuntingYard::send_everything_to_output(&mut operator_stack, output_stack);
                        dbg!(&output_stack);
                        v = ValidationState::new_from_index(output_stack.len());
                    }
                    // TODO nézd meg muszáj e klnozni, ne me tudja ez a fv átvenni az ownershipet
                    // a input_tokens felett, vagy az outputban nem e lehetnek pointerek
                    output_stack.push(Token::NumberLiteral(NumberToken {
                        num: if v.neg {
                            (&num.num).neg()
                        } else {
                            num.num.clone()
                        },
                        ptr: num.ptr,
                    }));
                    v.neg = false;
                    if v.can_be_valid_closing_token() {
                        if let Some(Token::Operator(OperatorToken {
                            typ: OperatorTokenType::Unit(unit),
                            ptr,
                        })) = tokens.get(input_index as usize + 1)
                        {
                            // if the next token is unit, push it to the stack immediately, and
                            // skip the next iteration
                            output_stack.push(Token::Operator(OperatorToken {
                                typ: OperatorTokenType::Unit(unit.clone()),
                                ptr,
                            }));
                            input_index += 1;
                        } else if let Some(Token::Operator(OperatorToken {
                            typ: OperatorTokenType::Perc,
                            ptr,
                        })) = tokens.get(input_index as usize + 1)
                        {
                            // if the next token is '%', push it to the stack immediately, and
                            // skip the next iteration
                            output_stack.push(Token::Operator(OperatorToken {
                                typ: OperatorTokenType::Perc,
                                ptr,
                            }));
                            input_index += 1;
                        }

                        dbg!("close2");
                        dbg!(&operator_stack);
                        ShuntingYard::send_everything_to_output(&mut operator_stack, output_stack);
                        last_valid_range = Some((v.tmp_start_index, output_stack.len() - 1));
                    }
                    v.prev_token_type = ValidationTokenType::Expr;
                    v.expect_expression = false;
                }
                // Token::UnitOfMeasure(str, unit) => {
                // output.push(input_token.clone());
                // }
                Token::Variable(str) => {
                    if !v.expect_expression {
                        dbg!("error8");
                        ShuntingYard::send_everything_to_output(&mut operator_stack, output_stack);
                        v = ValidationState::new_from_index(output_stack.len());
                        continue;
                    }
                    output_stack.push(input_token.clone());
                }
                Token::StringLiteral(str) => {
                    // it ignores strings
                    if false {
                        // if function_names.iter().any(|it| it == str) {
                        //  ShuntingYardStacks((operatorStack + Token.Operator("fun " + inputToken.str)), output + inputToken)
                    } else {
                        // ignore it
                        // output.push(input_token.clone());
                    }
                }
            }
        }

        for op in operator_stack.iter().rev() {
            ShuntingYard::send_to_output(op.clone(), output_stack);
        }

        // keep only the valid interval
        if let Some((last_valid_start_index, last_valid_end_index)) = last_valid_range {
            dbg!(&output_stack);
            dbg!(&last_valid_start_index);
            dbg!(&last_valid_end_index);
            output_stack.drain(last_valid_end_index + 1..);
            dbg!(&output_stack);
            dbg!(&v);
            output_stack.drain(0..last_valid_start_index);
        } else {
            output_stack.clear();
        }
    }

    // fn handle_unary_operator<'text_ptr, 'units>(
    //     input_token: &OperatorToken<'text_ptr, 'units>,
    //     unary_op: OperatorTokenType<'units>,
    //     last_token: Option<&Token<'text_ptr, 'units>>,
    //     operator_stack: &mut Vec<OperatorToken<'text_ptr, 'units>>,
    //     output: &mut Vec<Token<'text_ptr, 'units>>,
    // ) {
    //     if last_token
    //         .map(|it| match it {
    //             Token::Operator(last_op) => {
    //                 !matches!(last_op.typ, OperatorTokenType::ParenClose)
    //                     && !matches!(last_op.typ, OperatorTokenType::BracketClose)
    //                     && !matches!(last_op.typ, OperatorTokenType::Perc)
    //                     && !matches!(last_op.typ, OperatorTokenType::Unit(_))
    //             }
    //             _ => false,
    //         })
    //         .unwrap_or(true)
    //     {
    //         operator_stack.push(OperatorToken::new(unary_op, input_token.ptr));
    //     } else {
    //         ShuntingYard::operator_rule(input_token, operator_stack, output);
    //         operator_stack.push(input_token.clone());
    //     }
    // }

    fn operator_rule<'text_ptr, 'units>(
        incoming_op: &OperatorToken<'text_ptr, 'units>,
        operator_stack: &mut Vec<OperatorToken<'text_ptr, 'units>>,
        output: &mut Vec<Token<'text_ptr, 'units>>,
    ) {
        if operator_stack.is_empty() {
            return;
        }
        let top_of_stack = operator_stack[operator_stack.len() - 1].clone();

        if matches!(top_of_stack.typ, OperatorTokenType::ParenOpen)
            || matches!(top_of_stack.typ, OperatorTokenType::ParenClose)
            || matches!(top_of_stack.typ, OperatorTokenType::BracketOpen)
            || matches!(top_of_stack.typ, OperatorTokenType::BracketClose)
        {
            return;
        }
        let incoming_op_precedence = incoming_op.typ.precedence();
        let top_of_stack_precedence = top_of_stack.typ.precedence();
        let assoc = incoming_op.typ.assoc();
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

    fn send_everything_to_output<'text_ptr, 'units>(
        operator_stack: &mut Vec<OperatorToken<'text_ptr, 'units>>,
        output_stack: &mut Vec<Token<'text_ptr, 'units>>,
    ) {
        for op in operator_stack.drain(..).rev() {
            ShuntingYard::send_to_output(op, output_stack);
        }
    }

    fn send_anything_until_opening_bracket<'text_ptr, 'units>(
        operator_stack: &mut Vec<OperatorToken<'text_ptr, 'units>>,
        output: &mut Vec<Token<'text_ptr, 'units>>,
        open_paren_type: &OperatorTokenType,
        mut arg_count: usize,
    ) -> Option<(OperatorToken<'text_ptr, 'units>, usize)> {
        if operator_stack.is_empty() {
            return None;
        }
        let top_of_op_stack = operator_stack.pop().unwrap();
        if &top_of_op_stack.typ == open_paren_type {
            return Some((top_of_op_stack, arg_count));
        } else if top_of_op_stack.typ == OperatorTokenType::Comma {
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
        operator: OperatorToken<'text_ptr, 'units>,
        output: &mut Vec<Token<'text_ptr, 'units>>,
    ) {
        // TODO these should be enums
        match operator.typ {
            OperatorTokenType::Perc
            | OperatorTokenType::Add
            | OperatorTokenType::Sub
            | OperatorTokenType::UnitConverter
            | OperatorTokenType::UnaryPlus
            | OperatorTokenType::UnaryMinus => output.push(Token::Operator(operator)),
            OperatorTokenType::Pow => output.push(Token::Operator(operator)),
            OperatorTokenType::Mult => output.push(Token::Operator(operator)),
            OperatorTokenType::Div => output.push(Token::Operator(operator)),
            _ => output.push(Token::Operator(operator)),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::token_parser::{validate_tokens, NumberToken, TokenParser};
    use crate::units::consts::init_units;
    use crate::units::units::{UnitOutput, Units};
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    pub fn num<'text_ptr, 'units>(n: i64) -> Token<'text_ptr, 'units> {
        Token::NumberLiteral(NumberToken {
            num: n.into(),
            ptr: &[],
        })
    }
    pub fn op<'text_ptr, 'units>(op_repr: OperatorTokenType<'units>) -> Token<'text_ptr, 'units> {
        Token::Operator(OperatorToken {
            typ: op_repr,
            ptr: &[],
        })
    }
    pub fn str<'text_ptr, 'units>(op_repr: &'static str) -> Token<'text_ptr, 'units> {
        Token::StringLiteral(unsafe { std::mem::transmute(op_repr) })
    }

    pub fn unit<'text_ptr, 'units>(op_repr: &'static str) -> Token<'text_ptr, 'units> {
        Token::Operator(OperatorToken {
            typ: OperatorTokenType::Unit(UnitOutput::new()),
            ptr: unsafe { std::mem::transmute(op_repr) },
        })
    }

    pub fn var<'text_ptr, 'units>(op_repr: &'static str) -> Token<'text_ptr, 'units> {
        Token::Variable(unsafe { std::mem::transmute(op_repr) })
    }
    pub fn numf<'text_ptr, 'units>(n: f64) -> Token<'text_ptr, 'units> {
        Token::NumberLiteral(NumberToken {
            num: BigDecimal::from_f64(n).unwrap(),
            ptr: &[],
        })
    }

    pub fn test_tokens(expected_tokens: &[Token], actual_tokens: &[Token]) {
        assert_eq!(
            expected_tokens.len(),
            actual_tokens.len(),
            "actual tokens: {:?}",
            &actual_tokens
        );
        for (actual_token, expected_token) in actual_tokens.iter().zip(expected_tokens.iter()) {
            match (expected_token, actual_token) {
                (Token::NumberLiteral(expected_num), Token::NumberLiteral(actual_num)) => {
                    assert_eq!(
                        expected_num.num, actual_num.num,
                        "actual tokens: {:?}",
                        &actual_tokens
                    )
                }
                (Token::Operator(expected_op), Token::Operator(actual_op)) => {
                    match (expected_op, actual_op) {
                        (
                            OperatorToken {
                                ptr: expected_op,
                                typ: OperatorTokenType::Unit(_),
                            },
                            OperatorToken {
                                ptr: actual_op,
                                typ: OperatorTokenType::Unit(_),
                            },
                        ) => {
                            //     expected_op is an &str
                            let str_slice = unsafe { std::mem::transmute::<_, &str>(*expected_op) };
                            let expected_chars = str_slice.chars().collect::<Vec<char>>();
                            assert_eq!(&expected_chars, actual_op)
                        }
                        _ => assert_eq!(
                            expected_op.typ, actual_op.typ,
                            "actual tokens: {:?}",
                            &actual_tokens
                        ),
                    }
                }
                (Token::StringLiteral(expected_op), Token::StringLiteral(actual_op)) => {
                    // expected_op is an &str
                    let str_slice = unsafe { std::mem::transmute::<_, &str>(*expected_op) };
                    let expected_chars = str_slice.chars().collect::<Vec<char>>();
                    // in shunting yard, we don't care about whitespaces, they are tested in token_parser
                    let trimmed_actual: Vec<char> = actual_op
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
                (Token::Variable(expected_op), Token::Variable(actual_op)) => {
                    // expected_op is an &str
                    let str_slice = unsafe { std::mem::transmute::<_, &str>(*expected_op) };
                    let expected_chars = str_slice.chars().collect::<Vec<char>>();
                    // in shunting yard, we don't care about whitespaces, they are tested in token_parser
                    let trimmed_actual: Vec<char> = actual_op
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
    ) -> Vec<Token<'text_ptr, 'units>> {
        let mut output = vec![];
        let mut tokens = vec![];
        let vars = vec![String::from("var0"), String::from("var1")];
        TokenParser::parse_line(&text, &vars, &[], &mut tokens, &units);
        // validate_tokens(&mut tokens);
        ShuntingYard::shunting_yard(tokens, &[], &mut output);
        return output;
    }

    fn test(text: &str, expected_tokens: &[Token]) {
        println!("===================================================");
        println!("{}", text);
        let temp = text.chars().collect::<Vec<char>>();
        let mut units = Units::new();
        // units.init();
        units.units = init_units(&units.prefixes);
        let output = do_shunting_yard(&temp, &units);
        test_tokens(expected_tokens, &output);
    }

    #[test]
    fn test1() {
        test(
            "1/2s",
            &[num(1), num(2), unit("s"), op(OperatorTokenType::Div)],
        );

        //the shunting Yard should exclude assign operator and its lhs operand. because" +
        //                     "inputs like b = 100*100+ will be evaulated successfully in this case"
        // TODO
        // test(
        //     "var1 = var0 + 100",
        //     &[var("var0"), num(100), op(OperatorTokenType::Add)],
        // );

        test(
            "30% - 10%",
            &[
                num(30),
                op(OperatorTokenType::Perc),
                num(10),
                op(OperatorTokenType::Perc),
                op(OperatorTokenType::Sub),
            ],
        );

        test(
            "10km/h * 45min * 12 km to h",
            &[
                num(10),
                unit("km/h"),
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

        test(
            "space separated numbers 10 000 000 + 1 234",
            &[num(10000000), num(1234), op(OperatorTokenType::Add)],
        );

        test(
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
        test(
            "[2] + 1",
            &[
                num(2),
                op(OperatorTokenType::Matrix { arg_count: 1 }),
                num(1),
                op(OperatorTokenType::Add),
            ],
        );
        test(
            "[2, 3] + 1",
            &[
                num(2),
                num(3),
                op(OperatorTokenType::Matrix { arg_count: 2 }),
                num(1),
                op(OperatorTokenType::Add),
            ],
        );

        test(
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

        test(
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

        test("1 + [2,]", &[]);
        test(
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
        test("1-2", &[num(1), num(2), op(OperatorTokenType::Sub)]);
        test("-1 + -2", &[num(-1), num(-2), op(OperatorTokenType::Add)]);
        test("-1+-2", &[num(-1), num(-2), op(OperatorTokenType::Add)]);
        test("-1 - -2", &[num(-1), num(-2), op(OperatorTokenType::Sub)]);
        test("-1--2", &[num(-1), num(-2), op(OperatorTokenType::Sub)]);
        test("+1-+2", &[num(1), num(2), op(OperatorTokenType::Sub)]);
        test("+1++2", &[num(1), num(2), op(OperatorTokenType::Add)]);
        test("2^-2", &[num(2), num(-2), op(OperatorTokenType::Pow)]);

        test(
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
        test(
            "2m/3m",
            &[
                num(2),
                unit("m"),
                num(3),
                unit("m"),
                op(OperatorTokenType::Div),
            ],
        );

        test(
            "2/3m",
            &[num(2), num(3), unit("m"), op(OperatorTokenType::Div)],
        );

        test(
            "5km + 5cm",
            &[
                num(5),
                unit("km"),
                num(5),
                unit("cm"),
                op(OperatorTokenType::Add),
            ],
        );

        test(
            "100 ft * lbf to (in*lbf)",
            &[
                num(100),
                unit("ft * lbf"),
                unit("(in*lbf)"),
                op(OperatorTokenType::UnitConverter),
            ],
        );

        // typo: the text contain 'lbG' and not lbF
        test(
            "100 ft * lbf to (in*lbg)",
            // &[num(100), unit("ft * lbf"), op(OperatorTokenType::In)],
            &[], // invalid, no vlaue
        );

        // typo: the text contain 'lbG' and not lbF
        test(
            "100 ft * lbf to (in*lbg) 1 + 100",
            &[num(1), num(100), op(OperatorTokenType::Add)],
        );

        test("1szer sem jött el + *megjegyzés 2 éve...", &[]);
        test(
            "1+4szer sem jött el + *megjegyzés 2 éve...",
            &[num(1), num(4), op(OperatorTokenType::Add)],
        );
        test(
            "75-15 euróból kell adózni mert 15 EUR adómentes",
            &[num(75), num(15), op(OperatorTokenType::Sub)],
        );
        test(
            "15 EUR adómentes azaz 75-15 euróból kell adózni",
            &[num(75), num(15), op(OperatorTokenType::Sub)],
        );
    }
}
