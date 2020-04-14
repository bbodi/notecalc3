#![feature(ptr_offset_from, const_if_match, const_fn, const_panic, drain_filter)]

use crate::calc::evaluate_tokens;
use crate::editor::{Editor, InputKey, InputModifiers, Line, MAX_LINE_LEN};
use crate::shunting_yard::ShuntingYard;
use crate::token_parser::{OperatorTokenType, Token, TokenParser, TokenType};
use crate::units::consts::{create_prefixes, init_units};
use crate::units::units::Units;
use crate::units::UnitPrefixes;
use smallvec::SmallVec;

mod calc;
mod matrix;
mod shunting_yard;
mod token_parser;
mod units;

pub mod editor;
pub mod renderer;

const LINE_NUM_CONSTS: [[char; 3]; 256] = [
    [' ', ' ', '1'],
    [' ', ' ', '2'],
    [' ', ' ', '3'],
    [' ', ' ', '4'],
    [' ', ' ', '5'],
    [' ', ' ', '6'],
    [' ', ' ', '7'],
    [' ', ' ', '8'],
    [' ', ' ', '9'],
    [' ', '1', '0'],
    [' ', '1', '1'],
    [' ', '1', '2'],
    [' ', '1', '3'],
    [' ', '1', '4'],
    [' ', '1', '5'],
    [' ', '1', '6'],
    [' ', '1', '7'],
    [' ', '1', '8'],
    [' ', '1', '9'],
    [' ', '2', '0'],
    [' ', '2', '1'],
    [' ', '2', '2'],
    [' ', '2', '3'],
    [' ', '2', '4'],
    [' ', '2', '5'],
    [' ', '2', '6'],
    [' ', '2', '7'],
    [' ', '2', '8'],
    [' ', '2', '9'],
    [' ', '3', '0'],
    [' ', '3', '1'],
    [' ', '3', '2'],
    [' ', '3', '3'],
    [' ', '3', '4'],
    [' ', '3', '5'],
    [' ', '3', '6'],
    [' ', '3', '7'],
    [' ', '3', '8'],
    [' ', '3', '9'],
    [' ', '4', '0'],
    [' ', '4', '1'],
    [' ', '4', '2'],
    [' ', '4', '3'],
    [' ', '4', '4'],
    [' ', '4', '5'],
    [' ', '4', '6'],
    [' ', '4', '7'],
    [' ', '4', '8'],
    [' ', '4', '9'],
    [' ', '5', '0'],
    [' ', '5', '1'],
    [' ', '5', '2'],
    [' ', '5', '3'],
    [' ', '5', '4'],
    [' ', '5', '5'],
    [' ', '5', '6'],
    [' ', '5', '7'],
    [' ', '5', '8'],
    [' ', '5', '9'],
    [' ', '6', '0'],
    [' ', '6', '1'],
    [' ', '6', '2'],
    [' ', '6', '3'],
    [' ', '6', '4'],
    [' ', '6', '5'],
    [' ', '6', '6'],
    [' ', '6', '7'],
    [' ', '6', '8'],
    [' ', '6', '9'],
    [' ', '7', '0'],
    [' ', '7', '1'],
    [' ', '7', '2'],
    [' ', '7', '3'],
    [' ', '7', '4'],
    [' ', '7', '5'],
    [' ', '7', '6'],
    [' ', '7', '7'],
    [' ', '7', '8'],
    [' ', '7', '9'],
    [' ', '8', '0'],
    [' ', '8', '1'],
    [' ', '8', '2'],
    [' ', '8', '3'],
    [' ', '8', '4'],
    [' ', '8', '5'],
    [' ', '8', '6'],
    [' ', '8', '7'],
    [' ', '8', '8'],
    [' ', '8', '9'],
    [' ', '9', '0'],
    [' ', '9', '1'],
    [' ', '9', '2'],
    [' ', '9', '3'],
    [' ', '9', '4'],
    [' ', '9', '5'],
    [' ', '9', '6'],
    [' ', '9', '7'],
    [' ', '9', '8'],
    [' ', '9', '9'],
    ['1', '0', '0'],
    ['1', '0', '1'],
    ['1', '0', '2'],
    ['1', '0', '3'],
    ['1', '0', '4'],
    ['1', '0', '5'],
    ['1', '0', '6'],
    ['1', '0', '7'],
    ['1', '0', '8'],
    ['1', '0', '9'],
    ['1', '1', '0'],
    ['1', '1', '1'],
    ['1', '1', '2'],
    ['1', '1', '3'],
    ['1', '1', '4'],
    ['1', '1', '5'],
    ['1', '1', '6'],
    ['1', '1', '7'],
    ['1', '1', '8'],
    ['1', '1', '9'],
    ['1', '2', '0'],
    ['1', '2', '1'],
    ['1', '2', '2'],
    ['1', '2', '3'],
    ['1', '2', '4'],
    ['1', '2', '5'],
    ['1', '2', '6'],
    ['1', '2', '7'],
    ['1', '2', '8'],
    ['1', '2', '9'],
    ['1', '3', '0'],
    ['1', '3', '1'],
    ['1', '3', '2'],
    ['1', '3', '3'],
    ['1', '3', '4'],
    ['1', '3', '5'],
    ['1', '3', '6'],
    ['1', '3', '7'],
    ['1', '3', '8'],
    ['1', '3', '9'],
    ['1', '4', '0'],
    ['1', '4', '1'],
    ['1', '4', '2'],
    ['1', '4', '3'],
    ['1', '4', '4'],
    ['1', '4', '5'],
    ['1', '4', '6'],
    ['1', '4', '7'],
    ['1', '4', '8'],
    ['1', '4', '9'],
    ['1', '5', '0'],
    ['1', '5', '1'],
    ['1', '5', '2'],
    ['1', '5', '3'],
    ['1', '5', '4'],
    ['1', '5', '5'],
    ['1', '5', '6'],
    ['1', '5', '7'],
    ['1', '5', '8'],
    ['1', '5', '9'],
    ['1', '6', '0'],
    ['1', '6', '1'],
    ['1', '6', '2'],
    ['1', '6', '3'],
    ['1', '6', '4'],
    ['1', '6', '5'],
    ['1', '6', '6'],
    ['1', '6', '7'],
    ['1', '6', '8'],
    ['1', '6', '9'],
    ['1', '7', '0'],
    ['1', '7', '1'],
    ['1', '7', '2'],
    ['1', '7', '3'],
    ['1', '7', '4'],
    ['1', '7', '5'],
    ['1', '7', '6'],
    ['1', '7', '7'],
    ['1', '7', '8'],
    ['1', '7', '9'],
    ['1', '8', '0'],
    ['1', '8', '1'],
    ['1', '8', '2'],
    ['1', '8', '3'],
    ['1', '8', '4'],
    ['1', '8', '5'],
    ['1', '8', '6'],
    ['1', '8', '7'],
    ['1', '8', '8'],
    ['1', '8', '9'],
    ['1', '9', '0'],
    ['1', '9', '1'],
    ['1', '9', '2'],
    ['1', '9', '3'],
    ['1', '9', '4'],
    ['1', '9', '5'],
    ['1', '9', '6'],
    ['1', '9', '7'],
    ['1', '9', '8'],
    ['1', '9', '9'],
    ['2', '0', '0'],
    ['2', '0', '1'],
    ['2', '0', '2'],
    ['2', '0', '3'],
    ['2', '0', '4'],
    ['2', '0', '5'],
    ['2', '0', '6'],
    ['2', '0', '7'],
    ['2', '0', '8'],
    ['2', '0', '9'],
    ['2', '1', '0'],
    ['2', '1', '1'],
    ['2', '1', '2'],
    ['2', '1', '3'],
    ['2', '1', '4'],
    ['2', '1', '5'],
    ['2', '1', '6'],
    ['2', '1', '7'],
    ['2', '1', '8'],
    ['2', '1', '9'],
    ['2', '2', '0'],
    ['2', '2', '1'],
    ['2', '2', '2'],
    ['2', '2', '3'],
    ['2', '2', '4'],
    ['2', '2', '5'],
    ['2', '2', '6'],
    ['2', '2', '7'],
    ['2', '2', '8'],
    ['2', '2', '9'],
    ['2', '3', '0'],
    ['2', '3', '1'],
    ['2', '3', '2'],
    ['2', '3', '3'],
    ['2', '3', '4'],
    ['2', '3', '5'],
    ['2', '3', '6'],
    ['2', '3', '7'],
    ['2', '3', '8'],
    ['2', '3', '9'],
    ['2', '4', '0'],
    ['2', '4', '1'],
    ['2', '4', '2'],
    ['2', '4', '3'],
    ['2', '4', '4'],
    ['2', '4', '5'],
    ['2', '4', '6'],
    ['2', '4', '7'],
    ['2', '4', '8'],
    ['2', '4', '9'],
    ['2', '5', '0'],
    ['2', '5', '1'],
    ['2', '5', '2'],
    ['2', '5', '3'],
    ['2', '5', '4'],
    ['2', '5', '5'],
    ['2', '5', '6'],
];

pub const MAX_ROW_LEN: usize = 120;
pub const GUTTER_LEN: u8 = 1 + 3 + 1;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum TextStyle {
    Normal,
    Bold,
    Underline,
    Italy,
}

#[derive(Debug)]
pub struct RenderTextMsg<'a> {
    pub text: &'a [char],
    pub row: u8,
    pub column: u8,
}

#[repr(C)]
#[derive(Debug)]
pub enum OutputMessage<'a> {
    SetStyle(TextStyle),
    SetColor([u8; 4]),
    RenderText(RenderTextMsg<'a>),
    RenderRectangle { x: u8, y: u8, w: u8, h: u8 },
}

#[derive(Debug)]
pub struct RenderBuckets<'a> {
    pub texts: Vec<RenderTextMsg<'a>>,
    pub numbers: Vec<RenderTextMsg<'a>>,
    pub units: Vec<RenderTextMsg<'a>>,
    pub operators: Vec<RenderTextMsg<'a>>,
    pub variable: Vec<RenderTextMsg<'a>>,
    pub custom_commands: [Vec<OutputMessage<'a>>; 2],
}

#[repr(C)]
pub enum Layer {
    BehindText,
    AboveText,
}

impl<'a> RenderBuckets<'a> {
    pub fn new() -> RenderBuckets<'a> {
        RenderBuckets {
            texts: Vec::with_capacity(128),
            custom_commands: [Vec::with_capacity(128), Vec::with_capacity(128)],
            numbers: Vec::with_capacity(32),
            units: Vec::with_capacity(32),
            operators: Vec::with_capacity(32),
            variable: Vec::with_capacity(32),
        }
    }

    pub fn set_color(&mut self, layer: Layer, color: [u8; 4]) {
        self.custom_commands[layer as usize].push(OutputMessage::SetColor(color));
    }

    pub fn draw_rect(&mut self, layer: Layer, x: u8, y: u8, w: u8, h: u8) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderRectangle { x, y, w, h });
    }
}

pub struct NoteCalcApp<'a> {
    pub content: Vec<Line>,
    units: Units<'a>,
    pub editor: Editor,
    variables: Vec<String>,
    prefixes: &'static UnitPrefixes,
    result_buffer: [char; 1024],
}

impl<'a> NoteCalcApp<'a> {
    pub fn new() -> NoteCalcApp<'a> {
        let prefixes: &'static UnitPrefixes = Box::leak(Box::new(create_prefixes()));
        let units = Units::new(&prefixes);
        let mut lines = Vec::with_capacity(128);
        lines.push(Line::new());
        NoteCalcApp {
            content: lines,
            prefixes,
            units,
            editor: Editor::new(),
            variables: Vec::with_capacity(16),
            result_buffer: [0 as char; 1024],
        }
    }

    pub fn get_selected_text(&self) -> Option<String> {
        self.editor.get_selected_text(&self.content)
    }

    pub fn render(&mut self) -> RenderBuckets {
        // TODO: improve vec alloc
        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer_index = 0;
        let mut result_str_positions: SmallVec<[Option<(usize, usize)>; 256]> =
            SmallVec::with_capacity(256);
        let mut longest_row_len = 0;
        for (row_index, line) in self.content.iter().enumerate() {
            if line.len() > longest_row_len {
                longest_row_len = line.len();
            }

            // TODO optimize vec allocations
            let mut tokens = Vec::with_capacity(128);
            TokenParser::parse_line(
                &line.get_chars()[0..line.len()],
                &self.variables,
                &[],
                &mut tokens,
                &self.units,
            );

            let mut shunting_output_stack = Vec::with_capacity(128);
            ShuntingYard::shunting_yard(&mut tokens, &[], &mut shunting_output_stack);

            // render
            let mut column_index = 0;
            for token in &tokens {
                let dst = match &token.typ {
                    TokenType::StringLiteral => &mut render_buckets.texts,
                    TokenType::Variable => &mut render_buckets.variable,
                    TokenType::NumberLiteral(_) => &mut render_buckets.numbers,
                    TokenType::Operator(op_type) => match op_type {
                        OperatorTokenType::Unit(_) => &mut render_buckets.units,
                        _ => &mut render_buckets.operators,
                    },
                };
                dst.push(RenderTextMsg {
                    text: token.ptr,
                    row: row_index as u8,
                    column: column_index + GUTTER_LEN,
                });
                column_index += token.ptr.len() as u8;
            }
            if self.editor.show_cursor {
                render_buckets.texts.push(RenderTextMsg {
                    text: &['▏'],
                    row: self.editor.get_selection().get_cursor_pos().row as u8,
                    column: self.editor.get_selection().get_cursor_pos().column as u8 + GUTTER_LEN,
                });
            }

            let result = evaluate_tokens(&mut shunting_output_stack, &self.units);
            if let Some((result, there_was_unit_conversion)) = result {
                let start = result_buffer_index;
                for ch in result.to_string().chars() {
                    self.result_buffer[result_buffer_index] = ch;
                    result_buffer_index += 1;
                }
                result_str_positions.push(Some((start, result_buffer_index)));
            } else {
                result_str_positions.push(None);
            }
        }
        let result_gutter_x = GUTTER_LEN + MAX_LINE_LEN as u8;
        for (row_i, pos) in result_str_positions.iter().enumerate() {
            if let Some((start, end)) = pos {
                render_buckets.texts.push(RenderTextMsg {
                    text: &self.result_buffer[*start..*end],
                    row: row_i as u8,
                    column: result_gutter_x + 3,
                });
            }
        }

        // gutter
        render_buckets.set_color(Layer::BehindText, [242, 242, 242, 255]);
        render_buckets.draw_rect(Layer::BehindText, 0, 0, GUTTER_LEN, 255);

        // result gutter
        render_buckets.draw_rect(Layer::BehindText, result_gutter_x, 0, 3, 255);

        // highlight current line
        render_buckets.set_color(Layer::BehindText, [0xFC, 0xFA, 0xED, 200]);
        render_buckets.draw_rect(
            Layer::BehindText,
            0,
            self.editor.get_selection().get_cursor_pos().row as u8,
            result_gutter_x + 3 + 10,
            1,
        );
        // line numbers
        render_buckets.set_color(Layer::BehindText, [173, 173, 173, 255]);
        for i in 0..255 {
            render_buckets.custom_commands[Layer::BehindText as usize].push(
                OutputMessage::RenderText(RenderTextMsg {
                    text: &(LINE_NUM_CONSTS[i][..]),
                    row: i as u8,
                    column: 1,
                }),
            )
        }

        // selected text
        render_buckets.set_color(Layer::BehindText, [0xA6, 0xD2, 0xFF, 255]);
        if self.editor.get_selection().is_range() {
            let start = self.editor.get_selection().get_first();
            let end = self.editor.get_selection().get_second();
            if end.row > start.row {
                // first line
                render_buckets.draw_rect(
                    Layer::BehindText,
                    start.column as u8 + GUTTER_LEN,
                    start.row as u8,
                    (MAX_LINE_LEN - start.column) as u8,
                    1,
                );
                // full lines
                let height = end.row - start.row - 1;
                render_buckets.draw_rect(
                    Layer::BehindText,
                    GUTTER_LEN,
                    (start.row + 1) as u8,
                    MAX_LINE_LEN as u8,
                    height as u8,
                );
                // last line
                render_buckets.draw_rect(
                    Layer::BehindText,
                    GUTTER_LEN,
                    end.row as u8,
                    end.column as u8,
                    1,
                );
            } else {
                render_buckets.draw_rect(
                    Layer::BehindText,
                    start.column as u8 + GUTTER_LEN,
                    start.row as u8,
                    (end.column - start.column) as u8,
                    1,
                );
            }
        }

        return render_buckets;
    }

    pub fn handle_click(&mut self, x: u8, y: u8) {
        let lines = &self.content;
        let editor = &mut self.editor;
        if x < GUTTER_LEN {
            // clicked on gutter
        } else if x - GUTTER_LEN < MAX_LINE_LEN as u8 {
            editor.handle_click(lines, x - GUTTER_LEN, y);
        }
    }

    pub fn handle_drag(&mut self, x: u8, y: u8) {
        let lines = &self.content;
        let editor = &mut self.editor;
        if x < GUTTER_LEN {
            // clicked on gutter
        } else if x - GUTTER_LEN < MAX_LINE_LEN as u8 {
            editor.handle_drag(lines, x - GUTTER_LEN, y);
        }
    }

    pub fn handle_input(&mut self, input: InputKey, modifiers: InputModifiers) {
        self.editor
            .handle_input(&mut self.content, input, modifiers);
    }
}

fn digit_count(n: usize) -> usize {
    let mut n = n;
    let mut count = 1;
    while n > 9 {
        count += 1;
        n = n / 10;
    }
    return count;
}
