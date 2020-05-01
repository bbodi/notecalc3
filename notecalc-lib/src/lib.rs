#![feature(ptr_offset_from, const_if_match, const_fn, const_panic, drain_filter)]
#![feature(const_generics)]
#![feature(type_alias_impl_trait)]

use crate::calc::{add_op, evaluate_tokens, CalcResult, EvaluationResult};
use crate::consts::{LINE_NUM_CONSTS, STATIC_LINE_IDS};
use crate::editor::editor::{Editor, EditorInputEvent, InputModifiers, Pos, Selection};
use crate::editor::editor_content::EditorContent;
use crate::renderer::render_result;
use crate::shunting_yard::ShuntingYard;
use crate::token_parser::{OperatorTokenType, Token, TokenParser, TokenType};
use crate::units::consts::{create_prefixes, init_units};
use crate::units::units::Units;
use crate::units::UnitPrefixes;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::panic::resume_unwind;

mod calc;
mod matrix;
mod shunting_yard;
mod token_parser;
mod units;

pub mod consts;
pub mod editor;
pub mod renderer;

const MAX_EDITOR_WIDTH: usize = 120;
const LEFT_GUTTER_WIDTH: usize = 1 + 3 + 1;

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
pub struct RenderTextMsg<'a> {
    pub text: &'a [char],
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
    RenderText(RenderTextMsg<'a>),
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
    pub texts: Vec<RenderTextMsg<'a>>,
    pub numbers: Vec<RenderTextMsg<'a>>,
    pub units: Vec<RenderTextMsg<'a>>,
    pub operators: Vec<RenderTextMsg<'a>>,
    pub variable: Vec<RenderTextMsg<'a>>,
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
            texts: Vec::with_capacity(128),
            custom_commands: [Vec::with_capacity(128), Vec::with_capacity(128)],
            numbers: Vec::with_capacity(32),
            units: Vec::with_capacity(32),
            operators: Vec::with_capacity(32),
            variable: Vec::with_capacity(32),
            line_ref_results: Vec::with_capacity(32),
        }
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
        self.custom_commands[layer as usize].push(OutputMessage::RenderText(RenderTextMsg {
            text,
            row: y,
            column: x,
        }));
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
            cell_strings: Vec::with_capacity(row_count * col_count),
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
        if step_in_pos.row == row_index && step_in_pos.column > start_text_index {
            // from right
            mat_edit
                .editor
                .set_cursor_pos_r_c(0, mat_edit.editor_content.line_len(0));
        } else {
            mat_edit.editor.set_cursor_pos_r_c(0, 0);
        }

        mat_edit
    }

    fn change_cell(&mut self, new_pos: Pos) {
        self.save_editor_content();

        let new_content = &self.cell_strings[new_pos.row * self.col_count + new_pos.column];
        self.editor_content.set_content(new_content);

        self.current_cell = new_pos;
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
        render_buckets: &mut RenderBuckets<'b>,
        rendered_row_height: usize,
    ) -> usize {
        let vert_align_offset = (rendered_row_height - self.row_count) / 2;

        render_buckets.operators.push(RenderTextMsg {
            text: &['⎡'],
            row: render_y + vert_align_offset,
            column: render_x + LEFT_GUTTER_WIDTH,
        });
        for i in 1..self.row_count - 1 {
            render_buckets.operators.push(RenderTextMsg {
                text: &['⎢'],
                row: render_y + i + vert_align_offset,
                column: render_x + LEFT_GUTTER_WIDTH,
            });
        }
        render_buckets.operators.push(RenderTextMsg {
            text: &['⎣'],
            row: render_y + self.row_count - 1 + vert_align_offset,
            column: render_x + LEFT_GUTTER_WIDTH,
        });
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
                    render_buckets.set_color(Layer::AboveText, 0xBBBBBB_55);
                    render_buckets.draw_rect(
                        Layer::AboveText,
                        render_x + padding_x + LEFT_GUTTER_WIDTH,
                        render_y + row_i + vert_align_offset,
                        text_len,
                        1,
                    );
                    let chars = &self.editor_content.lines().next().unwrap();
                    render_buckets.set_color(Layer::AboveText, 0x000000_FF);
                    for (i, char) in chars.iter().enumerate() {
                        render_buckets.draw_char(
                            Layer::AboveText,
                            render_x + padding_x + LEFT_GUTTER_WIDTH + i,
                            render_y + row_i + vert_align_offset,
                            *char,
                        );
                    }
                } else {
                    let chars = &self.cell_strings[row_i * self.col_count + col_i];
                    render_buckets.set_color(Layer::AboveText, 0x000000_FF);
                    render_buckets.draw_string(
                        Layer::AboveText,
                        render_x + padding_x + LEFT_GUTTER_WIDTH,
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
                                + LEFT_GUTTER_WIDTH)
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

        render_buckets.operators.push(RenderTextMsg {
            text: &['⎤'],
            row: render_y + vert_align_offset,
            column: render_x + LEFT_GUTTER_WIDTH,
        });
        for i in 1..self.row_count - 1 {
            render_buckets.operators.push(RenderTextMsg {
                text: &['⎥'],
                row: render_y + i + vert_align_offset,
                column: render_x + LEFT_GUTTER_WIDTH,
            });
        }
        render_buckets.operators.push(RenderTextMsg {
            text: &['⎦'],
            row: render_y + self.row_count - 1 + vert_align_offset,
            column: render_x + LEFT_GUTTER_WIDTH,
        });
        render_x += 1;

        render_x
    }
}

#[derive(Eq, PartialEq)]
enum EditorObjectType {
    Matrix { row_count: usize, col_count: usize },
    LineReference,
}
struct EditorObject {
    typ: EditorObjectType,
    row: usize,
    start_x: usize,
    end_x: usize,
}

pub struct NoteCalcApp<'a> {
    client_width: usize,
    units: Units<'a>,
    pub editor: Editor,
    pub editor_content: EditorContent<LineData>,
    prefixes: &'static UnitPrefixes,
    result_buffer: [char; 1024],
    matrix_editing: Option<MatrixEditing>,
    editor_click: Option<Click>,
    editor_objects: Vec<EditorObject>,
    line_selector: Option<usize>,
    line_id_generator: usize,
    has_result_bitset: u64,
}

impl<'a> NoteCalcApp<'a> {
    pub fn new(client_width: usize) -> NoteCalcApp<'a> {
        let prefixes: &'static UnitPrefixes = Box::leak(Box::new(create_prefixes()));
        let units = Units::new(&prefixes);
        let mut editor_content = EditorContent::new(MAX_EDITOR_WIDTH);
        NoteCalcApp {
            line_selector: None,
            client_width,
            prefixes,
            units,
            editor: Editor::new(&mut editor_content),
            editor_content,
            result_buffer: [0 as char; 1024],
            matrix_editing: None,
            editor_click: None,
            editor_objects: Vec::with_capacity(8),
            line_id_generator: 1,
            has_result_bitset: 0,
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

    pub fn render<'b>(&'b mut self) -> RenderBuckets<'b> {
        let RIGHT_GUTTER_WIDTH = 3;
        let MIN_RESULT_PANEL_WIDTH = 20;
        let result_gutter_x = (LEFT_GUTTER_WIDTH + MAX_EDITOR_WIDTH)
            .min(self.client_width - (RIGHT_GUTTER_WIDTH + MIN_RESULT_PANEL_WIDTH));
        let current_editor_width = result_gutter_x - LEFT_GUTTER_WIDTH;

        // TODO: improve vec alloc
        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer_index = 0;
        let mut result_str_positions: SmallVec<[Option<(usize, usize)>; 64]> =
            SmallVec::with_capacity(64);
        let mut results: SmallVec<[Option<CalcResult>; 64]> = SmallVec::with_capacity(64);
        let mut longest_row_len = 0;
        self.editor_objects.clear();

        // result gutter
        render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
        render_buckets.draw_rect(
            Layer::BehindText,
            result_gutter_x,
            0,
            RIGHT_GUTTER_WIDTH,
            255,
        );

        let mut vars: Vec<(&[char], CalcResult)> = Vec::new();
        let mut render_y = 0;
        self.has_result_bitset = 0;
        let cursor_pos = self.editor.get_selection().get_cursor_pos();

        // contains the y position for each editor line
        let mut editor_y_to_render_y = [0; 64];
        let mut editor_y_to_vert_align = [0; 64];
        let mut clicked_editor_pos = None;
        for (editor_y, line) in self.editor_content.lines().take(64).enumerate() {
            editor_y_to_render_y[editor_y] = render_y;

            if line.len() > longest_row_len {
                longest_row_len = line.len();
            }

            // TODO optimize vec allocations
            let mut tokens = Vec::with_capacity(128);
            TokenParser::parse_line(line, &vars, &[], &mut tokens, &self.units);

            let mut shunting_output_stack = Vec::with_capacity(128);
            ShuntingYard::shunting_yard(&mut tokens, &[], &mut shunting_output_stack);

            // render
            let mut render_x = 0;
            let need_matrix_renderer = !self.editor.get_selection().is_range() || {
                let first = self.editor.get_selection().get_first();
                let second = self.editor.get_selection().get_second();
                !(editor_y >= first.row && editor_y <= second.row)
            };
            let rendered_row_height =
                NoteCalcApp::calc_rendered_row_height(&tokens, need_matrix_renderer);
            // "- 1" so if it is even, it always appear higher
            let vert_align_offset = (rendered_row_height - 1) / 2;
            editor_y_to_vert_align[editor_y] = vert_align_offset;
            let mut token_index = 0;
            let mut editor_x = 0;
            let mut cursor_render_x_offset: isize = 0;
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
                    let mut text_width = 0;
                    let mut end_token_index = token_index;
                    while tokens[end_token_index].typ
                        != TokenType::Operator(OperatorTokenType::BracketClose)
                    {
                        text_width += tokens[end_token_index].ptr.len();
                        end_token_index += 1;
                    }
                    // ignore the bracket as well
                    text_width += 1;
                    end_token_index += 1;

                    self.editor_objects.push(EditorObject {
                        typ: EditorObjectType::Matrix {
                            row_count: *row_count,
                            col_count: *col_count,
                        },
                        row: editor_y,
                        start_x: editor_x,
                        end_x: editor_x + text_width,
                    });

                    let cursor_isnide_matrix: bool = if !self.editor.get_selection().is_range()
                        && cursor_pos.row == editor_y
                        && cursor_pos.column > editor_x
                        && cursor_pos.column < editor_x + text_width
                    {
                        // cursor is inside the matrix
                        true
                    } else {
                        false
                    };

                    let new_render_x = if let (true, Some(mat_editor)) =
                        (cursor_isnide_matrix, &self.matrix_editing)
                    {
                        mat_editor.render(
                            render_x,
                            render_y,
                            current_editor_width,
                            &mut render_buckets,
                            rendered_row_height,
                        )
                    } else {
                        NoteCalcApp::render_matrix(
                            render_x,
                            render_y,
                            current_editor_width,
                            *row_count,
                            *col_count,
                            &tokens[token_index..],
                            &mut render_buckets,
                            rendered_row_height,
                        )
                    };

                    if cursor_pos.row == editor_y && cursor_pos.column >= editor_x + text_width {
                        let rendered_width = (new_render_x - render_x) as isize;
                        let diff = rendered_width - text_width as isize;
                        cursor_render_x_offset += diff;
                    }

                    // HANDLE CLICK
                    match self.editor_click {
                        Some(Click::Simple(clicked_pos)) | Some(Click::Drag(clicked_pos)) => {
                            if (render_y..render_y + rendered_row_height).contains(&clicked_pos.row)
                                && new_render_x >= clicked_pos.column
                            {
                                let fixed_pos = Pos::from_row_column(editor_y, editor_x + 1);
                                clicked_editor_pos =
                                    if matches!(self.editor_click, Some(Click::Simple(_))) {
                                        Some(Click::Simple(fixed_pos))
                                    } else {
                                        Some(Click::Drag(fixed_pos))
                                    };
                                self.editor_click = None;
                            }
                        }
                        None => {}
                    }

                    token_index = end_token_index;
                    render_x = new_render_x;
                    editor_x += text_width;
                } else if let (TokenType::LineReference { var_index }, true) =
                    (&token.typ, need_matrix_renderer)
                {
                    let (var_name, result) = &vars[*var_index];
                    let var_name_len = var_name.len();
                    let result_str = render_result(
                        &self.units,
                        &result,
                        &self.editor_content.get_data(editor_y).result_format,
                        // TODO: ide kellene a result.there_was_unit_conversion
                        false,
                    );
                    self.editor_objects.push(EditorObject {
                        typ: EditorObjectType::LineReference,
                        row: editor_y,
                        start_x: editor_x,
                        end_x: editor_x + var_name_len,
                    });
                    let text_len = result_str
                        .chars()
                        .count()
                        .min((current_editor_width as isize - render_x as isize).max(0) as usize);
                    render_buckets.line_ref_results.push(RenderStringMsg {
                        text: result_str[0..text_len].to_owned(),
                        row: render_y,
                        column: render_x + LEFT_GUTTER_WIDTH,
                    });

                    token_index += 1;
                    render_x += text_len;
                    editor_x += var_name_len;
                    cursor_render_x_offset += if cursor_pos.column >= editor_x {
                        let rendered_width = text_len as isize;
                        let diff = rendered_width - var_name_len as isize;
                        diff
                    } else {
                        0
                    };
                } else {
                    NoteCalcApp::draw_token(
                        token,
                        render_x,
                        render_y + vert_align_offset,
                        current_editor_width,
                        &mut render_buckets,
                    );

                    // HANDLE CLICK
                    match self.editor_click {
                        Some(Click::Simple(clicked_pos)) | Some(Click::Drag(clicked_pos)) => {
                            let new_x = render_x + token.ptr.len();
                            if render_y + rendered_row_height > clicked_pos.row
                                && new_x >= clicked_pos.column
                            {
                                let click_offset_x =
                                    (clicked_pos.column as isize - render_x as isize).max(0)
                                        as usize;
                                let fixed_pos =
                                    Pos::from_row_column(editor_y, editor_x + click_offset_x);
                                clicked_editor_pos =
                                    if matches!(self.editor_click, Some(Click::Simple(_))) {
                                        Some(Click::Simple(fixed_pos))
                                    } else {
                                        Some(Click::Drag(fixed_pos))
                                    };
                                self.editor_click = None;
                            }
                        }
                        None => {}
                    }

                    token_index += 1;
                    render_x += token.ptr.len();
                    editor_x += token.ptr.len();
                }
            }
            // CLICKING AFTER LAST TOKEN
            // HANDLE CLICK
            match self.editor_click {
                Some(Click::Simple(clicked_pos)) | Some(Click::Drag(clicked_pos)) => {
                    if render_y + rendered_row_height > clicked_pos.row {
                        let fixed_pos = Pos::from_row_column(editor_y, editor_x);
                        clicked_editor_pos = if matches!(self.editor_click, Some(Click::Simple(_)))
                        {
                            Some(Click::Simple(fixed_pos))
                        } else {
                            Some(Click::Drag(fixed_pos))
                        };
                        self.editor_click = None;
                    }
                }
                None => {}
            }

            if render_x > current_editor_width {
                render_buckets.draw_char(
                    Layer::AboveText,
                    current_editor_width + LEFT_GUTTER_WIDTH,
                    render_y,
                    '…',
                );
            }

            // highlight current line reference selector
            if let Some(selection_row) = self.line_selector {
                if selection_row == editor_y {
                    render_buckets.set_color(Layer::BehindText, 0xFFCCCC_FF);
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        0,
                        render_y,
                        result_gutter_x + RIGHT_GUTTER_WIDTH + MIN_RESULT_PANEL_WIDTH,
                        rendered_row_height,
                    );
                }
            }

            // highlight current line
            if cursor_pos.row == editor_y {
                render_buckets.set_color(Layer::BehindText, 0xFCFAED_C8);
                render_buckets.draw_rect(
                    Layer::BehindText,
                    0,
                    render_y,
                    result_gutter_x + RIGHT_GUTTER_WIDTH + MIN_RESULT_PANEL_WIDTH,
                    rendered_row_height,
                );
                // cursor is above it, so it is easier to spot
                render_buckets.set_color(Layer::AboveText, 0x000000_FF);
                if self.editor.is_cursor_shown()
                    && self.matrix_editing.is_none()
                    && ((cursor_pos.column as isize + cursor_render_x_offset) as usize)
                        < current_editor_width
                {
                    render_buckets.draw_char(
                        Layer::AboveText,
                        ((cursor_pos.column + LEFT_GUTTER_WIDTH) as isize + cursor_render_x_offset)
                            as usize,
                        render_y + vert_align_offset,
                        '▏',
                    );
                }
            }

            if let Some(result) = evaluate_tokens(&mut shunting_output_stack, &self.units, &vars) {
                self.has_result_bitset |= 1u64 << editor_y as u64;
                let result_str = render_result(
                    &self.units,
                    &result.result,
                    &self.editor_content.get_data(editor_y).result_format,
                    result.there_was_unit_conversion,
                );

                let start = result_buffer_index;
                for ch in result_str.chars() {
                    self.result_buffer[result_buffer_index] = ch;
                    result_buffer_index += 1;
                }
                result_str_positions.push(Some((start, result_buffer_index)));
                results.push(Some(result.result.clone()));
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
                    vars.push((var_name, result.result));
                } else if self.editor_content.get_data(editor_y).line_id != 0 {
                    let line_id = self.editor_content.get_data(editor_y).line_id;
                    vars.push((STATIC_LINE_IDS[line_id], result.result));
                }
            } else {
                result_str_positions.push(None);
                results.push(None);
            };

            match self.editor_content.get_data(editor_y).result_format {
                ResultFormat::Hex => {
                    render_buckets.operators.push(RenderTextMsg {
                        text: &['0', 'x'],
                        row: render_y,
                        column: result_gutter_x + 1,
                    });
                }
                ResultFormat::Bin => {
                    render_buckets.operators.push(RenderTextMsg {
                        text: &['0', 'b'],
                        row: render_y,
                        column: result_gutter_x + 1,
                    });
                }
                ResultFormat::Dec => {}
            }
            render_y += rendered_row_height;
        }

        match clicked_editor_pos {
            Some(Click::Simple(pos)) => {
                self.editor
                    .handle_click(pos.column, pos.row, &self.editor_content);
                self.editor.blink_cursor();
            }
            Some(Click::Drag(pos)) => {
                self.editor
                    .handle_drag(pos.column, pos.row, &self.editor_content);
                self.editor.blink_cursor();
            }
            None => {}
        }

        for (row_i, pos) in result_str_positions.iter().enumerate() {
            if let Some((start, end)) = pos {
                render_buckets.texts.push(RenderTextMsg {
                    text: &self.result_buffer[*start..*end],
                    row: editor_y_to_render_y[row_i] + editor_y_to_vert_align[row_i],
                    column: result_gutter_x + RIGHT_GUTTER_WIDTH,
                });
            }
        }

        // gutter
        render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
        render_buckets.draw_rect(Layer::BehindText, 0, 0, LEFT_GUTTER_WIDTH, 255);

        // line numbers
        render_buckets.set_color(Layer::BehindText, 0xADADAD_FF);
        for i in 0..64 {
            render_buckets.custom_commands[Layer::BehindText as usize].push(
                OutputMessage::RenderText(RenderTextMsg {
                    text: &(LINE_NUM_CONSTS[i][..]),
                    row: editor_y_to_render_y[i],
                    column: 1,
                }),
            )
        }

        // selected text
        render_buckets.set_color(Layer::BehindText, 0xA6D2FF_FF);
        if self.editor.get_selection().is_range() {
            let start = self.editor.get_selection().get_first();
            let end = self.editor.get_selection().get_second();
            if end.row > start.row {
                // first line
                let height = editor_y_to_render_y
                    .get(start.row + 1)
                    .map(|it| it - editor_y_to_render_y[start.row])
                    .unwrap_or(1);
                render_buckets.draw_rect(
                    Layer::BehindText,
                    start.column + LEFT_GUTTER_WIDTH,
                    editor_y_to_render_y[start.row],
                    self.editor_content
                        .line_len(start.row)
                        .min(current_editor_width),
                    height,
                );
                // full lines
                // let height = end.row - start.row - 1;
                for i in start.row + 1..end.row {
                    let height = editor_y_to_render_y
                        .get(i + 1)
                        .map(|it| it - editor_y_to_render_y[i])
                        .unwrap_or(1);
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        LEFT_GUTTER_WIDTH,
                        editor_y_to_render_y[i],
                        self.editor_content.line_len(i).min(current_editor_width),
                        height,
                    );
                }
                // last line
                let height = editor_y_to_render_y
                    .get(end.row + 1)
                    .map(|it| it - editor_y_to_render_y[end.row])
                    .unwrap_or(1);
                render_buckets.draw_rect(
                    Layer::BehindText,
                    LEFT_GUTTER_WIDTH,
                    editor_y_to_render_y[end.row],
                    end.column.min(current_editor_width),
                    height,
                );
            } else {
                let height = editor_y_to_render_y
                    .get(start.row + 1)
                    .map(|it| {
                        if *it != 0 {
                            it - editor_y_to_render_y[start.row]
                        } else {
                            1
                        }
                    })
                    .unwrap_or(1);
                render_buckets.draw_rect(
                    Layer::BehindText,
                    start.column + LEFT_GUTTER_WIDTH,
                    editor_y_to_render_y[start.row],
                    (end.column - start.column).min(current_editor_width),
                    height,
                );
            }
            // evaluated result of selection, selected text
            if let Some(mut partial_result) = self.evaluate_selection(&vars, &results) {
                if start.row == end.row {
                    let selection_center = start.column + ((end.column - start.column) / 2);
                    partial_result.insert_str(0, "= ");
                    let result_w = partial_result.chars().count();
                    let centered_x =
                        (selection_center as isize - (result_w / 2) as isize).max(0) as usize;
                    render_buckets.set_color(Layer::AboveText, 0xAAFFAA_FF);
                    render_buckets.draw_rect(
                        Layer::AboveText,
                        LEFT_GUTTER_WIDTH + centered_x,
                        editor_y_to_render_y[start.row] - 1,
                        result_w,
                        1,
                    );
                    render_buckets.set_color(Layer::AboveText, 0x000000_FF);
                    render_buckets.draw_string(
                        Layer::AboveText,
                        LEFT_GUTTER_WIDTH + centered_x,
                        editor_y_to_render_y[start.row] - 1,
                        partial_result,
                    );
                } else {
                    partial_result.insert_str(0, "⎬ ∑ = ");
                    let result_w = partial_result.chars().count();
                    let x = (start.row..=end.row)
                        .map(|it| self.editor_content.line_len(it))
                        .max_by(|a, b| a.cmp(b))
                        .unwrap()
                        + 3;
                    let height =
                        editor_y_to_render_y[end.row] - editor_y_to_render_y[start.row] + 1;
                    render_buckets.set_color(Layer::AboveText, 0xAAFFAA_FF);
                    render_buckets.draw_rect(
                        Layer::AboveText,
                        LEFT_GUTTER_WIDTH + x,
                        editor_y_to_render_y[start.row],
                        result_w + 1,
                        height,
                    );
                    // draw the parenthesis
                    render_buckets.set_color(Layer::AboveText, 0x000000_FF);

                    render_buckets.draw_char(
                        Layer::AboveText,
                        LEFT_GUTTER_WIDTH + x,
                        editor_y_to_render_y[start.row],
                        '⎫',
                    );
                    render_buckets.draw_char(
                        Layer::AboveText,
                        LEFT_GUTTER_WIDTH + x,
                        editor_y_to_render_y[end.row],
                        '⎭',
                    );
                    for i in 1..height {
                        render_buckets.draw_char(
                            Layer::AboveText,
                            LEFT_GUTTER_WIDTH + x,
                            editor_y_to_render_y[start.row] + i,
                            '⎪',
                        );
                    }
                    // center
                    render_buckets.draw_string(
                        Layer::AboveText,
                        LEFT_GUTTER_WIDTH + x,
                        editor_y_to_render_y[start.row] + height / 2,
                        partial_result,
                    );
                }
            }
        }

        return render_buckets;
    }

    fn evaluate_selection(
        &self,
        vars: &Vec<(&[char], CalcResult)>,
        results: &SmallVec<[Option<CalcResult>; 64]>,
    ) -> Option<String> {
        let sel = self.editor.get_selection();
        // TODO optimize vec allocations
        let mut tokens = Vec::with_capacity(128);
        if sel.start.row == sel.end.unwrap().row {
            if let Some(selected_text) =
                Editor::get_selected_text_single_line(sel, &self.editor_content)
            {
                if let Some(result) = self.evaluate_text(selected_text, &vars, &mut tokens) {
                    if result.there_was_operation {
                        let result_str = render_result(
                            &self.units,
                            &result.result,
                            &self.editor_content.get_data(sel.start.row).result_format,
                            result.there_was_unit_conversion,
                        );
                        return Some(result_str);
                    }
                }
            }
        } else {
            let mut sum: Option<&CalcResult> = None;
            let mut tmp_sum: CalcResult = unsafe { MaybeUninit::uninit().assume_init() };
            for row_index in sel.get_first().row..=sel.get_second().row {
                if let Some(line_result) = &results[row_index] {
                    if let Some(sum_r) = &sum {
                        if let Some(add_result) = add_op(sum_r, &line_result) {
                            tmp_sum = add_result;
                            sum = Some(&tmp_sum);
                        }
                    } else {
                        sum = Some(&line_result);
                    }
                }
            }
            if let Some(sum) = sum {
                let result_str = render_result(
                    &self.units,
                    sum,
                    &self.editor_content.get_data(sel.start.row).result_format,
                    false,
                );
                return Some(result_str);
            }
        }
        return None;
    }

    fn evaluate_text<'text_ptr, 'units>(
        &'units self,
        text: &'text_ptr [char],
        vars: &'units Vec<(&'text_ptr [char], CalcResult)>,
        tokens: &mut Vec<Token<'text_ptr, 'units>>,
    ) -> Option<EvaluationResult> {
        TokenParser::parse_line(text, vars, &[], tokens, &self.units);
        let mut shunting_output_stack = Vec::with_capacity(4);
        ShuntingYard::shunting_yard(tokens, &[], &mut shunting_output_stack);
        return evaluate_tokens(&mut shunting_output_stack, &self.units, &vars);
    }

    fn render_matrix<'b>(
        mut render_x: usize,
        mut render_y: usize,
        current_editor_width: usize,
        row_count: usize,
        col_count: usize,
        tokens: &[Token<'b, 'b>],
        render_buckets: &mut RenderBuckets<'b>,
        rendered_row_height: usize,
    ) -> usize {
        let vert_align_offset = (rendered_row_height - row_count) / 2;

        render_buckets.operators.push(RenderTextMsg {
            text: &['⎡'],
            row: render_y + vert_align_offset,
            column: render_x + LEFT_GUTTER_WIDTH,
        });
        for i in 1..row_count - 1 {
            render_buckets.operators.push(RenderTextMsg {
                text: &['⎢'],
                row: render_y + i + vert_align_offset,
                column: render_x + LEFT_GUTTER_WIDTH,
            });
        }
        render_buckets.operators.push(RenderTextMsg {
            text: &['⎣'],
            row: render_y + row_count - 1 + vert_align_offset,
            column: render_x + LEFT_GUTTER_WIDTH,
        });
        render_x += 1;

        let mut tokens_per_cell = {
            let mut tokens_per_cell: [MaybeUninit<&[Token]>; 32] =
                unsafe { MaybeUninit::uninit().assume_init() };

            let mut start_token_index = 0;
            let mut cell_index = 0;
            let mut can_ignore_ws = true;
            for (token_index, token) in tokens.iter().enumerate() {
                if token.typ == TokenType::Operator(OperatorTokenType::BracketClose) {
                    tokens_per_cell[cell_index] =
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
                    tokens_per_cell[cell_index] =
                        MaybeUninit::new(&tokens[start_token_index..token_index]);
                    start_token_index = token_index + 1;
                    cell_index += 1;
                    can_ignore_ws = true;
                } else {
                    can_ignore_ws = false;
                }
            }
            unsafe { std::mem::transmute::<_, [&[Token]; 32]>(tokens_per_cell) }
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

        render_buckets.operators.push(RenderTextMsg {
            text: &['⎤'],
            row: render_y + vert_align_offset,
            column: render_x + LEFT_GUTTER_WIDTH,
        });
        for i in 1..row_count - 1 {
            render_buckets.operators.push(RenderTextMsg {
                text: &['⎥'],
                row: render_y + i + vert_align_offset,
                column: render_x + LEFT_GUTTER_WIDTH,
            });
        }
        render_buckets.operators.push(RenderTextMsg {
            text: &['⎦'],
            row: render_y + row_count - 1 + vert_align_offset,
            column: render_x + LEFT_GUTTER_WIDTH,
        });
        render_x += 1;

        render_x
    }

    fn draw_token<'b>(
        token: &Token<'b, 'b>,
        render_x: usize,
        render_y: usize,
        current_editor_width: usize,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        let dst = match &token.typ {
            TokenType::StringLiteral => &mut render_buckets.texts,
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
        dst.push(RenderTextMsg {
            text: &token.ptr[0..text_len],
            row: render_y,
            column: render_x + LEFT_GUTTER_WIDTH,
        });
    }

    fn calc_rendered_row_height(tokens: &[Token], need_matrix_renderer: bool) -> usize {
        if !need_matrix_renderer {
            // return 1;
        }
        let mut max_height = 1;
        for token in tokens {
            match token.typ {
                TokenType::Operator(OperatorTokenType::Matrix {
                    row_count,
                    col_count,
                }) => {
                    if row_count > max_height {
                        max_height = row_count;
                    }
                }
                _ => {}
            }
        }
        return max_height;
    }

    pub fn handle_click(&mut self, x: usize, y: usize) {
        if x < LEFT_GUTTER_WIDTH {
            // clicked on gutter
        } else if x - LEFT_GUTTER_WIDTH < MAX_EDITOR_WIDTH {
            self.editor_click = Some(Click::Simple(Pos::from_row_column(
                y,
                x - LEFT_GUTTER_WIDTH,
            )));
            if self.matrix_editing.is_some() {
                self.end_matrix_editing(None);
            }
        }
    }

    pub fn handle_drag(&mut self, x: usize, y: usize) {
        if x < LEFT_GUTTER_WIDTH {
            // clicked on gutter
        } else if x - LEFT_GUTTER_WIDTH < MAX_EDITOR_WIDTH {
            self.editor_click = Some(Click::Drag(Pos::from_row_column(y, x - LEFT_GUTTER_WIDTH)));
        }
    }

    pub fn handle_resize(&mut self, new_client_width: usize) {
        self.client_width = new_client_width;
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
        if self.line_selector.is_none() {
            return;
        }
        let cur_row = self.editor.get_selection().get_cursor_pos().row;
        let row_index = self.line_selector.unwrap();
        self.line_selector = None;
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
                let cur_pos = self.editor.get_selection().get_cursor_pos();
                let new_format = match &self.editor_content.get_data(cur_pos.row).result_format {
                    ResultFormat::Bin => ResultFormat::Hex,
                    ResultFormat::Dec => ResultFormat::Bin,
                    ResultFormat::Hex => ResultFormat::Dec,
                };
                self.editor_content.mut_data(cur_pos.row).result_format = new_format;
            } else if input == EditorInputEvent::Right {
                let cur_pos = self.editor.get_selection().get_cursor_pos();
                let new_format = match &self.editor_content.get_data(cur_pos.row).result_format {
                    ResultFormat::Bin => ResultFormat::Dec,
                    ResultFormat::Dec => ResultFormat::Hex,
                    ResultFormat::Hex => ResultFormat::Bin,
                };
                self.editor_content.mut_data(cur_pos.row).result_format = new_format;
            } else if input == EditorInputEvent::Up {
                let cur_pos = self.editor.get_selection().get_cursor_pos();
                self.line_selector = if let Some(selector_row) = self.line_selector {
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
                self.line_selector = if let Some(selector_row) = self.line_selector {
                    if selector_row < self.editor_content.line_count() {
                        Some(selector_row + 1)
                    } else {
                        Some(selector_row)
                    }
                } else if cur_pos.row < self.editor_content.line_count() {
                    Some(cur_pos.row + 1)
                } else {
                    None
                }
            }
            false
        } else if self.matrix_editing.is_some() {
            self.handle_matrix_editor_input(input, modifiers);
            true
        } else {
            let before_cursor_pos = self.editor.get_selection();
            if input == EditorInputEvent::Backspace
                && !before_cursor_pos.is_range()
                && before_cursor_pos.start.column > 0
            {
                if let Some(index) = self
                    .index_of_editor_object_at(before_cursor_pos.get_cursor_pos().with_prev_col())
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
            } else if input == EditorInputEvent::Del && !before_cursor_pos.is_range() {
                if let Some(index) = self
                    .index_of_editor_object_at(before_cursor_pos.get_cursor_pos().with_next_col())
                {
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
                && !before_cursor_pos.is_range()
                && before_cursor_pos.start.column > 0
            {
                if let Some(obj) =
                    self.find_editor_object_at(before_cursor_pos.get_cursor_pos().with_prev_col())
                {
                    if obj.typ == EditorObjectType::LineReference {
                        //  jump over it
                        self.editor.set_cursor_pos_r_c(obj.row, obj.start_x);
                        return false;
                    }
                }
            } else if input == EditorInputEvent::Right && !before_cursor_pos.is_range() {
                if let Some(obj) = self.find_editor_object_at(before_cursor_pos.get_cursor_pos()) {
                    if obj.typ == EditorObjectType::LineReference {
                        //  jump over it
                        self.editor.set_cursor_pos_r_c(obj.row, obj.end_x);
                        return false;
                    }
                }
            }

            let modified = self
                .editor
                .handle_input(input, modifiers, &mut self.editor_content);

            if let Some(editor_obj) =
                self.is_pos_inside_an_obj(self.editor.get_selection().get_cursor_pos())
            {
                match editor_obj.typ {
                    EditorObjectType::Matrix {
                        row_count,
                        col_count,
                    } => {
                        if self.matrix_editing.is_none() && !self.editor.get_selection().is_range()
                        {
                            self.matrix_editing = Some(MatrixEditing::new(
                                row_count,
                                col_count,
                                &self.editor_content.get_line_chars(editor_obj.row)
                                    [editor_obj.start_x..editor_obj.end_x],
                                editor_obj.row,
                                editor_obj.start_x,
                                editor_obj.end_x,
                                before_cursor_pos.get_cursor_pos(),
                            ));
                        }
                    }
                    EditorObjectType::LineReference => {}
                }
            }
            return modified;
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

    fn is_pos_inside_an_obj(&self, pos: Pos) -> Option<&EditorObject> {
        for obj in &self.editor_objects {
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

        if input == EditorInputEvent::Esc || input == EditorInputEvent::Enter {
            self.end_matrix_editing(None);
        } else if input == EditorInputEvent::Left && mat_edit.editor.is_cursor_at_beginning() {
            if mat_edit.current_cell.column > 0 {
                mat_edit.change_cell(mat_edit.current_cell.with_prev_col());
                // cursor at end of line
                let cell_len = mat_edit.editor_content.line_len(0);
                mat_edit.editor.set_cursor_pos_r_c(0, cell_len);
            } else {
                let start_text_index = mat_edit.start_text_index;
                self.end_matrix_editing(Some(cur_pos.with_column(start_text_index)));
            }
        } else if input == EditorInputEvent::Right
            && mat_edit.editor.is_cursor_at_eol(&mat_edit.editor_content)
        {
            if mat_edit.current_cell.column + 1 < mat_edit.col_count {
                mat_edit.change_cell(mat_edit.current_cell.with_next_col());
                mat_edit.editor.set_cursor_pos_r_c(0, 0);
            } else {
                let end_text_index = mat_edit.end_text_index;
                self.end_matrix_editing(Some(cur_pos.with_column(end_text_index)));
            }
        } else if input == EditorInputEvent::Up {
            if mat_edit.current_cell.row > 0 {
                let pos = mat_edit.editor.get_selection().get_cursor_pos();
                let cols_from_right = mat_edit.editor_content.line_len(0) - pos.column;
                mat_edit.change_cell(mat_edit.current_cell.with_prev_row());
                let cell_len = mat_edit.editor_content.line_len(0);
                mat_edit
                    .editor
                    .set_cursor_pos_r_c(0, cell_len - cols_from_right.min(cell_len));
            } else {
                self.end_matrix_editing(None);
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else if input == EditorInputEvent::Down {
            if mat_edit.current_cell.row + 1 < mat_edit.row_count {
                let pos = mat_edit.editor.get_selection().get_cursor_pos();
                let cols_from_right = mat_edit.editor_content.line_len(0) - pos.column;
                mat_edit.change_cell(mat_edit.current_cell.with_next_row());
                let cell_len = mat_edit.editor_content.line_len(0);
                mat_edit
                    .editor
                    .set_cursor_pos_r_c(0, cell_len - cols_from_right.min(cell_len));
            } else {
                self.end_matrix_editing(None);
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else if input == EditorInputEvent::End {
            if mat_edit.current_cell.column != mat_edit.col_count - 1 {
                mat_edit.change_cell(mat_edit.current_cell.with_column(mat_edit.col_count - 1));
                let cell_len = mat_edit.editor_content.line_len(0);
                mat_edit.editor.set_cursor_pos_r_c(0, cell_len);
            } else {
                let end_text_index = mat_edit.end_text_index;
                self.end_matrix_editing(Some(cur_pos.with_column(end_text_index)));
                self.editor
                    .handle_input(input, modifiers, &mut self.editor_content);
            }
        } else if input == EditorInputEvent::Home {
            if mat_edit.current_cell.column != 0 {
                mat_edit.change_cell(mat_edit.current_cell.with_column(0));
                mat_edit.editor.set_cursor_pos_r_c(0, 0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::editor::Selection;

    #[test]
    fn bug1() {
        let mut app = NoteCalcApp::new(120);

        app.handle_input(
            EditorInputEvent::Text("[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 33));
        app.handle_input(EditorInputEvent::Right, InputModifiers::alt());
        app.render();
    }

    #[test]
    fn bug2() {
        let mut app = NoteCalcApp::new(120);
        app.handle_input(
            EditorInputEvent::Text("[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 1));
        app.handle_input(EditorInputEvent::Right, InputModifiers::alt());
        app.render();
        app.handle_input(EditorInputEvent::Down, InputModifiers::none());
        app.render();
    }

    #[test]
    fn bug3() {
        let mut app = NoteCalcApp::new(120);
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
        app.render();
    }

    #[test]
    fn remove_matrix_backspace() {
        let mut app = NoteCalcApp::new(120);
        app.handle_input(
            EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
            InputModifiers::none(),
        );
        app.render();
        app.handle_input(EditorInputEvent::Backspace, InputModifiers::none());
        assert_eq!("abcd ", app.editor_content.get_content());
    }

    #[test]
    fn matrix_step_in_dir() {
        // from right
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.render();
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.handle_input(EditorInputEvent::Char('1'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("abcd [1,2,31;4,5,6]", app.editor_content.get_content());
        }
        // from left
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render();
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Char('2'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("abcd [21,2,3;4,5,6]", app.editor_content.get_content());
        }
        // from below
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]\naaaaaaaaaaaaaaaaaa".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 7));
            app.render();
            app.handle_input(EditorInputEvent::Up, InputModifiers::none());
            app.handle_input(EditorInputEvent::Char('2'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!(
                "abcd [1,2,3;24,5,6]\naaaaaaaaaaaaaaaaaa",
                app.editor_content.get_content()
            );
        }
        // from above
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("aaaaaaaaaaaaaaaaaa\nabcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 7));
            app.render();
            app.handle_input(EditorInputEvent::Down, InputModifiers::none());
            app.handle_input(EditorInputEvent::Char('2'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!(
                "aaaaaaaaaaaaaaaaaa\nabcd [21,2,3;4,5,6]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn cursor_is_put_after_the_matrix_after_finished_editing() {
        let mut app = NoteCalcApp::new(120);
        app.handle_input(
            EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
            InputModifiers::none(),
        );
        app.render();
        app.handle_input(EditorInputEvent::Left, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('6'), InputModifiers::none());
        app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
        app.render();
        app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
        assert_eq!("abcd [1,2,36;4,5,6]9", app.editor_content.get_content());
    }

    #[test]
    fn remove_matrix_del() {
        let mut app = NoteCalcApp::new(120);
        app.handle_input(
            EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 5));
        app.render();
        app.handle_input(EditorInputEvent::Del, InputModifiers::none());
        assert_eq!("abcd ", app.editor_content.get_content());
    }

    #[test]
    fn test_moving_inside_a_matrix() {
        // right to left, cursor at end
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.render();
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            app.handle_input(EditorInputEvent::Char('6'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render();
            assert_eq!("abcd [1,26,3;4,5,6]", app.editor_content.get_content());
        }
        // left to right, cursor at start
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1,2,3;4,5,6]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render();
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Char('6'), InputModifiers::none());
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render();
            assert_eq!("abcd [1,62,3;4,5,6]", app.editor_content.get_content());
        }
        // vertical movement down, cursor tries to keep its position
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render();
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            // inside the matrix
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Down, InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Char('6'), InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render();
            assert_eq!(
                "abcd [1111,22,3;464,55555,666]",
                app.editor_content.get_content()
            );
        }

        // vertical movement up, cursor tries to keep its position
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666]".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render();
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            // inside the matrix
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            app.handle_input(EditorInputEvent::Down, InputModifiers::none());
            app.handle_input(EditorInputEvent::Up, InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Char('6'), InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render();
            assert_eq!(
                "abcd [11161,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn end_btn_matrix() {
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666] qq".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render();
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            // inside the matrix
            app.handle_input(EditorInputEvent::End, InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render();
            assert_eq!(
                "abcd [1111,22,39;44,55555,666] qq",
                app.editor_content.get_content()
            );
        }
        // pressing twice, exits the matrix
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666] qq".to_owned()),
                InputModifiers::none(),
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            app.render();
            app.handle_input(EditorInputEvent::Right, InputModifiers::none());
            // inside the matrix
            app.handle_input(EditorInputEvent::End, InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::End, InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.render();
            assert_eq!(
                "abcd [1111,22,3;44,55555,666] qq9",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn home_btn_matrix() {
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666]".to_owned()),
                InputModifiers::none(),
            );
            app.render();
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            // inside the matrix
            app.handle_input(EditorInputEvent::Home, InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Char('9'), InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Enter, InputModifiers::none());
            app.render();
            assert_eq!(
                "abcd [91111,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
        {
            let mut app = NoteCalcApp::new(120);
            app.handle_input(
                EditorInputEvent::Text("abcd [1111,22,3;44,55555,666]".to_owned()),
                InputModifiers::none(),
            );
            app.render();
            app.handle_input(EditorInputEvent::Left, InputModifiers::none());
            // inside the matrix
            app.handle_input(EditorInputEvent::Home, InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Home, InputModifiers::none());
            app.render();
            app.handle_input(EditorInputEvent::Char('6'), InputModifiers::none());
            app.render();
            assert_eq!(
                "6abcd [1111,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn bug8() {
        let mut app = NoteCalcApp::new(120);
        app.handle_input(
            EditorInputEvent::Text("16892313\n14 * ".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 5));
        app.render();
        app.handle_input(EditorInputEvent::Up, InputModifiers::alt());
        app.alt_key_released();
        assert_eq!("16892313\n14 * &[1]", app.editor_content.get_content());
        app.render();
        app.handle_time(1000);
        app.handle_input(EditorInputEvent::Backspace, InputModifiers::none());
        assert_eq!("16892313\n14 * ", app.editor_content.get_content());

        app.handle_input(EditorInputEvent::Char('z'), InputModifiers::ctrl());
        assert_eq!("16892313\n14 * &[1]", app.editor_content.get_content());

        app.handle_input(EditorInputEvent::Right, InputModifiers::none()); // end selection
        app.render();
        app.handle_input(EditorInputEvent::Left, InputModifiers::none());
        app.handle_input(EditorInputEvent::Char('a'), InputModifiers::none());
        assert_eq!("16892313\n14 * a&[1]", app.editor_content.get_content());

        app.handle_input(EditorInputEvent::Char(' '), InputModifiers::none());
        app.render();
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
        let mut app = NoteCalcApp::new(120);
        app.handle_input(
            EditorInputEvent::Text("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n".to_owned()),
            InputModifiers::none(),
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(12, 2));
        app.render();
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
        let mut app = NoteCalcApp::new(120);
        app.set_normalized_content("1111\n2222\n14 * &[2]&[2]&[2]\n");
        assert_eq!(1, app.editor_content.get_data(0).line_id);
        assert_eq!(2, app.editor_content.get_data(1).line_id);
        assert_eq!(3, app.editor_content.get_data(2).line_id);
    }
}
