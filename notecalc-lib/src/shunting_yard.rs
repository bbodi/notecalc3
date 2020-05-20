use crate::functions::FnType;
use crate::token_parser::{Assoc, OperatorTokenType, Token, TokenType};
use std::ops::Neg;

#[derive(Eq, PartialEq, Debug)]
enum ValidationTokenType {
    Nothing,
    Expr,
    Op,
}

#[derive(Debug)]
struct MatrixStackEntry {
    pub matrix_start_input_pos: usize,
    pub matrix_row_count: usize,
    pub matrix_prev_row_len: Option<usize>,
    pub matrix_current_row_len: usize,
}

#[derive(Debug)]
struct FnStackEntry {
    pub typ: FnType,
    pub fn_arg_count: usize,
}

#[derive(Debug)]
enum ParenStackEntry {
    /// e.g. [1, 2]
    Matrix(MatrixStackEntry),
    /// e.g. sin(60)
    Fn(FnStackEntry),
    /// e.g. (12 + 3)
    Simple,
}

impl ParenStackEntry {
    fn new_mat(index: isize) -> ParenStackEntry {
        ParenStackEntry::Matrix(MatrixStackEntry {
            matrix_start_input_pos: index as usize,
            matrix_row_count: 1,
            matrix_prev_row_len: None,
            matrix_current_row_len: 1,
        })
    }

    fn new_fn(typ: FnType) -> ParenStackEntry {
        ParenStackEntry::Fn(FnStackEntry {
            typ,
            fn_arg_count: 1,
        })
    }
}

#[derive(Debug)]
struct ValidationState {
    expect_expression: bool,
    open_brackets: usize,
    prev_token_type: ValidationTokenType,
    tmp_output_stack_start_index: usize,
    first_nonvalidated_token_index: usize,
    valid_range_start_token_index: usize,
    had_operator: bool,
    neg: bool,
    // output stack start and end index
    last_valid_input_token_range: Option<(usize, usize)>,
    last_valid_output_range: Option<(usize, usize)>,
    had_assign_op: bool,
    assign_op_input_token_pos: Option<usize>,
    had_non_ws_string_literal: bool,

    parenthesis_stack: Vec<ParenStackEntry>,
}

impl ValidationState {
    fn close_valid_range(&mut self, output_stack_len: usize, token_index: isize) {
        self.first_nonvalidated_token_index = token_index as usize + 1;
        self.last_valid_input_token_range =
            Some((self.valid_range_start_token_index, token_index as usize));
        self.last_valid_output_range =
            Some((self.tmp_output_stack_start_index, output_stack_len - 1));
        self.parenthesis_stack.clear();
    }

    fn reset(&mut self, output_stack_index: usize, token_index: isize) {
        self.tmp_output_stack_start_index = output_stack_index;
        self.first_nonvalidated_token_index = token_index as usize;
        self.valid_range_start_token_index = token_index as usize;
        self.expect_expression = true;
        self.open_brackets = 0;
        self.prev_token_type = ValidationTokenType::Nothing;
        self.neg = false;
        self.had_operator = false;
        self.parenthesis_stack.clear();
    }

    fn new() -> ValidationState {
        ValidationState {
            had_non_ws_string_literal: false,
            last_valid_output_range: None,
            last_valid_input_token_range: None,
            expect_expression: true,
            open_brackets: 0,
            valid_range_start_token_index: 0,
            prev_token_type: ValidationTokenType::Nothing,
            tmp_output_stack_start_index: 0,
            first_nonvalidated_token_index: 0,
            neg: false,
            had_operator: false,
            had_assign_op: false,
            assign_op_input_token_pos: None,
            parenthesis_stack: Vec::with_capacity(0),
        }
    }

    fn pop_as_mat(&mut self) -> MatrixStackEntry {
        match self.parenthesis_stack.pop() {
            Some(ParenStackEntry::Matrix(entry)) => entry,
            _ => panic!(),
        }
    }

    fn pop_as_fn(&mut self) -> Option<FnStackEntry> {
        match self.parenthesis_stack.pop() {
            Some(ParenStackEntry::Fn(entry)) => Some(entry),
            _ => None,
        }
    }

    fn is_matrix_row_len_err(&self) -> bool {
        match self.parenthesis_stack.last() {
            Some(ParenStackEntry::Matrix(MatrixStackEntry {
                matrix_start_input_pos: _,
                matrix_row_count: _,
                matrix_prev_row_len,
                matrix_current_row_len,
            })) => matrix_prev_row_len.map(|it| it != *matrix_current_row_len),
            _ => Some(true), // if there is no matrix at the top of stack, it is an error
        }
        .unwrap_or(false)
    }

    fn matrix_new_row(&mut self) {
        match self.parenthesis_stack.last_mut() {
            Some(ParenStackEntry::Matrix(MatrixStackEntry {
                matrix_start_input_pos: _,
                matrix_row_count,
                matrix_prev_row_len,
                matrix_current_row_len,
            })) => {
                *matrix_prev_row_len = Some(*matrix_current_row_len);
                *matrix_current_row_len = 1;
                *matrix_row_count += 1;
            }
            _ => panic!(),
        }
    }

    fn is_comma_not_allowed(&self) -> bool {
        match self.parenthesis_stack.last() {
            Some(ParenStackEntry::Matrix(MatrixStackEntry {
                matrix_start_input_pos: _,
                matrix_row_count: _,
                matrix_prev_row_len,
                matrix_current_row_len,
            })) => {
                self.open_brackets == 0
                    || matrix_prev_row_len
                        .map(|it| matrix_current_row_len + 1 > it)
                        .unwrap_or(false)
            }
            Some(ParenStackEntry::Fn(..)) => {
                // fn always allows comma
                false
            }
            Some(ParenStackEntry::Simple) => true,
            None => true, // if there is no matrix/fn at the top of stack, it is an error
        }
    }

    fn do_comma(&mut self) {
        match self.parenthesis_stack.last_mut() {
            Some(ParenStackEntry::Matrix(MatrixStackEntry {
                matrix_start_input_pos: _,
                matrix_row_count: _,
                matrix_prev_row_len: _,
                matrix_current_row_len,
            })) => {
                *matrix_current_row_len += 1;
            }
            Some(ParenStackEntry::Fn(FnStackEntry {
                typ: _,
                fn_arg_count,
            })) => {
                *fn_arg_count += 1;
            }
            Some(ParenStackEntry::Simple) | None => panic!(),
        }
    }

    fn can_be_valid_closing_token(&self) -> bool {
        self.parenthesis_stack.is_empty()
    }

    fn is_valid_assignment_expression(&self) -> bool {
        return self
            .assign_op_input_token_pos
            .map(|it| it == self.valid_range_start_token_index)
            .unwrap_or(false);
    }
}

pub struct ShuntingYard {}

impl ShuntingYard {
    pub fn shunting_yard<'text_ptr>(
        tokens: &mut Vec<Token<'text_ptr>>,
        output_stack: &mut Vec<TokenType>,
    ) {
        // TODO: into iter!!!
        // TODO extract out so no alloc SmallVec?
        let mut operator_stack: Vec<OperatorTokenType> = vec![];

        let mut v = ValidationState::new();
        let mut input_index: isize = -1;

        while input_index + 1 < tokens.len() as isize {
            input_index += 1; // it is here so it is incremented always when "continue"
            let input_token = &tokens[input_index as usize];
            match &input_token.typ {
                TokenType::StringLiteral => {
                    if let Some(fn_type) = FnType::value_of(input_token.ptr) {
                        // next token is parenthesis
                        if tokens
                            .get(input_index as usize + 1)
                            .map(|it| it.ptr[0] == '(')
                            .unwrap_or(false)
                            && v.expect_expression
                        {
                            tokens[input_index as usize].typ =
                                TokenType::Operator(OperatorTokenType::Fn {
                                    arg_count: 0, // unsued in tokens, so can be fixed 0
                                    typ: fn_type,
                                });

                            v.parenthesis_stack.push(ParenStackEntry::new_fn(fn_type));
                            v.prev_token_type = ValidationTokenType::Nothing;
                            v.expect_expression = true;
                            operator_stack.push(OperatorTokenType::ParenOpen);
                            // skip the next paren
                            input_index += 1;
                            continue;
                        }
                    }

                    if !input_token.ptr[0].is_ascii_whitespace() {
                        v.had_non_ws_string_literal = true;
                    }
                    if v.valid_range_start_token_index == input_index as usize {
                        v.valid_range_start_token_index += 1;
                    }
                }
                TokenType::Unit(_) => {
                    // TODO: a token ownershipjét nem vehetem el mert kell a rendereléshez (checkold le azért)
                    // de a toketyp-bol nből kivehetem a unit-ot, az már nem fog kelleni.
                    // CODESMELL!!!
                    // a shunting yard kapja meg a tokens owvershipjét, és adjon vissza egy csak r
                    // rendereléshez elegendő ptr + type-ot
                    output_stack.push(input_token.typ.clone());
                    v.prev_token_type = ValidationTokenType::Expr;
                    if v.can_be_valid_closing_token() {
                        ShuntingYard::send_everything_to_output(&mut operator_stack, output_stack);
                        v.close_valid_range(output_stack.len(), input_index);
                    }
                }
                TokenType::Operator(op) => match op {
                    OperatorTokenType::ParenOpen => {
                        operator_stack.push(op.clone());
                        v.parenthesis_stack.push(ParenStackEntry::Simple);
                        v.prev_token_type = ValidationTokenType::Nothing;
                    }
                    OperatorTokenType::ParenClose => {
                        let is_error = match v.parenthesis_stack.last() {
                            None | Some(ParenStackEntry::Matrix(..)) => true,
                            Some(ParenStackEntry::Simple) | Some(ParenStackEntry::Fn(..)) => false,
                        };
                        let prev_token_is_open_paren = input_index > 0
                            && matches!(
                                tokens[(input_index - 1) as usize].typ,
                                TokenType::Operator(OperatorTokenType::ParenOpen)
                            );

                        if !prev_token_is_open_paren && (v.expect_expression || is_error) {
                            operator_stack.clear();
                            v.reset(output_stack.len(), input_index + 1);
                            continue;
                        } else {
                            v.expect_expression = false;
                            v.prev_token_type = ValidationTokenType::Expr;
                        }
                        ShuntingYard::send_anything_until_opening_bracket(
                            &mut operator_stack,
                            output_stack,
                            &OperatorTokenType::ParenOpen,
                        );
                        if let Some(fn_entry) = v.pop_as_fn() {
                            let fn_token_type = TokenType::Operator(OperatorTokenType::Fn {
                                arg_count: if prev_token_is_open_paren {
                                    0
                                } else {
                                    fn_entry.fn_arg_count
                                },
                                typ: fn_entry.typ,
                            });
                            output_stack.push(fn_token_type.clone());
                        }
                        if v.can_be_valid_closing_token() {
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            v.close_valid_range(output_stack.len(), input_index);
                        }
                    }
                    OperatorTokenType::BracketOpen => {
                        if v.open_brackets > 0 || !v.expect_expression {
                            operator_stack.clear();
                            v.reset(output_stack.len(), input_index);
                        }
                        v.open_brackets += 1;
                        v.prev_token_type = ValidationTokenType::Nothing;
                        v.parenthesis_stack
                            .push(ParenStackEntry::new_mat(input_index));
                        operator_stack.push(op.clone());
                    }
                    OperatorTokenType::BracketClose => {
                        if v.expect_expression || v.open_brackets == 0 || v.is_matrix_row_len_err()
                        {
                            operator_stack.clear();
                            v.reset(output_stack.len(), input_index + 1);
                            continue;
                        } else {
                            v.expect_expression = false;
                            v.open_brackets -= 1;
                            v.prev_token_type = ValidationTokenType::Expr;
                        }
                        ShuntingYard::send_anything_until_opening_bracket(
                            &mut operator_stack,
                            output_stack,
                            &OperatorTokenType::BracketOpen,
                        );
                        // at this point it is sure that there is a matrix on top of paren_stack
                        let mat_entry = v.pop_as_mat();
                        let matrix_token_type = TokenType::Operator(OperatorTokenType::Matrix {
                            row_count: mat_entry.matrix_row_count,
                            col_count: mat_entry.matrix_current_row_len,
                        });
                        output_stack.push(matrix_token_type.clone());
                        tokens.insert(
                            mat_entry.matrix_start_input_pos,
                            Token {
                                ptr: &[],
                                typ: matrix_token_type.clone(),
                            },
                        );
                        // we inserted one element so increase it
                        input_index += 1;
                        if v.can_be_valid_closing_token() {
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            v.close_valid_range(output_stack.len(), input_index);
                        }
                    }
                    OperatorTokenType::Sub
                        if (v.prev_token_type == ValidationTokenType::Nothing
                        || v.prev_token_type == ValidationTokenType::Op) &&
                        /*next token is not whitespace/empty */ tokens
                        .get(input_index as usize + 1)
                        .map(|it| !it.ptr[0].is_ascii_whitespace())
                        .unwrap_or(false) =>
                    {
                        // it is a unary op
                        if !v.expect_expression {
                            operator_stack.clear();
                            v.reset(output_stack.len(), input_index + 1);
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
                    }
                    OperatorTokenType::Add
                        if (v.prev_token_type == ValidationTokenType::Nothing
                        || v.prev_token_type == ValidationTokenType::Op) &&
                        /*next token is not whitespace/empty */ tokens
                        .get(input_index as usize + 1)
                        .map(|it| !it.ptr[0].is_ascii_whitespace())
                        .unwrap_or(false) =>
                    {
                        // it is a unary op
                        if !v.expect_expression {
                            operator_stack.clear();
                            v.reset(output_stack.len(), input_index + 1);
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
                    }
                    OperatorTokenType::Assign => {
                        if v.had_assign_op || !v.had_non_ws_string_literal {
                            if let Some(assign_op_input_token_pos) = v.assign_op_input_token_pos {
                                tokens[assign_op_input_token_pos].typ = TokenType::StringLiteral;
                            }
                            v.assign_op_input_token_pos = None;
                            // make everything to string
                            ShuntingYard::set_tokens_to_string(tokens, 0, input_index as usize);
                            v.reset(output_stack.len(), input_index + 1);
                        } else {
                            v.had_assign_op = true;
                            v.assign_op_input_token_pos = Some(input_index as usize);
                            // assignment op should be part of valid tokens
                            v.reset(output_stack.len(), input_index);
                            ShuntingYard::send_to_output(op.clone(), output_stack);
                        }
                        operator_stack.clear();
                        continue;
                    }
                    OperatorTokenType::Comma => {
                        if v.is_comma_not_allowed() {
                            operator_stack.clear();
                            v.reset(output_stack.len(), input_index + 1);
                            continue;
                        }
                        v.prev_token_type = ValidationTokenType::Nothing;
                        v.expect_expression = true;
                        v.do_comma();
                        ShuntingYard::operator_rule(op, &mut operator_stack, output_stack);
                    }
                    OperatorTokenType::Semicolon => {
                        if v.open_brackets == 0 || v.is_matrix_row_len_err() {
                            operator_stack.clear();
                            v.reset(output_stack.len(), input_index + 1);
                            continue;
                        }
                        v.prev_token_type = ValidationTokenType::Nothing;
                        v.expect_expression = true;
                        v.matrix_new_row();
                        ShuntingYard::operator_rule(op, &mut operator_stack, output_stack);
                    }
                    OperatorTokenType::Perc => {
                        ShuntingYard::send_to_output(op.clone(), output_stack);
                        v.prev_token_type = ValidationTokenType::Expr;
                        if v.can_be_valid_closing_token() {
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            v.close_valid_range(output_stack.len(), input_index);
                        }
                    }
                    OperatorTokenType::UnitConverter => {
                        // the converter must be the last operator, only a unit can follow it
                        // so clear the operator stack, push the next unit onto the output

                        // push the unit onto the output, and close it
                        if let Some((
                            Token {
                                typ: TokenType::Unit(unit),
                                ..
                            },
                            offset,
                        )) =
                            ShuntingYard::get_next_nonstring_token(tokens, input_index as usize + 1)
                        {
                            if ShuntingYard::get_next_nonstring_token(
                                tokens,
                                input_index as usize + 1 + offset + 1,
                            )
                            .is_some()
                            {
                                // after 'to', there must be a single unit component, nothing else
                                continue;
                            }
                            v.expect_expression = false;
                            v.prev_token_type = ValidationTokenType::Op;

                            input_index += 1 + offset as isize;
                            if v.can_be_valid_closing_token() {
                                ShuntingYard::send_everything_to_output(
                                    &mut operator_stack,
                                    output_stack,
                                );
                                output_stack.push(TokenType::Unit(unit.clone()));
                                ShuntingYard::send_to_output(op.clone(), output_stack);
                                v.close_valid_range(output_stack.len(), input_index);
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
                            operator_stack.clear();
                            v.reset(output_stack.len(), input_index + 1);
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
                        operator_stack.clear();
                        v.reset(output_stack.len(), input_index);
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
                            if let TokenType::Unit(unit) = &next_token.typ {
                                // if the next token is unit, push it to the stack immediately, and
                                // skip the next iteration
                                output_stack.push(TokenType::Unit(unit.clone()));
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

                        if v.last_valid_output_range.is_none() || v.had_operator {
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                            );
                            // set everything to string which is in front of this expr
                            v.close_valid_range(output_stack.len(), input_index);
                        }
                    }
                    v.prev_token_type = ValidationTokenType::Expr;
                    v.expect_expression = false;
                }
                TokenType::Variable { .. } | TokenType::LineReference { .. } => {
                    if !v.expect_expression {
                        operator_stack.clear();
                        v.reset(output_stack.len(), input_index + 1);
                        continue;
                    }
                    // so variables can be reassigned
                    v.had_non_ws_string_literal = true;
                    output_stack.push(input_token.typ.clone());
                    if (v.last_valid_output_range.is_none() || v.had_operator)
                        && v.parenthesis_stack.is_empty()
                    {
                        ShuntingYard::send_everything_to_output(&mut operator_stack, output_stack);
                        // set everything to string which is in front of this expr
                        v.close_valid_range(output_stack.len(), input_index);
                    }
                    v.prev_token_type = ValidationTokenType::Expr;
                    v.expect_expression = false;
                }
            }
        }

        if v.is_valid_assignment_expression() {
            // close it
            // set everything to string which is in front of this expr
            v.close_valid_range(output_stack.len(), input_index);
            ShuntingYard::set_tokens_to_string(tokens, 0, v.valid_range_start_token_index - 1);
        }

        for op in operator_stack.iter().rev() {
            match op {
                OperatorTokenType::ParenOpen
                | OperatorTokenType::ParenClose
                | OperatorTokenType::BracketOpen
                | OperatorTokenType::BracketClose => {
                    // ignore
                }
                _ => ShuntingYard::send_to_output(op.clone(), output_stack),
            }
        }

        // set everything to string which is not closed
        if let Some((start, end)) = v.last_valid_input_token_range {
            if start > 0 {
                ShuntingYard::set_tokens_to_string(tokens, 0, start - 1);
            }
            ShuntingYard::set_tokens_to_string(tokens, end + 1, input_index as usize);
        } else if !tokens.is_empty() {
            // there is no valid range, everything is string
            ShuntingYard::set_tokens_to_string(tokens, 0, tokens.len() - 1);
        }

        // remove String tokens with empty content
        // they were Matrices but were unvalidated
        tokens.drain_filter(|it| it.is_string() && it.ptr.is_empty());

        // keep only the valid interval
        if let Some((last_valid_start_index, last_valid_end_index)) = v.last_valid_output_range {
            output_stack.drain(last_valid_end_index + 1..);
            output_stack.drain(0..last_valid_start_index);
        } else {
            output_stack.clear();
        }
    }

    fn set_tokens_to_string<'text_ptr>(tokens: &mut Vec<Token<'text_ptr>>, from: usize, to: usize) {
        for token in tokens[from..=to].iter_mut() {
            match token.typ {
                TokenType::LineReference { .. } => continue,
                _ => token.typ = TokenType::StringLiteral,
            }
        }
    }

    fn get_next_nonstring_token<'a, 'text_ptr>(
        tokens: &'a Vec<Token<'text_ptr>>,
        i: usize,
    ) -> Option<(&'a Token<'text_ptr>, usize)> {
        let mut offset = 0;
        while i + offset < tokens.len() {
            if !tokens[i + offset].is_string() {
                return Some((&tokens[i + offset], offset));
            }
            offset += 1;
        }
        return None;
    }

    fn operator_rule<'text_ptr>(
        incoming_op: &OperatorTokenType,
        operator_stack: &mut Vec<OperatorTokenType>,
        output: &mut Vec<TokenType>,
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

    fn send_everything_to_output(
        operator_stack: &mut Vec<OperatorTokenType>,
        output_stack: &mut Vec<TokenType>,
    ) {
        for op in operator_stack.drain(..).rev() {
            ShuntingYard::send_to_output(op, output_stack);
        }
    }

    fn send_anything_until_opening_bracket(
        operator_stack: &mut Vec<OperatorTokenType>,
        output: &mut Vec<TokenType>,
        open_paren_type: &OperatorTokenType,
    ) {
        if operator_stack.is_empty() {
            return;
        }
        let top_of_op_stack = operator_stack.pop().unwrap();
        if &top_of_op_stack == open_paren_type {
            return;
        } else {
            ShuntingYard::send_to_output(top_of_op_stack, output);
        }
        return ShuntingYard::send_anything_until_opening_bracket(
            operator_stack,
            output,
            open_paren_type,
        );
    }

    fn send_to_output<'text_ptr>(operator: OperatorTokenType, output: &mut Vec<TokenType>) {
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
    use crate::calc::CalcResult;
    use crate::token_parser::TokenParser;
    use crate::units::units::{UnitOutput, Units};
    use crate::Variable;
    use bigdecimal::BigDecimal;
    use typed_arena::Arena;

    pub fn num<'text_ptr>(n: i64) -> Token<'text_ptr> {
        Token {
            ptr: &[],
            typ: TokenType::NumberLiteral(n.into()),
        }
    }

    pub fn op<'text_ptr>(op_repr: OperatorTokenType) -> Token<'text_ptr> {
        Token {
            ptr: &[],
            typ: TokenType::Operator(op_repr),
        }
    }

    pub fn str<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::StringLiteral,
        }
    }

    pub fn unit<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::Unit(UnitOutput::new()),
        }
    }

    pub fn var<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::Variable { var_index: 0 },
        }
    }

    pub fn line_ref<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::LineReference { var_index: 0 },
        }
    }

    pub fn numf<'text_ptr>(n: f64) -> Token<'text_ptr> {
        use bigdecimal::FromPrimitive;
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
                (TokenType::Unit(..), TokenType::Unit(actual_unit)) => {
                    //     expected_op is an &str
                    let str_slice = unsafe { std::mem::transmute::<_, &str>(expected_token.ptr) };
                    assert_eq!(&actual_unit.to_string(), str_slice)
                }
                (TokenType::Operator(expected_op), TokenType::Operator(actual_op)) => {
                    match (expected_op, actual_op) {
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
                (TokenType::Variable { .. }, TokenType::Variable { .. })
                | (TokenType::LineReference { .. }, TokenType::LineReference { .. }) => {
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

    pub fn do_shunting_yard<'text_ptr, 'units, 'b>(
        text: &[char],
        units: &'units Units,
        tokens: &mut Vec<Token<'text_ptr>>,
        vars: &'b [Variable],
        allocator: &'text_ptr Arena<char>,
    ) -> Vec<TokenType> {
        let mut output = vec![];
        TokenParser::parse_line(&text, &vars, tokens, &units, 0, allocator);
        ShuntingYard::shunting_yard(tokens, &mut output);
        return output;
    }

    fn test_output_vars(var_names: &[&'static [char]], text: &str, expected_tokens: &[Token]) {
        let var_names: Vec<Variable> = var_names
            .into_iter()
            .map(|it| Variable {
                name: Box::from(*it),
                value: Err(()),
                defined_at_row: 0,
            })
            .collect();

        println!("===================================================");
        println!("{}", text);
        let temp = text.chars().collect::<Vec<char>>();
        let units = Units::new();
        let mut tokens = vec![];
        let output = do_shunting_yard(&temp, &units, &mut tokens, &var_names, &Arena::new());
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

    fn test_output(text: &str, expected_tokens: &[Token]) {
        test_output_vars(&[], text, expected_tokens);
    }

    fn test_tokens(text: &str, expected_tokens: &[Token]) {
        println!("===================================================");
        println!("{}", text);
        let temp = text.chars().collect::<Vec<char>>();
        let units = Units::new();
        let mut tokens = vec![];
        use bigdecimal::Zero;
        let arena = Arena::new();
        let _ = do_shunting_yard(
            &temp,
            &units,
            &mut tokens,
            &vec![
                Variable {
                    name: Box::from(&['b', '0'][..]),
                    value: Ok(CalcResult::Number(BigDecimal::zero())),
                    defined_at_row: 0,
                },
                Variable {
                    name: Box::from(&['&', '[', '1', ']'][..]),
                    value: Ok(CalcResult::Number(BigDecimal::zero())),
                    defined_at_row: 0,
                },
            ],
            &arena,
        );
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
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 1,
                }),
                num(1),
                op(OperatorTokenType::Add),
            ],
        );
        test_output(
            "[2, 3] + 1",
            &[
                num(2),
                num(3),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 2,
                }),
                num(1),
                op(OperatorTokenType::Add),
            ],
        );

        test_output(
            "[2, 3, 4; 5, 6, 7] + 1",
            &[
                num(2),
                num(3),
                num(4),
                num(5),
                num(6),
                num(7),
                op(OperatorTokenType::Matrix {
                    row_count: 2,
                    col_count: 3,
                }),
                num(1),
                op(OperatorTokenType::Add),
            ],
        );

        // invalid, only 2 elements in the second row
        test_output("[2, 3, 4; 5, 6] + 1", &[num(1)]);

        // invalid
        test_tokens(
            "[[2, 3, 4], [5, 6, 7]] + 1",
            &[
                str("["),
                str("["),
                str("2"),
                str(","),
                str(" "),
                str("3"),
                str(","),
                str(" "),
                str("4"),
                str("]"),
                str(","),
                str(" "),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 3,
                }),
                op(OperatorTokenType::BracketOpen),
                num(5),
                op(OperatorTokenType::Comma),
                str(" "),
                num(6),
                op(OperatorTokenType::Comma),
                str(" "),
                num(7),
                op(OperatorTokenType::BracketClose),
                str("]"),
                str(" "),
                str("+"),
                str(" "),
                str("1"),
            ],
        );

        test_tokens(
            "[1,2,3] *- [4;5;6]",
            &[
                str("["),
                str("1"),
                str(","),
                str("2"),
                str(","),
                str("3"),
                str("]"),
                str(" "),
                str("*"),
                str("-"),
                str(" "),
                op(OperatorTokenType::Matrix {
                    row_count: 3,
                    col_count: 1,
                }),
                op(OperatorTokenType::BracketOpen),
                num(4),
                op(OperatorTokenType::Semicolon),
                num(5),
                op(OperatorTokenType::Semicolon),
                num(6),
                op(OperatorTokenType::BracketClose),
            ],
        );

        // TODO: currently I allow unary op-s on matrix, but rethink it
        test_tokens(
            "[1,2,3] * -[4;5;6]",
            &[
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 3,
                }),
                op(OperatorTokenType::BracketOpen),
                num(1),
                op(OperatorTokenType::Comma),
                num(2),
                op(OperatorTokenType::Comma),
                num(3),
                op(OperatorTokenType::BracketClose),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                op(OperatorTokenType::Sub),
                op(OperatorTokenType::Matrix {
                    row_count: 3,
                    col_count: 1,
                }),
                op(OperatorTokenType::BracketOpen),
                num(4),
                op(OperatorTokenType::Semicolon),
                num(5),
                op(OperatorTokenType::Semicolon),
                num(6),
                op(OperatorTokenType::BracketClose),
            ],
        );

        test_tokens(
            "ez meg vala[41;2] [321,2] * [1;2] adasdsad",
            &[
                str("ez"),
                str(" "),
                str("meg"),
                str(" "),
                str("vala"),
                str("["),
                str("41"),
                str(";"),
                str("2"),
                str("]"),
                str(" "),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 2,
                }),
                op(OperatorTokenType::BracketOpen),
                num(321),
                op(OperatorTokenType::Comma),
                num(2),
                op(OperatorTokenType::BracketClose),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                op(OperatorTokenType::Matrix {
                    row_count: 2,
                    col_count: 1,
                }),
                op(OperatorTokenType::BracketOpen),
                num(1),
                op(OperatorTokenType::Semicolon),
                num(2),
                op(OperatorTokenType::BracketClose),
                str(" "),
                str("adasdsad"),
            ],
        );

        test_output(
            "[1,2,3]*[4;5;6]",
            &[
                num(1),
                num(2),
                num(3),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 3,
                }),
                num(4),
                num(5),
                num(6),
                op(OperatorTokenType::Matrix {
                    row_count: 3,
                    col_count: 1,
                }),
                op(OperatorTokenType::Mult),
            ],
        );

        test_tokens(
            "[1,2,3;4,5]",
            &[
                str("["),
                str("1"),
                str(","),
                str("2"),
                str(","),
                str("3"),
                str(";"),
                str("4"),
                str(","),
                str("5"),
                str("]"),
            ],
        );

        test_output(
            "[[2, 3, 4], [5, 6, 7]] + 1",
            &[
                num(5),
                num(6),
                num(7),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 3,
                }),
            ],
        );

        test_output(
            "[2 + 3, 4 * 5;  6 / 7, 8^9]",
            &[
                num(2),
                num(3),
                op(OperatorTokenType::Add),
                num(4),
                num(5),
                op(OperatorTokenType::Mult),
                num(6),
                num(7),
                op(OperatorTokenType::Div),
                num(8),
                num(9),
                op(OperatorTokenType::Pow),
                op(OperatorTokenType::Matrix {
                    row_count: 2,
                    col_count: 2,
                }),
            ],
        );

        test_output("1 + [2,]", &[num(1)]);
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
        // TODO
        // test(
        //     "var1 = var0 + 100",
        //     &[var("var0"), num(100), op(OperatorTokenType::Add)],
        // );

        test_output("", &[]);
        test_output("2", &[num(2)]);

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

        test_tokens(
            "1 Kib/s to b/s ",
            &[
                num(1),
                str(" "),
                unit("Kib / s"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("b / s"),
                str(" "),
            ],
        );
        // typo: the text contain 'lbG' and not lbF
        test_output("100 ft * lbf to (in*lbg)", &[num(100), unit("ft lbf")]);
        test_tokens(
            "100 ft * lbf to (in*lbg)",
            &[
                num(100),
                str(" "),
                unit("ft lbf"),
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

        test_output(
            "12km/h * 45s ^^",
            &[
                num(12),
                unit("km / h"),
                num(45),
                unit("s"),
                op(OperatorTokenType::Mult),
            ],
        );
        test_tokens(
            "12km/h * 45s ^^",
            &[
                num(12),
                unit("km / h"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(45),
                unit("s"),
                str(" "),
                str("^"),
                str("^"),
            ],
        );

        test_output("1szer sem jött el + *megjegyzés 2 éve...", &[num(1)]);
        test_tokens(
            "1szer sem jött el + *megjegyzés 2 éve...",
            &[
                num(1),
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

    #[test]
    fn invalid_inputs() {
        test_output(
            "1+4szer sem jött el + *megjegyzés 2 éve...",
            &[num(1), num(4), op(OperatorTokenType::Add)],
        );
        test_output(
            "1+4szer sem jött el + *megjegyzés 2éve...+ 3",
            &[num(2), num(3), op(OperatorTokenType::Add)],
        );
        test_tokens(
            "1+4szer sem jött el + *megjegyzés 2éve...+ 3",
            &[
                str("1"),
                str("+"),
                str("4"),
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
                num(2),
                str("éve..."),
                op(OperatorTokenType::Add),
                str(" "),
                num(3),
            ],
        );
    }

    #[test]
    fn variable_test() {
        test_tokens(
            "a = 12",
            &[
                str("a"),
                str(" "),
                op(OperatorTokenType::Assign),
                str(" "),
                num(12),
            ],
        );
        test_output("a = 12", &[op(OperatorTokenType::Assign), num(12)]);

        test_tokens(
            "alfa béta = 12*4",
            &[
                str("alfa"),
                str(" "),
                str("béta"),
                str(" "),
                op(OperatorTokenType::Assign),
                str(" "),
                num(12),
                op(OperatorTokenType::Mult),
                num(4),
            ],
        );
        test_output(
            "alfa béta = 12*4",
            &[
                op(OperatorTokenType::Assign),
                num(12),
                num(4),
                op(OperatorTokenType::Mult),
            ],
        );

        test_tokens(
            "var(12*4) = 13",
            &[
                str("var"),
                str("("),
                str("12"),
                str("*"),
                str("4"),
                str(")"),
                str(" "),
                op(OperatorTokenType::Assign),
                str(" "),
                num(13),
            ],
        );
        test_output("var(12*4) = 13", &[op(OperatorTokenType::Assign), num(13)]);
    }

    #[test]
    fn invalid_variable_test() {
        test_tokens("= 12", &[str("="), str(" "), num(12)]);
        test_output("= 12", &[num(12)]);

        test_tokens(" = 12", &[str(" "), str("="), str(" "), num(12)]);
        test_output(" = 12", &[num(12)]);

        test_tokens(
            "a == 12",
            &[str("a"), str(" "), str("="), str("="), str(" "), num(12)],
        );
        test_tokens(
            "a = 12 =",
            &[
                str("a"),
                str(" "),
                str("="),
                str(" "),
                str("12"),
                str(" "),
                str("="),
            ],
        );

        test_tokens(
            "12 = 13",
            &[str("12"), str(" "), str("="), str(" "), str("13")],
        );
    }

    #[test]
    fn simple_variables_are_reverted_to_str_in_case_of_error() {
        test_tokens("100 b0", &[num(100), str(" "), str("b0")]);
    }

    #[test]
    fn line_references_are_not_reverted_back_to_str() {
        test_tokens("100 &[1]", &[num(100), str(" "), line_ref("&[1]")]);
    }

    #[test]
    fn variable_usage() {
        test_output_vars(
            &[&['b'], &['b', '0']],
            "b0 + 100",
            &[var(""), num(100), op(OperatorTokenType::Add)],
        );

        test_output("a1 + 12", &[num(12)]);

        test_output_vars(&[&['b'], &['b', '0']], "b1 + 100", &[num(100)]);
        test_output_vars(&[&['b'], &['b', '0']], "b", &[var("")]);
    }

    #[test]
    fn test_var_reassignment() {
        test_output_vars(
            &[&['b'], &['b', '0']],
            "b0 = 100",
            &[op(OperatorTokenType::Assign), num(100)],
        );
    }

    #[test]
    fn test_fn_parsing() {
        test_tokens(
            "sin(60 degree)",
            &[
                op(OperatorTokenType::Fn {
                    arg_count: 0,
                    typ: FnType::Sin,
                }),
                op(OperatorTokenType::ParenOpen),
                num(60),
                str(" "),
                unit("degree"),
                op(OperatorTokenType::ParenClose),
            ],
        );
        test_tokens(
            "-sin(60 degree)",
            &[
                op(OperatorTokenType::Sub),
                op(OperatorTokenType::Fn {
                    arg_count: 0,
                    typ: FnType::Sin,
                }),
                op(OperatorTokenType::ParenOpen),
                num(60),
                str(" "),
                unit("degree"),
                op(OperatorTokenType::ParenClose),
            ],
        );

        test_tokens(
            "[sin(60), cos(30)]",
            &[
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 2,
                }),
                op(OperatorTokenType::BracketOpen),
                op(OperatorTokenType::Fn {
                    arg_count: 0,
                    typ: FnType::Sin,
                }),
                op(OperatorTokenType::ParenOpen),
                num(60),
                op(OperatorTokenType::ParenClose),
                op(OperatorTokenType::Comma),
                str(" "),
                op(OperatorTokenType::Fn {
                    arg_count: 0,
                    typ: FnType::Cos,
                }),
                op(OperatorTokenType::ParenOpen),
                num(30),
                op(OperatorTokenType::ParenClose),
                op(OperatorTokenType::BracketClose),
            ],
        );

        test_tokens(
            "sin([60, 30])",
            &[
                op(OperatorTokenType::Fn {
                    arg_count: 0,
                    typ: FnType::Sin,
                }),
                op(OperatorTokenType::ParenOpen),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 2,
                }),
                op(OperatorTokenType::BracketOpen),
                num(60),
                op(OperatorTokenType::Comma),
                str(" "),
                num(30),
                op(OperatorTokenType::BracketClose),
                op(OperatorTokenType::ParenClose),
            ],
        );

        test_tokens(
            "nth([5,6,7],1)",
            &[
                op(OperatorTokenType::Fn {
                    arg_count: 0,
                    typ: FnType::Nth,
                }),
                op(OperatorTokenType::ParenOpen),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 3,
                }),
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

        test_output_vars(
            &[&['b']],
            "nth(b, 1)",
            &[
                var(""),
                num(1),
                op(OperatorTokenType::Fn {
                    arg_count: 2,
                    typ: FnType::Nth,
                }),
            ],
        );
    }

    #[test]
    fn test_fn_output() {
        test_output(
            "sin(60 degree)",
            &[
                num(60),
                unit("degree"),
                op(OperatorTokenType::Fn {
                    arg_count: 1,
                    typ: FnType::Sin,
                }),
            ],
        );
        test_output(
            "-sin(60 degree)",
            &[
                num(60),
                unit("degree"),
                op(OperatorTokenType::Fn {
                    arg_count: 1,
                    typ: FnType::Sin,
                }),
                op(OperatorTokenType::UnaryMinus),
            ],
        );

        test_output(
            "[sin(60), cos(30)]",
            &[
                num(60),
                op(OperatorTokenType::Fn {
                    arg_count: 1,
                    typ: FnType::Sin,
                }),
                num(30),
                op(OperatorTokenType::Fn {
                    arg_count: 1,
                    typ: FnType::Cos,
                }),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 2,
                }),
            ],
        );

        test_output(
            "sin([60, 30])",
            &[
                num(60),
                num(30),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 2,
                }),
                op(OperatorTokenType::Fn {
                    arg_count: 1,
                    typ: FnType::Sin,
                }),
            ],
        );
    }

    #[test]
    fn test_fn_errors() {
        test_tokens(
            "nth([1,2]",
            &[
                str("nth"),
                str("("),
                str("["),
                str("1"),
                str(","),
                str("2"),
                str("]"),
            ],
        );
    }

    #[test]
    fn test_ignore_single_brackets() {
        test_tokens("[", &[str("[")]);
        test_output("[", &[]);
        test_tokens("]", &[str("]")]);
        test_output("]", &[]);
        test_tokens("(", &[str("(")]);
        test_output("(", &[]);
        test_tokens(")", &[str(")")]);
        test_output(")", &[]);
        test_tokens("=", &[str("=")]);
        test_output("=", &[]);
    }

    #[test]
    fn test_unary_minus() {
        test_output("-x -y", &[]);
    }
}
