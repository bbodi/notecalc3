use crate::calc::ShuntingYardResult;
use crate::functions::FnType;
use crate::token_parser::{debug_print, Assoc, OperatorTokenType, Token, TokenType};
use crate::units::units::{UnitOutput, Units};
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
    typ: FnType,
    fn_arg_count: usize,
    fn_token_index: usize,
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

    fn new_fn(typ: FnType, fn_token_index: usize) -> ParenStackEntry {
        ParenStackEntry::Fn(FnStackEntry {
            typ,
            fn_arg_count: 1,
            fn_token_index,
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
    // index of the last valid operator
    last_valid_operator_index: Option<usize>,
    had_assign_op: bool,
    assign_op_input_token_pos: Option<usize>,
    had_non_ws_string_literal: bool,
    // Todo: percentage op-ok legyenek külön enumba
    in_progress_percentage_op: Option<OperatorTokenType>,

    parenthesis_stack: Vec<ParenStackEntry>,
}

impl ValidationState {
    fn close_valid_range(
        &mut self,
        output_stack_len: usize,
        token_index: isize,
        operator_stack_len: usize,
    ) {
        self.first_nonvalidated_token_index = token_index as usize + 1;
        self.last_valid_input_token_range =
            Some((self.valid_range_start_token_index, token_index as usize));
        self.last_valid_output_range =
            Some((self.tmp_output_stack_start_index, output_stack_len - 1));
        self.parenthesis_stack.clear();
        self.last_valid_operator_index = if operator_stack_len > 0 {
            Some(operator_stack_len - 1)
        } else {
            None
        };
        debug_print(&format!("  close_valid_range"));
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
            in_progress_percentage_op: None,
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
            last_valid_operator_index: None,
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
                // it is not true, if self.expect_expression, then comma is not allowed,
                // but now I allow it, so it will be evaluated as fn and can be
                // red in case of e.g. missing/wrong parameter
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
            Some(ParenStackEntry::Fn(FnStackEntry { fn_arg_count, .. })) => {
                *fn_arg_count += 1;
            }
            Some(ParenStackEntry::Simple) | None => panic!(),
        }
    }

    fn can_be_valid_closing_token(&self) -> bool {
        self.parenthesis_stack.is_empty() && self.in_progress_percentage_op.is_none()
    }

    fn is_valid_assignment_expression(&self) -> bool {
        return self
            .assign_op_input_token_pos
            .map(|it| it == self.valid_range_start_token_index)
            .unwrap_or(false);
    }
}

pub struct ShuntingYard {}

fn to_out(output_stack: &mut Vec<ShuntingYardResult>, typ: &TokenType, input_index: isize) {
    output_stack.push(ShuntingYardResult::new(typ.clone(), input_index as usize))
}

fn to_out2(output_stack: &mut Vec<ShuntingYardResult>, typ: TokenType, input_index: isize) {
    output_stack.push(ShuntingYardResult::new(typ, input_index as usize))
}

#[derive(Debug, Clone)]
pub struct ShuntingYardOperatorResult {
    op_type: OperatorTokenType,
    index_into_tokens: isize,
}

impl ShuntingYard {
    pub fn shunting_yard<'text_ptr>(
        tokens: &mut Vec<Token<'text_ptr>>,
        output_stack: &mut Vec<ShuntingYardResult>,
        units: &Units,
    ) {
        // TODO: into iter!!!
        // TODO:mem extract out so no alloc SmallVec?
        let mut operator_stack: Vec<ShuntingYardOperatorResult> = vec![];

        let mut v = ValidationState::new();
        let mut input_index: isize = -1;

        while input_index + 1 < tokens.len() as isize {
            input_index += 1; // it is here so it is incremented always when "continue"
            let input_token = &tokens[input_index as usize];
            debug_print(&format!(
                "Input token: {:?} {:?}",
                input_token.typ, input_token.ptr
            ));
            match &input_token.typ {
                TokenType::Header => {
                    debug_print(&format!("  ignore ({:?})", input_token.ptr));
                    return;
                }
                TokenType::StringLiteral => {
                    if let Some(fn_type) = FnType::value_of(input_token.ptr) {
                        // next token is parenthesis
                        if tokens
                            .get(input_index as usize + 1)
                            .map(|it| it.ptr[0] == '(')
                            .unwrap_or(false)
                            && v.expect_expression
                        {
                            debug_print(&format!("  function"));
                            tokens[input_index as usize].typ =
                                TokenType::Operator(OperatorTokenType::Fn {
                                    arg_count: 0, // unused in tokens, so can be fixed 0
                                    typ: fn_type,
                                });

                            v.parenthesis_stack
                                .push(ParenStackEntry::new_fn(fn_type, input_index as usize));
                            v.prev_token_type = ValidationTokenType::Nothing;
                            v.expect_expression = true;
                            operator_stack.push(ShuntingYardOperatorResult {
                                op_type: OperatorTokenType::ParenOpen,
                                index_into_tokens: input_index + 1,
                            });
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
                    ShuntingYard::shunting_state_debug_print(
                        "    TokenType::Unit",
                        output_stack,
                        &operator_stack,
                    );

                    if tokens
                        .get(input_index as usize + 1)
                        .map(|it| {
                            matches!(it.typ, TokenType::Operator(OperatorTokenType::ApplyUnit))
                        })
                        .unwrap_or(false)
                    {
                        if ShuntingYard::get_next_nonstring_token(tokens, input_index as usize + 2)
                            .map(|(token, _offset)| matches!(token.typ, TokenType::Unit(_)))
                            .unwrap_or(false)
                            && input_token.ptr == &['i', 'n']
                        {
                            debug_print("  it is UnitConverter, promote it");
                            // this is an in operator, not an 'inch' unit
                            // remove the ApplyUnit and reparse this token as 'UnitConverter'
                            tokens.remove(input_index as usize + 1);
                            tokens[input_index as usize].typ =
                                TokenType::Operator(OperatorTokenType::UnitConverter);
                            // reparse this 'inch' as 'in' operator
                            input_index -= 1;
                            continue;
                        }
                        ShuntingYard::operator_rule(
                            OperatorTokenType::ApplyUnit.precedence(),
                            OperatorTokenType::ApplyUnit.assoc(),
                            &mut operator_stack,
                            output_stack,
                            &mut v.last_valid_operator_index,
                            &mut v.last_valid_output_range,
                            input_index,
                        );

                        to_out2(output_stack, input_token.typ.clone(), input_index);
                        to_out2(
                            output_stack,
                            TokenType::Operator(OperatorTokenType::ApplyUnit),
                            input_index,
                        );
                        v.prev_token_type = ValidationTokenType::Expr;
                        // Skip ApplyUnit
                        input_index += 1;
                        if v.can_be_valid_closing_token() {
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                                &mut v.last_valid_operator_index,
                                &mut v.last_valid_output_range,
                            );
                            v.close_valid_range(
                                output_stack.len(),
                                input_index,
                                operator_stack.len(),
                            );
                        }
                    } else {
                        // TODO: a token ownershipjét nem vehetem el mert kell a rendereléshez (checkold le azért)
                        // de a toketyp-bol nből kivehetem a unit-ot, az már nem fog kelleni.
                        // CODESMELL!!!
                        // a shunting yard kapja meg a tokens owvershipjét, és adjon vissza egy csak r
                        // rendereléshez elegendő ptr + type-ot
                        if !output_stack.is_empty() {
                            to_out(output_stack, &input_token.typ, input_index);
                            v.prev_token_type = ValidationTokenType::Expr;
                            if v.can_be_valid_closing_token() {
                                ShuntingYard::send_everything_to_output(
                                    &mut operator_stack,
                                    output_stack,
                                    &mut v.last_valid_operator_index,
                                    &mut v.last_valid_output_range,
                                );
                                v.close_valid_range(
                                    output_stack.len(),
                                    input_index,
                                    operator_stack.len(),
                                );
                            }
                            v.prev_token_type = ValidationTokenType::Expr;
                            v.expect_expression = false;
                        }
                    }
                }
                TokenType::Operator(op) => match op {
                    OperatorTokenType::ParenOpen => {
                        operator_stack.push(ShuntingYardOperatorResult {
                            op_type: op.clone(),
                            index_into_tokens: input_index,
                        });
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
                            ShuntingYard::rollback(
                                &mut operator_stack,
                                output_stack,
                                input_index + 1,
                                &mut v,
                            );
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
                            to_out(
                                output_stack,
                                &fn_token_type,
                                fn_entry.fn_token_index as isize,
                            );
                        }
                        if v.can_be_valid_closing_token() && !output_stack.is_empty() {
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                                &mut v.last_valid_operator_index,
                                &mut v.last_valid_output_range,
                            );
                            v.close_valid_range(
                                output_stack.len(),
                                input_index,
                                operator_stack.len(),
                            );
                        }
                    }
                    OperatorTokenType::BracketOpen => {
                        if v.open_brackets > 0 || !v.expect_expression {
                            ShuntingYard::rollback(
                                &mut operator_stack,
                                output_stack,
                                input_index,
                                &mut v,
                            );
                        }
                        if tokens
                            .get(input_index as usize + 1)
                            .map(|it| {
                                matches!(
                                    it.typ,
                                    TokenType::Operator(OperatorTokenType::BracketClose)
                                )
                            })
                            .unwrap_or(false)
                        {
                            let matrix_token_type =
                                TokenType::Operator(OperatorTokenType::Matrix {
                                    row_count: 1,
                                    col_count: 1,
                                });
                            to_out(output_stack, &matrix_token_type, input_index);
                            tokens.insert(
                                input_index as usize,
                                Token {
                                    ptr: &[],
                                    typ: matrix_token_type.clone(),
                                    has_error: false,
                                },
                            );
                            // we inserted one element and we parsed the next one
                            input_index += 2;
                            if v.can_be_valid_closing_token() {
                                ShuntingYard::send_everything_to_output(
                                    &mut operator_stack,
                                    output_stack,
                                    &mut v.last_valid_operator_index,
                                    &mut v.last_valid_output_range,
                                );
                                v.close_valid_range(
                                    output_stack.len(),
                                    input_index,
                                    operator_stack.len(),
                                );
                            }
                            continue;
                        }

                        v.open_brackets += 1;
                        v.prev_token_type = ValidationTokenType::Nothing;
                        v.parenthesis_stack
                            .push(ParenStackEntry::new_mat(input_index));
                        operator_stack.push(ShuntingYardOperatorResult {
                            op_type: op.clone(),
                            index_into_tokens: input_index,
                        });
                    }
                    OperatorTokenType::BracketClose => {
                        if v.expect_expression || v.open_brackets == 0 || v.is_matrix_row_len_err()
                        {
                            ShuntingYard::rollback(
                                &mut operator_stack,
                                output_stack,
                                input_index + 1,
                                &mut v,
                            );
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
                        to_out(output_stack, &matrix_token_type, input_index);
                        tokens.insert(
                            mat_entry.matrix_start_input_pos,
                            Token {
                                ptr: &[],
                                typ: matrix_token_type.clone(),
                                has_error: false,
                            },
                        );
                        // we inserted one element so increase it
                        input_index += 1;
                        if v.can_be_valid_closing_token() {
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                                &mut v.last_valid_operator_index,
                                &mut v.last_valid_output_range,
                            );
                            v.close_valid_range(
                                output_stack.len(),
                                input_index,
                                operator_stack.len(),
                            );
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
                            ShuntingYard::rollback(
                                &mut operator_stack,
                                output_stack,
                                input_index + 1,
                                &mut v,
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
                            operator_stack.push(ShuntingYardOperatorResult {
                                op_type: OperatorTokenType::UnaryMinus,
                                index_into_tokens: input_index,
                            });
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
                            ShuntingYard::rollback(
                                &mut operator_stack,
                                output_stack,
                                input_index + 1,
                                &mut v,
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
                        }
                        operator_stack.clear();
                        continue;
                    }
                    OperatorTokenType::Comma => {
                        if v.is_comma_not_allowed() {
                            ShuntingYard::rollback(
                                &mut operator_stack,
                                output_stack,
                                input_index + 1,
                                &mut v,
                            );
                            continue;
                        }
                        v.prev_token_type = ValidationTokenType::Nothing;
                        v.expect_expression = true;
                        v.do_comma();
                        ShuntingYard::operator_rule(
                            op.precedence(),
                            op.assoc(),
                            &mut operator_stack,
                            output_stack,
                            &mut v.last_valid_operator_index,
                            &mut v.last_valid_output_range,
                            input_index,
                        );
                    }
                    OperatorTokenType::Semicolon => {
                        if v.open_brackets == 0 || v.is_matrix_row_len_err() {
                            ShuntingYard::rollback(
                                &mut operator_stack,
                                output_stack,
                                input_index + 1,
                                &mut v,
                            );
                            continue;
                        }
                        v.prev_token_type = ValidationTokenType::Nothing;
                        v.expect_expression = true;
                        v.matrix_new_row();
                        ShuntingYard::operator_rule(
                            op.precedence(),
                            op.assoc(),
                            &mut operator_stack,
                            output_stack,
                            &mut v.last_valid_operator_index,
                            &mut v.last_valid_output_range,
                            input_index,
                        );
                    }
                    OperatorTokenType::Perc => {
                        to_out2(output_stack, TokenType::Operator(op.clone()), input_index);
                        v.prev_token_type = ValidationTokenType::Expr;
                        if v.can_be_valid_closing_token() {
                            ShuntingYard::send_everything_to_output(
                                &mut operator_stack,
                                output_stack,
                                &mut v.last_valid_operator_index,
                                &mut v.last_valid_output_range,
                            );
                            v.close_valid_range(
                                output_stack.len(),
                                input_index,
                                operator_stack.len(),
                            );
                        }
                    }
                    OperatorTokenType::ApplyUnit => {
                        // ignore it, It is handled above inside TokenType::Unit
                    }
                    OperatorTokenType::UnitConverter => {
                        // the converter must be the last operator, only a unit can follow it
                        // so clear the operator stack, push the next unit onto the output

                        // push the unit onto the output, and close it

                        // TODO asd ha a köv token egy unit és az azutánui az autolsó ami egy applyunit
                        match ShuntingYard::get_next_nonstring_token(
                            tokens,
                            input_index as usize + 1,
                        ) {
                            Some((
                                Token {
                                    typ: TokenType::Unit(unit),
                                    ..
                                },
                                offset,
                            )) if ShuntingYard::get_next_nonstring_token(
                                tokens,
                                input_index as usize + 1 + offset + 1,
                            )
                            .is_none() =>
                            {
                                ShuntingYard::operator_token_type_unit_converter(
                                    output_stack,
                                    &mut operator_stack,
                                    &mut v,
                                    &mut input_index,
                                    op,
                                    unit,
                                    offset,
                                );
                            }
                            // last token is UnitConverter a.k.a. 'inch'
                            Some((
                                Token {
                                    typ: TokenType::Operator(OperatorTokenType::UnitConverter),
                                    ..
                                },
                                offset,
                            )) if ShuntingYard::get_next_nonstring_token(
                                tokens,
                                input_index as usize + 1 + offset + 1,
                            )
                            .is_none() =>
                            {
                                let unit = UnitOutput::new_inch(units);
                                ShuntingYard::operator_token_type_unit_converter(
                                    output_stack,
                                    &mut operator_stack,
                                    &mut v,
                                    &mut input_index,
                                    op,
                                    &unit,
                                    offset,
                                );
                            }
                            _ => {
                                // demote it to a unit or a string
                                if let Some(top) = output_stack.last() {
                                    match top {
                                        ShuntingYardResult {
                                            typ: TokenType::Operator(OperatorTokenType::ApplyUnit),
                                            ..
                                        } => {
                                            debug_print("  convert to String");
                                            tokens[input_index as usize].typ =
                                                TokenType::StringLiteral;
                                        }
                                        _ => {
                                            debug_print("  convert to Unit(inch)");
                                            let unit = UnitOutput::new_inch(units);
                                            tokens[input_index as usize].typ =
                                                TokenType::Unit(unit);
                                        }
                                    }
                                }
                                // and reparse it
                                input_index -= 1;
                            }
                        }
                    }
                    OperatorTokenType::UnaryPlus | OperatorTokenType::UnaryMinus => {
                        panic!("Token parser does not generate unary operators");
                    }
                    _ => {
                        if !matches!(
                            op,
                            OperatorTokenType::BinNot
                                | OperatorTokenType::Percentage_Find_Base_From_X_Icrease_Result
                                | OperatorTokenType::Percentage_Find_Base_From_X_Decrease_Result
                        ) && v.expect_expression
                        {
                            ShuntingYard::rollback(
                                &mut operator_stack,
                                output_stack,
                                input_index + 1,
                                &mut v,
                            );
                            continue;
                        }
                        v.had_operator = true;
                        v.expect_expression = if matches!(
                            op,
                            OperatorTokenType::Percentage_Find_Base_From_Result_Increase_X
                                | OperatorTokenType::Percentage_Find_Base_From_Result_Decrease_X
                        ) {
                            false
                        } else {
                            true
                        };
                        v.prev_token_type = ValidationTokenType::Op;
                        ShuntingYard::operator_rule(
                            op.precedence(),
                            op.assoc(),
                            &mut operator_stack,
                            output_stack,
                            &mut v.last_valid_operator_index,
                            &mut v.last_valid_output_range,
                            input_index,
                        );
                        operator_stack.push(ShuntingYardOperatorResult {
                            op_type: op.clone(),
                            index_into_tokens: input_index,
                        });
                        ShuntingYard::update_in_progress_percentage_op(
                            &op,
                            &mut v,
                            input_index,
                            output_stack,
                            &operator_stack,
                        );
                        ShuntingYard::shunting_state_debug_print(
                            "  token is operator",
                            output_stack,
                            &operator_stack,
                        );
                    }
                },
                TokenType::NumberErr => {
                    ShuntingYard::handle_num_token(
                        TokenType::NumberErr,
                        &mut v,
                        tokens,
                        output_stack,
                        &mut operator_stack,
                        &mut input_index,
                    );
                }
                TokenType::NumberLiteral(num) => {
                    // TODO nézd meg muszáj e klnozni, ne me tudja ez a fv átvenni az ownershipet
                    // a input_tokens felett, vagy az outputban nem e lehetnek pointerek
                    let num = num.clone();
                    ShuntingYard::handle_num_token(
                        TokenType::NumberLiteral(if v.neg { (&num).neg() } else { num }),
                        &mut v,
                        tokens,
                        output_stack,
                        &mut operator_stack,
                        &mut input_index,
                    );
                }
                TokenType::Variable { .. } | TokenType::LineReference { .. } => {
                    if !v.expect_expression {
                        ShuntingYard::rollback(
                            &mut operator_stack,
                            output_stack,
                            input_index + 1,
                            &mut v,
                        );
                        continue;
                    }
                    // so variables can be reassigned
                    v.had_non_ws_string_literal = true;
                    to_out(output_stack, &input_token.typ, input_index);
                    if (v.last_valid_output_range.is_none() || v.had_operator)
                        && v.parenthesis_stack.is_empty()
                    {
                        // set everything to string which is in front of this expr
                        v.close_valid_range(output_stack.len(), input_index, operator_stack.len());
                    }
                    v.prev_token_type = ValidationTokenType::Expr;
                    v.expect_expression = false;
                }
            }
        }

        if v.last_valid_output_range.is_some() {
            ShuntingYard::send_everything_to_output(
                &mut operator_stack,
                output_stack,
                &mut v.last_valid_operator_index,
                &mut v.last_valid_output_range,
            );
        }

        // output_stack can be empty since the Assign operator is put
        // to the end of  the list at the end of this method
        if v.is_valid_assignment_expression() && !output_stack.is_empty() {
            // close it
            // set everything to string which is in front of this expr
            v.close_valid_range(output_stack.len(), input_index, operator_stack.len());
            ShuntingYard::set_tokens_to_string(tokens, 0, v.valid_range_start_token_index - 1);
        }

        ShuntingYard::shunting_state_debug_print(
            "before into iter rev",
            output_stack,
            &operator_stack,
        );
        for op in operator_stack.into_iter().rev() {
            match op.op_type {
                OperatorTokenType::Percentage_Find_Base_From_Result_Increase_X
                | OperatorTokenType::Percentage_Find_Base_From_X_Icrease_Result
                | OperatorTokenType::Percentage_Find_Base_From_Result_Rate => {
                    // the top must be PercentageIs to be valid
                    let output_stack_len = output_stack.len();
                    let ok = if let Some(top) = output_stack.last_mut() {
                        if matches!(
                            top.typ,
                            TokenType::Operator(OperatorTokenType::PercentageIs)
                        ) {
                            // is it the last valid token?
                            if let Some((_start, end)) = &v.last_valid_output_range {
                                if *end == output_stack_len - 1 {
                                    top.typ = TokenType::Operator(op.op_type);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if ok {
                        v.close_valid_range(output_stack.len(), input_index, 0);
                    }
                }
                _ => {
                    // ignore
                }
            }
        }
        ShuntingYard::shunting_state_debug_print("after into iter rev", output_stack, &[]);

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

        // in calc, the assignment operator does nothing else but flag
        // the expression as "assignment", so we can put it to the end of the stack,
        // it is simpler and won't cause any trouble
        if !output_stack.is_empty() && v.assign_op_input_token_pos.is_some() {
            if let Some(assign_op_input_token_pos) = v.assign_op_input_token_pos {
                output_stack.push(ShuntingYardResult::new(
                    TokenType::Operator(OperatorTokenType::Assign),
                    assign_op_input_token_pos,
                ))
            }
        }
    }

    fn update_in_progress_percentage_op(
        op: &OperatorTokenType,
        v: &mut ValidationState,
        input_index: isize,
        output_stack: &[ShuntingYardResult],
        operator_stack: &[ShuntingYardOperatorResult],
    ) {
        match (&v.in_progress_percentage_op, op) {
            // 41 is 17% on what
            // 41 is 17% off what
            (
                Some(OperatorTokenType::PercentageIs),
                OperatorTokenType::Percentage_Find_Base_From_Result_Increase_X,
            )
            | (
                Some(OperatorTokenType::PercentageIs),
                OperatorTokenType::Percentage_Find_Base_From_Result_Decrease_X,
            )
            | (
                Some(OperatorTokenType::PercentageIs),
                OperatorTokenType::Percentage_Find_Base_From_Result_Rate,
            ) => {
                v.in_progress_percentage_op = None;
                if v.can_be_valid_closing_token() {
                    v.close_valid_range(output_stack.len(), input_index, operator_stack.len());
                }
            }
            (None, OperatorTokenType::PercentageIs) => {
                v.in_progress_percentage_op = Some(OperatorTokenType::PercentageIs);
            }
            // what plus 17% is 41
            // what minus 17% is 41
            (None, OperatorTokenType::Percentage_Find_Base_From_X_Icrease_Result)
            | (None, OperatorTokenType::Percentage_Find_Base_From_X_Decrease_Result) => {
                v.in_progress_percentage_op = Some(op.clone());
            }
            (
                Some(OperatorTokenType::Percentage_Find_Base_From_X_Icrease_Result),
                OperatorTokenType::PercentageIs,
            )
            | (
                Some(OperatorTokenType::Percentage_Find_Base_From_X_Decrease_Result),
                OperatorTokenType::PercentageIs,
            ) => {
                v.in_progress_percentage_op = None;
            }
            // 17% on what is 41
            (None, OperatorTokenType::Percentage_Find_Base_From_Icrease_X_Result)
            | (None, OperatorTokenType::Percentage_Find_Base_From_Decrease_X_Result) => {
                v.in_progress_percentage_op = None;
            }
            _ => {
                // ignore
            }
        }
        debug_print(&format!(
            "  in_progress_percentage_op: {:?}",
            &v.in_progress_percentage_op
        ));
    }

    #[allow(dead_code)]
    #[allow(unused_variables)]
    fn shunting_state_debug_print<'text_ptr>(
        where_: &str,
        output_stack: &[ShuntingYardResult],
        operator_stack: &[ShuntingYardOperatorResult],
    ) {
        if false {
            return;
        }
        #[cfg(debug_assertions)]
        {
            use crate::token_parser::pad_rust_is_shit;
            let mut msg = String::with_capacity(200);
            msg.push_str(where_);
            msg.push('\n');
            msg.push_str("    [operator_stack]\n    ");
            for op in operator_stack {
                pad_rust_is_shit(&mut msg, &format!("        {:?}", op.op_type), 35);
            }
            msg.push_str("\n\n    [output_stack]\n    ");
            for token in output_stack {
                pad_rust_is_shit(&mut msg, &format!("        {:?}", token.typ), 35);
            }
            println!("{}", msg);
        }
    }

    fn operator_token_type_unit_converter(
        output_stack: &mut Vec<ShuntingYardResult>,
        operator_stack: &mut Vec<ShuntingYardOperatorResult>,
        v: &mut ValidationState,
        input_index: &mut isize,
        op: &OperatorTokenType,
        unit: &UnitOutput,
        offset: usize,
    ) {
        v.expect_expression = false;
        v.prev_token_type = ValidationTokenType::Op;

        *input_index += 1 + offset as isize;
        if v.can_be_valid_closing_token() {
            ShuntingYard::send_everything_to_output(
                operator_stack,
                output_stack,
                &mut v.last_valid_operator_index,
                &mut v.last_valid_output_range,
            );
            to_out2(output_stack, TokenType::Unit(unit.clone()), *input_index);
            to_out2(output_stack, TokenType::Operator(op.clone()), *input_index);
            v.close_valid_range(output_stack.len(), *input_index, operator_stack.len());
        }
    }

    fn handle_num_token<'text_ptr>(
        into_output: TokenType,
        v: &mut ValidationState,
        tokens: &[Token<'text_ptr>],
        output_stack: &mut Vec<ShuntingYardResult>,
        operator_stack: &mut Vec<ShuntingYardOperatorResult>,
        input_index: &mut isize,
    ) {
        if !v.expect_expression {
            ShuntingYard::rollback(operator_stack, output_stack, *input_index, v);
        }
        to_out2(output_stack, into_output, *input_index);
        v.neg = false;
        if v.can_be_valid_closing_token() {
            if let Some((next_token, offset)) =
                ShuntingYard::get_next_nonstring_token(tokens, *input_index as usize + 1)
            {
                if let TokenType::Operator(OperatorTokenType::Perc) = next_token.typ {
                    // if the next token is '%', push it to the stack immediately, and
                    // skip the next iteration
                    *input_index += 1 + offset as isize;
                    to_out2(
                        output_stack,
                        TokenType::Operator(OperatorTokenType::Perc),
                        *input_index,
                    );
                }
            }

            if v.last_valid_output_range.is_none() || v.had_operator {
                // // set everything to string which is in front of this expr
                v.close_valid_range(output_stack.len(), *input_index, operator_stack.len());
            }
        }
        v.prev_token_type = ValidationTokenType::Expr;
        v.expect_expression = false;
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
        tokens: &'a [Token<'text_ptr>],
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
        incoming_op_prec: usize,
        incoming_op_assoc: Assoc,
        operator_stack: &mut Vec<ShuntingYardOperatorResult>,
        output: &mut Vec<ShuntingYardResult>,
        maybe_last_valid_operator_index: &mut Option<usize>,
        last_valid_output_range: &mut Option<(usize, usize)>,
        input_token_index: isize,
    ) {
        if operator_stack.is_empty() {
            return;
        }
        let top_of_stack = &operator_stack[operator_stack.len() - 1];

        if matches!(top_of_stack.op_type, OperatorTokenType::ParenOpen)
            || matches!(top_of_stack.op_type, OperatorTokenType::ParenClose)
            || matches!(top_of_stack.op_type, OperatorTokenType::BracketOpen)
            || matches!(top_of_stack.op_type, OperatorTokenType::BracketClose)
        {
            return;
        }
        let top_of_stack_precedence = top_of_stack.op_type.precedence();
        let incoming_prec_left_assoc_and_equal =
            incoming_op_assoc == Assoc::Left && incoming_op_prec == top_of_stack_precedence;
        if incoming_op_prec < top_of_stack_precedence || incoming_prec_left_assoc_and_equal {
            if let Some(last_valid_operator_index) = maybe_last_valid_operator_index.as_mut() {
                if *last_valid_operator_index == (operator_stack.len() - 1) {
                    *maybe_last_valid_operator_index = None;
                    last_valid_output_range.as_mut().expect("ok").1 += 1;
                }
            }
            to_out2(
                output,
                TokenType::Operator(top_of_stack.op_type.clone()),
                top_of_stack.index_into_tokens,
            );
            operator_stack.pop();
            ShuntingYard::operator_rule(
                incoming_op_prec,
                incoming_op_assoc,
                operator_stack,
                output,
                maybe_last_valid_operator_index,
                last_valid_output_range,
                input_token_index,
            );
        } else {
            // do nothing
        }
    }

    fn rollback(
        operator_stack: &mut Vec<ShuntingYardOperatorResult>,
        output_stack: &mut Vec<ShuntingYardResult>,
        token_index: isize,
        v: &mut ValidationState,
    ) {
        debug_print(&format!("  rollback"));
        ShuntingYard::send_everything_to_output(
            operator_stack,
            output_stack,
            &mut v.last_valid_operator_index,
            &mut v.last_valid_output_range,
        );
        operator_stack.clear();
        v.reset(output_stack.len(), token_index);
    }

    fn send_everything_to_output(
        operator_stack: &mut Vec<ShuntingYardOperatorResult>,
        output_stack: &mut Vec<ShuntingYardResult>,
        maybe_last_valid_operator_index: &mut Option<usize>,
        last_valid_output_range: &mut Option<(usize, usize)>,
    ) {
        if let Some(last_valid_operator_index) = *maybe_last_valid_operator_index {
            if operator_stack.len() <= last_valid_operator_index {
                return;
            }
            for op in operator_stack.drain(0..=last_valid_operator_index).rev() {
                to_out2(
                    output_stack,
                    TokenType::Operator(op.op_type),
                    op.index_into_tokens,
                );
                last_valid_output_range.as_mut().expect("ok").1 += 1;
            }
            *maybe_last_valid_operator_index = None;
        }
        ShuntingYard::shunting_state_debug_print(
            "    send_everything_to_output",
            output_stack,
            operator_stack,
        );
    }

    fn send_anything_until_opening_bracket(
        operator_stack: &mut Vec<ShuntingYardOperatorResult>,
        output: &mut Vec<ShuntingYardResult>,
        open_paren_type: &OperatorTokenType,
    ) {
        if operator_stack.is_empty() {
            return;
        }
        let top_of_op_stack = operator_stack.pop().unwrap();
        if &top_of_op_stack.op_type == open_paren_type {
            return;
        } else {
            to_out2(
                output,
                TokenType::Operator(top_of_op_stack.op_type),
                top_of_op_stack.index_into_tokens,
            );
        }
        return ShuntingYard::send_anything_until_opening_bracket(
            operator_stack,
            output,
            open_paren_type,
        );
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::calc::{CalcResult, CalcResultType};
    use crate::helper::create_vars;
    use crate::token_parser::tests::print_tokens_compare_error_and_panic;
    use crate::token_parser::TokenParser;
    use crate::units::units::{UnitOutput, Units};
    use crate::{Variable, Variables, MAX_LINE_COUNT};
    use bumpalo::Bump;
    use rust_decimal::prelude::*;

    pub fn s_num<'text_ptr>(n: i64) -> Token<'text_ptr> {
        Token {
            ptr: &[],
            typ: TokenType::NumberLiteral(n.into()),
            has_error: false,
        }
    }

    pub fn num<'text_ptr>(n: u64) -> Token<'text_ptr> {
        Token {
            ptr: &[],
            typ: TokenType::NumberLiteral(n.into()),
            has_error: false,
        }
    }

    pub fn num_with_err<'text_ptr>(n: i64) -> Token<'text_ptr> {
        Token {
            ptr: &[],
            typ: TokenType::NumberLiteral(n.into()),
            has_error: true,
        }
    }

    pub fn num_err<'text_ptr>() -> Token<'text_ptr> {
        Token {
            ptr: &[],
            typ: TokenType::NumberErr,
            has_error: true,
        }
    }

    pub fn op<'text_ptr>(op_repr: OperatorTokenType) -> Token<'text_ptr> {
        Token {
            ptr: &[],
            typ: TokenType::Operator(op_repr),
            has_error: false,
        }
    }

    pub fn op_err<'text_ptr>(op_repr: OperatorTokenType) -> Token<'text_ptr> {
        Token {
            ptr: &[],
            typ: TokenType::Operator(op_repr),
            has_error: true,
        }
    }

    pub fn str<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::StringLiteral,
            has_error: false,
        }
    }

    pub fn header<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::Header,
            has_error: false,
        }
    }

    pub fn apply_to_prev_token_unit<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::Operator(OperatorTokenType::ApplyUnit),
            has_error: false,
        }
    }

    pub fn unit<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::Unit(UnitOutput::new()),
            has_error: false,
        }
    }

    pub fn var<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::Variable { var_index: 0 },
            has_error: false,
        }
    }

    pub fn line_ref<'text_ptr>(op_repr: &'static str) -> Token<'text_ptr> {
        Token {
            ptr: unsafe { std::mem::transmute(op_repr) },
            typ: TokenType::LineReference { var_index: 0 },
            has_error: false,
        }
    }

    pub fn numf<'text_ptr>(n: f64) -> Token<'text_ptr> {
        Token {
            ptr: &[],
            typ: TokenType::NumberLiteral(Decimal::from_f64(n).unwrap()),
            has_error: false,
        }
    }

    pub fn compare_tokens(input: &str, expected_tokens: &[Token], actual_tokens: &[Token]) {
        if expected_tokens.len() != actual_tokens.len() {
            print_tokens_compare_error_and_panic(
                &format!("Text: {}\nActual Tokens", input),
                actual_tokens,
                expected_tokens,
                None,
            );
        }
        for (index, (actual_token, expected_token)) in
            actual_tokens.iter().zip(expected_tokens.iter()).enumerate()
        {
            if actual_token.has_error != expected_token.has_error {
                print_tokens_compare_error_and_panic(
                    &format!("Text: {}\nActual Tokens\n", input),
                    actual_tokens,
                    &expected_tokens,
                    Some((index, expected_token)),
                );
            }
            match (&expected_token.typ, &actual_token.typ) {
                (TokenType::Unit(..), TokenType::Unit(actual_unit)) => {
                    //     expected_op is an &str
                    let str_slice = unsafe { std::mem::transmute::<_, &str>(expected_token.ptr) };
                    assert_eq!(&actual_unit.to_string(), str_slice)
                }
                (TokenType::StringLiteral, TokenType::StringLiteral)
                | (TokenType::Header, TokenType::Header)
                | (TokenType::Variable { .. }, TokenType::Variable { .. })
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
                        &trimmed_actual, &expected_chars,
                        "actual tokens: {:?}",
                        &actual_tokens
                    )
                }
                _ => {
                    if expected_token.typ != actual_token.typ {
                        print_tokens_compare_error_and_panic(
                            &format!("Text: {}\nActual Tokens\n", input),
                            actual_tokens,
                            &expected_tokens,
                            Some((index, expected_token)),
                        );
                    }
                }
            }
        }
    }

    pub fn do_shunting_yard<'text_ptr, 'units, 'b>(
        text: &[char],
        units: &'units Units,
        tokens: &mut Vec<Token<'text_ptr>>,
        vars: &'b Variables,
        allocator: &'text_ptr Bump,
    ) -> Vec<ShuntingYardResult> {
        let mut output = vec![];
        TokenParser::parse_line(&text, vars, tokens, &units, 10, allocator);
        ShuntingYard::shunting_yard(tokens, &mut output, units);
        return output;
    }

    fn test_output_vars(var_names: &[&'static [char]], text: &str, expected_tokens_: &[Token]) {
        let mut expected_tokens = Vec::with_capacity(expected_tokens_.len());
        for t in expected_tokens_ {
            if matches!(t.typ, TokenType::Operator(OperatorTokenType::ApplyUnit)) {
                expected_tokens.push(unit(unsafe { std::mem::transmute(t.ptr) }));
            }
            expected_tokens.push(t.clone());
        }
        let var_names: Vec<Option<Variable>> = (0..MAX_LINE_COUNT + 1)
            .into_iter()
            .map(|index| {
                if let Some(var_name) = var_names.get(index) {
                    Some(Variable {
                        name: Box::from(*var_name),
                        value: Err(()),
                    })
                } else {
                    None
                }
            })
            .collect();

        println!("===================================================");
        println!("{}", text);
        let temp = text.chars().collect::<Vec<char>>();
        let units = Units::new();
        let mut tokens = vec![];
        let output = do_shunting_yard(&temp, &units, &mut tokens, &var_names, &Bump::new());
        compare_tokens(
            text,
            &expected_tokens,
            output
                .iter()
                .map(|it| Token {
                    ptr: &[],
                    typ: it.typ.clone(),
                    has_error: false,
                })
                .collect::<Vec<_>>()
                .as_slice(),
        );
    }

    fn test_output(text: &str, expected_tokens: &[Token]) {
        test_output_vars(&[], text, expected_tokens);
    }

    fn test_tokens(text: &str, expected_tokens_: &[Token]) {
        let mut expected_tokens = Vec::with_capacity(expected_tokens_.len());
        for t in expected_tokens_ {
            if matches!(t.typ, TokenType::Operator(OperatorTokenType::ApplyUnit)) {
                expected_tokens.push(unit(unsafe { std::mem::transmute(t.ptr) }));
            }
            expected_tokens.push(t.clone());
        }
        println!("===================================================");
        println!("{}", text);
        let temp = text.chars().collect::<Vec<char>>();
        let units = Units::new();
        let mut tokens = vec![];
        let arena = Bump::new();
        let mut vars = create_vars();
        vars[0] = Some(Variable {
            name: Box::from(&['b', '0'][..]),
            value: Ok(CalcResult::new(CalcResultType::Number(Decimal::zero()), 0)),
        });
        vars[1] = Some(Variable {
            name: Box::from(&['&', '[', '1', ']'][..]),
            value: Ok(CalcResult::new(CalcResultType::Number(Decimal::zero()), 0)),
        });
        let _ = do_shunting_yard(&temp, &units, &mut tokens, &vars, &arena);
        compare_tokens(text, &expected_tokens, &tokens);
    }

    #[test]
    fn test1() {
        test_output(
            "1/2s",
            &[
                num(1),
                num(2),
                apply_to_prev_token_unit("s"),
                op(OperatorTokenType::Div),
            ],
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
                apply_to_prev_token_unit("km / h"),
                num(45),
                apply_to_prev_token_unit("min"),
                op(OperatorTokenType::Mult),
            ],
        );

        test_output(
            "10km/h * 45min * 12 km",
            &[
                num(10),
                apply_to_prev_token_unit("km / h"),
                num(45),
                apply_to_prev_token_unit("min"),
                op(OperatorTokenType::Mult),
                num(12),
                apply_to_prev_token_unit("km"),
                op(OperatorTokenType::Mult),
            ],
        );

        test_output(
            "10km/h * 45min * 12 km in h",
            &[
                num(10),
                apply_to_prev_token_unit("km / h"),
                num(45),
                apply_to_prev_token_unit("min"),
                op(OperatorTokenType::Mult),
                num(12),
                apply_to_prev_token_unit("km"),
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
    fn test_precedence() {
        test_output(
            "1+2*3",
            &[
                num(1),
                num(2),
                num(3),
                op(OperatorTokenType::Mult),
                op(OperatorTokenType::Add),
            ],
        );
        test_output(
            "1+2*3^4",
            &[
                num(1),
                num(2),
                num(3),
                num(4),
                op(OperatorTokenType::Pow),
                op(OperatorTokenType::Mult),
                op(OperatorTokenType::Add),
            ],
        );
    }

    #[test]
    fn test_binary_not() {
        test_output("NOT(0b11)", &[num(0b11), op(OperatorTokenType::BinNot)]);
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
        test_output(
            "-1 + -2",
            &[s_num(-1), s_num(-2), op(OperatorTokenType::Add)],
        );
        test_output("-1+-2", &[s_num(-1), s_num(-2), op(OperatorTokenType::Add)]);
        test_output(
            "-1 - -2",
            &[s_num(-1), s_num(-2), op(OperatorTokenType::Sub)],
        );
        test_output("-1--2", &[s_num(-1), s_num(-2), op(OperatorTokenType::Sub)]);
        test_output("+1-+2", &[num(1), num(2), op(OperatorTokenType::Sub)]);
        test_output("+1++2", &[num(1), num(2), op(OperatorTokenType::Add)]);
        test_output("2^-2", &[num(2), s_num(-2), op(OperatorTokenType::Pow)]);

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
    fn test_in_should_be_excluded_here() {
        // typo: the text contain 'lbG' and not lbF
        test_output(
            "100 ft * lbf in (in*lbg)",
            &[num(100), apply_to_prev_token_unit("ft lbf")],
        );
    }

    #[test]
    fn test2() {
        test_output("", &[]);
        test_output("2", &[num(2)]);

        test_output(
            "2m/3m",
            &[
                num(2),
                apply_to_prev_token_unit("m"),
                num(3),
                apply_to_prev_token_unit("m"),
                op(OperatorTokenType::Div),
            ],
        );

        test_output(
            "2/3m",
            &[
                num(2),
                num(3),
                apply_to_prev_token_unit("m"),
                op(OperatorTokenType::Div),
            ],
        );

        test_output(
            "5km + 5cm",
            &[
                num(5),
                apply_to_prev_token_unit("km"),
                num(5),
                apply_to_prev_token_unit("cm"),
                op(OperatorTokenType::Add),
            ],
        );

        test_output(
            "100 ft * lbf in (in*lbf)",
            &[
                num(100),
                apply_to_prev_token_unit("ft lbf"),
                unit("in lbf"),
                op(OperatorTokenType::UnitConverter),
            ],
        );

        test_tokens(
            "100 ft * lbf in (in*lbf)",
            &[
                num(100),
                str(" "),
                apply_to_prev_token_unit("ft lbf"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("in lbf"),
            ],
        );

        test_tokens(
            "1 Kib/s in b/s ",
            &[
                num(1),
                str(" "),
                apply_to_prev_token_unit("Kib / s"),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("b / s"),
                str(" "),
            ],
        );

        // typo: the text contain 'lbG' and not lbF
        test_output(
            "100 ft * lbf in (in*lbg) 1 + 100",
            &[num(1), num(100), op(OperatorTokenType::Add)],
        );
        test_tokens(
            "100 ft * lbf in (in*lbg) 1 + 100",
            &[
                str("100"),
                str(" "),
                str("ft * lbf"),
                str(" "),
                str("in"),
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
            "12km/h*45s ^^",
            &[
                num(12),
                apply_to_prev_token_unit("km / h"),
                num(45),
                apply_to_prev_token_unit("s"),
                op(OperatorTokenType::Mult),
            ],
        );

        test_output(
            "12km/h * 45s ^^",
            &[
                num(12),
                apply_to_prev_token_unit("km / h"),
                num(45),
                apply_to_prev_token_unit("s"),
                op(OperatorTokenType::Mult),
            ],
        );
        test_tokens(
            "12km/h * 45s ^^",
            &[
                num(12),
                apply_to_prev_token_unit("km / h"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(45),
                apply_to_prev_token_unit("s"),
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
            "75 - 15 euróból kell adózni mert 15 EUR adómentes",
            &[num(75), num(15), op(OperatorTokenType::Sub)],
        );
        test_output(
            "15 EUR adómentes azaz 75-15 euróból kell adózni",
            &[num(75), num(15), op(OperatorTokenType::Sub)],
        );
    }

    #[test]
    fn test_invalid_unit_in_conversion_target() {
        test_tokens(
            "100 ft * lbf in (in*lbg)",
            &[
                num(100),
                str(" "),
                apply_to_prev_token_unit("ft lbf"),
                str(" "),
                str("in"),
                str(" "),
                str("("),
                str("in"),
                str("*"),
                str("lbg"),
                str(")"),
            ],
        )
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
        test_output("a = 12", &[num(12), op(OperatorTokenType::Assign)]);

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
                num(12),
                num(4),
                op(OperatorTokenType::Mult),
                op(OperatorTokenType::Assign),
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
        test_output("var(12*4) = 13", &[num(13), op(OperatorTokenType::Assign)]);
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
    fn test_panic() {
        test_tokens("()", &[str("("), str(")")]);
        test_tokens("() Hz", &[str("("), str(")"), str(" "), str("Hz")]);
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
            &[num(100), op(OperatorTokenType::Assign)],
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
                apply_to_prev_token_unit("degree"),
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
                apply_to_prev_token_unit("degree"),
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
    fn test_missing_arg_nth_panic() {
        test_tokens(
            "nth(,[1])",
            &[
                op(OperatorTokenType::Fn {
                    arg_count: 0,
                    typ: FnType::Nth,
                }),
                op(OperatorTokenType::ParenOpen),
                op(OperatorTokenType::Comma),
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 1,
                }),
                op(OperatorTokenType::BracketOpen),
                num(1),
                op(OperatorTokenType::BracketClose),
                op(OperatorTokenType::ParenClose),
            ],
        )
    }

    #[test]
    fn test_empty_matrix() {
        test_tokens(
            "[]",
            &[
                op(OperatorTokenType::Matrix {
                    row_count: 1,
                    col_count: 1,
                }),
                op(OperatorTokenType::BracketOpen),
                op(OperatorTokenType::BracketClose),
            ],
        )
    }

    #[test]
    fn test_fn_output() {
        test_output(
            "sin(60 degree)",
            &[
                num(60),
                apply_to_prev_token_unit("degree"),
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
                apply_to_prev_token_unit("degree"),
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
    fn test_header() {
        test_tokens("# header", &[header("# header")]);
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

    #[test]
    fn test_unit_in_denominator_tokens_with_parens() {
        test_tokens(
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
    fn test_that_pow_has_higher_precedence_than_unit() {
        test_output(
            "10^24kg",
            &[
                num(10),
                num(24),
                op(OperatorTokenType::Pow),
                apply_to_prev_token_unit("kg"),
            ],
        );
    }

    #[test]
    fn test_multiple_equal_signs() {
        test_output("z=1=2", &[num(1)]);
    }

    #[test]
    fn test_multiple_equal_signs2() {
        test_output(
            "=(Blq9h/Oq=7y^$o[/kR]*$*oReyMo-M++]",
            &[num(7), op(OperatorTokenType::Assign)],
        );
    }

    #[test]
    fn test_yl_parsing() {
        test_output("909636Yl", &[num(909636), apply_to_prev_token_unit("Yl")]);
    }

    #[test]
    fn test_fuzzing_issue1() {
        test_output(
            "90-/9b^72^4",
            &[
                num(9),
                apply_to_prev_token_unit("b^72"),
                num(4),
                op(OperatorTokenType::Pow),
            ],
        );
    }

    #[test]
    fn test_shunting_ininin() {
        test_output(
            "12 in in in",
            &[
                num(12),
                apply_to_prev_token_unit("in"),
                unit("in"),
                op(OperatorTokenType::UnitConverter),
            ],
        );
    }

    #[test]
    fn test_longer_texts2() {
        test_output(
            "transfer of around 1.587GB in about / 3 seconds",
            &[
                numf(1.587),
                apply_to_prev_token_unit("GB"),
                num(3),
                apply_to_prev_token_unit("second"),
                op(OperatorTokenType::Div),
            ],
        );
    }

    #[test]
    fn test_shunting_num_perc_on_what() {
        test_output(
            "41 is 17% on what",
            &[
                num(41),
                num(17),
                op(OperatorTokenType::Perc),
                op(OperatorTokenType::PercentageIs),
                op(OperatorTokenType::Percentage_Find_Base_From_Result_Increase_X),
            ],
        );
    }

    #[test]
    fn test_shunting_num_perc_on_what_tokens() {
        test_tokens(
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
    fn test_shunting_num_perc_on_what_paren_tokens() {
        test_tokens(
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
    fn test_shunting_percentage_what_plus() {
        test_output(
            "what plus 17% is 41",
            &[
                num(17),
                op(OperatorTokenType::Perc),
                num(41),
                op(OperatorTokenType::PercentageIs),
                op(OperatorTokenType::Percentage_Find_Base_From_X_Icrease_Result),
            ],
        );
    }

    #[test]
    fn test_shunting_percentage_what_plus_tokens() {
        test_tokens(
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
    fn test_shunting_percentage_on_what_is() {
        test_output(
            "17% on what is 41",
            &[
                num(17),
                op(OperatorTokenType::Perc),
                num(41),
                op(OperatorTokenType::Percentage_Find_Base_From_Icrease_X_Result),
            ],
        );
    }

    #[test]
    fn test_shunting_percentage_on_what_is_tokens() {
        test_tokens(
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
    fn test_shunting_num_what_perc_on_num_tokens() {
        test_tokens(
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
    fn test_shunting_num_what_perc_on_num() {
        test_output(
            "41 is what % on 35",
            &[
                num(41),
                num(35),
                op(OperatorTokenType::Percentage_Find_Incr_Rate_From_Result_X_Base),
            ],
        );
    }

    #[test]
    fn test_shunting_num_perc_off_what() {
        test_output(
            "41 is 17% off what",
            &[
                num(41),
                num(17),
                op(OperatorTokenType::Perc),
                op(OperatorTokenType::PercentageIs),
                op(OperatorTokenType::Percentage_Find_Base_From_Result_Decrease_X),
            ],
        );
    }

    #[test]
    fn test_shunting_num_perc_off_what_tokens() {
        test_tokens(
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
    fn test_shunting_num_perc_off_what_paren_tokens() {
        test_tokens(
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
    fn test_shunting_percentage_what_minus() {
        test_output(
            "what minus 17% is 41",
            &[
                num(17),
                op(OperatorTokenType::Perc),
                num(41),
                op(OperatorTokenType::PercentageIs),
                op(OperatorTokenType::Percentage_Find_Base_From_X_Decrease_Result),
            ],
        );
    }

    #[test]
    fn test_shunting_percentage_what_minus_tokens() {
        test_tokens(
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
    fn test_shunting_percentage_off_what_is() {
        test_output(
            "17% off what is 41",
            &[
                num(17),
                op(OperatorTokenType::Perc),
                num(41),
                op(OperatorTokenType::Percentage_Find_Base_From_Decrease_X_Result),
            ],
        );
    }

    #[test]
    fn test_shunting_percentage_off_what_is_tokens() {
        test_tokens(
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
    fn test_shunting_num_what_perc_off_num_tokens() {
        test_tokens(
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
    fn test_shunting_num_what_perc_off_num() {
        test_output(
            "41 is what % off 35",
            &[
                num(41),
                num(35),
                op(OperatorTokenType::Percentage_Find_Decr_Rate_From_Result_X_Base),
            ],
        );
    }

    #[test]
    fn test_shunting_percentage_find_rate_from_result_base() {
        test_output(
            "20 is what percent of 60",
            &[
                num(20),
                num(60),
                op(OperatorTokenType::Percentage_Find_Rate_From_Result_Base),
            ],
        );
    }

    #[test]
    fn test_shunting_percentage_find_rate_from_result_base_tokens() {
        test_tokens(
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
    fn test_shunting_percentage_find_base_from_result_rate() {
        test_output(
            "5 is 25% of what",
            &[
                num(5),
                num(25),
                op(OperatorTokenType::Perc),
                op(OperatorTokenType::PercentageIs),
                op(OperatorTokenType::Percentage_Find_Base_From_Result_Rate),
            ],
        );
    }

    #[test]
    fn test_shunting_percentage_find_base_from_result_rate_tokens() {
        test_tokens(
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
        test_tokens(
            "(256byte * 120) in MiB",
            &[
                op(OperatorTokenType::ParenOpen),
                num(256),
                apply_to_prev_token_unit("bytes"),
                str(" "),
                op(OperatorTokenType::Mult),
                str(" "),
                num(120),
                op(OperatorTokenType::ParenClose),
                str(" "),
                op(OperatorTokenType::UnitConverter),
                str(" "),
                unit("MiB"),
            ],
        );
    }

    #[test]
    fn test_unit_conversion_26_output() {
        test_output(
            "(256byte * 120) in MiB",
            &[
                num(256),
                apply_to_prev_token_unit("bytes"),
                num(120),
                op(OperatorTokenType::Mult),
                unit("MiB"),
                op(OperatorTokenType::UnitConverter),
            ],
        );
    }

    #[test]
    fn test_explicit_multipl_is_mandatory_before_units() {
        test_tokens(
            "2m^4kg/s^3",
            &[
                num(2),
                apply_to_prev_token_unit("m^4"),
                unit("kg / s^3"), // here, it must be unit for now, 'calc' can recognize that it is illegal and transform it to string
            ],
        );
        // it is the accepted form
        test_tokens(
            "2m^4*kg/s^3",
            &[num(2), apply_to_prev_token_unit("(m^4 kg) / s^3")],
        );
    }

    #[test]
    fn not_in_must_be_str_if_we_are_sure_it_cant_be_unit() {
        test_tokens(
            "12 m in",
            &[
                num(12),
                str(" "),
                apply_to_prev_token_unit("m"),
                str(" "),
                str("in"),
            ],
        );
    }
}
