#![feature(ptr_offset_from, const_if_match, const_fn, const_panic, drain_filter)]
#![feature(const_generics)]
#![feature(type_alias_impl_trait)]

use crate::calc::{add_op, evaluate_tokens, CalcResult, EvaluationResult};
use crate::consts::{LINE_NUM_CONSTS, STATIC_LINE_IDS};
use crate::editor::editor::{Editor, EditorInputEvent, InputModifiers, Pos, Selection};
use crate::editor::editor_content::EditorContent;
use crate::matrix::MatrixData;
use crate::renderer::{render_result, render_result_into};
use crate::shunting_yard::ShuntingYard;
use crate::token_parser::{OperatorTokenType, Token, TokenParser, TokenType};
use crate::units::units::Units;
use crate::units::UnitPrefixes;
use bigdecimal::BigDecimal;
use smallvec::SmallVec;
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::ops::Range;

mod calc;
mod functions;
mod matrix;
mod shunting_yard;
mod token_parser;
pub mod units;

pub mod consts;
pub mod editor;
pub mod renderer;

const MAX_EDITOR_WIDTH: usize = 120;
const LEFT_GUTTER_WIDTH: usize = 1 + 2 + 1;
const MAX_LINE_COUNT: usize = 64;
const RIGHT_GUTTER_WIDTH: usize = 3;
const MIN_RESULT_PANEL_WIDTH: usize = 30;

pub enum Click {
    Simple(Pos),
    Drag(Pos),
}

impl Click {
    fn with_pos(&self, pos: Pos) -> Click {
        if matches!(self, Click::Simple(_)) {
            Click::Simple(pos)
        } else {
            Click::Drag(pos)
        }
    }
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
#[derive(Debug)]
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
    const TMP_BUF_LEN_PER_CELL: usize = 32;
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
        let mut row_i = 0;
        let mut col_i = 0;
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
                    col_i += 1;
                    mat_edit.cell_strings.push(str);
                    str = String::with_capacity(8);
                    str = String::with_capacity(8);
                    can_ignore_ws = true;
                }
                ';' => {
                    row_i += 1;
                    col_i = 0;
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
        mut render_y: usize,
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

#[derive(Eq, PartialEq)]
pub enum EditorObjectType {
    Matrix { row_count: usize, col_count: usize },
    LineReference,
}

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

pub struct NoteCalcApp {
    pub client_width: usize,
    pub editor: Editor,
    pub editor_content: EditorContent<LineData>,
    pub prev_cursor_pos: Pos,
    pub matrix_editing: Option<MatrixEditing>,
    pub editor_click: Option<Click>,
    pub editor_objects: Vec<EditorObject>,
    pub line_reference_chooser: Option<usize>,
    pub line_id_generator: usize,
    pub has_result_bitset: u64,
    pub result_gutter_x: usize,
    pub right_gutter_is_dragged: bool,
    // contains variable names separated by 0
    pub autocompletion_src: Vec<char>,
    pub results: Vec<Result<Option<CalcResult>, ()>>,
}

struct GlobalRenderData {
    left_gutter_width: usize,
    right_gutter_width: usize,

    current_editor_width: usize,
    current_result_width: usize,
    editor_y_to_render_y: [usize; MAX_LINE_COUNT],
    editor_y_to_rendered_height: [usize; MAX_LINE_COUNT],
    clicked_editor_pos: Option<Click>,
}

impl GlobalRenderData {
    fn new(
        client_width: usize,
        result_gutter_x: usize,
        left_gutter_width: usize,
        right_gutter_width: usize,
    ) -> GlobalRenderData {
        let mut r = GlobalRenderData {
            left_gutter_width,
            right_gutter_width,
            current_editor_width: 0,
            current_result_width: 0,
            editor_y_to_render_y: [0; MAX_LINE_COUNT],
            editor_y_to_rendered_height: [0; MAX_LINE_COUNT],
            clicked_editor_pos: None,
        };

        r.current_editor_width = result_gutter_x - left_gutter_width;
        r.current_result_width = client_width - result_gutter_x - right_gutter_width;
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
        let mut r = PerLineRenderData {
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

    fn line_render_ended(&mut self) {
        self.editor_pos.row += 1;
        self.render_pos.row += self.rendered_row_height;
    }

    fn set_fix_row_height(&mut self, height: usize) {
        self.rendered_row_height = height;
        self.vert_align_offset = 0;
    }

    fn calc_rendered_row_height(
        &mut self,
        tokens: &[Token],
        result_row_height: usize,
        vars: &[(&[char], CalcResult)],
        mat_edit_height: Option<usize>,
    ) {
        let mut max_height = mat_edit_height.unwrap_or(1);
        for token in tokens {
            let token_height = match token.typ {
                TokenType::Operator(OperatorTokenType::Matrix {
                    row_count,
                    col_count,
                }) => row_count,
                TokenType::LineReference { var_index } => {
                    let (_var_name, result) = &vars[var_index];
                    match &result {
                        CalcResult::Matrix(mat) => mat.row_count,
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

impl NoteCalcApp {
    pub fn new(client_width: usize) -> NoteCalcApp {
        let mut editor_content = EditorContent::new(MAX_EDITOR_WIDTH);
        NoteCalcApp {
            prev_cursor_pos: Pos::from_row_column(0, 0),
            line_reference_chooser: None,
            client_width,
            editor: Editor::new(&mut editor_content),
            editor_content,
            matrix_editing: None,
            editor_click: None,
            editor_objects: Vec::with_capacity(8),
            autocompletion_src: Vec::with_capacity(256),
            line_id_generator: 1,
            has_result_bitset: 0,
            result_gutter_x: NoteCalcApp::calc_result_gutter_x(None, client_width),
            right_gutter_is_dragged: false,
            // TODO: array?
            results: Vec::with_capacity(MAX_LINE_COUNT),
        }
    }

    pub fn set_normalized_content(&mut self, text: &str) {
        self.editor_content.set_content(text);
        self.editor.set_cursor_pos_r_c(0, 0);
        for (i, data) in self.editor_content.data_mut().iter_mut().enumerate() {
            data.line_id = i + 1;
        }
        self.line_id_generator = self.editor_content.line_count() + 1;
    }

    pub fn end_matrix_editing(&mut self, new_cursor_pos: Option<Pos>) {
        let mat_editor = {
            let mut mat_editor = self.matrix_editing.as_mut().unwrap();
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
        self.editor.set_selection_save_col(selection);
        // TODO: máshogy oldd meg, mert ez modositja az undo stacket is
        // és az miért baj, legalább tudom ctrl z-zni a mátrix edition-t
        self.editor.handle_input(
            EditorInputEvent::Del,
            InputModifiers::none(),
            &mut self.editor_content,
        );
        self.editor.handle_input(
            EditorInputEvent::Text(concat),
            InputModifiers::none(),
            &mut self.editor_content,
        );
        self.matrix_editing = None;

        if let Some(new_cursor_pos) = new_cursor_pos {
            self.editor
                .set_selection_save_col(Selection::single(new_cursor_pos));
        }
        self.editor.blink_cursor();
    }

    const SUM_VARIABLE_INDEX: usize = 0;

    pub fn renderr<'a>(
        client_width: usize,
        result_gutter_x: usize,
        editor_objects: &mut Vec<EditorObject>,
        autocompletion_src: &mut Vec<char>,
        results: &mut Vec<Result<Option<CalcResult>, ()>>,
        editor: &mut Editor,
        editor_content: &'a EditorContent<LineData>,
        has_result_bitset: &mut u64,
        editor_click: &mut Option<Click>,
        units: &Units,
        matrix_editing: &mut Option<MatrixEditing>,
        line_reference_chooser: &mut Option<usize>,
        prev_cursor_pos: Pos,
        render_buckets: &mut RenderBuckets<'a>,
        result_buffer: &'a mut [u8],
    ) {
        results.clear();

        let mut gr = GlobalRenderData::new(
            client_width,
            result_gutter_x,
            LEFT_GUTTER_WIDTH,
            RIGHT_GUTTER_WIDTH,
        );

        // result gutter
        render_buckets.set_color(Layer::BehindText, 0xD2D2D2_FF);
        render_buckets.draw_rect(
            Layer::BehindText,
            result_gutter_x,
            0,
            gr.right_gutter_width,
            64,
        );
        // result background
        render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
        render_buckets.draw_rect(
            Layer::BehindText,
            result_gutter_x + gr.right_gutter_width,
            0,
            gr.current_result_width,
            64,
        );

        editor_objects.clear();

        // TODO avoid alloc
        let mut vars: Vec<(&[char], CalcResult)> = Vec::with_capacity(32);
        autocompletion_src.clear();
        vars.push((&['s', 'u', 'm'], CalcResult::zero()));
        let mut sum_is_null = true;

        *has_result_bitset = 0;
        {
            let mut r = PerLineRenderData::new();
            for line in editor_content.lines().take(MAX_LINE_COUNT) {
                r.new_line_started();
                gr.editor_y_to_render_y[r.editor_pos.row] = r.render_pos.row;

                if line.starts_with(&['-', '-']) || line.starts_with(&['\'']) {
                    if line.starts_with(&['-', '-']) {
                        sum_is_null = true;
                    }
                    NoteCalcApp::render_simple_text_line(
                        line,
                        &mut r,
                        &mut gr,
                        render_buckets,
                        editor_click,
                    );
                    NoteCalcApp::highlight_current_line(
                        render_buckets,
                        &r,
                        &gr,
                        &editor,
                        result_gutter_x,
                    );
                    results.push(Ok(None));
                } else {
                    // TODO optimize vec allocations
                    let mut tokens = Vec::with_capacity(128);
                    TokenParser::parse_line(line, &vars, &mut tokens, &units);

                    // TODO: measure is 128 necessary?
                    // and remove allocation
                    let mut shunting_output_stack = Vec::with_capacity(128);
                    ShuntingYard::shunting_yard(&mut tokens, &mut shunting_output_stack);

                    let result = NoteCalcApp::evaluate_tokens(
                        has_result_bitset,
                        &mut vars,
                        r.editor_pos.row,
                        &editor_content,
                        &mut shunting_output_stack,
                        line,
                        &mut sum_is_null,
                        autocompletion_src,
                    );
                    let result_row_height = if let Ok(result) = result {
                        if let Some(result) = result {
                            let result_row_height = match &result.result {
                                CalcResult::Matrix(mat) => mat.row_count,
                                _ => 1,
                            };
                            results.push(Ok(Some(result.result)));
                            result_row_height
                        } else {
                            results.push(Ok(None));
                            1
                        }
                    } else {
                        results.push(Err(()));
                        1
                    };
                    // todo ,merge it with the previous block
                    let editing_mat_height = matrix_editing.as_ref().and_then(|it| {
                        if it.row_index == r.editor_pos.row {
                            Some(it.row_count)
                        } else {
                            None
                        }
                    });
                    r.calc_rendered_row_height(
                        &tokens,
                        result_row_height,
                        &vars,
                        editing_mat_height,
                    );
                    gr.editor_y_to_rendered_height[r.editor_pos.row] = r.rendered_row_height;
                    NoteCalcApp::highlight_current_line(
                        render_buckets,
                        &r,
                        &gr,
                        editor,
                        result_gutter_x,
                    );

                    let need_matrix_renderer = !editor.get_selection().is_range() || {
                        let first = editor.get_selection().get_first();
                        let second = editor.get_selection().get_second();
                        !(first.row..=second.row).contains(&r.editor_pos.row)
                    };
                    // Todo: refactor the parameters into a struct
                    NoteCalcApp::render_tokens(
                        &tokens,
                        &mut r,
                        &mut gr,
                        render_buckets,
                        // TODO &mut code smell
                        editor_objects,
                        editor,
                        matrix_editing,
                        // TODO &mut code smell
                        editor_click,
                        &vars,
                        &units,
                        need_matrix_renderer,
                    );
                    NoteCalcApp::handle_click_after_last_token(editor_click, &r, &mut gr);
                }

                NoteCalcApp::render_wrap_dots(render_buckets, &r, &gr);

                NoteCalcApp::draw_line_ref_chooser(
                    render_buckets,
                    &r,
                    &gr,
                    &line_reference_chooser,
                    result_gutter_x,
                );

                NoteCalcApp::draw_cursor(render_buckets, &r, &gr, &editor, &matrix_editing);

                NoteCalcApp::draw_right_gutter_num_prefixes(
                    render_buckets,
                    result_gutter_x,
                    &editor_content,
                    &r,
                );

                r.line_render_ended();
            }
        }

        for editor_obj in editor_objects.iter() {
            if matches!(editor_obj.typ, EditorObjectType::LineReference) {
                let row_height = gr.editor_y_to_rendered_height[editor_obj.row];
                let vert_align_offset = (row_height - editor_obj.rendered_h) / 2;
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

        if let Some(editor_obj) = NoteCalcApp::is_pos_inside_an_obj(
            editor_objects,
            editor.get_selection().get_cursor_pos(),
        ) {
            match editor_obj.typ {
                EditorObjectType::Matrix {
                    row_count,
                    col_count,
                } => {
                    if matrix_editing.is_none() && !editor.get_selection().is_range() {
                        *matrix_editing = Some(MatrixEditing::new(
                            row_count,
                            col_count,
                            &editor_content.get_line_chars(editor_obj.row)
                                [editor_obj.start_x..editor_obj.end_x],
                            editor_obj.row,
                            editor_obj.start_x,
                            editor_obj.end_x,
                            prev_cursor_pos,
                        ));
                    }
                }
                EditorObjectType::LineReference => {}
            }
        }

        match gr.clicked_editor_pos {
            Some(Click::Simple(pos)) => {
                editor.handle_click(pos.column, pos.row, &editor_content);
                editor.blink_cursor();
            }
            Some(Click::Drag(pos)) => {
                editor.handle_drag(pos.column, pos.row, &editor_content);
                editor.blink_cursor();
            }
            None => {}
        }

        // gutter
        render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
        render_buckets.draw_rect(Layer::BehindText, 0, 0, gr.left_gutter_width, 255);

        // line numbers
        render_buckets.set_color(Layer::BehindText, 0xADADAD_FF);
        for i in 0..editor_content.line_count().min(MAX_LINE_COUNT) {
            let rendered_row_height = gr.editor_y_to_rendered_height[i];
            let vert_align_offset = (rendered_row_height - 1) / 2;
            render_buckets.custom_commands[Layer::BehindText as usize].push(
                OutputMessage::RenderUtf8Text(RenderUtf8TextMsg {
                    text: &(LINE_NUM_CONSTS[i][..]),
                    row: gr.editor_y_to_render_y[i] + vert_align_offset,
                    column: 1,
                }),
            )
        }

        // selected text
        NoteCalcApp::render_selection_and_its_sum(
            &units,
            render_buckets,
            &results,
            &editor,
            &editor_content,
            &gr,
            &vars,
        );

        NoteCalcApp::render_results(
            &units,
            render_buckets,
            &results,
            result_buffer,
            &editor_content,
            &gr,
            result_gutter_x,
        );
    }

    fn render_simple_text_line<'text_ptr>(
        line: &'text_ptr [char],
        r: &mut PerLineRenderData,
        mut gr: &mut GlobalRenderData,
        render_buckets: &mut RenderBuckets<'text_ptr>,
        editor_click: &mut Option<Click>,
    ) {
        r.set_fix_row_height(1);
        gr.editor_y_to_rendered_height[r.editor_pos.row] = 1;

        let text_len = line.len().min(gr.current_editor_width);

        render_buckets.utf8_texts.push(RenderUtf8TextMsg {
            text: &line[0..text_len],
            row: r.render_pos.row,
            column: gr.left_gutter_width,
        });

        NoteCalcApp::handle_click_for_simple_token(editor_click, text_len, &r, &mut gr);

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
        editor_click: &mut Option<Click>,
        vars: &[(&[char], CalcResult)],
        units: &Units,
        need_matrix_renderer: bool,
    ) {
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
                    editor_click,
                );
            } else if let (TokenType::LineReference { var_index }, true) =
                (&token.typ, need_matrix_renderer)
            {
                let (var_name, result) = &vars[*var_index];

                let (rendered_width, rendered_height) =
                    NoteCalcApp::render_single_result(units, render_buckets, result, r, gr);

                let var_name_len = var_name.len();
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
                NoteCalcApp::draw_token(
                    token,
                    r.render_pos.column,
                    r.render_pos.row + r.vert_align_offset,
                    gr.current_editor_width,
                    gr.left_gutter_width,
                    render_buckets,
                );

                NoteCalcApp::handle_click_for_simple_token(editor_click, token.ptr.len(), r, gr);

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

    fn evaluate_tokens<'text_ptr>(
        has_result_bitset: &mut u64,
        vars: &mut Vec<(&'text_ptr [char], CalcResult)>,
        editor_y: usize,
        editor_content: &EditorContent<LineData>,
        shunting_output_stack: &mut Vec<TokenType>,
        line: &'text_ptr [char],
        sum_is_null: &mut bool,
        autocompletion_src: &mut Vec<char>,
    ) -> Result<Option<EvaluationResult>, ()> {
        let result = evaluate_tokens(shunting_output_stack, &vars);
        if let Ok(Some(result)) = &result {
            *has_result_bitset |= 1u64 << editor_y as u64;

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
                if *sum_is_null {
                    vars[NoteCalcApp::SUM_VARIABLE_INDEX].1 = result.result.clone();
                    *sum_is_null = false;
                } else {
                    vars[NoteCalcApp::SUM_VARIABLE_INDEX].1 =
                        add_op(&vars[NoteCalcApp::SUM_VARIABLE_INDEX].1, &result.result)
                            .unwrap_or(CalcResult::zero());
                }
                // variable redeclaration
                if let Some(i) = vars.iter().position(|it| it.0 == var_name) {
                    vars[i].1 = result.result.clone();
                } else {
                    vars.push((var_name, result.result.clone()));
                }
                for ch in var_name {
                    autocompletion_src.push(*ch);
                }
                autocompletion_src.push(0 as char);
            } else if line_data.line_id != 0 {
                let line_id = line_data.line_id;
                {
                    if *sum_is_null {
                        vars[NoteCalcApp::SUM_VARIABLE_INDEX].1 = result.result.clone();
                        *sum_is_null = false;
                    } else {
                        vars[NoteCalcApp::SUM_VARIABLE_INDEX].1 =
                            add_op(&vars[NoteCalcApp::SUM_VARIABLE_INDEX].1, &result.result)
                                .unwrap_or(CalcResult::zero());
                    }
                }
                vars.push((STATIC_LINE_IDS[line_id], result.result.clone()));
            } else {
                // TODO extract the sum addition
                if *sum_is_null {
                    vars[NoteCalcApp::SUM_VARIABLE_INDEX].1 = result.result.clone();
                    *sum_is_null = false;
                } else {
                    vars[NoteCalcApp::SUM_VARIABLE_INDEX].1 =
                        add_op(&vars[NoteCalcApp::SUM_VARIABLE_INDEX].1, &result.result)
                            .unwrap_or(CalcResult::zero());
                }
            }
        };
        result
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
        editor_click: &mut Option<Click>,
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
        NoteCalcApp::handle_click_for_matrix_token(editor_click, new_render_x, r, gr);

        r.token_render_done(text_width, rendered_width, x_diff);
        return end_token_index;
    }

    fn handle_click_for_matrix_token(
        editor_click: &mut Option<Click>,
        new_render_x: usize,
        r: &PerLineRenderData,
        gr: &mut GlobalRenderData,
    ) {
        match *editor_click {
            Some(Click::Simple(clicked_pos)) | Some(Click::Drag(clicked_pos)) => {
                if (r.render_pos.row..r.render_pos.row + r.rendered_row_height)
                    .contains(&clicked_pos.row)
                    && new_render_x >= clicked_pos.column
                {
                    let fixed_pos = Pos::from_row_column(r.editor_pos.row, r.editor_pos.column + 1);
                    gr.clicked_editor_pos =
                        Some(editor_click.as_ref().unwrap().with_pos(fixed_pos));
                    *editor_click = None;
                }
            }
            None => {}
        }
    }

    fn handle_click_for_simple_token(
        editor_click: &mut Option<Click>,
        token_len: usize,
        r: &PerLineRenderData,
        gr: &mut GlobalRenderData,
    ) {
        match editor_click {
            Some(Click::Simple(clicked_pos)) | Some(Click::Drag(clicked_pos)) => {
                let new_x = r.render_pos.column + token_len;
                if r.render_pos.row + r.rendered_row_height > clicked_pos.row
                    && new_x >= clicked_pos.column
                {
                    let click_offset_x = (clicked_pos.column as isize
                        - r.render_pos.column as isize)
                        .max(0) as usize;
                    let fixed_pos = Pos::from_row_column(
                        r.editor_pos.row,
                        r.editor_pos.column + click_offset_x,
                    );

                    gr.clicked_editor_pos =
                        Some(editor_click.as_ref().unwrap().with_pos(fixed_pos));
                    *editor_click = None;
                }
            }
            None => {}
        }
    }

    fn handle_click_after_last_token(
        editor_click: &mut Option<Click>,
        r: &PerLineRenderData,
        gr: &mut GlobalRenderData,
    ) {
        match editor_click {
            Some(Click::Simple(clicked_pos)) | Some(Click::Drag(clicked_pos)) => {
                if r.render_pos.row + r.rendered_row_height > clicked_pos.row {
                    let fixed_pos = Pos::from_row_column(r.editor_pos.row, r.editor_pos.column);
                    gr.clicked_editor_pos =
                        Some(editor_click.as_ref().unwrap().with_pos(fixed_pos));
                    *editor_click = None;
                }
            }
            _ => {}
        }
    }

    fn evaluate_selection<'text_ptr>(
        units: &Units,
        editor: &Editor,
        editor_content: &EditorContent<LineData>,
        vars: &[(&[char], CalcResult)],
        results: &[Result<Option<CalcResult>, ()>],
    ) -> Option<String> {
        let sel = editor.get_selection();
        // TODO optimize vec allocations
        let mut tokens = Vec::with_capacity(128);
        if sel.start.row == sel.end.unwrap().row {
            if let Some(selected_text) = Editor::get_selected_text_single_line(sel, &editor_content)
            {
                if let Ok(Some(result)) =
                    NoteCalcApp::evaluate_text(units, selected_text, vars, &mut tokens)
                {
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
        text: &'text_ptr [char],
        vars: &[(&'text_ptr [char], CalcResult)],
        tokens: &mut Vec<Token<'text_ptr>>,
    ) -> Result<Option<EvaluationResult>, ()> {
        TokenParser::parse_line(text, vars, tokens, &units);
        let mut shunting_output_stack = Vec::with_capacity(4);
        ShuntingYard::shunting_yard(tokens, &mut shunting_output_stack);
        return evaluate_tokens(&mut shunting_output_stack, &vars);
    }

    fn render_matrix_obj<'text_ptr>(
        mut render_x: usize,
        mut render_y: usize,
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

        let mut tokens_per_cell = {
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
        mut render_y: usize,
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

        let mut cells_strs = {
            let mut tokens_per_cell: SmallVec<[String; 32]> = SmallVec::with_capacity(32);

            let mut cell_index = 0;
            for cell in mat.cells.iter() {
                let result_str = render_result(units, cell, &ResultFormat::Dec, false, 4);
                tokens_per_cell.push(result_str);
                cell_index += 1;
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

    fn calc_matrix_max_lengths<'text_ptr>(units: &Units, mat: &MatrixData) -> ResultLengths {
        let mut cells_strs = {
            let mut tokens_per_cell: SmallVec<[String; 32]> = SmallVec::with_capacity(32);

            let mut cell_index = 0;
            for cell in mat.cells.iter() {
                let result_str = render_result(units, cell, &ResultFormat::Dec, false, 4);
                tokens_per_cell.push(result_str);
                cell_index += 1;
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
        result: &CalcResult,
        r: &PerLineRenderData,
        gr: &GlobalRenderData,
    ) -> (usize, usize) {
        return match &result {
            CalcResult::Matrix(mat) => {
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
            _ => {
                // TODO: optimize string alloc
                let result_str = render_result(&units, &result, &ResultFormat::Dec, false, 2);
                let text_len = result_str.chars().count().min(
                    (gr.current_editor_width as isize - r.render_pos.column as isize).max(0)
                        as usize,
                );
                render_buckets.line_ref_results.push(RenderStringMsg {
                    text: result_str[0..text_len].to_owned(),
                    row: r.render_pos.row,
                    column: r.render_pos.column + gr.left_gutter_width,
                });
                (text_len, 1)
            }
        };
    }

    fn render_results<'text_ptr>(
        units: &Units,
        render_buckets: &mut RenderBuckets<'text_ptr>,
        results: &[Result<Option<CalcResult>, ()>],
        result_buffer: &'text_ptr mut [u8],
        editor_content: &EditorContent<LineData>,
        gr: &GlobalRenderData,
        result_gutter_x: usize,
    ) {
        let mut result_buffer_index = 0;
        let mut result_ranges: SmallVec<[Option<Range<usize>>; MAX_LINE_COUNT]> =
            SmallVec::with_capacity(MAX_LINE_COUNT);

        let mut max_lengths = ResultLengths {
            int_part_len: 0,
            frac_part_len: 0,
            unit_part_len: 0,
        };
        let mut prev_result_matrix_length = None;
        // calc max length and render results into buffer
        for (editor_y, result) in results.iter().enumerate() {
            if editor_y >= editor_content.line_count() {
                // asdd
                // TODO, ez minek kell? miért van több result?
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
                        NoteCalcApp::render_matrix_result(
                            units,
                            result_gutter_x + gr.right_gutter_width,
                            gr.editor_y_to_render_y[editor_y],
                            mat,
                            render_buckets,
                            prev_result_matrix_length.as_ref(),
                            gr.editor_y_to_rendered_height[editor_y],
                        );
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
                        let s =
                            unsafe { std::str::from_utf8_unchecked(&result_buffer[range.clone()]) };
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

        // render results from the buffer
        for (editor_y, result_range) in result_ranges.iter().enumerate() {
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

    fn calc_consecutive_matrices_max_lengths<'text_ptr>(
        units: &Units,
        results: &[Result<Option<CalcResult>, ()>],
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
            TokenType::Operator(op_type) => match op_type {
                OperatorTokenType::Unit(_) => &mut render_buckets.units,
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
        &'b self,
        units: &'b Units,
        render_buckets: &'b mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) -> String {
        let sel = self.editor.get_selection();
        let first_row = sel.get_first().row;
        let second_row = sel.get_second().row;
        let row_nums = second_row - first_row + 1;

        let mut vars: Vec<(&[char], CalcResult)> = Vec::with_capacity(32);
        vars.push((&['s', 'u', 'm'], CalcResult::zero()));
        let mut tokens = Vec::with_capacity(128);

        let mut gr = GlobalRenderData::new(self.client_width, self.result_gutter_x, 0, 2);
        // evaluate all the lines so variables are defined even if they are not selected
        let mut render_height = 0;
        {
            let mut r = PerLineRenderData::new();
            for (i, line) in self.editor_content.lines().enumerate() {
                // TODO "--"
                tokens.clear();
                TokenParser::parse_line(line, &vars, &mut tokens, &units);

                let mut shunting_output_stack = Vec::with_capacity(32);
                ShuntingYard::shunting_yard(&mut tokens, &mut shunting_output_stack);

                // asdd
                let result_row_height = 1;

                if i >= first_row && i <= second_row {
                    r.new_line_started();
                    gr.editor_y_to_render_y[r.editor_pos.row] = r.render_pos.row;
                    r.calc_rendered_row_height(&tokens, result_row_height, &vars, None);
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
                        &mut None,
                        &vars,
                        &units,
                        true, // force matrix rendering
                    );
                    r.line_render_ended();
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
            &self.results[first_row..=second_row],
            result_buffer,
            &self.editor_content,
            &gr,
            result_gutter_x,
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
                OutputMessage::SetStyle(style) => {}
                OutputMessage::SetColor(color) => {}
                OutputMessage::RenderRectangle { x, y, w, h } => {}
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
        results: &[Result<Option<CalcResult>, ()>],
        editor: &Editor,
        editor_content: &EditorContent<LineData>,
        r: &GlobalRenderData,
        vars: &[(&[char], CalcResult)],
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
            if let Some(mut partial_result) =
                NoteCalcApp::evaluate_selection(&units, editor, editor_content, &vars, &results)
            {
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

    pub fn handle_mouse_up(&mut self, x: usize, y: usize) {
        self.right_gutter_is_dragged = false;
    }

    pub fn handle_click(&mut self, x: usize, y: usize) {
        if x < LEFT_GUTTER_WIDTH {
            // clicked on left gutter
        } else if x < self.result_gutter_x {
            self.editor_click = Some(Click::Simple(Pos::from_row_column(
                y,
                x - LEFT_GUTTER_WIDTH,
            )));
            if self.matrix_editing.is_some() {
                self.end_matrix_editing(None);
            }
        } else if x - self.result_gutter_x < RIGHT_GUTTER_WIDTH {
            // clicked on right gutter
            self.right_gutter_is_dragged = true;
        }
    }

    pub fn handle_drag(&mut self, x: usize, y: usize) {
        if self.right_gutter_is_dragged {
            self.result_gutter_x = NoteCalcApp::calc_result_gutter_x(Some(x), self.client_width);
        } else if x < LEFT_GUTTER_WIDTH {
            // clicked on left gutter
        } else if x - LEFT_GUTTER_WIDTH < MAX_EDITOR_WIDTH {
            self.editor_click = Some(Click::Drag(Pos::from_row_column(y, x - LEFT_GUTTER_WIDTH)));
        }
    }

    pub fn handle_resize(&mut self, new_client_width: usize) {
        self.client_width = new_client_width;
        self.result_gutter_x =
            NoteCalcApp::calc_result_gutter_x(Some(self.result_gutter_x), new_client_width);
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
        return if let Some(mat_editor) = &mut self.matrix_editing {
            mat_editor.editor.handle_tick(now)
        } else {
            self.editor.handle_tick(now)
        };
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

    pub fn alt_key_released(&mut self) {
        if self.line_reference_chooser.is_none() {
            return;
        }
        let cur_row = self.editor.get_selection().get_cursor_pos().row;
        let row_index = self.line_reference_chooser.unwrap();
        self.line_reference_chooser = None;
        if cur_row == row_index || (self.has_result_bitset & (1u64 << row_index as u64)) == 0 {
            return;
        }
        let line_id = {
            let line_data = self.editor_content.mut_data(row_index);
            if line_data.line_id == 0 {
                line_data.line_id = self.line_id_generator;
                self.line_id_generator += 1;
            }
            line_data.line_id
        };
        let inserting_text = format!("&[{}]", line_id);
        self.editor.handle_input(
            EditorInputEvent::Text(inserting_text),
            InputModifiers::none(),
            &mut self.editor_content,
        );
    }

    pub fn handle_input(&mut self, input: EditorInputEvent, modifiers: InputModifiers) -> bool {
        if self.matrix_editing.is_none() && modifiers.alt {
            if input == EditorInputEvent::Left {
                for row_i in self.editor.get_selection().get_row_iter_incl() {
                    let new_format = match &self.editor_content.get_data(row_i).result_format {
                        ResultFormat::Bin => ResultFormat::Hex,
                        ResultFormat::Dec => ResultFormat::Bin,
                        ResultFormat::Hex => ResultFormat::Dec,
                    };
                    self.editor_content.mut_data(row_i).result_format = new_format;
                }
            } else if input == EditorInputEvent::Right {
                for row_i in self.editor.get_selection().get_row_iter_incl() {
                    let new_format = match &self.editor_content.get_data(row_i).result_format {
                        ResultFormat::Bin => ResultFormat::Dec,
                        ResultFormat::Dec => ResultFormat::Hex,
                        ResultFormat::Hex => ResultFormat::Bin,
                    };
                    self.editor_content.mut_data(row_i).result_format = new_format;
                }
            } else if input == EditorInputEvent::Up {
                let cur_pos = self.editor.get_selection().get_cursor_pos();
                self.line_reference_chooser =
                    if let Some(selector_row) = self.line_reference_chooser {
                        if selector_row > 0 {
                            Some(selector_row - 1)
                        } else {
                            Some(selector_row)
                        }
                    } else if cur_pos.row > 0 {
                        Some(cur_pos.row - 1)
                    } else {
                        None
                    }
            } else if input == EditorInputEvent::Down {
                let cur_pos = self.editor.get_selection().get_cursor_pos();
                self.line_reference_chooser =
                    if let Some(selector_row) = self.line_reference_chooser {
                        if selector_row < cur_pos.row - 1 {
                            Some(selector_row + 1)
                        } else {
                            Some(selector_row)
                        }
                    } else {
                        None
                    }
            }
            false
        } else if self.matrix_editing.is_some() {
            self.handle_matrix_editor_input(input, modifiers);
            true
        } else {
            let cursor_pos = self.editor.get_selection();
            if input == EditorInputEvent::Tab && cursor_pos.get_cursor_pos().column > 0 {
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
                    self.editor.set_selection_save_col(Selection::single(
                        cursor_pos.with_column(prev_col),
                    ));
                    return true;
                } else {
                    // check for autocompletion
                    // find space
                    let (begin_index, expected_len) = {
                        let mut begin_index = cursor_pos.column - 1;
                        let mut len = 1;
                        while begin_index > 0
                            && (line[begin_index - 1].is_alphanumeric()
                                || line[begin_index - 1] == '_')
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
                    let autocompletion_entries = &self.autocompletion_src;
                    'main: while autocomp_src_i < autocompletion_entries.len()
                        && autocompletion_entries[autocomp_src_i] != 0 as char
                    {
                        let start_autocomp_src_i = autocomp_src_i;
                        while autocomp_src_i < autocompletion_entries.len()
                            && autocompletion_entries[autocomp_src_i] != 0 as char
                            && autocompletion_entries[autocomp_src_i] == line[word_i]
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
                        while autocompletion_entries[autocomp_src_i] != 0 as char {
                            autocomp_src_i += 1;
                        }
                        // skip 0
                        autocomp_src_i += 1;
                        word_i = begin_index;
                    }
                    if let Some(match_begin_index) = match_begin_index {
                        let mut i = match_begin_index + expected_len;
                        while i < autocompletion_entries.len()
                            && autocompletion_entries[i] != 0 as char
                        {
                            self.editor.handle_input(
                                EditorInputEvent::Char(autocompletion_entries[i]),
                                InputModifiers::none(),
                                &mut self.editor_content,
                            );
                            i += 1;
                        }
                        return true;
                    }
                }
            }

            self.prev_cursor_pos = cursor_pos.get_cursor_pos();
            if input == EditorInputEvent::Backspace
                && !cursor_pos.is_range()
                && cursor_pos.start.column > 0
            {
                if let Some(index) =
                    self.index_of_editor_object_at(cursor_pos.get_cursor_pos().with_prev_col())
                {
                    // remove it
                    let obj = self.editor_objects.remove(index);
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
                    return true;
                }
            } else if input == EditorInputEvent::Del && !cursor_pos.is_range() {
                if let Some(index) = self.index_of_editor_object_at(cursor_pos.get_cursor_pos()) {
                    // remove it
                    let obj = self.editor_objects.remove(index);
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
                    return true;
                }
            } else if input == EditorInputEvent::Left
                && !cursor_pos.is_range()
                && cursor_pos.start.column > 0
                && modifiers.shift == false
            {
                if let Some(obj) =
                    self.find_editor_object_at(cursor_pos.get_cursor_pos().with_prev_col())
                {
                    if obj.typ == EditorObjectType::LineReference {
                        //  jump over it
                        self.editor.set_cursor_pos_r_c(obj.row, obj.start_x);
                        return false;
                    }
                }
            } else if input == EditorInputEvent::Right
                && !cursor_pos.is_range()
                && modifiers.shift == false
            {
                if let Some(obj) = self.find_editor_object_at(cursor_pos.get_cursor_pos()) {
                    if obj.typ == EditorObjectType::LineReference {
                        //  jump over it
                        self.editor.set_cursor_pos_r_c(obj.row, obj.end_x);
                        return false;
                    }
                }
            }

            let modif_type = self
                .editor
                .handle_input(input, modifiers, &mut self.editor_content);

            return modif_type.is_some();
        }
    }

    fn find_editor_object_at(&self, pos: Pos) -> Option<&EditorObject> {
        for obj in &self.editor_objects {
            if obj.row == pos.row && (obj.start_x..obj.end_x).contains(&pos.column) {
                return Some(obj);
            }
        }
        return None;
    }

    fn is_pos_inside_an_obj(editor_objects: &[EditorObject], pos: Pos) -> Option<&EditorObject> {
        for obj in editor_objects {
            if obj.row == pos.row && (obj.start_x + 1..obj.end_x).contains(&pos.column) {
                return Some(obj);
            }
        }
        return None;
    }

    fn index_of_editor_object_at(&self, pos: Pos) -> Option<usize> {
        return self
            .editor_objects
            .iter()
            .position(|obj| obj.row == pos.row && (obj.start_x..obj.end_x).contains(&pos.column));
    }

    fn handle_matrix_editor_input(&mut self, input: EditorInputEvent, modifiers: InputModifiers) {
        let mat_edit = self.matrix_editing.as_mut().unwrap();
        let cur_pos = self.editor.get_selection().get_cursor_pos();

        let simple = !modifiers.shift && !modifiers.alt;
        let alt = modifiers.alt;
        if input == EditorInputEvent::Esc || input == EditorInputEvent::Enter {
            self.end_matrix_editing(None);
        } else if input == EditorInputEvent::Tab {
            if mat_edit.current_cell.column + 1 < mat_edit.col_count {
                mat_edit.change_cell(mat_edit.current_cell.with_next_col());
            } else if mat_edit.current_cell.row + 1 < mat_edit.row_count {
                mat_edit.change_cell(mat_edit.current_cell.with_next_row().with_column(0));
            } else {
                let end_text_index = mat_edit.end_text_index;
                self.end_matrix_editing(Some(cur_pos.with_column(end_text_index)));
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
                self.end_matrix_editing(Some(cur_pos.with_column(start_text_index)));
            }
        } else if simple
            && input == EditorInputEvent::Right
            && mat_edit.editor.is_cursor_at_eol(&mat_edit.editor_content)
        {
            if mat_edit.current_cell.column + 1 < mat_edit.col_count {
                mat_edit.change_cell(mat_edit.current_cell.with_next_col());
            } else {
                let end_text_index = mat_edit.end_text_index;
                self.end_matrix_editing(Some(cur_pos.with_column(end_text_index)));
            }
        } else if simple && input == EditorInputEvent::Up {
            if mat_edit.current_cell.row > 0 {
                mat_edit.change_cell(mat_edit.current_cell.with_prev_row());
            } else {
                self.end_matrix_editing(None);
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else if simple && input == EditorInputEvent::Down {
            if mat_edit.current_cell.row + 1 < mat_edit.row_count {
                mat_edit.change_cell(mat_edit.current_cell.with_next_row());
            } else {
                self.end_matrix_editing(None);
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else if simple && input == EditorInputEvent::End {
            if mat_edit.current_cell.column != mat_edit.col_count - 1 {
                mat_edit.change_cell(mat_edit.current_cell.with_column(mat_edit.col_count - 1));
            } else {
                let end_text_index = mat_edit.end_text_index;
                self.end_matrix_editing(Some(cur_pos.with_column(end_text_index)));
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else if simple && input == EditorInputEvent::Home {
            if mat_edit.current_cell.column != 0 {
                mat_edit.change_cell(mat_edit.current_cell.with_column(0));
            } else {
                let start_index = mat_edit.start_text_index;
                self.end_matrix_editing(Some(cur_pos.with_column(start_index)));
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
        &'a mut self,
        units: &Units,
        render_buckets: &'b mut RenderBuckets<'a>,
        result_buffer: &'a mut [u8],
    ) {
        NoteCalcApp::renderr(
            self.client_width,
            self.result_gutter_x,
            &mut self.editor_objects,
            &mut self.autocompletion_src,
            &mut self.results,
            &mut self.editor,
            &self.editor_content,
            &mut self.has_result_bitset,
            &mut self.editor_click,
            units,
            &mut self.matrix_editing,
            &mut self.line_reference_chooser,
            self.prev_cursor_pos,
            render_buckets,
            result_buffer,
        );
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

    fn create_app() -> (NoteCalcApp, Units) {
        let app = NoteCalcApp::new(120);
        let units = Units::new();
        return (app, units);
    }

    #[test]
    fn bug1() {
        let (mut app, units) = create_app();

        app.handle_input(
            EditorInputEvent::Text("[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 33));
        app.handle_input(EditorInputEvent::Right, InputModifiers::alt());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
    }

    #[test]
    fn bug2() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 1));
        app.handle_input(EditorInputEvent::Right, InputModifiers::alt());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Down, InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
    }

    #[test]
    fn bug3() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text(
                "1\n\
                2+"
                .to_owned(),
            ),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 2));
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
        app.alt_key_released();
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
    }

    #[test]
    fn it_is_not_allowed_to_ref_lines_below() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text(
                "1\n\
                2+\n3\n4"
                    .to_owned(),
            ),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 2));
        app.handle_input(EditorInputEvent::Down, InputModifiers::alt());
        app.alt_key_released();
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        assert_eq!(
            "1\n\
                2+\n3\n4",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn it_is_not_allowed_to_ref_lines_below2() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text(
                "1\n\
                2+\n3\n4"
                    .to_owned(),
            ),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 2));
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
        app.handle_input(EditorInputEvent::Down, InputModifiers::alt());
        app.alt_key_released();
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        assert_eq!(
            "1\n\
                2+&[1]\n3\n4",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn remove_matrix_backspace() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Backspace, InputModifiers::none());
        assert_eq!("abcd ", app.editor_content.get_content());
    }

    #[test]
    fn matrix_step_in_dir() {
        // from right
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('1'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("abcd [1,2,1;4,5,6]", app.editor_content.get_content());
        }
        // from left
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("abcd [9,2,3;4,5,6]", app.editor_content.get_content());
        }
        // from below
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]\naaaaaaaaaaaaaaaaaa".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 7));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Up, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!(
                "abcd [1,2,3;9,5,6]\naaaaaaaaaaaaaaaaaa",
                app.editor_content.get_content()
            );
        }
        // from above
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("aaaaaaaaaaaaaaaaaa\nabcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 7));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Down, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!(
                "aaaaaaaaaaaaaaaaaa\nabcd [9,2,3;4,5,6]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn cursor_is_put_after_the_matrix_after_finished_editing() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Left, InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Char('6'), InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
        assert_eq!(app.editor_content.get_content(), "abcd [1,2,6;4,5,6]9");
    }

    #[test]
    fn remove_matrix_del() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 5));
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Del, InputModifiers::none());
        assert_eq!("abcd ", app.editor_content.get_content());
    }

    #[test]
    fn test_moving_inside_a_matrix() {
        // right to left, cursor at end
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!("abcd [1,9,3;4,5,6]", app.editor_content.get_content());
        }
        // left to right, cursor at start
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!("abcd [1,2,9;4,5,6]", app.editor_content.get_content());
        }
        // vertical movement down, cursor tries to keep its position
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            // inside the matrix
            app.handle_input(EditorInputEvent::Down, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!(
                "abcd [1111,22,3;9,55555,666]",
                app.editor_content.get_content()
            );
        }

        // vertical movement up, cursor tries to keep its position
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            // inside the matrix
            app.handle_input(EditorInputEvent::Down, InputModifiers::none());
            app.handle_input(EditorInputEvent::Up, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!(
                "abcd [9,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn test_moving_inside_a_matrix_with_tab() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("[1,2,3;4,5,6]".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 5));
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Right, InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('7'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('8'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('0'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        assert_eq!("[1,2,7;8,9,0]9", app.editor_content.get_content());
    }

    #[test]
    fn test_leaving_a_matrix_with_tab() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("[1,2,3;4,5,6]".to_owned()),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Left, InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        // the next tab should leave the matrix
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('7'), InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        assert_eq!("[1,2,3;4,5,6]7", app.editor_content.get_content());
    }

    #[test]
    fn end_btn_matrix() {
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666] qq".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            // inside the matrix
            app.handle_input(EditorInputEvent::End, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!(
                "abcd [1111,22,9;44,55555,666] qq",
                app.editor_content.get_content()
            );
        }
        // pressing twice, exits the matrix
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666] qq".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            // inside the matrix
            app.handle_input(EditorInputEvent::End, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::End, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!(
                "abcd [1111,22,3;44,55555,666] qq9",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn home_btn_matrix() {
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            // inside the matrix
            app.handle_input(EditorInputEvent::Home, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!(
                "abcd [9,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            // inside the matrix
            app.handle_input(EditorInputEvent::Home, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Home, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Char('6'), InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!(
                "6abcd [1111,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn bug8() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("16892313\n14 * ".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 5));
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
        app.alt_key_released();
        assert_eq!("16892313\n14 * &[1]", app.editor_content.get_content());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_time(1000);
        app.handle_input(EditorInputEvent::Backspace, InputModifiers::none());
        assert_eq!("16892313\n14 * ", app.editor_content.get_content());

        app.handle_input(EditorInputEvent::Char('z'), InputModifiers::ctrl());
        assert_eq!("16892313\n14 * &[1]", app.editor_content.get_content());

        app.handle_input(EditorInputEvent::Right, InputModifiers::none()); // end selection
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Left, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('a'), InputModifiers::none());
        assert_eq!("16892313\n14 * a&[1]", app.editor_content.get_content());

        app.handle_input(EditorInputEvent::Char(' '), InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Right, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('b'), InputModifiers::none());
        assert_eq!("16892313\n14 * a &[1]b", app.editor_content.get_content());

        app.handle_input(EditorInputEvent::Left, InputModifiers::none());
        app.handle_input(EditorInputEvent::Left, InputModifiers::none());
        app.handle_input(EditorInputEvent::Left, InputModifiers::none());
        app.handle_input(EditorInputEvent::Right, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('c'), InputModifiers::none());
        assert_eq!("16892313\n14 * a c&[1]b", app.editor_content.get_content());
    }

    #[test]
    fn test_line_ref_normalization() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(12, 2));
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
        app.alt_key_released();
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
        app.alt_key_released();
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
        app.alt_key_released();
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
        let (mut app, units) = create_app();
        app.set_normalized_content("1111\n2222\n14 * &[2]&[2]&[2]\n");
        assert_eq!(1, app.editor_content.get_data(0).line_id);
        assert_eq!(2, app.editor_content.get_data(1).line_id);
        assert_eq!(3, app.editor_content.get_data(2).line_id);
    }

    #[test]
    fn no_memory_deallocation_bug_in_line_selection() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(12, 2));
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
    }

    #[test]
    fn matrix_deletion() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text(" [1,2,3]".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Del, InputModifiers::none());
        assert_eq!("[1,2,3]", app.editor_content.get_content());
    }

    #[test]
    fn matrix_insertion_bug() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("[1,2,3]".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Char('a'), InputModifiers::none());
        assert_eq!("a[1,2,3]", app.editor_content.get_content());
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("a\n[1,2,3]", app.editor_content.get_content());
    }

    #[test]
    fn matrix_insertion_bug2() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("'[X] nth, sum fv".to_owned()),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        app.handle_input(EditorInputEvent::Del, InputModifiers::none());
        let mut result_buffer = [0; 128];
        app.render(&units, &mut RenderBuckets::new(), &mut result_buffer);
        assert_results(app, &["0"][..], &result_buffer);
    }

    fn assert_results(app: NoteCalcApp, expected_results: &[&str], result_buffer: &[u8]) {
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
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text(
                "3m * 2m
--
1
2
sum"
                .to_owned(),
            ),
            InputModifiers::none(),
        );
        let mut result_buffer = [0; 128];
        app.render(&units, &mut RenderBuckets::new(), &mut result_buffer);
        assert_results(app, &["6 m^2", "", "1", "2", "3"][..], &result_buffer);
    }

    #[test]
    fn no_sum_value_in_case_of_error() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text(
                "3m * 2m\n\
                4\n\
                sum"
                .to_owned(),
            ),
            InputModifiers::none(),
        );
        let mut result_buffer = [0; 128];
        app.render(&units, &mut RenderBuckets::new(), &mut result_buffer);
        assert_results(app, &["6 m^2", "4", "0"][..], &result_buffer);
    }

    #[test]
    fn test_ctrl_c() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("aaaaaaaaa".to_owned()),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Left, InputModifiers::shift());
        app.handle_input(EditorInputEvent::Left, InputModifiers::shift());
        app.handle_input(EditorInputEvent::Left, InputModifiers::shift());
        app.handle_input(EditorInputEvent::Char('c'), InputModifiers::ctrl());
        assert_eq!("aaa", &app.editor.clipboard);
    }

    #[test]
    fn test_changing_output_style_for_selected_rows() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text(
                "2\n\
                    4\n\
                    5"
                .to_owned(),
            ),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift());
        app.handle_input(EditorInputEvent::Up, InputModifiers::shift());
        app.handle_input(EditorInputEvent::Left, InputModifiers::alt());
        let mut result_buffer = [0; 128];
        app.render(&units, &mut RenderBuckets::new(), &mut result_buffer);
        assert_results(app, &["10", "100", "101"][..], &result_buffer);
    }

    #[test]
    fn test_matrix_sum() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("[1,2,3]\nsum".to_owned()),
            InputModifiers::none(),
        );
        let mut result_buffer = [0; 128];
        app.render(&units, &mut RenderBuckets::new(), &mut result_buffer);
        // both the first line and the 'sum' line renders a matrix, which leaves the result buffer empty
        assert_results(app, &["\u{0}"][..], &result_buffer);
    }

    #[test]
    fn test_rich_copy() {
        fn t(content: &str, expected: &str, selected_range: RangeInclusive<usize>) {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text(content.to_owned()),
                InputModifiers::none(),
            );
            app.editor.set_selection_save_col(Selection::range(
                Pos::from_row_column(*selected_range.start(), 0),
                Pos::from_row_column(*selected_range.end(), 0),
            ));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!(
                expected,
                &app.copy_selected_rows_with_result_to_clipboard(
                    &units,
                    &mut RenderBuckets::new(),
                    &mut [0; 128]
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
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("16892313\n14 * ".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 5));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
            app.alt_key_released();
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::shift());
            app.handle_input(EditorInputEvent::Backspace, InputModifiers::none());
            assert_eq!("16892313\n14 * &[1", app.editor_content.get_content());
        }
        // right
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("16892313\n14 * ".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 5));
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
            app.alt_key_released();
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.handle_input(EditorInputEvent::Right, InputModifiers::shift());
            app.handle_input(EditorInputEvent::Del, InputModifiers::none());
            assert_eq!("16892313\n14 * [1]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_pressing_tab_on_m_char() {
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("m".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!("[0]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("am".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!("am  ", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("a m".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            assert_eq!("a [0]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_that_cursor_is_inside_matrix_on_creation() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("m".to_owned()),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Char('1'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1]", app.editor_content.get_content());
    }

    #[test]
    fn test_matrix_alt_plus_right() {
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1,0]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1,0,0,0]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1;2]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Right, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1,0;2,0]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_matrix_alt_plus_left() {
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1, 2, 3]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1,2]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1, 2, 3]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1, 2, 3; 4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1,2;4,5]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_matrix_alt_plus_down() {
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1;0]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1;0;0;0]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1,2]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Down, InputModifiers::alt());
            // this render is important, it tests a bug!
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1,2;0,0]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_matrix_alt_plus_up() {
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1; 2; 3]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1;2]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1; 2; 3]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units) = create_app();
            app.handle_input(
                EditorInputEvent::Text("[1, 2, 3; 4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
            app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("[1,2,3]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_autocompletion_single() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("apple = 12$".to_owned()),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('a'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        assert_eq!("apple = 12$\napple", app.editor_content.get_content());
    }

    #[test]
    fn test_autocompletion_two_sars() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("apple = 12$\nbanana = 7$\n".to_owned()),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Char('a'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        assert_eq!(
            "apple = 12$\nbanana = 7$\napple",
            app.editor_content.get_content()
        );

        app.handle_input(EditorInputEvent::Char(' '), InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('b'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        assert_eq!(
            "apple = 12$\nbanana = 7$\napple banana",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_that_no_autocompletion_for_multiple_results() {
        let (mut app, units) = create_app();
        app.handle_input(
            EditorInputEvent::Text("apple = 12$\nananas = 7$\n".to_owned()),
            InputModifiers::none(),
        );
        app.render(&units, &mut RenderBuckets::new(), &mut [0; 128]);
        app.handle_input(EditorInputEvent::Char('a'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Tab, InputModifiers::none());
        assert_eq!(
            "apple = 12$\nananas = 7$\na   ",
            app.editor_content.get_content()
        );
    }
}
