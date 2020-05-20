#![feature(ptr_offset_from, const_if_match, const_fn, const_panic, drain_filter)]
#![feature(type_alias_impl_trait)]
#![feature(const_in_array_repeat_expressions)]
// #![deny(
//     warnings,
//     anonymous_parameters,
//     unused_extern_crates,
//     unused_import_braces,
//     trivial_casts,
//     variant_size_differences,
//     //missing_debug_implementations,
//     trivial_numeric_casts,
//     unused_qualifications,
//     clippy::all
// )]

use crate::calc::{add_op, evaluate_tokens, CalcResult, EvaluationResult};
use crate::consts::{LINE_NUM_CONSTS, STATIC_LINE_IDS};
use crate::editor::editor::{
    Editor, EditorInputEvent, InputModifiers, Pos, RowModificationType, Selection,
};
use crate::editor::editor_content::EditorContent;
use crate::matrix::MatrixData;
use crate::renderer::{render_result, render_result_into};
use crate::shunting_yard::ShuntingYard;
use crate::token_parser::{OperatorTokenType, Token, TokenParser, TokenType};
use crate::units::units::Units;
use smallvec::SmallVec;
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::ops::Range;
use std::time::Duration;
use strum_macros::EnumDiscriminants;
use typed_arena::Arena;

mod functions;
mod matrix;
mod shunting_yard;
mod token_parser;
pub mod units;

pub mod calc;
pub mod consts;
pub mod editor;
pub mod renderer;

const MAX_EDITOR_WIDTH: usize = 120;
const LEFT_GUTTER_WIDTH: usize = 1 + 2 + 1;
pub const MAX_LINE_COUNT: usize = 64;
const RIGHT_GUTTER_WIDTH: usize = 3;
const MIN_RESULT_PANEL_WIDTH: usize = 30;
const SUM_VARIABLE_INDEX: usize = 0;

pub enum Click {
    Simple(Pos),
    Drag(Pos),
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum TextStyle {
    Normal,
    Bold,
    Underline,
    Italy,
}

#[derive(Debug)]
pub struct RenderUtf8TextMsg<'a> {
    pub text: &'a [char],
    pub row: usize,
    pub column: usize,
}

#[derive(Debug)]
pub struct RenderAsciiTextMsg<'a> {
    pub text: &'a [u8],
    pub row: usize,
    pub column: usize,
}

#[derive(Debug)]
pub struct RenderStringMsg {
    pub text: String,
    pub row: usize,
    pub column: usize,
}

#[repr(C)]
#[derive(Debug, EnumDiscriminants)]
#[strum_discriminants(name(OutputMessageCommandId))]
pub enum OutputMessage<'a> {
    SetStyle(TextStyle),
    SetColor(u32),
    RenderChar(usize, usize, char),
    RenderUtf8Text(RenderUtf8TextMsg<'a>),
    RenderAsciiText(RenderAsciiTextMsg<'a>),
    RenderString(RenderStringMsg),
    RenderRectangle {
        x: usize,
        y: usize,
        w: usize,
        h: usize,
    },
    PulsingRectangle {
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        start_color: u32,
        end_color: u32,
        animation_time: Duration,
    },
}

#[derive(Debug)]
pub struct RenderBuckets<'a> {
    pub ascii_texts: Vec<RenderAsciiTextMsg<'a>>,
    pub utf8_texts: Vec<RenderUtf8TextMsg<'a>>,
    pub numbers: Vec<RenderUtf8TextMsg<'a>>,
    pub units: Vec<RenderUtf8TextMsg<'a>>,
    pub operators: Vec<RenderUtf8TextMsg<'a>>,
    pub variable: Vec<RenderUtf8TextMsg<'a>>,
    pub line_ref_results: Vec<RenderStringMsg>,
    pub custom_commands: [Vec<OutputMessage<'a>>; 2],
    pub clear_commands: Vec<OutputMessage<'a>>,
}

#[repr(C)]
pub enum Layer {
    BehindText,
    AboveText,
}

impl<'a> RenderBuckets<'a> {
    pub fn new() -> RenderBuckets<'a> {
        RenderBuckets {
            ascii_texts: Vec::with_capacity(128),
            utf8_texts: Vec::with_capacity(128),
            custom_commands: [Vec::with_capacity(128), Vec::with_capacity(128)],
            numbers: Vec::with_capacity(32),
            units: Vec::with_capacity(32),
            operators: Vec::with_capacity(32),
            variable: Vec::with_capacity(32),
            line_ref_results: Vec::with_capacity(32),
            clear_commands: Vec::with_capacity(8),
        }
    }

    pub fn clear(&mut self) {
        self.ascii_texts.clear();
        self.utf8_texts.clear();
        self.custom_commands[0].clear();
        self.custom_commands[1].clear();
        self.numbers.clear();
        self.units.clear();
        self.operators.clear();
        self.variable.clear();
        self.line_ref_results.clear();
        self.clear_commands.clear();
    }

    pub fn set_color(&mut self, layer: Layer, color: u32) {
        self.custom_commands[layer as usize].push(OutputMessage::SetColor(color));
    }

    pub fn draw_rect(&mut self, layer: Layer, x: usize, y: usize, w: usize, h: usize) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderRectangle { x, y, w, h });
    }

    pub fn draw_char(&mut self, layer: Layer, x: usize, y: usize, ch: char) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderChar(x, y, ch));
    }

    pub fn draw_text(&mut self, layer: Layer, x: usize, y: usize, text: &'static [char]) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderUtf8Text(
            RenderUtf8TextMsg {
                text,
                row: y,
                column: x,
            },
        ));
    }

    pub fn draw_ascii_text(&mut self, layer: Layer, x: usize, y: usize, text: &'static [u8]) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderAsciiText(
            RenderAsciiTextMsg {
                text,
                row: y,
                column: x,
            },
        ));
    }

    pub fn draw_string(&mut self, layer: Layer, x: usize, y: usize, text: String) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderString(RenderStringMsg {
            text: text.clone(),
            row: y,
            column: x,
        }));
    }
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum ResultFormat {
    Bin,
    Dec,
    Hex,
}

#[derive(Clone)]
pub struct LineData {
    line_id: usize,
    result_format: ResultFormat,
}

impl Default for LineData {
    fn default() -> Self {
        LineData {
            line_id: 0,
            result_format: ResultFormat::Dec,
        }
    }
}

pub struct MatrixEditing {
    editor_content: EditorContent<usize>,
    editor: Editor,
    row_count: usize,
    col_count: usize,
    current_cell: Pos,
    start_text_index: usize,
    end_text_index: usize,
    row_index: usize,
    cell_strings: Vec<String>,
}

impl MatrixEditing {
    pub fn new<'a>(
        row_count: usize,
        col_count: usize,
        src_canvas: &[char],
        row_index: usize,
        start_text_index: usize,
        end_text_index: usize,
        step_in_pos: Pos,
    ) -> MatrixEditing {
        let current_cell = if step_in_pos.row == row_index {
            if step_in_pos.column > start_text_index {
                // from right
                Pos::from_row_column(0, col_count - 1)
            } else {
                // from left
                Pos::from_row_column(0, 0)
            }
        } else if step_in_pos.row < row_index {
            // from above
            Pos::from_row_column(0, 0)
        } else {
            // from below
            Pos::from_row_column(row_count - 1, 0)
        };

        let mut editor_content = EditorContent::new(32);
        let mut mat_edit = MatrixEditing {
            row_index,
            start_text_index,
            end_text_index,
            editor: Editor::new(&mut editor_content),
            editor_content,
            row_count,
            col_count,
            current_cell,
            cell_strings: Vec::with_capacity((row_count * col_count).max(4)),
        };
        let mut str: String = String::with_capacity(8);
        let mut can_ignore_ws = true;
        for ch in src_canvas {
            match ch {
                '[' => {
                    //ignore
                }
                ']' => {
                    break;
                }
                ',' => {
                    mat_edit.cell_strings.push(str);
                    str = String::with_capacity(8);
                    can_ignore_ws = true;
                }
                ';' => {
                    mat_edit.cell_strings.push(str);
                    str = String::with_capacity(8);
                    can_ignore_ws = true;
                }
                _ if ch.is_ascii_whitespace() && can_ignore_ws => {
                    // ignore
                }
                _ => {
                    can_ignore_ws = false;
                    str.push(*ch);
                }
            }
        }
        if str.len() > 0 {
            mat_edit.cell_strings.push(str);
        }

        let cell_index = mat_edit.current_cell.row * col_count + mat_edit.current_cell.column;
        mat_edit
            .editor_content
            .set_content(&mat_edit.cell_strings[cell_index]);
        // select all
        mat_edit.editor.set_cursor_range(
            Pos::from_row_column(0, 0),
            Pos::from_row_column(0, mat_edit.editor_content.line_len(0)),
        );

        mat_edit
    }

    fn add_column(&mut self) {
        if self.col_count == 6 {
            return;
        }
        self.cell_strings
            .reserve(self.row_count * (self.col_count + 1));
        for row_i in (0..self.row_count).rev() {
            let index = row_i * self.col_count + self.col_count;
            // TODO alloc :(, but at least not in the hot path
            self.cell_strings.insert(index, "0".to_owned());
        }
        self.col_count += 1;
    }

    fn add_row(&mut self) {
        if self.row_count == 6 {
            return;
        }
        self.cell_strings
            .reserve((self.row_count + 1) * self.col_count);
        self.row_count += 1;
        for _ in 0..self.col_count {
            // TODO alloc :(, but at least not in the hot path
            self.cell_strings.push("0".to_owned());
        }
    }

    fn remove_column(&mut self) {
        self.col_count -= 1;
        if self.current_cell.column >= self.col_count {
            self.change_cell(self.current_cell.with_column(self.col_count - 1));
        }
        for row_i in (0..self.row_count).rev() {
            let index = row_i * (self.col_count + 1) + self.col_count;
            self.cell_strings.remove(index);
        }
    }

    fn remove_row(&mut self) {
        self.row_count -= 1;
        if self.current_cell.row >= self.row_count {
            self.change_cell(self.current_cell.with_row(self.row_count - 1));
        }
        for _ in 0..self.col_count {
            self.cell_strings.pop();
        }
    }

    fn change_cell(&mut self, new_pos: Pos) {
        self.save_editor_content();

        let new_content = &self.cell_strings[new_pos.row * self.col_count + new_pos.column];
        self.editor_content.set_content(new_content);

        self.current_cell = new_pos;
        // select all
        self.editor.set_cursor_range(
            Pos::from_row_column(0, 0),
            Pos::from_row_column(0, self.editor_content.line_len(0)),
        );
    }

    fn save_editor_content(&mut self) {
        let mut backend = &mut self.cell_strings
            [self.current_cell.row * self.col_count + self.current_cell.column];
        backend.clear();
        self.editor_content.write_content_into(&mut backend);
    }

    fn render<'b>(
        &self,
        mut render_x: usize,
        render_y: usize,
        current_editor_width: usize,
        left_gutter_width: usize,
        render_buckets: &mut RenderBuckets<'b>,
        rendered_row_height: usize,
    ) -> usize {
        let vert_align_offset = (rendered_row_height - self.row_count) / 2;

        if self.row_count == 1 {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['['],
                row: render_y + vert_align_offset,
                column: render_x + left_gutter_width,
            });
        } else {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎡'],
                row: render_y + vert_align_offset,
                column: render_x + left_gutter_width,
            });
            for i in 1..self.row_count - 1 {
                render_buckets.operators.push(RenderUtf8TextMsg {
                    text: &['⎢'],
                    row: render_y + i + vert_align_offset,
                    column: render_x + left_gutter_width,
                });
            }
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎣'],
                row: render_y + self.row_count - 1 + vert_align_offset,
                column: render_x + left_gutter_width,
            });
        }
        render_x += 1;

        for col_i in 0..self.col_count {
            if render_x >= current_editor_width {
                return render_x;
            }
            let max_width: usize = (0..self.row_count)
                .map(|row_i| {
                    if self.current_cell == Pos::from_row_column(row_i, col_i) {
                        self.editor_content.line_len(0)
                    } else {
                        self.cell_strings[row_i * self.col_count + col_i].len()
                    }
                })
                .max()
                .unwrap();
            for row_i in 0..self.row_count {
                let len: usize = if self.current_cell == Pos::from_row_column(row_i, col_i) {
                    self.editor_content.line_len(0)
                } else {
                    self.cell_strings[row_i * self.col_count + col_i].len()
                };
                let padding_x = max_width - len;
                let text_len = len.min(
                    (current_editor_width as isize - (render_x + padding_x) as isize).max(0)
                        as usize,
                );

                if self.current_cell == Pos::from_row_column(row_i, col_i) {
                    render_buckets.set_color(Layer::BehindText, 0xBBBBBB_55);
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        render_x + padding_x + left_gutter_width,
                        render_y + row_i + vert_align_offset,
                        text_len,
                        1,
                    );
                    let chars = &self.editor_content.lines().next().unwrap();
                    render_buckets.set_color(Layer::AboveText, 0x000000_FF);
                    for (i, char) in chars.iter().enumerate() {
                        render_buckets.draw_char(
                            Layer::AboveText,
                            render_x + padding_x + left_gutter_width + i,
                            render_y + row_i + vert_align_offset,
                            *char,
                        );
                    }
                    let sel = self.editor.get_selection();
                    if sel.is_range() {
                        let first = sel.get_first();
                        let second = sel.get_second();
                        let len = second.column - first.column;
                        render_buckets.set_color(Layer::BehindText, 0xA6D2FF_FF);
                        render_buckets.draw_rect(
                            Layer::BehindText,
                            render_x + padding_x + left_gutter_width + first.column,
                            render_y + row_i + vert_align_offset,
                            len,
                            1,
                        );
                    }
                } else {
                    let chars = &self.cell_strings[row_i * self.col_count + col_i];
                    render_buckets.set_color(Layer::AboveText, 0x000000_FF);
                    render_buckets.draw_string(
                        Layer::AboveText,
                        render_x + padding_x + left_gutter_width,
                        render_y + row_i + vert_align_offset,
                        (&chars[0..text_len]).to_owned(),
                    );
                }

                if self.current_cell == Pos::from_row_column(row_i, col_i) {
                    if self.editor.is_cursor_shown() {
                        render_buckets.set_color(Layer::AboveText, 0x000000_FF);
                        render_buckets.draw_char(
                            Layer::AboveText,
                            (self.editor.get_selection().get_cursor_pos().column
                                + left_gutter_width)
                                + render_x
                                + padding_x,
                            render_y + row_i + vert_align_offset,
                            '▏',
                        );
                    }
                }
            }
            render_x += if col_i + 1 < self.col_count {
                max_width + 2
            } else {
                max_width
            };
        }

        if self.row_count == 1 {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &[']'],
                row: render_y + vert_align_offset,
                column: render_x + left_gutter_width,
            });
        } else {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎤'],
                row: render_y + vert_align_offset,
                column: render_x + left_gutter_width,
            });
            for i in 1..self.row_count - 1 {
                render_buckets.operators.push(RenderUtf8TextMsg {
                    text: &['⎥'],
                    row: render_y + i + vert_align_offset,
                    column: render_x + left_gutter_width,
                });
            }
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎦'],
                row: render_y + self.row_count - 1 + vert_align_offset,
                column: render_x + left_gutter_width,
            });
            render_x += 1;
        }

        render_x
    }
}

#[derive(Eq, PartialEq, Clone, Copy)]
pub enum EditorObjectType {
    Matrix { row_count: usize, col_count: usize },
    LineReference,
    SimpleTokens,
}

#[derive(Clone)]
pub struct EditorObject {
    typ: EditorObjectType,
    row: usize,
    start_x: usize,
    end_x: usize,
    rendered_x: usize,
    rendered_y: usize,
    rendered_w: usize,
    rendered_h: usize,
}

pub struct Variable {
    pub name: Box<[char]>,
    pub value: Result<CalcResult, ()>,
    pub defined_at_row: usize,
}

type LineResult = Result<Option<CalcResult>, ()>;

pub struct Tokens<'a> {
    tokens: Vec<Token<'a>>,
    shunting_output_stack: Vec<TokenType>,
}

// TODO: better name
pub struct Holder<'a> {
    pub editor_objects: Vec<Vec<EditorObject>>,
    // pub editor_objects: Vec<EditorObject>,
    pub results: [LineResult; MAX_LINE_COUNT],
    pub vars: Vec<Variable>,
    pub tokens: [Option<Tokens<'a>>; MAX_LINE_COUNT],

    // contains variable names separated by 0
    // the first char is the row index the variable was defined at
    pub autocompletion_src: Vec<char>,
}

pub struct NoteCalcApp {
    pub client_width: usize,
    pub editor: Editor,
    pub editor_content: EditorContent<LineData>,
    pub matrix_editing: Option<MatrixEditing>,
    pub line_reference_chooser: Option<usize>,
    pub line_id_generator: usize,
    pub right_gutter_is_dragged: bool,
    pub last_unrendered_modifications: UpdateRequirement,
    pub render_data: GlobalRenderData,
}

pub struct GlobalRenderData {
    result_gutter_x: usize,
    left_gutter_width: usize,
    right_gutter_width: usize,

    current_editor_width: usize,
    current_result_panel_width: usize,
    editor_y_to_render_y: [usize; MAX_LINE_COUNT],
    editor_y_to_rendered_height: [usize; MAX_LINE_COUNT],
}

impl GlobalRenderData {
    fn new(
        client_width: usize,
        result_gutter_x: usize,
        left_gutter_width: usize,
        right_gutter_width: usize,
    ) -> GlobalRenderData {
        let mut r = GlobalRenderData {
            result_gutter_x,
            left_gutter_width,
            right_gutter_width,
            current_editor_width: 0,
            current_result_panel_width: 0,
            editor_y_to_render_y: [0; MAX_LINE_COUNT],
            editor_y_to_rendered_height: [0; MAX_LINE_COUNT],
        };

        r.current_editor_width = result_gutter_x - left_gutter_width;
        r.current_result_panel_width = client_width - result_gutter_x - right_gutter_width;
        r
    }
}

struct PerLineRenderData {
    editor_pos: Pos,
    render_pos: Pos,
    // contains the y position for each editor line
    rendered_row_height: usize,
    vert_align_offset: usize,
    cursor_render_x_offset: isize,
}

impl PerLineRenderData {
    fn new() -> PerLineRenderData {
        let r = PerLineRenderData {
            editor_pos: Default::default(),
            render_pos: Default::default(),
            rendered_row_height: 0,
            vert_align_offset: 0,
            cursor_render_x_offset: 0,
        };
        r
    }
    pub fn new_line_started(&mut self) {
        self.editor_pos.column = 0;
        self.render_pos.column = 0;
        self.cursor_render_x_offset = 0;
    }

    fn line_render_ended(&mut self, row_height: usize) {
        self.editor_pos.row += 1;
        self.render_pos.row += row_height;
    }

    fn set_fix_row_height(&mut self, height: usize) {
        self.rendered_row_height = height;
        self.vert_align_offset = 0;
    }

    fn calc_rendered_row_height(
        &mut self,
        result: &LineResult,
        tokens: &[Token],
        vars: &[Variable],
        active_mat_edit_height: Option<usize>,
    ) {
        let mut max_height = active_mat_edit_height.unwrap_or(1);
        let result_row_height = if let Ok(result) = result {
            if let Some(result) = result {
                let result_row_height = match &result {
                    CalcResult::Matrix(mat) => mat.row_count,
                    _ => max_height,
                };
                result_row_height
            } else {
                max_height
            }
        } else {
            max_height
        };

        for token in tokens {
            let token_height = match token.typ {
                TokenType::Operator(OperatorTokenType::Matrix {
                    row_count,
                    col_count: _,
                }) => row_count,
                TokenType::LineReference { var_index } => {
                    let var = &vars[var_index];
                    match &var.value {
                        Ok(CalcResult::Matrix(mat)) => mat.row_count,
                        _ => 1,
                    }
                }
                _ => 1,
            };
            if token_height > max_height {
                max_height = token_height;
            }
        }
        self.rendered_row_height = max_height.max(result_row_height);
        // "- 1" so if it is even, it always appear higher
        self.vert_align_offset = (self.rendered_row_height - 1) / 2;
    }

    fn token_render_done(&mut self, editor_len: usize, render_len: usize, x_offset: isize) {
        self.render_pos.column += render_len;
        self.editor_pos.column += editor_len;
        self.cursor_render_x_offset += x_offset;
    }
}

enum UpdateReqType {
    RerenderWholeLine,
    RerenderResult,
    ReparseTokens,
    RecalcResult,
}

#[derive(Copy, Clone)]
pub struct UpdateRequirement {
    bitsets: [u64; 4],
}

impl UpdateRequirement {
    fn single_row(row_index: usize, typ: UpdateReqType) -> UpdateRequirement {
        let mut bitsets = [0u64; 4];
        bitsets[typ as usize] = 1u64 << row_index;
        UpdateRequirement { bitsets }
    }

    fn clear(&mut self) {
        self.bitsets = [0u64; 4];
    }

    fn all_rows_starting_at(row_index: usize, typ: UpdateReqType) -> UpdateRequirement {
        let s = 1u64 << row_index;
        let right_to_s_bits = s - 1;
        let left_to_s_and_s_bits = !right_to_s_bits;
        let mut bitsets = [0u64; 4];
        bitsets[typ as usize] = left_to_s_and_s_bits;

        UpdateRequirement { bitsets }
    }

    fn multiple(indices: &[usize], typ: UpdateReqType) -> UpdateRequirement {
        let mut b = 0;
        for i in indices {
            b |= 1 << *i;
        }
        let mut bitsets = [0u64; 4];
        bitsets[typ as usize] = b;

        UpdateRequirement { bitsets }
    }

    fn range(from: usize, to: usize, typ: UpdateReqType) -> UpdateRequirement {
        debug_assert!(to >= from);
        let top = 1 << to;
        let right_to_top_bits = top - 1;
        let bottom = 1 << from;
        let right_to_bottom_bits = bottom - 1;
        let mut bitsets = [0u64; 4];
        bitsets[typ as usize] = (right_to_top_bits ^ right_to_bottom_bits) | top;

        UpdateRequirement { bitsets }
    }

    fn merge(&mut self, other: &UpdateRequirement) {
        self.bitsets[0] |= other.bitsets[0];
        self.bitsets[1] |= other.bitsets[1];
        self.bitsets[2] |= other.bitsets[2];
        self.bitsets[3] |= other.bitsets[3];
    }

    fn combine(mut a: UpdateRequirement, b: UpdateRequirement) -> UpdateRequirement {
        a.merge(&b);
        a
    }

    fn need(&self, line_index: usize, typ: UpdateReqType) -> bool {
        ((1 << line_index) & self.bitsets[typ as usize]) != 0
    }

    fn need_any_of2(&self, line_index: usize, typ: UpdateReqType, typ2: UpdateReqType) -> bool {
        self.need(line_index, typ) || self.need(line_index, typ2)
    }

    fn need_any_of3(
        &self,
        line_index: usize,
        typ: UpdateReqType,
        typ2: UpdateReqType,
        typ3: UpdateReqType,
    ) -> bool {
        self.need(line_index, typ) || self.need(line_index, typ2) || self.need(line_index, typ3)
    }
}

impl NoteCalcApp {
    pub fn new(client_width: usize) -> NoteCalcApp {
        let mut editor_content = EditorContent::new(MAX_EDITOR_WIDTH);
        NoteCalcApp {
            line_reference_chooser: None,
            client_width,
            editor: Editor::new(&mut editor_content),
            editor_content,
            matrix_editing: None,
            line_id_generator: 1,
            right_gutter_is_dragged: false,
            last_unrendered_modifications: UpdateRequirement::all_rows_starting_at(
                0,
                UpdateReqType::ReparseTokens,
            ),
            render_data: GlobalRenderData::new(
                client_width,
                NoteCalcApp::calc_result_gutter_x(None, client_width),
                LEFT_GUTTER_WIDTH,
                RIGHT_GUTTER_WIDTH,
            ),
        }
    }

    pub fn set_normalized_content(&mut self, mut text: &str) {
        if text.is_empty() {
            text = "\n\n\n\n\n\n\n\n\n\n";
        }
        self.editor_content.set_content(text);
        self.editor.set_cursor_pos_r_c(0, 0);
        for (i, data) in self.editor_content.data_mut().iter_mut().enumerate() {
            data.line_id = i + 1;
        }
        self.line_id_generator = self.editor_content.line_count() + 1;
        self.last_unrendered_modifications =
            UpdateRequirement::all_rows_starting_at(0, UpdateReqType::ReparseTokens);
    }

    pub fn end_matrix_editing(
        matrix_editing: &mut Option<MatrixEditing>,
        editor: &mut Editor,
        editor_content: &mut EditorContent<LineData>,
        new_cursor_pos: Option<Pos>,
    ) {
        let mat_editor = {
            let mat_editor = matrix_editing.as_mut().unwrap();
            mat_editor.save_editor_content();
            mat_editor
        };
        let mut concat = String::with_capacity(32);
        concat.push('[');
        for row_i in 0..mat_editor.row_count {
            if row_i > 0 {
                concat.push(';');
            }
            for col_i in 0..mat_editor.col_count {
                if col_i > 0 {
                    concat.push(',');
                }
                let cell_str = &mat_editor.cell_strings[row_i * mat_editor.col_count + col_i];
                concat += &cell_str;
            }
        }
        concat.push(']');
        let selection = Selection::range(
            Pos::from_row_column(mat_editor.row_index, mat_editor.start_text_index),
            Pos::from_row_column(mat_editor.row_index, mat_editor.end_text_index),
        );
        editor.set_selection_save_col(selection);
        // TODO: máshogy oldd meg, mert ez modositja az undo stacket is
        // és az miért baj, legalább tudom ctrl z-zni a mátrix edition-t
        editor.handle_input(
            EditorInputEvent::Del,
            InputModifiers::none(),
            editor_content,
        );
        editor.insert_text(&concat, editor_content);
        *matrix_editing = None;

        if let Some(new_cursor_pos) = new_cursor_pos {
            editor.set_selection_save_col(Selection::single(new_cursor_pos));
        }
        editor.blink_cursor();
    }

    pub fn renderr<'a, 'b>(
        editor: &mut Editor,
        editor_content: &EditorContent<LineData>,
        units: &Units,
        matrix_editing: &mut Option<MatrixEditing>,
        line_reference_chooser: &mut Option<usize>,
        render_buckets: &mut RenderBuckets<'a>,
        result_buffer: &'a mut [u8],
        // RerenderRequirement
        modif_type: UpdateRequirement,
        gr: &mut GlobalRenderData,
        allocator: &'a Arena<char>,
        holder: &mut Holder<'a>,
    ) {
        // holder.results.clear();
        // holder.vars.clear();
        // holder.editor_objects.clear();
        // holder.autocompletion_src.clear();

        // swap lines
        // line selector mozgása
        // selected text mozgása
        // ha változik a rendered row height
        // változik a result bin hex dec (csak a result)
        // kurzorvillogás, csak az editor részét, a resultot nem kell
        // a line numbert, szélekeet, guttert, cska átméretezéskor meg mozgatáskor
        // TODO: result megváltoztatása hatással lehet minden előtte
        //      és utána levp matrix alignra...
        // TODO: ha változik a sormagasság, minden utánalevő rerender
        // TODO: ctrl a rerender

        // TODO: kell ez?
        {
            let mut sum_is_null = true;
            let mut r = PerLineRenderData::new();
            for line in editor_content.lines().take(MAX_LINE_COUNT) {
                r.new_line_started();
                gr.editor_y_to_render_y[r.editor_pos.row] = r.render_pos.row;
                //
                let editor_y = r.editor_pos.row;
                //
                let mut height_might_changed = false;
                if modif_type.need(editor_y, UpdateReqType::ReparseTokens) {
                    holder.tokens[editor_y] = NoteCalcApp::reparse_tokens(
                        line,
                        editor_y,
                        units,
                        &mut holder.vars,
                        allocator,
                    );
                    height_might_changed = true;
                }
                if modif_type.need_any_of2(
                    editor_y,
                    UpdateReqType::ReparseTokens,
                    UpdateReqType::RecalcResult,
                ) {
                    if let Some(tokens) = &mut holder.tokens[editor_y] {
                        let result = NoteCalcApp::evaluate_tokens(
                            &mut holder.vars,
                            editor_y,
                            &editor_content,
                            &mut tokens.shunting_output_stack,
                            line,
                            &mut holder.autocompletion_src,
                        );
                        let result = result.map(|it| it.map(|it| it.result));
                        holder.results[editor_y] = result;
                    } else {
                        holder.results[editor_y] = Ok(None);
                    }
                    height_might_changed = true;
                }

                if height_might_changed
                    || modif_type.need(editor_y, UpdateReqType::RerenderWholeLine)
                /*e.g. change the size of matrix with alt-key*/
                {
                    r.calc_rendered_row_height(
                        &holder.results[editor_y],
                        &holder.tokens[editor_y]
                            .as_ref()
                            .map(|it| &it.tokens)
                            .unwrap_or(&vec![]),
                        &holder.vars,
                        matrix_editing
                            .as_ref()
                            .filter(|it| it.row_index == editor_y)
                            .map(|it| it.row_count),
                    );
                    gr.editor_y_to_rendered_height[editor_y] = r.rendered_row_height;
                // if uj más mint a régi, rerender mindent
                } else {
                    r.rendered_row_height = gr.editor_y_to_rendered_height[editor_y];
                }

                let rendered_row_height = gr.editor_y_to_rendered_height[editor_y];

                if modif_type.need_any_of2(
                    editor_y,
                    UpdateReqType::RerenderWholeLine,
                    UpdateReqType::ReparseTokens,
                ) {
                    if let Some(tokens) = &holder.tokens[editor_y] {
                        NoteCalcApp::highlight_current_line(
                            render_buckets,
                            &r,
                            &gr,
                            editor,
                            gr.result_gutter_x,
                        );
                        let need_matrix_renderer = !editor.get_selection().is_range() || {
                            let first = editor.get_selection().get_first();
                            let second = editor.get_selection().get_second();
                            !(first.row..=second.row).contains(&editor_y)
                        };
                        // Todo: refactor the parameters into a struct
                        NoteCalcApp::render_tokens(
                            &tokens.tokens,
                            &mut r,
                            gr,
                            render_buckets,
                            // TODO &mut code smell
                            &mut holder.editor_objects[editor_y],
                            editor,
                            matrix_editing,
                            // TODO &mut code smell
                            &holder.vars,
                            &units,
                            need_matrix_renderer,
                        );
                        NoteCalcApp::render_wrap_dots(render_buckets, &r, &gr);

                        NoteCalcApp::draw_line_ref_chooser(
                            render_buckets,
                            &r,
                            &gr,
                            &line_reference_chooser,
                            gr.result_gutter_x,
                        );

                        NoteCalcApp::draw_cursor(render_buckets, &r, &gr, &editor, &matrix_editing);

                        for editor_obj in holder.editor_objects[editor_y].iter() {
                            if matches!(editor_obj.typ, EditorObjectType::LineReference) {
                                let vert_align_offset =
                                    (rendered_row_height - editor_obj.rendered_h) / 2;
                                render_buckets.set_color(Layer::BehindText, 0xFFCCCC_FF);
                                render_buckets.draw_rect(
                                    Layer::BehindText,
                                    gr.left_gutter_width + editor_obj.rendered_x,
                                    editor_obj.rendered_y + vert_align_offset,
                                    editor_obj.rendered_w,
                                    editor_obj.rendered_h,
                                );
                            }
                        }

                        // result
                        NoteCalcApp::draw_right_gutter_num_prefixes(
                            render_buckets,
                            gr.result_gutter_x,
                            &editor_content,
                            &r,
                        );
                        // result background
                        render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
                        render_buckets.draw_rect(
                            Layer::BehindText,
                            gr.result_gutter_x + gr.right_gutter_width,
                            gr.editor_y_to_render_y[editor_y],
                            gr.current_result_panel_width,
                            rendered_row_height,
                        );
                        // result gutter
                        render_buckets.set_color(Layer::BehindText, 0xD2D2D2_FF);
                        render_buckets.draw_rect(
                            Layer::BehindText,
                            gr.result_gutter_x,
                            gr.editor_y_to_render_y[editor_y],
                            gr.right_gutter_width,
                            rendered_row_height,
                        );

                        // line number
                        {
                            render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
                            render_buckets.draw_rect(
                                Layer::BehindText,
                                0,
                                gr.editor_y_to_render_y[editor_y],
                                gr.left_gutter_width,
                                rendered_row_height,
                            );
                            if editor_y == editor.get_selection().get_cursor_pos().row {
                                render_buckets.set_color(Layer::BehindText, 0x000000_FF);
                            } else {
                                render_buckets.set_color(Layer::BehindText, 0xADADAD_FF);
                            }
                            let vert_align_offset = (rendered_row_height - 1) / 2;
                            render_buckets.custom_commands[Layer::BehindText as usize].push(
                                OutputMessage::RenderUtf8Text(RenderUtf8TextMsg {
                                    text: &(LINE_NUM_CONSTS[editor_y][..]),
                                    row: gr.editor_y_to_render_y[editor_y] + vert_align_offset,
                                    column: 1,
                                }),
                            )
                        }
                    } else {
                        r.rendered_row_height = 1;
                        NoteCalcApp::render_simple_text_line(
                            line,
                            &mut r,
                            gr,
                            render_buckets,
                            allocator,
                        );
                    }
                } else if modif_type.need(editor_y, UpdateReqType::RerenderResult) {
                    NoteCalcApp::draw_right_gutter_num_prefixes(
                        render_buckets,
                        gr.result_gutter_x,
                        &editor_content,
                        &r,
                    );
                    // result background
                    render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        gr.result_gutter_x + gr.right_gutter_width,
                        gr.editor_y_to_render_y[editor_y],
                        gr.current_result_panel_width,
                        rendered_row_height,
                    );
                    // result gutter
                    render_buckets.set_color(Layer::BehindText, 0xD2D2D2_FF);
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        gr.result_gutter_x,
                        gr.editor_y_to_render_y[editor_y],
                        gr.right_gutter_width,
                        rendered_row_height,
                    );
                }

                if line.starts_with(&['-', '-']) {
                    sum_is_null = true;
                }

                match &holder.results[editor_y] {
                    Ok(Some(result)) => {
                        NoteCalcApp::sum_result(
                            &mut holder.vars[SUM_VARIABLE_INDEX],
                            result,
                            &mut sum_is_null,
                        );
                    }
                    Err(_) | Ok(None) => {}
                }

                r.line_render_ended(gr.editor_y_to_rendered_height[editor_y]);
            }
        }

        // selected text
        NoteCalcApp::render_selection_and_its_sum(
            &units,
            render_buckets,
            &holder.results,
            &editor,
            &editor_content,
            &gr,
            &holder.vars,
            allocator,
        );

        NoteCalcApp::render_results(
            &units,
            render_buckets,
            &holder.results,
            result_buffer,
            &editor_content,
            &gr,
            gr.result_gutter_x,
            modif_type,
        );

        render_buckets
            .clear_commands
            .push(OutputMessage::SetColor(0xFFFFFF_FF));
        for editor_y in 0..editor_content.line_count() {
            if modif_type.need_any_of2(
                editor_y,
                UpdateReqType::ReparseTokens,
                UpdateReqType::RerenderWholeLine,
            ) {
                render_buckets
                    .clear_commands
                    .push(OutputMessage::RenderRectangle {
                        x: LEFT_GUTTER_WIDTH,
                        y: gr.editor_y_to_render_y[editor_y],
                        w: gr.current_result_panel_width
                            + gr.current_editor_width
                            + gr.left_gutter_width
                            + gr.right_gutter_width,
                        h: gr.editor_y_to_rendered_height[editor_y],
                    });
            }
        }
    }

    pub fn reparse_tokens<'b>(
        line: &[char],
        editor_y: usize,
        units: &Units,
        vars: &mut Vec<Variable>,
        allocator: &'b Arena<char>,
    ) -> Option<Tokens<'b>> {
        vars.drain_filter(|it| &*it.name != &['s', 'u', 'm'][..] && it.defined_at_row == editor_y);

        return if line.starts_with(&['-', '-']) || line.starts_with(&['\'']) {
            None
        } else {
            // TODO optimize vec allocations
            let mut tokens = Vec::with_capacity(128);
            TokenParser::parse_line(line, &vars, &mut tokens, &units, editor_y, allocator);

            // TODO: measure is 128 necessary?
            // and remove allocation
            let mut shunting_output_stack = Vec::with_capacity(128);
            ShuntingYard::shunting_yard(&mut tokens, &mut shunting_output_stack);
            Some(Tokens {
                tokens,
                shunting_output_stack,
            })
        };
    }

    fn render_simple_text_line<'text_ptr>(
        line: &[char],
        r: &mut PerLineRenderData,
        mut gr: &mut GlobalRenderData,
        render_buckets: &mut RenderBuckets<'text_ptr>,
        allocator: &'text_ptr Arena<char>,
    ) {
        r.set_fix_row_height(1);
        gr.editor_y_to_rendered_height[r.editor_pos.row] = 1;

        let text_len = line.len().min(gr.current_editor_width);

        render_buckets.utf8_texts.push(RenderUtf8TextMsg {
            text: allocator.alloc_extend(line.iter().map(|it| *it).take(text_len)),
            row: r.render_pos.row,
            column: gr.left_gutter_width,
        });

        r.token_render_done(text_len, text_len, 0);
    }

    fn render_tokens<'text_ptr>(
        tokens: &[Token<'text_ptr>],
        r: &mut PerLineRenderData,
        gr: &mut GlobalRenderData,
        render_buckets: &mut RenderBuckets<'text_ptr>,
        editor_objects: &mut Vec<EditorObject>,
        editor: &Editor,
        matrix_editing: &Option<MatrixEditing>,
        vars: &[Variable],
        units: &Units,
        need_matrix_renderer: bool,
    ) {
        editor_objects.clear();
        let cursor_pos = editor.get_selection().get_cursor_pos();

        let mut token_index = 0;
        while token_index < tokens.len() {
            let token = &tokens[token_index];
            if let (
                TokenType::Operator(OperatorTokenType::Matrix {
                    row_count,
                    col_count,
                }),
                true,
            ) = (&token.typ, need_matrix_renderer)
            {
                token_index = NoteCalcApp::render_matrix(
                    token_index,
                    &tokens,
                    *row_count,
                    *col_count,
                    r,
                    gr,
                    render_buckets,
                    editor_objects,
                    &editor,
                    &matrix_editing,
                );
            } else if let (TokenType::LineReference { var_index }, true) =
                (&token.typ, need_matrix_renderer)
            {
                let var = &vars[*var_index];

                let (rendered_width, rendered_height) =
                    NoteCalcApp::render_single_result(units, render_buckets, &var.value, r, gr);

                let var_name_len = var.name.len();
                editor_objects.push(EditorObject {
                    typ: EditorObjectType::LineReference,
                    row: r.editor_pos.row,
                    start_x: r.editor_pos.column,
                    end_x: r.editor_pos.column + var_name_len,
                    rendered_x: r.render_pos.column,
                    rendered_y: r.render_pos.row,
                    rendered_w: rendered_width,
                    rendered_h: rendered_height,
                });

                token_index += 1;
                r.token_render_done(
                    var_name_len,
                    rendered_width,
                    if cursor_pos.column > r.editor_pos.column {
                        let diff = rendered_width as isize - var_name_len as isize;
                        diff
                    } else {
                        0
                    },
                );
            } else {
                // last token was a simple token too, extend it
                if let Some(EditorObject {
                    typ: EditorObjectType::SimpleTokens,
                    end_x,
                    rendered_w,
                    ..
                }) = editor_objects.last_mut()
                {
                    *end_x += token.ptr.len();
                    *rendered_w += token.ptr.len();
                } else {
                    editor_objects.push(EditorObject {
                        typ: EditorObjectType::SimpleTokens,
                        row: r.editor_pos.row,
                        start_x: r.editor_pos.column,
                        end_x: r.editor_pos.column + token.ptr.len(),
                        rendered_x: r.render_pos.column,
                        rendered_y: r.render_pos.row,
                        rendered_w: token.ptr.len(),
                        rendered_h: r.rendered_row_height,
                    });
                }
                NoteCalcApp::draw_token(
                    token,
                    r.render_pos.column,
                    r.render_pos.row + r.vert_align_offset,
                    gr.current_editor_width,
                    gr.left_gutter_width,
                    render_buckets,
                );

                token_index += 1;
                r.token_render_done(token.ptr.len(), token.ptr.len(), 0);
            }
        }
    }

    fn render_wrap_dots(
        render_buckets: &mut RenderBuckets,
        r: &PerLineRenderData,
        gr: &GlobalRenderData,
    ) {
        if r.render_pos.column > gr.current_editor_width {
            render_buckets.draw_char(
                Layer::AboveText,
                gr.current_editor_width + gr.left_gutter_width,
                r.render_pos.row,
                '…',
            );
        }
    }

    fn draw_line_ref_chooser(
        render_buckets: &mut RenderBuckets,
        r: &PerLineRenderData,
        gr: &GlobalRenderData,
        line_reference_chooser: &Option<usize>,
        result_gutter_x: usize,
    ) {
        if let Some(selection_row) = line_reference_chooser {
            if *selection_row == r.editor_pos.row {
                render_buckets.set_color(Layer::BehindText, 0xFFCCCC_FF);
                render_buckets.draw_rect(
                    Layer::BehindText,
                    0,
                    r.render_pos.row,
                    result_gutter_x + gr.right_gutter_width + MIN_RESULT_PANEL_WIDTH,
                    r.rendered_row_height,
                );
            }
        }
    }

    fn draw_right_gutter_num_prefixes(
        render_buckets: &mut RenderBuckets,
        result_gutter_x: usize,
        editor_content: &EditorContent<LineData>,
        r: &PerLineRenderData,
    ) {
        match editor_content.get_data(r.editor_pos.row).result_format {
            ResultFormat::Hex => {
                render_buckets.operators.push(RenderUtf8TextMsg {
                    text: &['0', 'x'],
                    row: r.render_pos.row,
                    column: result_gutter_x + 1,
                });
            }
            ResultFormat::Bin => {
                render_buckets.operators.push(RenderUtf8TextMsg {
                    text: &['0', 'b'],
                    row: r.render_pos.row,
                    column: result_gutter_x + 1,
                });
            }
            ResultFormat::Dec => {}
        }
    }

    fn highlight_current_line(
        render_buckets: &mut RenderBuckets,
        r: &PerLineRenderData,
        gr: &GlobalRenderData,
        editor: &Editor,
        result_gutter_x: usize,
    ) {
        let cursor_pos = editor.get_selection().get_cursor_pos();
        if cursor_pos.row == r.editor_pos.row {
            render_buckets.set_color(Layer::BehindText, 0xFFFFCC_C8);
            render_buckets.draw_rect(
                Layer::BehindText,
                0,
                r.render_pos.row,
                result_gutter_x + gr.right_gutter_width + MIN_RESULT_PANEL_WIDTH,
                r.rendered_row_height,
            );
        }
    }

    fn draw_cursor(
        render_buckets: &mut RenderBuckets,
        r: &PerLineRenderData,
        gr: &GlobalRenderData,
        editor: &Editor,
        matrix_editing: &Option<MatrixEditing>,
    ) {
        let cursor_pos = editor.get_selection().get_cursor_pos();
        if cursor_pos.row == r.editor_pos.row {
            render_buckets.set_color(Layer::AboveText, 0x000000_FF);
            if editor.is_cursor_shown()
                && matrix_editing.is_none()
                && ((cursor_pos.column as isize + r.cursor_render_x_offset) as usize)
                    < gr.current_editor_width
            {
                render_buckets.draw_char(
                    Layer::AboveText,
                    ((cursor_pos.column + gr.left_gutter_width) as isize + r.cursor_render_x_offset)
                        as usize,
                    r.render_pos.row + r.vert_align_offset,
                    '▏',
                );
            }
        }
    }

    fn evaluate_tokens(
        vars: &mut Vec<Variable>,
        editor_y: usize,
        editor_content: &EditorContent<LineData>,
        shunting_output_stack: &mut Vec<TokenType>,
        line: &[char],
        autocompletion_src: &mut Vec<char>,
    ) -> Result<Option<EvaluationResult>, ()> {
        let result = evaluate_tokens(shunting_output_stack, &vars);
        if let Ok(Some(result)) = &result {
            let line_data = editor_content.get_data(editor_y);
            if result.assignment {
                let var_name = {
                    let mut i = 0;
                    // skip whitespaces
                    while line[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    let start = i;
                    // take until '='
                    while line[i] != '=' {
                        i += 1;
                    }
                    // remove trailing whitespaces
                    i -= 1;
                    while line[i].is_ascii_whitespace() {
                        i -= 1;
                    }
                    let end = i;
                    &line[start..=end]
                };
                vars.push(Variable {
                    name: Box::from(var_name),
                    value: Ok(result.result.clone()),
                    defined_at_row: editor_y,
                });
                autocompletion_src.push(editor_y as u8 as char);
                for ch in var_name {
                    autocompletion_src.push(*ch);
                }
                autocompletion_src.push(0 as char);
            } else if line_data.line_id != 0 {
                let line_id = line_data.line_id;
                vars.push(Variable {
                    name: Box::from(STATIC_LINE_IDS[line_id]),
                    value: Ok(result.result.clone()),
                    defined_at_row: editor_y,
                });
            }
        };
        result
    }

    fn sum_result(sum_var: &mut Variable, result: &CalcResult, sum_is_null: &mut bool) {
        if *sum_is_null {
            sum_var.value = Ok(result.clone());
            *sum_is_null = false;
        } else {
            sum_var.value = match &sum_var.value {
                Ok(current_sum) => {
                    if let Some(ok) = add_op(&current_sum, &result) {
                        Ok(ok)
                    } else {
                        Err(())
                    }
                }
                _ => Err(()),
            }
        }
    }

    fn render_matrix<'text_ptr>(
        token_index: usize,
        tokens: &[Token<'text_ptr>],
        row_count: usize,
        col_count: usize,
        r: &mut PerLineRenderData,
        gr: &mut GlobalRenderData,
        render_buckets: &mut RenderBuckets<'text_ptr>,
        editor_objects: &mut Vec<EditorObject>,
        editor: &Editor,
        matrix_editing: &Option<MatrixEditing>,
    ) -> usize {
        let mut text_width = 0;
        let mut end_token_index = token_index;
        while tokens[end_token_index].typ != TokenType::Operator(OperatorTokenType::BracketClose) {
            text_width += tokens[end_token_index].ptr.len();
            end_token_index += 1;
        }
        // ignore the bracket as well
        text_width += 1;
        end_token_index += 1;

        let cursor_pos = editor.get_selection().get_cursor_pos();
        let cursor_isnide_matrix: bool = if !editor.get_selection().is_range()
            && cursor_pos.row == r.editor_pos.row
            && cursor_pos.column > r.editor_pos.column
            && cursor_pos.column < r.editor_pos.column + text_width
        {
            // cursor is inside the matrix
            true
        } else {
            false
        };

        let new_render_x = if let (true, Some(mat_editor)) = (cursor_isnide_matrix, matrix_editing)
        {
            mat_editor.render(
                r.render_pos.column,
                r.render_pos.row,
                gr.current_editor_width,
                gr.left_gutter_width,
                render_buckets,
                r.rendered_row_height,
            )
        } else {
            NoteCalcApp::render_matrix_obj(
                r.render_pos.column,
                r.render_pos.row,
                gr.current_editor_width,
                gr.left_gutter_width,
                row_count,
                col_count,
                &tokens[token_index..],
                render_buckets,
                r.rendered_row_height,
            )
        };

        let rendered_width = new_render_x - r.render_pos.column;
        editor_objects.push(EditorObject {
            typ: EditorObjectType::Matrix {
                row_count,
                col_count,
            },
            row: r.editor_pos.row,
            start_x: r.editor_pos.column,
            end_x: r.editor_pos.column + text_width,
            rendered_x: r.render_pos.column,
            rendered_y: r.render_pos.column,
            rendered_w: rendered_width,
            rendered_h: row_count,
        });

        let x_diff = if cursor_pos.row == r.editor_pos.row
            && cursor_pos.column >= r.editor_pos.column + text_width
        {
            let diff = rendered_width as isize - text_width as isize;
            diff
        } else {
            0
        };

        r.token_render_done(text_width, rendered_width, x_diff);
        return end_token_index;
    }

    fn evaluate_selection(
        units: &Units,
        editor: &Editor,
        editor_content: &EditorContent<LineData>,
        vars: &[Variable],
        results: &[LineResult],
        allocator: &Arena<char>,
    ) -> Option<String> {
        let sel = editor.get_selection();
        // TODO optimize vec allocations
        let mut tokens = Vec::with_capacity(128);
        // TODO we should be able to mark the arena allcoator and free it at the eond of the function
        if sel.start.row == sel.end.unwrap().row {
            if let Some(selected_text) = Editor::get_selected_text_single_line(sel, &editor_content)
            {
                if let Ok(Some(result)) = NoteCalcApp::evaluate_text(
                    units,
                    selected_text,
                    vars,
                    &mut tokens,
                    sel.start.row,
                    allocator,
                ) {
                    if result.there_was_operation {
                        let result_str = render_result(
                            &units,
                            &result.result,
                            &editor_content.get_data(sel.start.row).result_format,
                            result.there_was_unit_conversion,
                            4,
                        );
                        return Some(result_str);
                    }
                }
            }
        } else {
            let mut sum: Option<&CalcResult> = None;
            // so sum can contain references to temp values
            #[allow(unused_assignments)]
            let mut tmp_sum = CalcResult::hack_empty();
            for row_index in sel.get_first().row..=sel.get_second().row {
                if let Err(..) = &results[row_index] {
                    return None;
                } else if let Ok(Some(line_result)) = &results[row_index] {
                    if let Some(sum_r) = &sum {
                        if let Some(add_result) = add_op(sum_r, &line_result) {
                            tmp_sum = add_result;
                            sum = Some(&tmp_sum);
                        } else {
                            return None; // don't show anything if can't add all the rows
                        }
                    } else {
                        sum = Some(&line_result);
                    }
                }
            }
            if let Some(sum) = sum {
                let result_str = render_result(
                    &units,
                    sum,
                    &editor_content.get_data(sel.start.row).result_format,
                    false,
                    4,
                );
                return Some(result_str);
            }
        }
        return None;
    }

    fn evaluate_text<'text_ptr>(
        units: &Units,
        text: &[char],
        vars: &[Variable],
        tokens: &mut Vec<Token<'text_ptr>>,
        editor_y: usize,
        allocator: &'text_ptr Arena<char>,
    ) -> Result<Option<EvaluationResult>, ()> {
        TokenParser::parse_line(text, vars, tokens, &units, editor_y, allocator);
        let mut shunting_output_stack = Vec::with_capacity(4);
        ShuntingYard::shunting_yard(tokens, &mut shunting_output_stack);
        return evaluate_tokens(&mut shunting_output_stack, &vars);
    }

    fn render_matrix_obj<'text_ptr>(
        mut render_x: usize,
        render_y: usize,
        current_editor_width: usize,
        left_gutter_width: usize,
        row_count: usize,
        col_count: usize,
        tokens: &[Token<'text_ptr>],
        render_buckets: &mut RenderBuckets<'text_ptr>,
        rendered_row_height: usize,
    ) -> usize {
        let vert_align_offset = (rendered_row_height - row_count) / 2;

        if row_count == 1 {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['['],
                row: render_y + vert_align_offset,
                column: render_x + left_gutter_width,
            });
        } else {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎡'],
                row: render_y + vert_align_offset,
                column: render_x + left_gutter_width,
            });
            for i in 1..row_count - 1 {
                render_buckets.operators.push(RenderUtf8TextMsg {
                    text: &['⎢'],
                    row: render_y + i + vert_align_offset,
                    column: render_x + left_gutter_width,
                });
            }
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎣'],
                row: render_y + row_count - 1 + vert_align_offset,
                column: render_x + left_gutter_width,
            });
        }
        render_x += 1;

        let tokens_per_cell = {
            // TODO smallvec
            // so it can hold a 6*6 matrix maximum
            let mut matrix_cells_for_tokens: [MaybeUninit<&[Token]>; 36] =
                unsafe { MaybeUninit::uninit().assume_init() };

            let mut start_token_index = 0;
            let mut cell_index = 0;
            let mut can_ignore_ws = true;
            for (token_index, token) in tokens.iter().enumerate() {
                if token.typ == TokenType::Operator(OperatorTokenType::BracketClose) {
                    matrix_cells_for_tokens[cell_index] =
                        MaybeUninit::new(&tokens[start_token_index..token_index]);
                    break;
                } else if token.typ
                    == TokenType::Operator(OperatorTokenType::Matrix {
                        row_count,
                        col_count,
                    })
                    || token.typ == TokenType::Operator(OperatorTokenType::BracketOpen)
                {
                    // skip them
                    start_token_index = token_index + 1;
                } else if can_ignore_ws && token.ptr[0].is_ascii_whitespace() {
                    start_token_index = token_index + 1;
                } else if token.typ == TokenType::Operator(OperatorTokenType::Comma)
                    || token.typ == TokenType::Operator(OperatorTokenType::Semicolon)
                {
                    matrix_cells_for_tokens[cell_index] =
                        MaybeUninit::new(&tokens[start_token_index..token_index]);
                    start_token_index = token_index + 1;
                    cell_index += 1;
                    can_ignore_ws = true;
                } else {
                    can_ignore_ws = false;
                }
            }
            unsafe { std::mem::transmute::<_, [&[Token]; 36]>(matrix_cells_for_tokens) }
        };

        for col_i in 0..col_count {
            if render_x >= current_editor_width {
                return render_x;
            }
            let max_width: usize = (0..row_count)
                .map(|row_i| {
                    tokens_per_cell[row_i * col_count + col_i]
                        .iter()
                        .map(|it| it.ptr.len())
                        .sum()
                })
                .max()
                .unwrap();
            for row_i in 0..row_count {
                let tokens = &tokens_per_cell[row_i * col_count + col_i];
                let len: usize = tokens.iter().map(|it| it.ptr.len()).sum();
                let offset_x = max_width - len;
                let mut local_x = 0;
                for token in tokens.iter() {
                    NoteCalcApp::draw_token(
                        token,
                        render_x + offset_x + local_x,
                        render_y + row_i + vert_align_offset,
                        current_editor_width,
                        left_gutter_width,
                        render_buckets,
                    );
                    local_x += token.ptr.len();
                }
            }
            render_x += if col_i + 1 < col_count {
                max_width + 2
            } else {
                max_width
            };
        }

        if row_count == 1 {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &[']'],
                row: render_y + vert_align_offset,
                column: render_x + left_gutter_width,
            });
        } else {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎤'],
                row: render_y + vert_align_offset,
                column: render_x + left_gutter_width,
            });
            for i in 1..row_count - 1 {
                render_buckets.operators.push(RenderUtf8TextMsg {
                    text: &['⎥'],
                    row: render_y + i + vert_align_offset,
                    column: render_x + left_gutter_width,
                });
            }
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎦'],
                row: render_y + row_count - 1 + vert_align_offset,
                column: render_x + left_gutter_width,
            });
        }
        render_x += 1;

        render_x
    }

    fn render_matrix_result<'text_ptr>(
        units: &Units,
        mut render_x: usize,
        render_y: usize,
        mat: &MatrixData,
        render_buckets: &mut RenderBuckets<'text_ptr>,
        prev_mat_result_lengths: Option<&ResultLengths>,
        rendered_row_height: usize,
    ) -> usize {
        let vert_align_offset = (rendered_row_height - mat.row_count) / 2;
        let start_x = render_x;
        if mat.row_count == 1 {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['['],
                row: render_y + vert_align_offset,
                column: render_x,
            });
        } else {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎡'],
                row: render_y + vert_align_offset,
                column: render_x,
            });
            for i in 1..mat.row_count - 1 {
                render_buckets.operators.push(RenderUtf8TextMsg {
                    text: &['⎢'],
                    row: render_y + i + vert_align_offset,
                    column: render_x,
                });
            }
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎣'],
                row: render_y + mat.row_count - 1 + vert_align_offset,
                column: render_x,
            });
        }
        render_x += 1;

        let cells_strs = {
            let mut tokens_per_cell: SmallVec<[String; 32]> = SmallVec::with_capacity(32);

            for cell in mat.cells.iter() {
                let result_str = render_result(units, cell, &ResultFormat::Dec, false, 4);
                tokens_per_cell.push(result_str);
            }
            tokens_per_cell
        };

        let max_lengths = {
            let mut max_lengths = ResultLengths {
                int_part_len: prev_mat_result_lengths
                    .as_ref()
                    .map(|it| it.int_part_len)
                    .unwrap_or(0),
                frac_part_len: prev_mat_result_lengths
                    .as_ref()
                    .map(|it| it.frac_part_len)
                    .unwrap_or(0),
                unit_part_len: prev_mat_result_lengths
                    .as_ref()
                    .map(|it| it.unit_part_len)
                    .unwrap_or(0),
            };
            for cell_str in &cells_strs {
                let lengths = get_int_frac_part_len(cell_str);
                max_lengths.set_max(&lengths);
            }
            max_lengths
        };
        for col_i in 0..mat.col_count {
            for row_i in 0..mat.row_count {
                let cell_str = &cells_strs[row_i * mat.col_count + col_i];
                let lengths = get_int_frac_part_len(cell_str);
                // Draw integer part
                let offset_x = max_lengths.int_part_len - lengths.int_part_len;
                render_buckets.draw_string(
                    Layer::AboveText,
                    render_x + offset_x,
                    render_y + row_i + vert_align_offset,
                    // TOOD nem kell clone, csinálj iter into vhogy
                    cell_str[0..lengths.int_part_len].to_owned(),
                );

                let mut frac_offset_x = 0;
                if lengths.frac_part_len > 0 {
                    render_buckets.draw_string(
                        Layer::AboveText,
                        render_x + offset_x + lengths.int_part_len,
                        render_y + row_i + vert_align_offset,
                        // TOOD nem kell clone, csinálj iter into vhogy
                        cell_str
                            [lengths.int_part_len..lengths.int_part_len + lengths.frac_part_len]
                            .to_owned(),
                    )
                } else if max_lengths.frac_part_len > 0 {
                    render_buckets.draw_char(
                        Layer::AboveText,
                        render_x + offset_x + lengths.int_part_len,
                        render_y + row_i + vert_align_offset,
                        '.',
                    );
                    frac_offset_x = 1;
                }
                for i in 0..max_lengths.frac_part_len - lengths.frac_part_len - frac_offset_x {
                    render_buckets.draw_char(
                        Layer::AboveText,
                        render_x
                            + offset_x
                            + lengths.int_part_len
                            + lengths.frac_part_len
                            + frac_offset_x
                            + i,
                        render_y + row_i + vert_align_offset,
                        '0',
                    )
                }
                if lengths.unit_part_len > 0 {
                    render_buckets.draw_string(
                        Layer::AboveText,
                        render_x + offset_x + lengths.int_part_len + max_lengths.frac_part_len + 1,
                        render_y + row_i + vert_align_offset,
                        // TOOD nem kell clone, csinálj iter into vhogy
                        // +1, skip space
                        cell_str[lengths.int_part_len + lengths.frac_part_len + 1..].to_owned(),
                    )
                }
            }
            render_x += if col_i + 1 < mat.col_count {
                (max_lengths.int_part_len + max_lengths.frac_part_len + max_lengths.unit_part_len)
                    + 2
            } else {
                max_lengths.int_part_len + max_lengths.frac_part_len + max_lengths.unit_part_len
            };
        }

        if mat.row_count == 1 {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &[']'],
                row: render_y + vert_align_offset,
                column: render_x,
            });
        } else {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎤'],
                row: render_y + vert_align_offset,
                column: render_x,
            });
            for i in 1..mat.row_count - 1 {
                render_buckets.operators.push(RenderUtf8TextMsg {
                    text: &['⎥'],
                    row: render_y + i + vert_align_offset,
                    column: render_x,
                });
            }
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['⎦'],
                row: render_y + mat.row_count - 1 + vert_align_offset,
                column: render_x,
            });
        }
        render_x += 1;
        return render_x - start_x;
    }

    fn calc_matrix_max_lengths(units: &Units, mat: &MatrixData) -> ResultLengths {
        let cells_strs = {
            let mut tokens_per_cell: SmallVec<[String; 32]> = SmallVec::with_capacity(32);

            for cell in mat.cells.iter() {
                let result_str = render_result(units, cell, &ResultFormat::Dec, false, 4);
                tokens_per_cell.push(result_str);
            }
            tokens_per_cell
        };
        let max_lengths = {
            let mut max_lengths = ResultLengths {
                int_part_len: 0,
                frac_part_len: 0,
                unit_part_len: 0,
            };
            for cell_str in &cells_strs {
                let lengths = get_int_frac_part_len(cell_str);
                max_lengths.set_max(&lengths);
            }
            max_lengths
        };
        return max_lengths;
    }

    fn render_single_result<'text_ptr>(
        units: &Units,
        render_buckets: &mut RenderBuckets<'text_ptr>,
        result: &Result<CalcResult, ()>,
        r: &PerLineRenderData,
        gr: &GlobalRenderData,
    ) -> (usize, usize) {
        return match &result {
            Ok(CalcResult::Matrix(mat)) => {
                let rendered_width = NoteCalcApp::render_matrix_result(
                    units,
                    gr.left_gutter_width + r.render_pos.column,
                    r.render_pos.row,
                    mat,
                    render_buckets,
                    None,
                    r.rendered_row_height,
                );
                (rendered_width, mat.row_count)
            }
            Ok(result) => {
                // TODO: optimize string alloc
                let result_str = render_result(&units, result, &ResultFormat::Dec, false, 2);
                let text_len = result_str.chars().count().min(
                    (gr.current_editor_width as isize - r.render_pos.column as isize).max(0)
                        as usize,
                );
                // TODO avoid String
                render_buckets.line_ref_results.push(RenderStringMsg {
                    text: result_str[0..text_len].to_owned(),
                    row: r.render_pos.row,
                    column: r.render_pos.column + gr.left_gutter_width,
                });
                (text_len, 1)
            }
            Err(_) => {
                render_buckets.line_ref_results.push(RenderStringMsg {
                    text: "Err".to_owned(),
                    row: r.render_pos.row,
                    column: r.render_pos.column + gr.left_gutter_width,
                });
                (3, 1)
            }
        };
    }

    fn render_results<'text_ptr>(
        units: &Units,
        render_buckets: &mut RenderBuckets<'text_ptr>,
        results: &[LineResult],
        result_buffer: &'text_ptr mut [u8],
        editor_content: &EditorContent<LineData>,
        gr: &GlobalRenderData,
        result_gutter_x: usize,
        modif_type: UpdateRequirement,
    ) {
        let (max_lengths, result_ranges) = {
            let mut result_ranges: SmallVec<[Option<Range<usize>>; MAX_LINE_COUNT]> =
                SmallVec::with_capacity(MAX_LINE_COUNT);
            let mut result_buffer_index = 0;
            let mut max_lengths = ResultLengths {
                int_part_len: 0,
                frac_part_len: 0,
                unit_part_len: 0,
            };
            let mut prev_result_matrix_length = None;
            // calc max length and render results into buffer
            // TODO: extract
            for (editor_y, result) in results.iter().enumerate() {
                if editor_y >= editor_content.line_count() {
                    break;
                }
                if let Err(..) = result {
                    result_buffer[result_buffer_index] = b'E';
                    result_buffer[result_buffer_index + 1] = b'r';
                    result_buffer[result_buffer_index + 2] = b'r';
                    result_ranges.push(Some(result_buffer_index..result_buffer_index + 3));
                    result_buffer_index += 3;
                    prev_result_matrix_length = None;
                } else if let Ok(Some(result)) = result {
                    match &result {
                        CalcResult::Matrix(mat) => {
                            if prev_result_matrix_length.is_none() {
                                prev_result_matrix_length =
                                    NoteCalcApp::calc_consecutive_matrices_max_lengths(
                                        units,
                                        &results[editor_y..],
                                    );
                            }
                            if modif_type.need_any_of3(
                                editor_y,
                                UpdateReqType::RecalcResult,
                                UpdateReqType::RerenderResult,
                                UpdateReqType::ReparseTokens,
                            ) {
                                NoteCalcApp::render_matrix_result(
                                    units,
                                    result_gutter_x + gr.right_gutter_width,
                                    gr.editor_y_to_render_y[editor_y],
                                    mat,
                                    render_buckets,
                                    prev_result_matrix_length.as_ref(),
                                    gr.editor_y_to_rendered_height[editor_y],
                                );
                            }
                            result_ranges.push(None);
                        }
                        _ => {
                            prev_result_matrix_length = None;
                            let start = result_buffer_index;
                            let mut c = Cursor::new(&mut result_buffer[start..]);
                            render_result_into(
                                &units,
                                &result,
                                &editor_content.get_data(editor_y).result_format,
                                false,
                                &mut c,
                                4,
                            );
                            let len = c.position() as usize;
                            let range = start..start + len;
                            let s = unsafe {
                                std::str::from_utf8_unchecked(&result_buffer[range.clone()])
                            };
                            let lengths = get_int_frac_part_len(s);
                            max_lengths.set_max(&lengths);
                            result_ranges.push(Some(range));
                            result_buffer_index += len;
                        }
                    };
                } else {
                    prev_result_matrix_length = None;
                    result_ranges.push(None);
                }
            }
            (max_lengths, result_ranges)
        };

        // render results from the buffer
        for (editor_y, result_range) in result_ranges.iter().enumerate() {
            if !modif_type.need_any_of3(
                editor_y,
                UpdateReqType::RecalcResult,
                UpdateReqType::RerenderResult,
                UpdateReqType::ReparseTokens,
            ) {
                continue;
            }
            if let Some(result_range) = result_range {
                let s =
                    unsafe { std::str::from_utf8_unchecked(&result_buffer[result_range.clone()]) };
                let lengths = get_int_frac_part_len(s);
                let from = result_range.start;
                let rendered_row_height = gr.editor_y_to_rendered_height[editor_y];
                let vert_align_offset = (rendered_row_height - 1) / 2;
                let row = gr.editor_y_to_render_y[editor_y] + vert_align_offset;
                let offset_x = if max_lengths.int_part_len < lengths.int_part_len {
                    // it is an "Err"
                    0
                } else {
                    max_lengths.int_part_len - lengths.int_part_len
                };
                let x = result_gutter_x + gr.right_gutter_width + offset_x;
                render_buckets.ascii_texts.push(RenderAsciiTextMsg {
                    text: &result_buffer[from..from + lengths.int_part_len],
                    row,
                    column: x,
                });
                if lengths.frac_part_len > 0 {
                    let from = result_range.start + lengths.int_part_len;
                    render_buckets.ascii_texts.push(RenderAsciiTextMsg {
                        text: &result_buffer[from..from + lengths.frac_part_len],
                        row,
                        column: x + lengths.int_part_len,
                    });
                }
                if lengths.unit_part_len > 0 {
                    let from =
                        result_range.start + lengths.int_part_len + lengths.frac_part_len + 1;
                    render_buckets.ascii_texts.push(RenderAsciiTextMsg {
                        text: &result_buffer[from..result_range.end],
                        row,
                        column: x + lengths.int_part_len + lengths.frac_part_len + 1,
                    });
                }
            }
        }
    }

    fn calc_consecutive_matrices_max_lengths(
        units: &Units,
        results: &[LineResult],
    ) -> Option<ResultLengths> {
        let mut max_lengths: Option<ResultLengths> = None;
        for result in results.iter() {
            match result {
                Ok(Some(CalcResult::Matrix(mat))) => {
                    let lengths = NoteCalcApp::calc_matrix_max_lengths(units, mat);
                    if let Some(max_lengths) = &mut max_lengths {
                        max_lengths.set_max(&lengths);
                    } else {
                        max_lengths = Some(lengths);
                    }
                }
                _ => {
                    break;
                }
            }
        }
        return max_lengths;
    }

    fn draw_token<'text_ptr>(
        token: &Token<'text_ptr>,
        render_x: usize,
        render_y: usize,
        current_editor_width: usize,
        left_gutter_width: usize,
        render_buckets: &mut RenderBuckets<'text_ptr>,
    ) {
        let dst = match &token.typ {
            TokenType::StringLiteral => &mut render_buckets.utf8_texts,
            TokenType::Variable { .. } => &mut render_buckets.variable,
            TokenType::LineReference { .. } => &mut render_buckets.variable,
            TokenType::NumberLiteral(_) => &mut render_buckets.numbers,
            TokenType::Unit(_) => &mut render_buckets.units,
            TokenType::Operator(op_type) => match op_type {
                _ => &mut render_buckets.operators,
            },
        };
        let text_len = token
            .ptr
            .len()
            .min((current_editor_width as isize - render_x as isize).max(0) as usize);
        dst.push(RenderUtf8TextMsg {
            text: &token.ptr[0..text_len],
            row: render_y,
            column: render_x + left_gutter_width,
        });
    }

    pub fn copy_selected_rows_with_result_to_clipboard<'b>(
        &'b mut self,
        units: &'b Units,
        render_buckets: &'b mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
        token_char_alloator: &'b Arena<char>,
        holder: &Holder<'b>,
    ) -> String {
        let sel = self.editor.get_selection();
        let first_row = sel.get_first().row;
        let second_row = sel.get_second().row;
        let row_nums = second_row - first_row + 1;

        let mut vars: Vec<Variable> = Vec::with_capacity(32);
        vars.push(Variable {
            name: Box::from(&['s', 'u', 'm'][..]),
            value: Err(()),
            defined_at_row: 0,
        });
        let mut tokens = Vec::with_capacity(128);

        let mut gr =
            GlobalRenderData::new(self.client_width, self.render_data.result_gutter_x, 0, 2);
        // evaluate all the lines so variables are defined even if they are not selected
        let mut render_height = 0;
        {
            let mut r = PerLineRenderData::new();
            for (i, line) in self.editor_content.lines().enumerate() {
                // TODO "--"
                tokens.clear();
                TokenParser::parse_line(line, &vars, &mut tokens, &units, i, token_char_alloator);

                let mut shunting_output_stack = Vec::with_capacity(32);
                ShuntingYard::shunting_yard(&mut tokens, &mut shunting_output_stack);

                if i >= first_row && i <= second_row {
                    r.new_line_started();
                    gr.editor_y_to_render_y[r.editor_pos.row] = r.render_pos.row;
                    r.calc_rendered_row_height(&holder.results[i], &tokens, &vars, None);
                    gr.editor_y_to_rendered_height[r.editor_pos.row] = r.rendered_row_height;
                    render_height += r.rendered_row_height;
                    // Todo: refactor the parameters into a struct
                    NoteCalcApp::render_tokens(
                        &tokens,
                        &mut r,
                        &mut gr,
                        render_buckets,
                        // TODO &mut code smell
                        &mut Vec::new(),
                        &self.editor,
                        &self.matrix_editing,
                        // TODO &mut code smell
                        &vars,
                        &units,
                        true, // force matrix rendering
                    );
                    r.line_render_ended(r.rendered_row_height);
                }
            }
        }

        let mut tmp_canvas: Vec<[char; 256]> = Vec::with_capacity(render_height);
        for _ in 0..render_height {
            tmp_canvas.push([' '; 256]);
        }
        // render all tokens to the tmp canvas, so we can measure the longest row
        NoteCalcApp::render_buckets_into(&render_buckets, &mut tmp_canvas);
        let mut max_len = 0;
        for canvas_line in &tmp_canvas {
            let mut len = 256;
            for ch in canvas_line.iter().rev() {
                if *ch != ' ' {
                    break;
                }
                len -= 1;
            }
            if len > max_len {
                max_len = len;
            }
        }

        render_buckets.clear();
        let result_gutter_x = max_len + 2;
        NoteCalcApp::render_results(
            &units,
            render_buckets,
            &holder.results[first_row..=second_row],
            result_buffer,
            &self.editor_content,
            &gr,
            result_gutter_x,
            UpdateRequirement::all_rows_starting_at(0, UpdateReqType::ReparseTokens),
        );
        for i in 0..render_height {
            render_buckets.draw_char(Layer::AboveText, result_gutter_x, i, '‖');
        }
        NoteCalcApp::render_buckets_into(&render_buckets, &mut tmp_canvas);
        let mut result_str = String::with_capacity(row_nums * 64);
        for canvas_line in &tmp_canvas {
            result_str.extend(canvas_line.iter());
            while result_str.chars().last().unwrap_or('x') == ' ' {
                result_str.pop();
            }
            result_str.push('\n');
        }

        return result_str;
    }

    fn render_buckets_into(buckets: &RenderBuckets, canvas: &mut [[char; 256]]) {
        fn write_char_slice(canvas: &mut [[char; 256]], row: usize, col: usize, src: &[char]) {
            let str = &mut canvas[row];
            for (dst_char, src_char) in str[col..].iter_mut().zip(src.iter()) {
                *dst_char = *src_char;
            }
        }

        fn write_str(canvas: &mut [[char; 256]], row: usize, col: usize, src: &str) {
            let str = &mut canvas[row];
            for (dst_char, src_char) in str[col..].iter_mut().zip(src.chars()) {
                *dst_char = src_char;
            }
        }

        fn write_ascii(canvas: &mut [[char; 256]], row: usize, col: usize, src: &[u8]) {
            let str = &mut canvas[row];
            for (dst_char, src_char) in str[col..].iter_mut().zip(src.iter()) {
                *dst_char = *src_char as char;
            }
        }

        fn write_command(canvas: &mut [[char; 256]], command: &OutputMessage) {
            match command {
                OutputMessage::RenderUtf8Text(text) => {
                    write_char_slice(canvas, text.row, text.column, text.text);
                }
                OutputMessage::SetStyle(..) => {}
                OutputMessage::SetColor(..) => {}
                OutputMessage::RenderRectangle { .. } => {}
                OutputMessage::RenderChar(x, y, ch) => {
                    let str = &mut canvas[*y];
                    str[*x] = *ch;
                }
                OutputMessage::RenderString(text) => {
                    write_str(canvas, text.row, text.column, &text.text);
                }
                OutputMessage::RenderAsciiText(text) => {
                    write_ascii(canvas, text.row, text.column, &text.text);
                }
                OutputMessage::PulsingRectangle { .. } => {}
            }
        }

        for command in &buckets.custom_commands[Layer::BehindText as usize] {
            write_command(canvas, command);
        }

        for command in &buckets.utf8_texts {
            write_char_slice(canvas, command.row, command.column, command.text);
        }
        for text in &buckets.ascii_texts {
            write_ascii(canvas, text.row, text.column, text.text);
        }
        for command in &buckets.numbers {
            write_char_slice(canvas, command.row, command.column, command.text);
        }

        for command in &buckets.units {
            write_char_slice(canvas, command.row, command.column, command.text);
        }

        for command in &buckets.operators {
            write_char_slice(canvas, command.row, command.column, command.text);
        }

        for command in &buckets.line_ref_results {
            write_str(canvas, command.row, command.column, &command.text);
        }

        for command in &buckets.variable {
            write_char_slice(canvas, command.row, command.column, command.text);
        }
        for command in &buckets.custom_commands[Layer::AboveText as usize] {
            write_command(canvas, command);
        }
    }

    fn render_selection_and_its_sum<'text_ptr>(
        units: &Units,
        render_buckets: &mut RenderBuckets<'text_ptr>,
        results: &[LineResult],
        editor: &Editor,
        editor_content: &EditorContent<LineData>,
        r: &GlobalRenderData,
        vars: &[Variable],
        allocator: &'text_ptr Arena<char>,
    ) {
        render_buckets.set_color(Layer::BehindText, 0xA6D2FF_FF);
        if editor.get_selection().is_range() {
            let start = editor.get_selection().get_first();
            let end = editor.get_selection().get_second();
            if end.row > start.row {
                // first line
                let height = r
                    .editor_y_to_render_y
                    .get(start.row + 1)
                    .map(|it| it - r.editor_y_to_render_y[start.row])
                    .unwrap_or(1);
                render_buckets.draw_rect(
                    Layer::BehindText,
                    start.column + r.left_gutter_width,
                    r.editor_y_to_render_y[start.row],
                    editor_content
                        .line_len(start.row)
                        .min(r.current_editor_width),
                    height,
                );
                // full lines
                // let height = end.row - start.row - 1;
                for i in start.row + 1..end.row {
                    let height = r
                        .editor_y_to_render_y
                        .get(i + 1)
                        .map(|it| it - r.editor_y_to_render_y[i])
                        .unwrap_or(1);
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        r.left_gutter_width,
                        r.editor_y_to_render_y[i],
                        editor_content.line_len(i).min(r.current_editor_width),
                        height,
                    );
                }
                // last line
                let height = r
                    .editor_y_to_render_y
                    .get(end.row + 1)
                    .map(|it| {
                        if *it > r.editor_y_to_render_y[end.row] {
                            it - r.editor_y_to_render_y[end.row]
                        } else {
                            1
                        }
                    })
                    .unwrap_or(1);
                render_buckets.draw_rect(
                    Layer::BehindText,
                    r.left_gutter_width,
                    r.editor_y_to_render_y[end.row],
                    end.column.min(r.current_editor_width),
                    height,
                );
            } else {
                let height = r
                    .editor_y_to_render_y
                    .get(start.row + 1)
                    .map(|it| {
                        if *it != 0 {
                            it - r.editor_y_to_render_y[start.row]
                        } else {
                            1
                        }
                    })
                    .unwrap_or(1);
                render_buckets.draw_rect(
                    Layer::BehindText,
                    start.column + r.left_gutter_width,
                    r.editor_y_to_render_y[start.row],
                    (end.column - start.column).min(r.current_editor_width),
                    height,
                );
            }
            // evaluated result of selection, selected text
            if let Some(mut partial_result) = NoteCalcApp::evaluate_selection(
                &units,
                editor,
                editor_content,
                &vars,
                &results,
                allocator,
            ) {
                if start.row == end.row {
                    let selection_center = start.column + ((end.column - start.column) / 2);
                    partial_result.insert_str(0, "= ");
                    let result_w = partial_result.chars().count();
                    let centered_x =
                        (selection_center as isize - (result_w / 2) as isize).max(0) as usize;
                    render_buckets.set_color(Layer::AboveText, 0xAAFFAA_FF);
                    render_buckets.draw_rect(
                        Layer::AboveText,
                        r.left_gutter_width + centered_x,
                        r.editor_y_to_render_y[start.row] - 1,
                        result_w,
                        1,
                    );
                    render_buckets.set_color(Layer::AboveText, 0x000000_FF);
                    render_buckets.draw_string(
                        Layer::AboveText,
                        r.left_gutter_width + centered_x,
                        r.editor_y_to_render_y[start.row] - 1,
                        partial_result,
                    );
                } else {
                    partial_result.insert_str(0, "⎬ ∑ = ");
                    let result_w = partial_result.chars().count();
                    let x = (start.row..=end.row)
                        .map(|it| editor_content.line_len(it))
                        .max_by(|a, b| a.cmp(b))
                        .unwrap()
                        + 3;
                    let height =
                        r.editor_y_to_render_y[end.row] - r.editor_y_to_render_y[start.row] + 1;
                    render_buckets.set_color(Layer::AboveText, 0xAAFFAA_FF);
                    render_buckets.draw_rect(
                        Layer::AboveText,
                        r.left_gutter_width + x,
                        r.editor_y_to_render_y[start.row],
                        result_w + 1,
                        height,
                    );
                    // draw the parenthesis
                    render_buckets.set_color(Layer::AboveText, 0x000000_FF);

                    render_buckets.draw_char(
                        Layer::AboveText,
                        r.left_gutter_width + x,
                        r.editor_y_to_render_y[start.row],
                        '⎫',
                    );
                    render_buckets.draw_char(
                        Layer::AboveText,
                        r.left_gutter_width + x,
                        r.editor_y_to_render_y[end.row],
                        '⎭',
                    );
                    for i in 1..height {
                        render_buckets.draw_char(
                            Layer::AboveText,
                            r.left_gutter_width + x,
                            r.editor_y_to_render_y[start.row] + i,
                            '⎪',
                        );
                    }
                    // center
                    render_buckets.draw_string(
                        Layer::AboveText,
                        r.left_gutter_width + x,
                        r.editor_y_to_render_y[start.row] + height / 2,
                        partial_result,
                    );
                }
            }
        }
    }

    pub fn handle_mouse_up(&mut self, _x: usize, _y: usize) {
        self.right_gutter_is_dragged = false;
    }

    pub fn handle_click<'b>(&mut self, x: usize, clicked_y: usize, holder: &Holder<'b>) {
        if x < LEFT_GUTTER_WIDTH {
            // clicked on left gutter
        } else if x < self.render_data.result_gutter_x {
            let clicked_x = x - LEFT_GUTTER_WIDTH;
            let prev_selection = self.editor.get_selection();
            let editor_click_pos = if let Some(editor_obj) =
                self.get_obj_at_rendered_pos(clicked_x, clicked_y, holder)
            {
                match editor_obj.typ {
                    EditorObjectType::LineReference => {
                        Some(Pos::from_row_column(editor_obj.row, editor_obj.end_x))
                    }
                    EditorObjectType::Matrix {
                        row_count,
                        col_count,
                    } => {
                        if self.matrix_editing.is_some() {
                            NoteCalcApp::end_matrix_editing(
                                &mut self.matrix_editing,
                                &mut self.editor,
                                &mut self.editor_content,
                                None,
                            );
                        } else {
                            self.matrix_editing = Some(MatrixEditing::new(
                                row_count,
                                col_count,
                                &self.editor_content.get_line_chars(editor_obj.row)
                                    [editor_obj.start_x..editor_obj.end_x],
                                editor_obj.row,
                                editor_obj.start_x,
                                editor_obj.end_x,
                                Pos::from_row_column(0, 0),
                            ))
                        }
                        // Some(Pos::from_row_column(editor_obj.row, editor_obj.end_x))
                        None
                    }
                    EditorObjectType::SimpleTokens => {
                        let x_pos_within = clicked_x - editor_obj.rendered_x;
                        Some(Pos::from_row_column(
                            editor_obj.row,
                            editor_obj.start_x + x_pos_within,
                        ))
                    }
                }
            } else {
                self.rendered_y_to_editor_y(clicked_y)
                    .map(|it| Pos::from_row_column(it, clicked_x))
            };

            if let Some(editor_click_pos) = editor_click_pos {
                if self.matrix_editing.is_some() {
                    NoteCalcApp::end_matrix_editing(
                        &mut self.matrix_editing,
                        &mut self.editor,
                        &mut self.editor_content,
                        None,
                    );
                }
                self.editor.handle_click(
                    editor_click_pos.column,
                    editor_click_pos.row,
                    &self.editor_content,
                );
                let new_row = self.editor.get_selection().get_cursor_pos().row;
                let modif = if prev_selection.is_range() {
                    let mut m = UpdateRequirement::range(
                        prev_selection.get_first().row,
                        prev_selection.get_second().row,
                        UpdateReqType::RerenderWholeLine,
                    );
                    m.merge(&UpdateRequirement::single_row(
                        new_row,
                        UpdateReqType::RerenderWholeLine,
                    ));
                    m
                } else {
                    UpdateRequirement::multiple(
                        &[prev_selection.start.row, new_row],
                        UpdateReqType::RerenderWholeLine,
                    )
                };
                self.last_unrendered_modifications.merge(&modif);
                self.editor.blink_cursor();
            }
        } else if x - self.render_data.result_gutter_x < RIGHT_GUTTER_WIDTH {
            // clicked on right gutter
            self.right_gutter_is_dragged = true;
        }
    }

    pub fn rendered_y_to_editor_y(&self, clicked_y: usize) -> Option<usize> {
        for (ed_y, r_y) in self.render_data.editor_y_to_render_y.iter().enumerate() {
            if *r_y == clicked_y {
                return Some(ed_y);
            } else if *r_y > clicked_y {
                return Some(ed_y - 1);
            }
        }
        return None;
    }

    pub fn get_obj_at_rendered_pos<'b>(
        &self,
        x: usize,
        render_y: usize,
        holder: &'b Holder<'b>,
    ) -> Option<&'b EditorObject> {
        if let Some(editor_y) = self.rendered_y_to_editor_y(render_y) {
            holder.editor_objects[editor_y].iter().find(|editor_obj| {
                (editor_obj.rendered_x..editor_obj.rendered_x + editor_obj.rendered_w).contains(&x)
                    && (editor_obj.rendered_y..editor_obj.rendered_y + editor_obj.rendered_h)
                        .contains(&render_y)
            })
        } else {
            None
        }
    }

    pub fn handle_drag(&mut self, x: usize, y: usize) {
        if self.right_gutter_is_dragged {
            self.set_result_gutter_x(NoteCalcApp::calc_result_gutter_x(
                Some(x),
                self.client_width,
            ));
        } else if x < LEFT_GUTTER_WIDTH {
            // clicked on left gutter
        } else if x - LEFT_GUTTER_WIDTH < MAX_EDITOR_WIDTH {
            if let Some(y) = self.rendered_y_to_editor_y(y) {
                self.editor
                    .handle_drag(x - LEFT_GUTTER_WIDTH, y, &self.editor_content);
                self.editor.blink_cursor();
            }
        }
    }

    pub fn set_result_gutter_x(&mut self, x: usize) {
        self.render_data.result_gutter_x = x;
        // todo rerender everything
        self.last_unrendered_modifications
            .merge(&UpdateRequirement::all_rows_starting_at(
                0,
                UpdateReqType::RerenderWholeLine,
            ));
    }

    pub fn handle_resize(&mut self, new_client_width: usize) {
        self.client_width = new_client_width;
        self.set_result_gutter_x(NoteCalcApp::calc_result_gutter_x(
            Some(self.render_data.result_gutter_x),
            new_client_width,
        ));
    }

    fn calc_result_gutter_x(current_x: Option<usize>, client_width: usize) -> usize {
        return (if let Some(current_x) = current_x {
            current_x
        } else {
            LEFT_GUTTER_WIDTH + MAX_EDITOR_WIDTH
        })
        .min(client_width - (RIGHT_GUTTER_WIDTH + MIN_RESULT_PANEL_WIDTH));
    }

    pub fn handle_time(&mut self, now: u32) -> bool {
        let need_rerender = if let Some(mat_editor) = &mut self.matrix_editing {
            mat_editor.editor.handle_tick(now)
        } else {
            self.editor.handle_tick(now)
        };
        if need_rerender {
            let (from, to) = self.editor.get_selection().get_range();
            let modif =
                UpdateRequirement::range(from.row, to.row, UpdateReqType::RerenderWholeLine);
            UpdateRequirement::merge(&mut self.last_unrendered_modifications, &modif);
        }
        need_rerender
    }

    pub fn get_normalized_content(&self) -> String {
        let mut result: String = String::with_capacity(self.editor_content.line_count() * 40);
        for line in self.editor_content.lines() {
            let mut i = 0;
            'i: while i < line.len() {
                if i + 3 < line.len() && line[i] == '&' && line[i + 1] == '[' {
                    let mut end = i + 2;
                    let mut num: u32 = 0;
                    while end < line.len() {
                        if line[end] == ']' && num > 0 {
                            // which row has the id of 'num'?
                            let row_index = self
                                .editor_content
                                .data()
                                .iter()
                                .position(|it| it.line_id == num as usize)
                                .unwrap_or(0)
                                + 1; // '+1' line id cannot be 0
                            result.push('&');
                            result.push('[');
                            let mut line_id = row_index;
                            while line_id > 0 {
                                let to_insert = line_id % 10;
                                result.push((48 + to_insert as u8) as char);
                                line_id /= 10;
                            }
                            // reverse the number
                            {
                                let last_i = result.len() - 1;
                                let replace_i = if row_index > 99 {
                                    2
                                } else if row_index > 9 {
                                    1
                                } else {
                                    0
                                };
                                if replace_i != 0 {
                                    unsafe {
                                        let tmp = result.as_bytes()[last_i];
                                        result.as_bytes_mut()[last_i] =
                                            result.as_bytes()[last_i - replace_i];
                                        result.as_bytes_mut()[last_i - replace_i] = tmp;
                                    }
                                }
                            }
                            result.push(']');
                            i = end + 1;
                            continue 'i;
                        } else if let Some(digit) = line[end].to_digit(10) {
                            num = if num == 0 { digit } else { num * 10 + digit };
                        } else {
                            break;
                        }
                        end += 1;
                    }
                }
                result.push(line[i]);
                i += 1;
            }
            result.push('\n');
        }

        return result;
    }

    pub fn alt_key_released<'b>(&mut self, holder: &Holder<'b>) {
        if self.line_reference_chooser.is_none() {
            return;
        }

        let cursor_row = self.editor.get_selection().get_cursor_pos().row;
        let line_ref_row = self.line_reference_chooser.unwrap();

        self.last_unrendered_modifications
            .merge(&UpdateRequirement::single_row(
                line_ref_row,
                UpdateReqType::RerenderWholeLine,
            ));

        self.line_reference_chooser = None;

        if cursor_row == line_ref_row || matches!(&holder.results[line_ref_row], Err(_) | Ok(None))
        {
            return;
        }
        let line_id = {
            let line_data = self.editor_content.mut_data(line_ref_row);
            if line_data.line_id == 0 {
                line_data.line_id = self.line_id_generator;
                self.line_id_generator += 1;
            }
            line_data.line_id
        };
        let inserting_text = format!("&[{}]", line_id);
        self.editor
            .insert_text(&inserting_text, &mut self.editor_content);
        self.last_unrendered_modifications
            .merge(&UpdateRequirement::single_row(
                cursor_row,
                UpdateReqType::RerenderWholeLine,
            ));
    }

    pub fn handle_paste(&mut self, text: String) -> bool {
        self.editor
            .insert_text(&text, &mut self.editor_content)
            .is_some()
    }

    pub fn handle_input<'b>(
        &mut self,
        input: EditorInputEvent,
        modifiers: InputModifiers,
        // TODO azt adjuk át ami kell neki ne az egész holdert
        holder: &mut Holder<'b>,
    ) -> bool {
        let mut content_was_modified = false;
        let modif = if self.matrix_editing.is_none() && modifiers.alt {
            if input == EditorInputEvent::Left {
                let selection = self.editor.get_selection();
                let (start, end) = selection.get_range();
                for row_i in start.row..=end.row {
                    let new_format = match &self.editor_content.get_data(row_i).result_format {
                        ResultFormat::Bin => ResultFormat::Hex,
                        ResultFormat::Dec => ResultFormat::Bin,
                        ResultFormat::Hex => ResultFormat::Dec,
                    };
                    self.editor_content.mut_data(row_i).result_format = new_format;
                }
                Some(UpdateRequirement::range(
                    start.row,
                    end.row,
                    UpdateReqType::RerenderResult,
                ))
            } else if input == EditorInputEvent::Right {
                let selection = self.editor.get_selection();
                let (start, end) = selection.get_range();
                for row_i in start.row..=end.row {
                    let new_format = match &self.editor_content.get_data(row_i).result_format {
                        ResultFormat::Bin => ResultFormat::Dec,
                        ResultFormat::Dec => ResultFormat::Hex,
                        ResultFormat::Hex => ResultFormat::Bin,
                    };
                    self.editor_content.mut_data(row_i).result_format = new_format;
                }
                Some(UpdateRequirement::range(
                    start.row,
                    end.row,
                    UpdateReqType::RerenderResult,
                ))
            } else if input == EditorInputEvent::Up {
                let cur_pos = self.editor.get_selection().get_cursor_pos();
                let rows = if let Some(selector_row) = self.line_reference_chooser {
                    if selector_row > 0 {
                        Some((selector_row, selector_row - 1))
                    } else {
                        Some((selector_row, selector_row))
                    }
                } else if cur_pos.row > 0 {
                    Some((cur_pos.row - 1, cur_pos.row - 1))
                } else {
                    None
                };
                if let Some((prev_selected_row, new_selected_row)) = rows {
                    self.line_reference_chooser = Some(new_selected_row);
                    Some(UpdateRequirement::range(
                        new_selected_row,
                        prev_selected_row,
                        UpdateReqType::RerenderWholeLine,
                    ))
                } else {
                    None
                }
            } else if input == EditorInputEvent::Down {
                let cur_pos = self.editor.get_selection().get_cursor_pos();
                let rows = if let Some(selector_row) = self.line_reference_chooser {
                    if selector_row < cur_pos.row - 1 {
                        Some((selector_row, selector_row + 1))
                    } else {
                        Some((selector_row, selector_row))
                    }
                } else {
                    None
                };
                if let Some((prev_selected_row, new_selected_row)) = rows {
                    self.line_reference_chooser = Some(new_selected_row);
                    Some(UpdateRequirement::range(
                        prev_selected_row,
                        new_selected_row,
                        UpdateReqType::RerenderWholeLine,
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else if self.matrix_editing.is_some() {
            let prev_row = self.editor.get_selection().get_cursor_pos().row;
            self.handle_matrix_editor_input(input, modifiers);
            if self.matrix_editing.is_none() {
                content_was_modified = true;
                let new_row = self.editor.get_selection().get_cursor_pos().row;
                Some(UpdateRequirement::combine(
                    UpdateRequirement::single_row(prev_row, UpdateReqType::ReparseTokens),
                    UpdateRequirement::single_row(new_row, UpdateReqType::RerenderWholeLine),
                ))
            } else {
                Some(UpdateRequirement::single_row(
                    prev_row,
                    UpdateReqType::RerenderWholeLine,
                ))
            }
        } else {
            if let Some(render_req) = self.handle_completion(
                &input,
                &mut holder.editor_objects,
                &holder.autocompletion_src,
            ) {
                content_was_modified = true;
                Some(render_req)
            } else if let Some(render_req) =
                self.handle_obj_deletion(&input, &mut holder.editor_objects)
            {
                content_was_modified = true;
                Some(render_req)
            } else if let Some(render_req) =
                self.handle_obj_jump_over(&input, modifiers, &holder.editor_objects)
            {
                content_was_modified = false;
                Some(render_req)
            } else {
                let prev_selection = self.editor.get_selection();
                let prev_cursor_pos = prev_selection.get_cursor_pos();

                let modif_type =
                    self.editor
                        .handle_input(input, modifiers, &mut self.editor_content);
                if modif_type.is_none() {
                    // it is possible to step into a matrix only through navigation
                    self.check_stepping_into_matrix(prev_cursor_pos, &holder.editor_objects);
                }

                content_was_modified = modif_type.is_some();
                match modif_type {
                    Some(RowModificationType::SingleLine(index)) => Some(
                        UpdateRequirement::single_row(index, UpdateReqType::ReparseTokens),
                    ),
                    Some(RowModificationType::AllLinesFrom(index)) => {
                        Some(UpdateRequirement::all_rows_starting_at(
                            index,
                            UpdateReqType::ReparseTokens,
                        ))
                    }
                    None => {
                        let cursor_pos = self.editor.get_selection().get_cursor_pos();
                        let new_cursor_y = cursor_pos.row;
                        if prev_selection.is_range() {
                            let from = prev_selection.get_first().row.min(new_cursor_y);
                            let to = prev_selection.get_second().row.max(new_cursor_y);
                            Some(UpdateRequirement::range(
                                from,
                                to,
                                UpdateReqType::RerenderWholeLine,
                            ))
                        } else {
                            let old_cursor_y = prev_cursor_pos.row;
                            if old_cursor_y > new_cursor_y {
                                Some(UpdateRequirement::range(
                                    new_cursor_y,
                                    old_cursor_y,
                                    UpdateReqType::RerenderWholeLine,
                                ))
                            } else if old_cursor_y < new_cursor_y {
                                Some(UpdateRequirement::range(
                                    old_cursor_y,
                                    new_cursor_y,
                                    UpdateReqType::RerenderWholeLine,
                                ))
                            } else {
                                let new_cursor_x = cursor_pos.column;
                                let old_cursor_x = prev_cursor_pos.column;
                                if old_cursor_x != new_cursor_x {
                                    Some(UpdateRequirement::single_row(
                                        new_cursor_y,
                                        UpdateReqType::RerenderWholeLine,
                                    ))
                                } else {
                                    None
                                }
                            }
                        }
                    }
                }
            }
        };

        if let Some(modif) = modif {
            UpdateRequirement::merge(&mut self.last_unrendered_modifications, &modif);
        }
        return content_was_modified;
    }

    fn handle_completion<'b>(
        &mut self,
        input: &EditorInputEvent,
        editor_objects: &mut Vec<Vec<EditorObject>>,
        autocompletion_src: &[char],
    ) -> Option<UpdateRequirement> {
        let cursor_pos = self.editor.get_selection();
        if *input == EditorInputEvent::Tab && cursor_pos.get_cursor_pos().column > 0 {
            let cursor_pos = cursor_pos.get_cursor_pos();
            let line = self.editor_content.get_line_chars(cursor_pos.row);
            let is_m = line[cursor_pos.column - 1] == 'm';
            if is_m
                && (cursor_pos.column == 1
                    || (!line[cursor_pos.column - 2].is_alphanumeric()
                        && line[cursor_pos.column - 2] != '_'))
            {
                let prev_col = cursor_pos.column;
                self.editor.handle_input(
                    EditorInputEvent::Backspace,
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                self.editor.handle_input(
                    EditorInputEvent::Char('['),
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                self.editor.handle_input(
                    EditorInputEvent::Char('0'),
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                self.editor.handle_input(
                    EditorInputEvent::Char(']'),
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                self.editor
                    .set_selection_save_col(Selection::single(cursor_pos.with_column(prev_col)));
                // TODO asdd ujra kell renderelni a sortol kezdve mindent
                editor_objects[cursor_pos.row].push(EditorObject {
                    typ: EditorObjectType::Matrix {
                        row_count: 1,
                        col_count: 1,
                    },
                    row: cursor_pos.row,
                    start_x: prev_col - 1,
                    end_x: prev_col + 2,
                    rendered_x: 0, // dummy
                    rendered_y: 0, // dummy
                    rendered_w: 3,
                    rendered_h: 1,
                });
                self.check_stepping_into_matrix(Pos::from_row_column(0, 0), &editor_objects);
                return Some(UpdateRequirement::single_row(
                    cursor_pos.row,
                    UpdateReqType::ReparseTokens,
                ));
            } else {
                // check for autocompletion
                // find space
                let (begin_index, expected_len) = {
                    let mut begin_index = cursor_pos.column - 1;
                    let mut len = 1;
                    while begin_index > 0
                        && (line[begin_index - 1].is_alphanumeric() || line[begin_index - 1] == '_')
                    {
                        begin_index -= 1;
                        len += 1;
                    }
                    (begin_index, len)
                };
                // find the best match
                let mut autocomp_src_i = 0;
                let mut word_i = begin_index;
                let mut match_begin_index = None;
                let curr_row_index = cursor_pos.row;
                let aut_comp_entrs = &autocompletion_src;
                'main: while autocomp_src_i < aut_comp_entrs.len()
                    && (aut_comp_entrs[autocomp_src_i] as usize) < curr_row_index
                    && aut_comp_entrs[autocomp_src_i + 1] != 0 as char
                {
                    // skip the row index
                    autocomp_src_i += 1;
                    let start_autocomp_src_i = autocomp_src_i;
                    while autocomp_src_i < aut_comp_entrs.len()
                        && aut_comp_entrs[autocomp_src_i] != 0 as char
                        && aut_comp_entrs[autocomp_src_i] == line[word_i]
                    {
                        autocomp_src_i += 1;
                        word_i += 1;
                    }
                    if (autocomp_src_i - start_autocomp_src_i) == expected_len {
                        if match_begin_index.is_some() {
                            // multiple match, don't autocomplete
                            match_begin_index = None;
                            break 'main;
                        } else {
                            match_begin_index = Some(autocomp_src_i - expected_len);
                        }
                    }
                    // jump to the next autocompletion entry
                    while aut_comp_entrs[autocomp_src_i] != 0 as char {
                        autocomp_src_i += 1;
                    }
                    // skip 0
                    autocomp_src_i += 1;
                    word_i = begin_index;
                }
                if let Some(match_begin_index) = match_begin_index {
                    let mut i = match_begin_index + expected_len;
                    while i < aut_comp_entrs.len() && aut_comp_entrs[i] != 0 as char {
                        self.editor.handle_input(
                            EditorInputEvent::Char(aut_comp_entrs[i]),
                            InputModifiers::none(),
                            &mut self.editor_content,
                        );
                        i += 1;
                    }
                    return Some(UpdateRequirement::single_row(
                        cursor_pos.row,
                        UpdateReqType::ReparseTokens,
                    ));
                }
            }
        }
        return None;
    }

    fn handle_obj_deletion<'b>(
        &mut self,
        input: &EditorInputEvent,
        editor_objects: &mut Vec<Vec<EditorObject>>,
    ) -> Option<UpdateRequirement> {
        let selection = self.editor.get_selection();
        let cursor_pos = selection.get_cursor_pos();
        if *input == EditorInputEvent::Backspace
            && !selection.is_range()
            && selection.start.column > 0
        {
            if let Some(index) =
                self.index_of_matrix_or_lineref_at(cursor_pos.with_prev_col(), editor_objects)
            {
                // remove it
                let obj = editor_objects[cursor_pos.row].remove(index);
                let sel = Selection::range(
                    Pos::from_row_column(obj.row, obj.start_x),
                    Pos::from_row_column(obj.row, obj.end_x),
                );
                self.editor.set_selection_save_col(sel);
                self.editor.handle_input(
                    EditorInputEvent::Backspace,
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                return Some(UpdateRequirement::single_row(
                    cursor_pos.row,
                    UpdateReqType::ReparseTokens,
                ));
            }
        } else if *input == EditorInputEvent::Del && !selection.is_range() {
            if let Some(index) = self.index_of_matrix_or_lineref_at(cursor_pos, editor_objects) {
                // remove it
                let obj = editor_objects[cursor_pos.row].remove(index);
                let sel = Selection::range(
                    Pos::from_row_column(obj.row, obj.start_x),
                    Pos::from_row_column(obj.row, obj.end_x),
                );
                self.editor.set_selection_save_col(sel);
                self.editor.handle_input(
                    EditorInputEvent::Del,
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                return Some(UpdateRequirement::single_row(
                    cursor_pos.row,
                    UpdateReqType::ReparseTokens,
                ));
            }
        }
        return None;
    }

    fn handle_obj_jump_over<'b>(
        &mut self,
        input: &EditorInputEvent,
        modifiers: InputModifiers,
        editor_objects: &Vec<Vec<EditorObject>>,
    ) -> Option<UpdateRequirement> {
        let selection = self.editor.get_selection();
        let cursor_pos = selection.get_cursor_pos();
        if *input == EditorInputEvent::Left
            && !selection.is_range()
            && selection.start.column > 0
            && modifiers.shift == false
        {
            let obj = self
                .find_editor_object_at(cursor_pos.with_prev_col(), editor_objects)
                .map(|it| (it.typ, it.row, it.start_x));
            if let Some((obj_typ, row, start_x)) = obj {
                if obj_typ == EditorObjectType::LineReference {
                    //  jump over it
                    self.editor.set_cursor_pos_r_c(row, start_x);
                    return Some(UpdateRequirement::single_row(
                        cursor_pos.row,
                        UpdateReqType::RerenderWholeLine,
                    ));
                }
            }
        } else if *input == EditorInputEvent::Right
            && !selection.is_range()
            && modifiers.shift == false
        {
            let obj = self
                .find_editor_object_at(cursor_pos, editor_objects)
                .map(|it| (it.typ, it.row, it.end_x));

            if let Some((obj_typ, row, end_x)) = obj {
                if obj_typ == EditorObjectType::LineReference {
                    //  jump over it
                    self.editor.set_cursor_pos_r_c(row, end_x);
                    return Some(UpdateRequirement::single_row(
                        cursor_pos.row,
                        UpdateReqType::RerenderWholeLine,
                    ));
                }
            }
        }
        return None;
    }

    fn check_stepping_into_matrix<'b>(
        &mut self,
        enter_from_pos: Pos,
        editor_objects: &Vec<Vec<EditorObject>>,
    ) {
        if let Some(editor_obj) = NoteCalcApp::is_pos_inside_an_obj(
            editor_objects,
            self.editor.get_selection().get_cursor_pos(),
        ) {
            match editor_obj.typ {
                EditorObjectType::Matrix {
                    row_count,
                    col_count,
                } => {
                    if self.matrix_editing.is_none() && !self.editor.get_selection().is_range() {
                        self.matrix_editing = Some(MatrixEditing::new(
                            row_count,
                            col_count,
                            &self.editor_content.get_line_chars(editor_obj.row)
                                [editor_obj.start_x..editor_obj.end_x],
                            editor_obj.row,
                            editor_obj.start_x,
                            editor_obj.end_x,
                            enter_from_pos,
                        ));
                    }
                }
                EditorObjectType::SimpleTokens | EditorObjectType::LineReference => {}
            }
        }
    }

    fn find_editor_object_at<'b>(
        &self,
        pos: Pos,
        editor_objects: &'b Vec<Vec<EditorObject>>,
    ) -> Option<&'b EditorObject> {
        for obj in &editor_objects[pos.row] {
            if (obj.start_x..obj.end_x).contains(&pos.column) {
                return Some(obj);
            }
        }
        return None;
    }

    fn is_pos_inside_an_obj(
        editor_objects: &[Vec<EditorObject>],
        pos: Pos,
    ) -> Option<&EditorObject> {
        for obj in &editor_objects[pos.row] {
            if (obj.start_x + 1..obj.end_x).contains(&pos.column) {
                return Some(obj);
            }
        }
        return None;
    }

    fn index_of_matrix_or_lineref_at<'b>(
        &self,
        pos: Pos,
        editor_objects: &Vec<Vec<EditorObject>>,
    ) -> Option<usize> {
        return editor_objects[pos.row].iter().position(|obj| {
            matches!(obj.typ, EditorObjectType::LineReference | EditorObjectType::Matrix {..})
                && (obj.start_x..obj.end_x).contains(&pos.column)
        });
    }

    fn handle_matrix_editor_input(&mut self, input: EditorInputEvent, modifiers: InputModifiers) {
        let mat_edit = self.matrix_editing.as_mut().unwrap();
        let cur_pos = self.editor.get_selection().get_cursor_pos();

        let simple = !modifiers.shift && !modifiers.alt;
        let alt = modifiers.alt;
        if input == EditorInputEvent::Esc || input == EditorInputEvent::Enter {
            NoteCalcApp::end_matrix_editing(
                &mut self.matrix_editing,
                &mut self.editor,
                &mut self.editor_content,
                None,
            );
        } else if input == EditorInputEvent::Tab {
            if mat_edit.current_cell.column + 1 < mat_edit.col_count {
                mat_edit.change_cell(mat_edit.current_cell.with_next_col());
            } else if mat_edit.current_cell.row + 1 < mat_edit.row_count {
                mat_edit.change_cell(mat_edit.current_cell.with_next_row().with_column(0));
            } else {
                let end_text_index = mat_edit.end_text_index;
                NoteCalcApp::end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    Some(cur_pos.with_column(end_text_index)),
                );
            }
        } else if alt && input == EditorInputEvent::Right {
            mat_edit.add_column();
        } else if alt && input == EditorInputEvent::Left && mat_edit.col_count > 1 {
            mat_edit.remove_column();
        } else if alt && input == EditorInputEvent::Down {
            mat_edit.add_row();
        } else if alt && input == EditorInputEvent::Up && mat_edit.row_count > 1 {
            mat_edit.remove_row();
        } else if simple
            && input == EditorInputEvent::Left
            && mat_edit.editor.is_cursor_at_beginning()
        {
            if mat_edit.current_cell.column > 0 {
                mat_edit.change_cell(mat_edit.current_cell.with_prev_col());
            } else {
                let start_text_index = mat_edit.start_text_index;
                NoteCalcApp::end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    Some(cur_pos.with_column(start_text_index)),
                );
            }
        } else if simple
            && input == EditorInputEvent::Right
            && mat_edit.editor.is_cursor_at_eol(&mat_edit.editor_content)
        {
            if mat_edit.current_cell.column + 1 < mat_edit.col_count {
                mat_edit.change_cell(mat_edit.current_cell.with_next_col());
            } else {
                let end_text_index = mat_edit.end_text_index;
                NoteCalcApp::end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    Some(cur_pos.with_column(end_text_index)),
                );
            }
        } else if simple && input == EditorInputEvent::Up {
            if mat_edit.current_cell.row > 0 {
                mat_edit.change_cell(mat_edit.current_cell.with_prev_row());
            } else {
                NoteCalcApp::end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    None,
                );
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else if simple && input == EditorInputEvent::Down {
            if mat_edit.current_cell.row + 1 < mat_edit.row_count {
                mat_edit.change_cell(mat_edit.current_cell.with_next_row());
            } else {
                NoteCalcApp::end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    None,
                );
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else if simple && input == EditorInputEvent::End {
            if mat_edit.current_cell.column != mat_edit.col_count - 1 {
                mat_edit.change_cell(mat_edit.current_cell.with_column(mat_edit.col_count - 1));
            } else {
                let end_text_index = mat_edit.end_text_index;
                NoteCalcApp::end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    Some(cur_pos.with_column(end_text_index)),
                );
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else if simple && input == EditorInputEvent::Home {
            if mat_edit.current_cell.column != 0 {
                mat_edit.change_cell(mat_edit.current_cell.with_column(0));
            } else {
                let start_index = mat_edit.start_text_index;
                NoteCalcApp::end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    Some(cur_pos.with_column(start_index)),
                );
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else {
            mat_edit
                .editor
                .handle_input(input, modifiers, &mut mat_edit.editor_content);
        }
    }

    pub fn render<'a, 'b>(
        &mut self,
        units: &Units,
        render_buckets: &mut RenderBuckets<'a>,
        result_buffer: &'a mut [u8],
        allocator: &'a Arena<char>,
        holder: &mut Holder<'a>,
    ) {
        // Arena::with_capacity(MAX_LINE_COUNT * MAX_EDITOR_WIDTH)
        let _sh = self.last_unrendered_modifications;
        NoteCalcApp::renderr(
            &mut self.editor,
            &self.editor_content,
            units,
            &mut self.matrix_editing,
            &mut self.line_reference_chooser,
            render_buckets,
            result_buffer,
            self.last_unrendered_modifications,
            &mut self.render_data,
            allocator,
            holder,
        );
        self.last_unrendered_modifications.clear();
    }
}

struct ResultLengths {
    int_part_len: usize,
    frac_part_len: usize,
    unit_part_len: usize,
}

impl ResultLengths {
    fn set_max(&mut self, other: &ResultLengths) {
        if self.int_part_len < other.int_part_len {
            self.int_part_len = other.int_part_len;
        }
        if self.frac_part_len < other.frac_part_len {
            self.frac_part_len = other.frac_part_len;
        }
        if self.unit_part_len < other.unit_part_len {
            self.unit_part_len = other.unit_part_len;
        }
    }
}

fn get_int_frac_part_len(cell_str: &str) -> ResultLengths {
    let mut int_part_len = 0;
    let mut frac_part_len = 0;
    let mut unit_part_len = 0;
    let mut was_point = false;
    let mut was_space = false;
    for ch in cell_str.as_bytes() {
        if *ch == b'.' {
            was_point = true;
        } else if *ch == b' ' {
            was_space = true;
        }
        if was_space {
            unit_part_len += 1;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::editor::Selection;
    use std::ops::RangeInclusive;

    fn create_holder<'b>() -> Holder<'b> {
        let mut holder = Holder {
            editor_objects: std::iter::repeat(Vec::with_capacity(8))
                .take(MAX_LINE_COUNT)
                .collect::<Vec<_>>(),
            autocompletion_src: Vec::with_capacity(256),
            // TODO: array?
            results: [Ok(None); MAX_LINE_COUNT],
            vars: Vec::with_capacity(MAX_LINE_COUNT),
            tokens: [None; MAX_LINE_COUNT],
        };
        holder.vars.push(Variable {
            name: Box::from(&['s', 'u', 'm'][..]),
            value: Err(()),
            defined_at_row: 0,
        });
        holder
    }

    fn create_app<'a>() -> (NoteCalcApp, Units, Holder<'a>) {
        let app = NoteCalcApp::new(120);
        let units = Units::new();
        return (app, units, create_holder());
    }

    #[test]
    fn bug1() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]",
            &mut app.editor_content,
        );
        app.editor.insert_text(
            "[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]",
            &mut app.editor_content,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 33));
        app.handle_input(EditorInputEvent::Right, InputModifiers::alt(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
    }

    #[test]
    fn bug2() {
        let (mut app, units, mut holder) = create_app();
        app.editor.insert_text(
            "[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]",
            &mut app.editor_content,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 1));

        app.handle_input(EditorInputEvent::Right, InputModifiers::alt(), &mut holder);
        let arena = Arena::new();
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Down, InputModifiers::none(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
    }

    #[test]
    fn bug3() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "1\n\
                2+",
            &mut app.editor_content,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 2));
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        app.alt_key_released(&mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
    }

    #[test]
    fn bug4() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "1\n\
                ",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 0));
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        app.alt_key_released(&mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_eq!(
            "1\n\
             &[1]",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn it_is_not_allowed_to_ref_lines_below() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "1\n\
                2+\n3\n4",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 2));
        app.handle_input(EditorInputEvent::Down, InputModifiers::alt(), &mut holder);
        app.alt_key_released(&mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_eq!(
            "1\n\
                2+\n3\n4",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn it_is_not_allowed_to_ref_lines_below2() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            &"1\n\
                2+\n3\n4",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 2));
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        app.handle_input(EditorInputEvent::Down, InputModifiers::alt(), &mut holder);
        app.alt_key_released(&mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_eq!(
            "1\n\
                2+&[1]\n3\n4",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn remove_matrix_backspace() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("abcd [1,2,3;4,5,6]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!("abcd ", app.editor_content.get_content());
    }

    #[test]
    fn matrix_step_in_dir() {
        // from right
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1,2,3;4,5,6]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('1'),
                InputModifiers::none(),
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("abcd [1,2,1;4,5,6]", app.editor_content.get_content());
        }
        // from left
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1,2,3;4,5,6]", &mut app.editor_content);
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("abcd [9,2,3;4,5,6]", app.editor_content.get_content());
        }
        // from below
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text(
                "abcd [1,2,3;4,5,6]\naaaaaaaaaaaaaaaaaa",
                &mut app.editor_content,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 7));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!(
                "abcd [1,2,3;9,5,6]\naaaaaaaaaaaaaaaaaa",
                app.editor_content.get_content()
            );
        }
        // from above
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text(
                "aaaaaaaaaaaaaaaaaa\nabcd [1,2,3;4,5,6]",
                &mut app.editor_content,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 7));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Down, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!(
                "aaaaaaaaaaaaaaaaaa\nabcd [9,2,3;4,5,6]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn cursor_is_put_after_the_matrix_after_finished_editing() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("abcd [1,2,3;4,5,6]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(
            EditorInputEvent::Char('6'),
            InputModifiers::none(),
            &mut holder,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(
            EditorInputEvent::Char('9'),
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!(app.editor_content.get_content(), "abcd [1,2,6;4,5,6]9");
    }

    #[test]
    fn remove_matrix_del() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("abcd [1,2,3;4,5,6]", &mut app.editor_content);
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 5));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Del, InputModifiers::none(), &mut holder);
        assert_eq!("abcd ", app.editor_content.get_content());
    }

    #[test]
    fn test_moving_inside_a_matrix() {
        // right to left, cursor at end
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1,2,3;4,5,6]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!("abcd [1,9,3;4,5,6]", app.editor_content.get_content());
        }
        // left to right, cursor at start
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1,2,3;4,5,6]", &mut app.editor_content);
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
            app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!("abcd [1,2,9;4,5,6]", app.editor_content.get_content());
        }
        // vertical movement down, cursor tries to keep its position
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1111,22,3;44,55555,666]", &mut app.editor_content);
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            // inside the matrix
            app.handle_input(EditorInputEvent::Down, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!(
                "abcd [1111,22,3;9,55555,666]",
                app.editor_content.get_content()
            );
        }

        // vertical movement up, cursor tries to keep its position
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1111,22,3;44,55555,666]", &mut app.editor_content);
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            // inside the matrix
            app.handle_input(EditorInputEvent::Down, InputModifiers::none(), &mut holder);
            app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!(
                "abcd [9,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn test_moving_inside_a_matrix_with_tab() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("[1,2,3;4,5,6]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Home, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);

        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('7'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('8'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('9'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('0'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('9'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('4'),
            InputModifiers::none(),
            &mut holder,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_eq!("[1,7,8;9,0,9]4", app.editor_content.get_content());
    }

    #[test]
    fn test_leaving_a_matrix_with_tab() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("[1,2,3;4,5,6]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        // the next tab should leave the matrix
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('7'),
            InputModifiers::none(),
            &mut holder,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_eq!("[1,2,3;4,5,6]7", app.editor_content.get_content());
    }

    #[test]
    fn end_btn_matrix() {
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1111,22,3;44,55555,666] qq", &mut app.editor_content);
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            // inside the matrix
            app.handle_input(EditorInputEvent::End, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!(
                "abcd [1111,22,9;44,55555,666] qq",
                app.editor_content.get_content()
            );
        }
        // pressing twice, exits the matrix
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1111,22,3;44,55555,666] qq", &mut app.editor_content);
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
            // inside the matrix
            app.handle_input(EditorInputEvent::End, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::End, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!(
                "abcd [1111,22,3;44,55555,666] qq9",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn home_btn_matrix() {
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1111,22,3;44,55555,666]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            // inside the matrix
            app.handle_input(EditorInputEvent::Home, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &mut holder,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!(
                "abcd [9,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("abcd [1111,22,3;44,55555,666]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            // inside the matrix
            app.handle_input(EditorInputEvent::Home, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Home, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(
                EditorInputEvent::Char('6'),
                InputModifiers::none(),
                &mut holder,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!(
                "6abcd [1111,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn bug8() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("16892313\n14 * ", &mut app.editor_content);
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 5));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        app.alt_key_released(&mut holder);
        assert_eq!("16892313\n14 * &[1]", app.editor_content.get_content());
        app.last_unrendered_modifications =
            UpdateRequirement::all_rows_starting_at(0, UpdateReqType::ReparseTokens);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_time(1000);
        app.handle_input(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!("16892313\n14 * ", app.editor_content.get_content());

        app.handle_input(
            EditorInputEvent::Char('z'),
            InputModifiers::ctrl(),
            &mut holder,
        );
        assert_eq!("16892313\n14 * &[1]", app.editor_content.get_content());

        app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder); // end selection
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!("16892313\n14 * a&[1]", app.editor_content.get_content());

        app.handle_input(
            EditorInputEvent::Char(' '),
            InputModifiers::none(),
            &mut holder,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('b'),
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!("16892313\n14 * a &[1]b", app.editor_content.get_content());

        app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Right, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('c'),
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!("16892313\n14 * a c&[1]b", app.editor_content.get_content());
    }

    #[test]
    fn test_line_ref_normalization() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n",
            &mut app.editor_content,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(12, 2));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        app.alt_key_released(&mut holder);
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        app.alt_key_released(&mut holder);
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        app.alt_key_released(&mut holder);
        assert_eq!(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13&[1]&[1]&[1]\n",
            &app.editor_content.get_content()
        );
        assert_eq!(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13&[12]&[12]&[12]\n\n",
            &app.get_normalized_content()
        );
    }

    #[test]
    fn test_line_ref_denormalization() {
        let (mut app, _units, _holder) = create_app();
        app.set_normalized_content("1111\n2222\n14 * &[2]&[2]&[2]\n");
        assert_eq!(1, app.editor_content.get_data(0).line_id);
        assert_eq!(2, app.editor_content.get_data(1).line_id);
        assert_eq!(3, app.editor_content.get_data(2).line_id);
    }

    #[test]
    fn test_that_set_content_rerenders_everything() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::ReparseTokens));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::ReparseTokens));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::ReparseTokens));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert!(!app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
        app.set_normalized_content("1111\n2222\n14 * &[2]&[2]&[2]\n");
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::ReparseTokens));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::ReparseTokens));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::ReparseTokens));
    }

    #[test]
    fn no_memory_deallocation_bug_in_line_selection() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n",
            &mut app.editor_content,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(12, 2));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
    }

    #[test]
    fn matrix_deletion() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(" [1,2,3]", &mut app.editor_content);
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Del, InputModifiers::none(), &mut holder);
        assert_eq!("[1,2,3]", app.editor_content.get_content());
    }

    #[test]
    fn matrix_insertion_bug() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text("[1,2,3]", &mut app.editor_content);
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!("a[1,2,3]", app.editor_content.get_content());
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        assert_eq!("a\n[1,2,3]", app.editor_content.get_content());
    }

    #[test]
    fn matrix_insertion_bug2() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("'[X] nth, sum fv", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        app.handle_input(EditorInputEvent::Del, InputModifiers::none(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_results(&["Err"][..], &result_buffer);
    }

    fn assert_results(expected_results: &[&str], result_buffer: &[u8]) {
        let mut i = 0;
        let mut ok_chars = Vec::with_capacity(32);
        let expected_len = expected_results.iter().map(|it| it.len()).sum();
        for r in expected_results.iter() {
            for ch in r.bytes() {
                assert_eq!(
                    result_buffer[i] as char,
                    ch as char,
                    "at {}: {:?}, result_buffer: {:?}",
                    i,
                    String::from_utf8(ok_chars).unwrap(),
                    &result_buffer[0..expected_len]
                        .iter()
                        .map(|it| *it as char)
                        .collect::<Vec<char>>()
                );
                ok_chars.push(ch);
                i += 1;
            }
            ok_chars.push(',' as u8);
            ok_chars.push(' ' as u8);
        }
        assert_eq!(result_buffer[i], 0, "more results than expected",);
    }

    #[test]
    fn sum_can_be_nullified() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "3m * 2m
--
1
2
sum",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_results(&["6 m^2", "", "1", "2", "3"][..], &result_buffer);
    }

    #[test]
    fn no_sum_value_in_case_of_error() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            &"3m * 2m\n\
                4\n\
                sum",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_results(&["6 m^2", "4", "Err"][..], &result_buffer);
    }

    #[test]
    fn test_ctrl_c() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text("aaaaaaaaa", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Left, InputModifiers::shift(), &mut holder);
        app.handle_input(EditorInputEvent::Left, InputModifiers::shift(), &mut holder);
        app.handle_input(EditorInputEvent::Left, InputModifiers::shift(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('c'),
            InputModifiers::ctrl(),
            &mut holder,
        );
        assert_eq!("aaa", &app.editor.clipboard);
    }

    #[test]
    fn test_changing_output_style_for_selected_rows() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            &"2\n\
                    4\n\
                    5",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        app.handle_input(EditorInputEvent::Left, InputModifiers::alt(), &mut holder);
        app.last_unrendered_modifications =
            UpdateRequirement::all_rows_starting_at(0, UpdateReqType::ReparseTokens);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_results(&["10", "100", "101"][..], &result_buffer);
    }

    #[test]
    fn test_matrix_sum() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("[1,2,3]\nsum", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        // both the first line and the 'sum' line renders a matrix, which leaves the result buffer empty
        assert_results(&["\u{0}"][..], &result_buffer);
    }

    #[test]
    fn test_rich_copy() {
        fn t(content: &str, expected: &str, selected_range: RangeInclusive<usize>) {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text(&content, &mut app.editor_content);
            app.editor.set_selection_save_col(Selection::range(
                Pos::from_row_column(*selected_range.start(), 0),
                Pos::from_row_column(*selected_range.end(), 0),
            ));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            let mut result_buffer = [0; 128];
            assert_eq!(
                expected,
                &app.copy_selected_rows_with_result_to_clipboard(
                    &units,
                    &mut RenderBuckets::new(),
                    &mut result_buffer,
                    &&arena,
                    &mut holder
                )
            );
        }
        t("1", "1  ‖ 1\n", 0..=0);
        t("1 + 2", "1 + 2  ‖ 3\n", 0..=0);
        t("23", "23  ‖ 23\n", 0..=0);
        t(
            "1\n\
           23",
            "1   ‖  1\n\
             23  ‖ 23\n",
            0..=1,
        );
        t(
            "1\n\
           23\n\
           99999.66666",
            "1   ‖  1\n\
                 23  ‖ 23\n",
            0..=1,
        );
        t(
            "1\n\
           23\n\
           99999.66666",
            "1            ‖     1\n\
             23           ‖    23\n\
             99999.66666  ‖ 99999.66666\n",
            0..=2,
        );
        t("[1]", "[1]  ‖ [1]\n", 0..=0);
        t(
            "[1]\n\
             [23]",
            "[1]  ‖ [1]\n",
            0..=0,
        );
        t(
            "[1]\n\
             [23]",
            "[1]   ‖ [ 1]\n\
             [23]  ‖ [23]\n",
            0..=1,
        );
        t("[1,2,3]", "[1  2  3]  ‖ [1  2  3]\n", 0..=0);
        t(
            "[1,2,3]\n[33, 44, 55]",
            "[1  2  3]     ‖ [ 1   2   3]\n\
             [33  44  55]  ‖ [33  44  55]\n",
            0..=1,
        );
        t(
            "[1;2;3]",
            "⎡1⎤  ‖ ⎡1⎤\n\
             ⎢2⎥  ‖ ⎢2⎥\n\
             ⎣3⎦  ‖ ⎣3⎦\n",
            0..=0,
        );
        t(
            "[1, 2, 3] * [1;2;3]",
            "            ⎡1⎤  ‖\n\
             [1  2  3] * ⎢2⎥  ‖ [14]
            ⎣3⎦  ‖\n",
            0..=0,
        );
        // test alignment
        t(
            "[1, 2, 3]\n'asd\n[1, 2, 3]\n[10, 20, 30]",
            "[1  2  3]     ‖ [1  2  3]\n\
             'asd          ‖\n\
             [1  2  3]     ‖ [ 1   2   3]\n\
             [10  20  30]  ‖ [10  20  30]\n",
            0..=3,
        );
    }

    #[test]
    fn test_line_ref_selection() {
        // left
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("16892313\n14 * ", &mut app.editor_content);
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
            app.alt_key_released(&mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::shift(), &mut holder);
            app.handle_input(
                EditorInputEvent::Backspace,
                InputModifiers::none(),
                &mut holder,
            );
            assert_eq!("16892313\n14 * &[1", app.editor_content.get_content());
        }
        // right
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("16892313\n14 * ", &mut app.editor_content);
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
            app.alt_key_released(&mut holder);
            app.last_unrendered_modifications =
                UpdateRequirement::all_rows_starting_at(0, UpdateReqType::ReparseTokens);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            app.handle_input(
                EditorInputEvent::Right,
                InputModifiers::shift(),
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Del, InputModifiers::none(), &mut holder);
            assert_eq!("16892313\n14 * [1]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_pressing_tab_on_m_char() {
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("m", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!("[0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("am", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!("am  ", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("a m", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_eq!("a [0]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_that_cursor_is_inside_matrix_on_creation() {
        let (mut app, _units, mut holder) = create_app();
        app.editor.insert_text("m", &mut app.editor_content);
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('1'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        assert_eq!("[1]", app.editor_content.get_content());
    }

    #[test]
    fn test_matrix_alt_plus_right() {
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1,0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1,0,0,0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1;2]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1,0;2,0]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_matrix_alt_plus_left() {
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1, 2, 3]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1,2]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1, 2, 3]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("[1, 2, 3; 4,5,6]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1,2;4,5]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_matrix_alt_plus_down() {
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1;0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1;0;0;0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1,2]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt(), &mut holder);
            // this render is important, it tests a bug!
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1,2;0,0]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_matrix_alt_plus_up() {
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1; 2; 3]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1;2]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor.insert_text("[1; 2; 3]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("[1, 2, 3; 4,5,6]", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
            assert_eq!("[1,2,3]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_autocompletion_single() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("apple = 12$", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        assert_eq!("apple = 12$\napple", app.editor_content.get_content());
    }

    #[test]
    fn test_autocompletion_only_above_vars() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("apple = 12$", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Home, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
        app.handle_input(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        assert_eq!("a   \napple = 12$", app.editor_content.get_content());
    }

    #[test]
    fn test_autocompletion_two_vars() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("apple = 12$\nbanana = 7$\n", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        assert_eq!(
            "apple = 12$\nbanana = 7$\napple",
            app.editor_content.get_content()
        );

        app.handle_input(
            EditorInputEvent::Char(' '),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(
            EditorInputEvent::Char('b'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        assert_eq!(
            "apple = 12$\nbanana = 7$\napple banana",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_that_no_autocompletion_for_multiple_results() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("apple = 12$\nananas = 7$\n", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none(), &mut holder);
        assert_eq!(
            "apple = 12$\nananas = 7$\na   ",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_click_1() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "'1st row\n[1;2;3] some text\n'3rd row",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        // click after the vector in 2nd row
        app.handle_click(LEFT_GUTTER_WIDTH + 4, 2, &mut holder);
        app.handle_input(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!(
            "'1st row\n[1;2;3] Xsome text\n'3rd row",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_click_2() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "'1st row\nsome text [1;2;3]\n'3rd row",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        // click after the vector in 2nd row
        app.handle_click(LEFT_GUTTER_WIDTH + 4, 2, &mut holder);
        app.handle_input(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!(
            "'1st row\nsomeX text [1;2;3]\n'3rd row",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_click_after_eof() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "'1st row\n[1;2;3] some text\n'3rd row",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_click(LEFT_GUTTER_WIDTH + 40, 2, &mut holder);
        app.handle_input(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!(
            "'1st row\n[1;2;3] some textX\n'3rd row",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_click_after_eof2() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor.insert_text(
            "'1st row\n[1;2;3] some text\n'3rd row",
            &mut app.editor_content,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_click(LEFT_GUTTER_WIDTH + 40, 40, &mut holder);
        app.handle_input(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!(
            "'1st row\n[1;2;3] some text\n'3rd rowX",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_variable() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("apple = 12", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        app.editor.insert_text("apple + 2", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_results(&["12", "14"][..], &result_buffer);
    }

    #[test]
    fn test_variable_must_be_defined() {
        // ez majd akkor fog müködni ha a változókból ki lesz törölve az, ami ujra kell kalkulálva legyen
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("apple = 12", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Home, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
        app.editor.insert_text("apple + 2", &mut app.editor_content);
        app.last_unrendered_modifications =
            UpdateRequirement::all_rows_starting_at(0, UpdateReqType::ReparseTokens);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_results(&["2", "12"][..], &result_buffer);
    }

    #[test]
    fn test_variable_redefine() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("apple = 12", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        app.editor.insert_text("apple + 2", &mut app.editor_content);
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        app.editor.insert_text("apple = 0", &mut app.editor_content);
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none(), &mut holder);
        app.editor.insert_text("apple + 3", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert_results(&["12", "14", "0", "3"][..], &result_buffer);
    }

    #[test]
    fn test_backspace_bug_editor_obj_deletion_for_simple_tokens() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("asd sad asd asd sX", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &mut holder,
        );
        assert_eq!("asd sad asd asd s", app.editor_content.get_content());
    }

    #[test]
    fn test_rendering_while_cursor_move() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("apple = 12$\nasd q", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
    }

    #[test]
    fn stepping_into_a_matrix_renders_it_some_lines_below() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("asdsad\n[1;2;3;4]", &mut app.editor_content);
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 2));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );

        app.handle_input(EditorInputEvent::Down, InputModifiers::none(), &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));

        assert_eq!(holder.editor_objects[0].len(), 1);
        assert_eq!(holder.editor_objects[1].len(), 1);

        assert_eq!(app.render_data.editor_y_to_rendered_height[0], 1);
        assert_eq!(app.render_data.editor_y_to_rendered_height[1], 4);
        assert_eq!(app.render_data.editor_y_to_render_y[0], 0);
        assert_eq!(app.render_data.editor_y_to_render_y[1], 1);

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert!(!app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));

        assert_eq!(holder.editor_objects[0].len(), 1);
        assert_eq!(holder.editor_objects[1].len(), 1);
        assert_eq!(app.render_data.editor_y_to_rendered_height[0], 1);
        assert_eq!(app.render_data.editor_y_to_rendered_height[1], 4);
        assert_eq!(app.render_data.editor_y_to_render_y[0], 0);
        assert_eq!(app.render_data.editor_y_to_render_y[1], 1);
    }

    #[test]
    fn end_matrix_editing_should_rerender_matrix_row_too() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("asdsad\n[1;2;3;4]", &mut app.editor_content);
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 2));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );

        // step into matrix
        app.handle_input(EditorInputEvent::Down, InputModifiers::none(), &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert!(!app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));

        // leave matrix
        app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::ReparseTokens));
    }

    #[test]
    fn clicks_rerender_prev_row_too() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("asdsad\n[1;2;3;4]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );

        app.handle_click(4, 0, &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn all_the_selected_rows_are_rerendered() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("first\nasdsad\n[1;2;3;4]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        assert!(!app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn when_selection_shrinks_upward_rerender_prev_row() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("first\nasdsad\n[1;2;3;4]", &mut app.editor_content);
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        app.handle_input(EditorInputEvent::Down, InputModifiers::shift(), &mut holder);
        app.handle_input(EditorInputEvent::Down, InputModifiers::shift(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );

        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn when_selection_shrinks_downward_rerender_prev_row() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("first\nasdsad\n[1;2;3;4]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Down, InputModifiers::shift(), &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn all_the_selected_rows_are_rerendered_on_ticking() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("first\nasdsad\n[1;2;3;4]", &mut app.editor_content);
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        assert!(!app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));

        app.handle_time(1000);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn all_the_selected_rows_are_rerendered_on_cancellation() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("r1\nr2 asdsad\nr3 [1;2;3;4]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        // cancels selection
        app.handle_input(EditorInputEvent::Left, InputModifiers::none(), &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn navigating_up_renders() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("r1\nr2 asdsad\nr3 [1;2;3;4]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
        assert!(!app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn all_the_selected_rows_are_rerendered_on_cancellation2() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("asdsad\n[1;2;3;4]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift(), &mut holder);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        // cancels selection
        app.handle_click(4, 0, &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn line_ref_movement_render() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("first\nasdsad\n[1;2;3;4]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        assert!(!app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.alt_key_released(&mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn line_ref_movement_render_with_actual_insertion() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("firs 1t\nasdsad\n[1;2;3;4]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        assert!(!app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt(), &mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.alt_key_released(&mut holder);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn dragging_right_gutter_rerenders_everyhting() {
        let (mut app, units, mut holder) = create_app();
        let arena = Arena::new();

        app.editor
            .insert_text("firs 1t\nasdsad\n[1;2;3;4]", &mut app.editor_content);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &mut holder,
        );
        app.handle_click(app.render_data.result_gutter_x, 0, &mut holder);
        assert!(!app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(!app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));

        app.handle_drag(app.render_data.result_gutter_x - 1, 0);
        assert!(app
            .last_unrendered_modifications
            .need(0, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(1, UpdateReqType::RerenderWholeLine));
        assert!(app
            .last_unrendered_modifications
            .need(2, UpdateReqType::RerenderWholeLine));
    }

    #[test]
    fn test_sum_rerender() {
        // rust's shitty borrow checker forces me to do this
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("1\n2\n3\nsum", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("1\n2\n3\nsum", &mut app.editor_content);
            app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("1\n2\n3\nsum", &mut app.editor_content);
            app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
            app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("1\n2\n3\nsum", &mut app.editor_content);
            app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
            app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
            app.handle_input(EditorInputEvent::Down, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("1\n2\n3\nsum", &mut app.editor_content);
            app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
            app.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut holder);
            app.handle_input(EditorInputEvent::Down, InputModifiers::none(), &mut holder);
            app.handle_input(EditorInputEvent::Down, InputModifiers::none(), &mut holder);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
    }

    #[test]
    fn test_sum_rerender_with_ignored_lines() {
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("1\n'2\n3\nsum", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let (mut app, units, _holder) = create_app();
            let arena = Arena::new();
            app.editor
                .insert_text("1\n'2\n3\nsum", &mut app.editor_content);
            app.handle_input(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &mut create_holder(),
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut create_holder(),
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let (mut app, units, _holder) = create_app();
            let arena = Arena::new();
            app.editor
                .insert_text("1\n'2\n3\nsum", &mut app.editor_content);
            app.handle_input(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &mut create_holder(),
            );
            app.handle_input(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &mut create_holder(),
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut create_holder(),
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let (mut app, units, _holder) = create_app();
            let arena = Arena::new();
            app.editor
                .insert_text("1\n'2\n3\nsum", &mut app.editor_content);
            app.handle_input(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &mut create_holder(),
            );
            app.handle_input(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &mut create_holder(),
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut create_holder(),
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let (mut app, units, _holder) = create_app();
            let arena = Arena::new();
            app.editor
                .insert_text("1\n'2\n3\nsum", &mut app.editor_content);
            app.handle_input(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &mut create_holder(),
            );
            app.handle_input(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &mut create_holder(),
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut create_holder(),
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
    }

    #[test]
    fn test_sum_rerender_with_sum_reset() {
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("1\n--2\n3\nsum", &mut app.editor_content);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut holder,
            );
            assert_results(&["1", "3", "3"][..], &result_buffer);
        }
        {
            let (mut app, units, mut holder) = create_app();
            let arena = Arena::new();

            app.editor
                .insert_text("1\n--2\n3\nsum", &mut app.editor_content);
            app.handle_input(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &mut create_holder(),
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &mut create_holder(),
            );
            assert_results(&["1", "3", "3"][..], &result_buffer);
        }
    }
}
