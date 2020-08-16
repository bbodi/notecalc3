#![feature(ptr_offset_from, const_fn, const_panic, drain_filter)]
#![feature(type_alias_impl_trait)]
#![feature(const_in_array_repeat_expressions)]
#![deny(
    warnings,
    anonymous_parameters,
    unused_extern_crates,
    unused_import_braces,
    trivial_casts,
    variant_size_differences,
    trivial_numeric_casts,
    unused_qualifications,
    clippy::all
)]

use crate::calc::{add_op, evaluate_tokens, CalcResult, EvaluationResult};
use crate::consts::{LINE_NUM_CONSTS, STATIC_LINE_IDS};
use crate::editor::editor::{
    Editor, EditorInputEvent, InputModifiers, Pos, RowModificationType, Selection,
};
use crate::editor::editor_content::EditorContent;
use crate::matrix::MatrixData;
use crate::renderer::{get_int_frac_part_len, render_result, render_result_into};
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
const SCROLL_BAR_WIDTH: usize = 1;
const RIGHT_GUTTER_WIDTH: usize = 2;
const MIN_RESULT_PANEL_WIDTH: usize = 30;
const SUM_VARIABLE_INDEX: usize = MAX_LINE_COUNT;
const MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT: usize = 2;

pub enum Click {
    Simple(Pos),
    Drag(Pos),
}

pub mod helper {
    // so code from the lib module can't access the private parts

    use crate::{MAX_LINE_COUNT, *};
    use std::ops::{Index, IndexMut};

    pub fn create_vars() -> [Option<Variable>; MAX_LINE_COUNT + 1] {
        let mut vars = [None; MAX_LINE_COUNT + 1];
        vars[SUM_VARIABLE_INDEX] = Some(Variable {
            name: Box::from(&['s', 'u', 'm'][..]),
            value: Err(()),
        });
        return vars;
    }

    pub struct EditorObjects(Vec<Vec<EditorObject>>);

    impl EditorObjects {
        pub fn new() -> EditorObjects {
            EditorObjects(
                std::iter::repeat(Vec::with_capacity(8))
                    .take(MAX_LINE_COUNT)
                    .collect::<Vec<_>>(),
            )
        }

        pub fn clear(&mut self) {
            self.0.clear();
        }

        pub fn push(&mut self, d: Vec<EditorObject>) {
            self.0.push(d);
        }
    }

    impl Index<EditorY> for EditorObjects {
        type Output = Vec<EditorObject>;

        fn index(&self, index: EditorY) -> &Self::Output {
            &self.0[index.0]
        }
    }

    impl IndexMut<EditorY> for EditorObjects {
        fn index_mut(&mut self, index: EditorY) -> &mut Self::Output {
            &mut self.0[index.0]
        }
    }

    pub struct Results([LineResult; MAX_LINE_COUNT]);

    impl Results {
        pub fn new() -> Results {
            Results([Ok(None); MAX_LINE_COUNT])
        }
        pub fn as_slice(&self) -> &[LineResult] {
            &self.0[..]
        }

        pub fn as_mut_slice(&mut self) -> &mut [LineResult] {
            &mut self.0[..]
        }
    }

    impl Index<EditorY> for Results {
        type Output = LineResult;

        fn index(&self, index: EditorY) -> &Self::Output {
            &self.0[index.0]
        }
    }

    impl IndexMut<EditorY> for Results {
        fn index_mut(&mut self, index: EditorY) -> &mut Self::Output {
            &mut self.0[index.0]
        }
    }

    pub struct AppTokens<'a>([Option<Tokens<'a>>; MAX_LINE_COUNT]);

    impl<'a> AppTokens<'a> {
        pub fn new() -> AppTokens<'a> {
            AppTokens([None; MAX_LINE_COUNT])
        }

        pub fn iter(&self) -> std::slice::Iter<Option<Tokens<'a>>> {
            self.0.iter()
        }
    }

    impl<'a> Index<EditorY> for AppTokens<'a> {
        type Output = Option<Tokens<'a>>;

        fn index(&self, index: EditorY) -> &Self::Output {
            &self.0[index.0]
        }
    }

    impl<'a> IndexMut<EditorY> for AppTokens<'a> {
        fn index_mut(&mut self, index: EditorY) -> &mut Self::Output {
            &mut self.0[index.0]
        }
    }

    #[derive(Copy, Clone)]
    pub struct EditorRowFlags {
        bitset: u64,
    }

    impl EditorRowFlags {
        pub fn empty() -> EditorRowFlags {
            EditorRowFlags { bitset: 0 }
        }

        pub fn single_row(row_index: usize) -> EditorRowFlags {
            let bitset = 1u64 << row_index;
            EditorRowFlags { bitset }
        }

        pub fn clear(&mut self) {
            self.bitset = 0;
        }

        pub fn all_rows_starting_at(row_index: usize) -> EditorRowFlags {
            let s = 1u64 << row_index;
            let right_to_s_bits = s - 1;
            let left_to_s_and_s_bits = !right_to_s_bits;
            let bitset = left_to_s_and_s_bits;

            EditorRowFlags { bitset }
        }

        pub fn multiple(indices: &[usize]) -> EditorRowFlags {
            let mut b = 0;
            for i in indices {
                b |= 1 << i;
            }
            let bitset = b;

            EditorRowFlags { bitset }
        }

        pub fn range(from: usize, to: usize) -> EditorRowFlags {
            debug_assert!(to >= from);
            let top = 1 << to;
            let right_to_top_bits = top - 1;
            let bottom = 1 << from;
            let right_to_bottom_bits = bottom - 1;
            let bitset = (right_to_top_bits ^ right_to_bottom_bits) | top;

            EditorRowFlags { bitset }
        }

        pub fn merge(&mut self, other: EditorRowFlags) {
            self.bitset |= other.bitset;
        }

        pub fn need(&self, line_index: EditorY) -> bool {
            ((1 << line_index.0) & self.bitset) != 0
        }
    }

    pub struct GlobalRenderData {
        pub client_height: usize,
        pub scroll_y: usize,
        pub result_gutter_x: usize,
        pub left_gutter_width: usize,
        pub latest_bottom: RenderPosY,

        pub current_editor_width: usize,
        pub current_result_panel_width: usize,
        editor_y_to_render_y: [Option<RenderPosY>; MAX_LINE_COUNT],
        editor_y_to_rendered_height: [usize; MAX_LINE_COUNT],
    }

    impl GlobalRenderData {
        pub fn new(
            client_width: usize,
            client_height: usize,
            result_gutter_x: usize,
            left_gutter_width: usize,
            right_gutter_width: usize,
        ) -> GlobalRenderData {
            let mut r = GlobalRenderData {
                scroll_y: 0,
                result_gutter_x,
                left_gutter_width,
                latest_bottom: RenderPosY::new(0),
                current_editor_width: 0,
                current_result_panel_width: 0,
                editor_y_to_render_y: [None; MAX_LINE_COUNT],
                editor_y_to_rendered_height: [0; MAX_LINE_COUNT],
                client_height,
            };

            r.current_editor_width = result_gutter_x - left_gutter_width;
            r.current_result_panel_width = client_width - result_gutter_x - right_gutter_width;
            r
        }

        pub fn clear(&mut self) {
            for e in self.editor_y_to_render_y.iter_mut() {
                *e = None;
            }
            for e in self.editor_y_to_rendered_height.iter_mut() {
                *e = 0;
            }
            self.scroll_y = 0;
            self.latest_bottom = RenderPosY::new(0);
        }

        pub fn get_render_y(&self, y: EditorY) -> Option<RenderPosY> {
            self.editor_y_to_render_y[y.0]
        }

        pub fn set_render_y(&mut self, y: EditorY, newy: Option<RenderPosY>) {
            self.editor_y_to_render_y[y.0] = newy;
        }

        pub fn editor_y_to_render_y(&self) -> &[Option<RenderPosY>] {
            &self.editor_y_to_render_y
        }

        pub fn get_rendered_height(&self, y: EditorY) -> usize {
            self.editor_y_to_rendered_height[y.0]
        }

        pub fn set_rendered_height(&mut self, y: EditorY, h: usize) {
            self.editor_y_to_rendered_height[y.0] = h;
        }
    }

    pub struct PerLineRenderData {
        pub editor_x: usize,
        pub editor_y: EditorY,
        pub render_x: usize,
        pub render_y: RenderPosY,
        // contains the y position for each editor line
        pub rendered_row_height: usize,
        pub vert_align_offset: usize,
        pub cursor_render_x_offset: isize,
    }

    impl PerLineRenderData {
        pub fn new() -> PerLineRenderData {
            let r = PerLineRenderData {
                editor_x: 0,
                editor_y: EditorY::new(0),
                render_x: 0,
                render_y: RenderPosY::new(0),
                rendered_row_height: 0,
                vert_align_offset: 0,
                cursor_render_x_offset: 0,
            };
            r
        }

        pub fn inc_editor_y(&mut self) {
            self.editor_y.0 += 1;
        }

        pub fn new_line_started(&mut self) {
            self.editor_x = 0;
            self.render_x = 0;
            self.cursor_render_x_offset = 0;
        }

        pub fn line_render_ended(&mut self, row_height: usize) {
            self.render_y.0 += row_height;
            self.editor_y.0 += 1;
        }

        pub fn set_fix_row_height(&mut self, height: usize) {
            self.rendered_row_height = height;
            self.vert_align_offset = 0;
        }

        pub fn calc_rendered_row_height(
            result: &LineResult,
            tokens: &[Token],
            vars: &Variables,
            active_mat_edit_height: Option<usize>,
        ) -> usize {
            let mut max_height = active_mat_edit_height.unwrap_or(1);
            // determine max height based on result's height
            let result_row_height = if let Ok(result) = result {
                if let Some(result) = result {
                    let result_row_height = match &result {
                        CalcResult::Matrix(mat) => {
                            if mat.row_count == 1 {
                                1
                            } else {
                                mat.row_count + MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT
                            }
                        }
                        _ => max_height,
                    };
                    result_row_height
                } else {
                    max_height
                }
            } else {
                max_height
            };

            // determine max height based on tokens' height
            for token in tokens {
                let token_height = match token.typ {
                    TokenType::Operator(OperatorTokenType::Matrix {
                        row_count,
                        col_count: _,
                    }) => {
                        if row_count == 1 {
                            1
                        } else {
                            row_count + MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT
                        }
                    }
                    TokenType::LineReference { var_index } => {
                        let var = &vars[var_index];
                        match &var {
                            Some(Variable {
                                value: Ok(CalcResult::Matrix(mat)),
                                ..
                            }) => mat.row_count,
                            _ => 1,
                        }
                    }
                    _ => 1,
                };
                if token_height > max_height {
                    max_height = token_height;
                }
            }
            return max_height.max(result_row_height);
        }

        pub fn token_render_done(&mut self, editor_len: usize, render_len: usize, x_offset: isize) {
            self.render_x += render_len;
            self.editor_x += editor_len;
            self.cursor_render_x_offset += x_offset;
        }
    }

    #[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
    pub struct EditorY(usize);

    impl EditorY {
        pub fn new(n: usize) -> EditorY {
            EditorY(n)
        }

        #[inline]
        pub fn as_usize(self) -> usize {
            self.0
        }

        pub fn add(&self, n: usize) -> EditorY {
            EditorY(self.0 + n)
        }

        pub fn sub(&self, n: usize) -> EditorY {
            EditorY(self.0 - n)
        }
    }

    #[derive(Clone, Copy, Eq, PartialEq, Debug, Ord, PartialOrd)]
    pub struct RenderPosY(usize);

    impl RenderPosY {
        pub fn new(n: usize) -> RenderPosY {
            RenderPosY(n)
        }

        pub fn as_usize(self) -> usize {
            self.0
        }

        pub fn add(&self, n: usize) -> RenderPosY {
            RenderPosY(self.0 + n)
        }

        pub fn sub(&self, n: usize) -> RenderPosY {
            RenderPosY(self.0 - n)
        }
    }
}

use helper::*;

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
    pub row: RenderPosY,
    pub column: usize,
}

#[derive(Debug)]
pub struct RenderAsciiTextMsg<'a> {
    pub text: &'a [u8],
    pub row: RenderPosY,
    pub column: usize,
}

#[derive(Debug)]
pub struct RenderStringMsg {
    pub text: String,
    pub row: RenderPosY,
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
        y: RenderPosY,
        w: usize,
        h: usize,
    },
    PulsingRectangle {
        x: usize,
        y: RenderPosY,
        w: usize,
        h: usize,
        start_color: u32,
        end_color: u32,
        animation_time: Duration,
    },
}

#[repr(C)]
pub enum Layer {
    BehindText,
    Text,
    AboveText,
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
    pub custom_commands: [Vec<OutputMessage<'a>>; 3],
    pub clear_commands: Vec<OutputMessage<'a>>,
}

impl<'a> RenderBuckets<'a> {
    pub fn new() -> RenderBuckets<'a> {
        RenderBuckets {
            ascii_texts: Vec::with_capacity(128),
            utf8_texts: Vec::with_capacity(128),
            custom_commands: [
                Vec::with_capacity(128),
                Vec::with_capacity(128),
                Vec::with_capacity(128),
            ],
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
        self.custom_commands[2].clear();
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

    pub fn draw_rect(&mut self, layer: Layer, x: usize, y: RenderPosY, w: usize, h: usize) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderRectangle { x, y, w, h });
    }

    pub fn draw_char(&mut self, layer: Layer, x: usize, y: RenderPosY, ch: char) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderChar(x, y.as_usize(), ch));
    }

    pub fn draw_text(&mut self, layer: Layer, x: usize, y: RenderPosY, text: &'static [char]) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderUtf8Text(
            RenderUtf8TextMsg {
                text,
                row: y,
                column: x,
            },
        ));
    }

    pub fn draw_ascii_text(&mut self, layer: Layer, x: usize, y: RenderPosY, text: &'static [u8]) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderAsciiText(
            RenderAsciiTextMsg {
                text,
                row: y,
                column: x,
            },
        ));
    }

    pub fn draw_string(&mut self, layer: Layer, x: usize, y: RenderPosY, text: String) {
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
    row_index: EditorY,
    cell_strings: Vec<String>,
}

impl MatrixEditing {
    pub fn new<'a>(
        row_count: usize,
        col_count: usize,
        src_canvas: &[char],
        row_index: EditorY,
        start_text_index: usize,
        end_text_index: usize,
        step_in_pos: Pos,
    ) -> MatrixEditing {
        let current_cell = if step_in_pos.row == row_index.as_usize() {
            if step_in_pos.column > start_text_index {
                // from right
                Pos::from_row_column(0, col_count - 1)
            } else {
                // from left
                Pos::from_row_column(0, 0)
            }
        } else if step_in_pos.row < row_index.as_usize() {
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
        render_y: RenderPosY,
        current_editor_width: usize,
        left_gutter_width: usize,
        render_buckets: &mut RenderBuckets<'b>,
        rendered_row_height: usize,
    ) -> usize {
        let vert_align_offset = if self.row_count == 1 {
            (rendered_row_height - 1) / 2
        } else {
            (rendered_row_height - (self.row_count + MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT)) / 2
        };

        render_matrix_left_brackets(
            render_x + left_gutter_width,
            render_y,
            self.row_count,
            render_buckets,
            vert_align_offset,
        );
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
                // the content of the matrix starts from the second row
                let matrix_ascii_header_offset = if self.row_count == 1 { 0 } else { 1 };
                let dst_y = render_y.add(row_i + vert_align_offset + matrix_ascii_header_offset);
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
                        dst_y,
                        text_len,
                        1,
                    );
                    let chars = &self.editor_content.lines().next().unwrap();
                    render_buckets.set_color(Layer::Text, 0x000000_FF);
                    for (i, char) in chars.iter().enumerate() {
                        render_buckets.draw_char(
                            Layer::Text,
                            render_x + padding_x + left_gutter_width + i,
                            dst_y,
                            *char,
                        );
                    }
                    let sel = self.editor.get_selection();
                    if let Some((first, second)) = sel.is_range() {
                        let len = second.column - first.column;
                        render_buckets.set_color(Layer::BehindText, 0xA6D2FF_FF);
                        render_buckets.draw_rect(
                            Layer::BehindText,
                            render_x + padding_x + left_gutter_width + first.column,
                            dst_y,
                            len,
                            1,
                        );
                    }
                } else {
                    let chars = &self.cell_strings[row_i * self.col_count + col_i];
                    render_buckets.set_color(Layer::Text, 0x000000_FF);
                    render_buckets.draw_string(
                        Layer::Text,
                        render_x + padding_x + left_gutter_width,
                        dst_y,
                        (&chars[0..text_len]).to_owned(),
                    );
                }

                if self.current_cell == Pos::from_row_column(row_i, col_i) {
                    if self.editor.is_cursor_shown() {
                        render_buckets.set_color(Layer::Text, 0x000000_FF);
                        render_buckets.draw_char(
                            Layer::Text,
                            (self.editor.get_selection().get_cursor_pos().column
                                + left_gutter_width)
                                + render_x
                                + padding_x,
                            dst_y,
                            '▏',
                        );
                    }
                }
            }
            render_x += if col_i + 1 < self.col_count {
                max_width + MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT
            } else {
                max_width
            };
        }

        render_matrix_right_brackets(
            render_x + left_gutter_width,
            render_y,
            self.row_count,
            render_buckets,
            vert_align_offset,
        );

        render_x += 1;
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
    row: EditorY,
    start_x: usize,
    end_x: usize,
    rendered_x: usize,
    rendered_y: RenderPosY,
    rendered_w: usize,
    rendered_h: usize,
}

#[derive(Debug)]
pub struct Variable {
    pub name: Box<[char]>,
    pub value: Result<CalcResult, ()>,
}

type LineResult = Result<Option<CalcResult>, ()>;
type Variables = [Option<Variable>];

pub struct Tokens<'a> {
    tokens: Vec<Token<'a>>,
    shunting_output_stack: Vec<TokenType>,
}

pub enum MouseState {
    ClickedInEditor,
    ClickedInScrollBar {
        original_click_y: RenderPosY,
        original_scroll_y: usize,
    },
    RightGutterIsDragged,
}

pub struct NoteCalcApp {
    pub client_width: usize,
    pub editor: Editor,
    pub editor_content: EditorContent<LineData>,
    pub matrix_editing: Option<MatrixEditing>,
    pub line_reference_chooser: Option<EditorY>,
    pub line_id_generator: usize,
    pub mouse_state: Option<MouseState>,
    pub result_area_redraw: EditorRowFlags,
    pub editor_area_redraw: EditorRowFlags,
    pub render_data: GlobalRenderData,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum RedrawTarget {
    EditorArea,
    ResultArea,
    Both,
}

impl NoteCalcApp {
    pub fn new(client_width: usize, client_height: usize) -> NoteCalcApp {
        let mut editor_content = EditorContent::new(MAX_EDITOR_WIDTH);
        NoteCalcApp {
            line_reference_chooser: None,
            client_width,
            editor: Editor::new(&mut editor_content),
            editor_content,
            matrix_editing: None,
            line_id_generator: 1,
            mouse_state: None,
            result_area_redraw: EditorRowFlags::all_rows_starting_at(0),
            editor_area_redraw: EditorRowFlags::all_rows_starting_at(0),
            render_data: GlobalRenderData::new(
                client_width,
                client_height,
                calc_result_gutter_x(None, client_width),
                LEFT_GUTTER_WIDTH,
                RIGHT_GUTTER_WIDTH,
            ),
        }
    }

    pub fn set_normalized_content<'b>(
        &mut self,
        mut text: &str,
        units: &Units,
        allocator: &'b Arena<char>,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
    ) {
        if text.is_empty() {
            text = "\n\n\n\n\n\n\n\n\n\n";
        }
        self.editor_content.set_content(text);
        self.editor.set_cursor_pos_r_c(0, 0);
        for (i, data) in self.editor_content.data_mut().iter_mut().enumerate() {
            data.line_id = i + 1;
        }
        self.line_id_generator = self.editor_content.line_count() + 1;

        for r in results.as_mut_slice() {
            *r = Ok(None)
        }
        for v in vars.iter_mut() {
            *v = None;
        }
        vars[SUM_VARIABLE_INDEX] = Some(Variable {
            name: Box::from(&['s', 'u', 'm'][..]),
            value: Err(()),
        });
        self.render_data.clear();
        self.update_tokens_and_redraw_requirements(
            RowModificationType::AllLinesFrom(0),
            units,
            allocator,
            tokens,
            results,
            vars,
        );
    }

    pub fn renderr<'a, 'b>(
        editor: &mut Editor,
        editor_content: &EditorContent<LineData>,
        units: &Units,
        matrix_editing: &mut Option<MatrixEditing>,
        line_reference_chooser: &mut Option<EditorY>,
        render_buckets: &mut RenderBuckets<'a>,
        result_buffer: &'a mut [u8],
        redraw_editor_area: EditorRowFlags,
        redraw_result_area: EditorRowFlags,
        gr: &mut GlobalRenderData,
        allocator: &'a Arena<char>,
        //  TODO csak az editor objectet add át mut-ként
        // mivel azt itt egyszeröbb kitölteni (tudni kell hozzá
        // a renderelt magasságot és szélességet)
        tokens: &AppTokens<'a>,
        results: &Results,
        vars: &Variables,
        editor_objs: &mut EditorObjects,
    ) {
        // x, h
        let mut rerendered_lines: SmallVec<[((RenderPosY, usize), RedrawTarget); MAX_LINE_COUNT]> =
            SmallVec::with_capacity(MAX_LINE_COUNT);

        let scroll_coeff = gr.client_height as f64 / editor_content.line_count() as f64;
        if scroll_coeff < 1.0 {
            let scroll_bar_h = gr.client_height
                - (editor_content.line_count() - gr.client_height).min(gr.client_height - 1);
            render_buckets.set_color(Layer::BehindText, 0xFFCCCC_FF);
            render_buckets.draw_rect(
                Layer::BehindText,
                gr.result_gutter_x - SCROLL_BAR_WIDTH,
                RenderPosY::new(gr.scroll_y),
                SCROLL_BAR_WIDTH,
                scroll_bar_h,
            );
        }

        // TODO: kell ez?
        {
            let mut r = PerLineRenderData::new();
            for line in editor_content.lines().take(MAX_LINE_COUNT) {
                r.new_line_started();
                let editor_y = r.editor_y;

                if gr.scroll_y > editor_y.as_usize()
                    || editor_y.as_usize() >= gr.scroll_y + gr.client_height
                {
                    gr.set_render_y(editor_y, None);
                    // TODO ezt nem lehetne kódban kifejezni valahogy?
                    // TILOS, ezt nem nullázhatod le, ez automatikusan változik
                    // ha változik 1 sor tartalma és újraszámolódnak a tokenjei
                    // gr.set_rendered_height(editor_y, 0);
                    r.inc_editor_y();
                    continue;
                }

                let render_y = r.render_y;
                gr.set_render_y(editor_y, Some(render_y));
                r.rendered_row_height = gr.get_rendered_height(editor_y);
                // "- 1" so if it is even, it always appear higher
                r.vert_align_offset = (r.rendered_row_height - 1) / 2;

                if redraw_editor_area.need(editor_y) && redraw_result_area.need(editor_y) {
                    rerendered_lines.push(((render_y, r.rendered_row_height), RedrawTarget::Both));
                } else if redraw_result_area.need(editor_y) {
                    rerendered_lines
                        .push(((render_y, r.rendered_row_height), RedrawTarget::ResultArea));
                } else if redraw_editor_area.need(editor_y) {
                    rerendered_lines
                        .push(((render_y, r.rendered_row_height), RedrawTarget::EditorArea));
                }

                if redraw_editor_area.need(editor_y) {
                    highlight_current_line(render_buckets, &r, editor, &gr, redraw_result_area);

                    if let Some(tokens) = &tokens[editor_y] {
                        let need_matrix_renderer =
                            if let Some((first, second)) = editor.get_selection().is_range() {
                                !(first.row..=second.row).contains(&(editor_y.as_usize()))
                            } else {
                                true
                            };
                        // Todo: refactor the parameters into a struct
                        render_tokens(
                            &tokens.tokens,
                            &mut r,
                            gr,
                            render_buckets,
                            // TODO &mut code smell
                            &mut editor_objs[editor_y],
                            editor,
                            matrix_editing,
                            vars,
                            &units,
                            need_matrix_renderer,
                            Some(4),
                        );
                        // TODO extract
                        for editor_obj in editor_objs[editor_y].iter() {
                            if matches!(editor_obj.typ, EditorObjectType::LineReference) {
                                let vert_align_offset =
                                    (r.rendered_row_height - editor_obj.rendered_h) / 2;
                                render_buckets.set_color(Layer::BehindText, 0xFFCCCC_FF);
                                render_buckets.draw_rect(
                                    Layer::BehindText,
                                    gr.left_gutter_width + editor_obj.rendered_x,
                                    editor_obj.rendered_y.add(vert_align_offset),
                                    editor_obj.rendered_w,
                                    editor_obj.rendered_h,
                                );
                            }
                        }
                    } else {
                        r.rendered_row_height = 1;
                        render_simple_text_line(line, &mut r, gr, render_buckets, allocator);
                    }
                    render_wrap_dots(render_buckets, &r, &gr);

                    draw_line_ref_chooser(
                        render_buckets,
                        &r,
                        &gr,
                        &line_reference_chooser,
                        gr.result_gutter_x,
                    );

                    draw_cursor(render_buckets, &r, &gr, &editor, &matrix_editing);

                    draw_right_gutter_num_prefixes(
                        render_buckets,
                        gr.result_gutter_x,
                        &editor_content,
                        &r,
                    );
                    // result gutter
                    render_buckets.set_color(Layer::BehindText, 0xD2D2D2_FF);
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        gr.result_gutter_x,
                        render_y,
                        RIGHT_GUTTER_WIDTH,
                        r.rendered_row_height,
                    );

                    // line number
                    {
                        render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
                        render_buckets.draw_rect(
                            Layer::BehindText,
                            0,
                            render_y,
                            gr.left_gutter_width,
                            r.rendered_row_height,
                        );
                        if editor_y.as_usize() == editor.get_selection().get_cursor_pos().row {
                            render_buckets.set_color(Layer::Text, 0x000000_FF);
                        } else {
                            render_buckets.set_color(Layer::Text, 0xADADAD_FF);
                        }
                        let vert_align_offset = (r.rendered_row_height - 1) / 2;
                        render_buckets.draw_text(
                            Layer::Text,
                            1,
                            render_y.add(vert_align_offset),
                            &(LINE_NUM_CONSTS[editor_y.as_usize()][..]),
                        );
                    }
                } else if redraw_result_area.need(editor_y) {
                    draw_right_gutter_num_prefixes(
                        render_buckets,
                        gr.result_gutter_x,
                        &editor_content,
                        &r,
                    );
                    // result background
                    render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        gr.result_gutter_x + RIGHT_GUTTER_WIDTH,
                        render_y,
                        gr.current_result_panel_width,
                        r.rendered_row_height,
                    );
                    // result gutter
                    render_buckets.set_color(Layer::BehindText, 0xD2D2D2_FF);
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        gr.result_gutter_x,
                        render_y,
                        RIGHT_GUTTER_WIDTH,
                        r.rendered_row_height,
                    );
                }
                r.line_render_ended(gr.get_rendered_height(editor_y));
            }
        }

        render_buckets
            .clear_commands
            .push(OutputMessage::SetColor(0xFFFFFF_FF));

        // clear the whole scrollbar area
        render_buckets
            .clear_commands
            .push(OutputMessage::RenderRectangle {
                x: gr.result_gutter_x - SCROLL_BAR_WIDTH,
                y: RenderPosY::new(0),
                w: SCROLL_BAR_WIDTH,
                h: gr.client_height,
            });

        render_selection_and_its_sum(
            &units,
            render_buckets,
            results,
            &editor,
            &editor_content,
            &gr,
            vars,
            allocator,
        );

        render_results(
            &units,
            render_buckets,
            results.as_slice(),
            result_buffer,
            &editor_content,
            &gr,
            gr.result_gutter_x,
            redraw_result_area,
            Some(4),
        );

        clear_rerendered_lines(render_buckets, gr, &rerendered_lines);

        pulse_rerendered_lines(render_buckets, gr, &rerendered_lines);

        // CLEAR area below the editor
        let last_row_i = editor_content.line_count();
        clear_bottom_due_to_line_removal(last_row_i, render_buckets, gr);
        clear_bottom_due_to_line_shrinking(last_row_i, render_buckets, gr);
    }

    pub fn handle_mouse_up(&mut self, _x: usize, _y: usize) {
        self.mouse_state = None;
    }

    pub fn handle_wheel(&mut self, dir: usize) {
        if dir == 0 && self.render_data.scroll_y > 0 {
            self.render_data.scroll_y -= 1;
            self.set_redraw_flag(EditorRowFlags::all_rows_starting_at(0), RedrawTarget::Both);
        } else if dir == 1
            && (self.render_data.scroll_y + self.render_data.client_height)
                < self.editor_content.line_count()
        {
            self.render_data.scroll_y += 1;
            self.set_redraw_flag(EditorRowFlags::all_rows_starting_at(0), RedrawTarget::Both);
        }
    }

    pub fn set_redraw_flag<'b>(&mut self, flags: EditorRowFlags, target: RedrawTarget) {
        match target {
            RedrawTarget::Both => {
                self.result_area_redraw.merge(flags);
                self.editor_area_redraw.merge(flags);
            }
            RedrawTarget::EditorArea => {
                self.editor_area_redraw.merge(flags);
            }
            RedrawTarget::ResultArea => {
                self.result_area_redraw.merge(flags);
            }
        }
    }

    pub fn handle_click<'b>(
        &mut self,
        x: usize,
        clicked_y: RenderPosY,
        editor_objs: &EditorObjects,
        units: &Units,
        allocator: &'b Arena<char>,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
    ) {
        let scroll_bar_x = self.render_data.result_gutter_x - SCROLL_BAR_WIDTH;
        if x < LEFT_GUTTER_WIDTH {
            // clicked on left gutter
        } else if x < scroll_bar_x {
            self.handle_editor_area_click(x, clicked_y, editor_objs);
        } else if self.mouse_state.is_none() {
            self.mouse_state = if x - scroll_bar_x < SCROLL_BAR_WIDTH {
                Some(MouseState::ClickedInScrollBar {
                    original_click_y: clicked_y,
                    original_scroll_y: self.render_data.scroll_y,
                })
            } else if x - self.render_data.result_gutter_x < RIGHT_GUTTER_WIDTH {
                Some(MouseState::RightGutterIsDragged)
            } else {
                // clicked in result
                if let Some(editor_y) = self.rendered_y_to_editor_y(clicked_y) {
                    self.insert_line_ref(units, allocator, tokens, results, vars, editor_y);
                }
                None
            };
        }
    }

    fn handle_editor_area_click(
        &mut self,
        x: usize,
        clicked_y: RenderPosY,
        editor_objs: &EditorObjects,
    ) {
        let clicked_x = x - LEFT_GUTTER_WIDTH;
        let prev_selection = self.editor.get_selection();
        let editor_click_pos = if let Some(editor_obj) =
            self.get_obj_at_rendered_pos(clicked_x, clicked_y, editor_objs)
        {
            match editor_obj.typ {
                EditorObjectType::LineReference => Some(Pos::from_row_column(
                    editor_obj.row.as_usize(),
                    editor_obj.end_x,
                )),
                EditorObjectType::Matrix {
                    row_count,
                    col_count,
                } => {
                    if self.matrix_editing.is_some() {
                        end_matrix_editing(
                            &mut self.matrix_editing,
                            &mut self.editor,
                            &mut self.editor_content,
                            None,
                        );
                    } else {
                        self.matrix_editing = Some(MatrixEditing::new(
                            row_count,
                            col_count,
                            &self
                                .editor_content
                                .get_line_valid_chars(editor_obj.row.as_usize())
                                [editor_obj.start_x..editor_obj.end_x],
                            editor_obj.row,
                            editor_obj.start_x,
                            editor_obj.end_x,
                            Pos::from_row_column(0, 0),
                        ))
                    }
                    None
                }
                EditorObjectType::SimpleTokens => {
                    let x_pos_within = clicked_x - editor_obj.rendered_x;
                    Some(Pos::from_row_column(
                        editor_obj.row.as_usize(),
                        editor_obj.start_x + x_pos_within,
                    ))
                }
            }
        } else {
            self.rendered_y_to_editor_y(clicked_y)
                .map(|it| Pos::from_row_column(it.as_usize(), clicked_x))
        };

        if let Some(editor_click_pos) = editor_click_pos {
            if self.matrix_editing.is_some() {
                end_matrix_editing(
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

            let flag = self.calc_rerender_for_cursor_movement(prev_selection, false);

            self.set_redraw_flag(flag, RedrawTarget::Both);
            self.editor.blink_cursor();
        }
        if self.mouse_state.is_none() {
            self.mouse_state = Some(MouseState::ClickedInEditor);
        }
    }

    pub fn rendered_y_to_editor_y(&self, clicked_y: RenderPosY) -> Option<EditorY> {
        for (ed_y, r_y) in self.render_data.editor_y_to_render_y().iter().enumerate() {
            if r_y.map(|it| it == clicked_y).unwrap_or(false) {
                return Some(EditorY::new(ed_y));
            } else if r_y.map(|it| it > clicked_y).unwrap_or(false) {
                return Some(EditorY::new(ed_y - 1));
            }
        }
        return None;
    }

    pub fn get_obj_at_rendered_pos<'a>(
        &self,
        x: usize,
        render_y: RenderPosY,
        editor_objects: &'a EditorObjects,
    ) -> Option<&'a EditorObject> {
        if let Some(editor_y) = self.rendered_y_to_editor_y(render_y) {
            editor_objects[editor_y].iter().find(|editor_obj| {
                (editor_obj.rendered_x..editor_obj.rendered_x + editor_obj.rendered_w).contains(&x)
                    && (editor_obj.rendered_y.as_usize()
                        ..editor_obj.rendered_y.as_usize() + editor_obj.rendered_h)
                        .contains(&render_y.as_usize())
            })
        } else {
            None
        }
    }

    pub fn handle_drag(&mut self, x: usize, y: RenderPosY) {
        match self.mouse_state {
            Some(MouseState::RightGutterIsDragged) => {
                self.set_result_gutter_x(
                    self.client_width,
                    calc_result_gutter_x(Some(x), self.client_width),
                );
            }
            Some(MouseState::ClickedInEditor) => {
                if let Some(y) = self.rendered_y_to_editor_y(y) {
                    self.editor.handle_drag(
                        x - LEFT_GUTTER_WIDTH,
                        y.as_usize(),
                        &self.editor_content,
                    );
                    self.editor.blink_cursor();
                }
            }
            Some(MouseState::ClickedInScrollBar {
                original_click_y,
                original_scroll_y,
            }) => {
                let gr = &mut self.render_data;
                let scroll_coeff =
                    gr.client_height as f64 / self.editor_content.line_count() as f64;
                if scroll_coeff < 1.0 {
                    let delta_y = y.as_usize() as isize - original_click_y.as_usize() as isize;

                    // gr.scroll_y = y / scroll_bar_step;
                    gr.scroll_y = (original_scroll_y as isize + delta_y).max(0) as usize;
                }
            }
            None => {}
        }
    }

    pub fn set_result_gutter_x(&mut self, client_width: usize, x: usize) {
        self.render_data.result_gutter_x = x;
        self.render_data.current_editor_width = x - self.render_data.left_gutter_width;
        self.render_data.current_result_panel_width = client_width - x - RIGHT_GUTTER_WIDTH;
        self.set_redraw_flag(EditorRowFlags::all_rows_starting_at(0), RedrawTarget::Both);
    }

    pub fn handle_resize(&mut self, new_client_width: usize) {
        self.client_width = new_client_width;
        self.set_result_gutter_x(
            new_client_width,
            calc_result_gutter_x(Some(self.render_data.result_gutter_x), new_client_width),
        );
    }

    pub fn handle_time(&mut self, now: u32) -> bool {
        let need_rerender = if let Some(mat_editor) = &mut self.matrix_editing {
            mat_editor.editor.handle_tick(now)
        } else {
            self.editor.handle_tick(now)
        };
        if need_rerender {
            let (from, to) = self.editor.get_selection().get_range();
            self.set_redraw_flag(
                EditorRowFlags::range(from.row, to.row),
                RedrawTarget::EditorArea,
            );
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

    pub fn alt_key_released<'b>(
        &mut self,
        units: &Units,
        allocator: &'b Arena<char>,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
    ) {
        if self.line_reference_chooser.is_none() {
            return;
        }

        let line_ref_row = self.line_reference_chooser.unwrap();

        self.insert_line_ref(units, allocator, tokens, results, vars, line_ref_row);
        self.set_redraw_flag(
            EditorRowFlags::single_row(line_ref_row.as_usize()),
            RedrawTarget::Both,
        );

        self.line_reference_chooser = None;
    }

    pub fn insert_line_ref<'b>(
        &mut self,
        units: &Units,
        allocator: &'b Arena<char>,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        line_ref_row: EditorY,
    ) {
        let cursor_row = self.editor.get_selection().get_cursor_pos().row;
        if cursor_row == line_ref_row.as_usize()
            || matches!(&results[line_ref_row], Err(_) | Ok(None))
        {
            return;
        }
        let line_id = {
            let line_data = self.editor_content.get_data(line_ref_row.as_usize());
            line_data.line_id
        };
        // TODO STATIC_LINE_IDS[line_id]
        let inserting_text = format!("&[{}]", line_id);
        self.editor
            .insert_text(&inserting_text, &mut self.editor_content);

        self.update_tokens_and_redraw_requirements(
            RowModificationType::SingleLine(cursor_row),
            units,
            allocator,
            tokens,
            results,
            vars,
        );
    }

    pub fn handle_paste<'b>(
        &mut self,
        text: String,
        units: &Units,
        allocator: &'b Arena<char>,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
    ) {
        match self.editor.insert_text(&text, &mut self.editor_content) {
            Some(modif) => {
                self.update_tokens_and_redraw_requirements(
                    modif, units, allocator, tokens, results, vars,
                );
            }
            None => {}
        };
    }

    pub fn calc_rerender_for_cursor_movement<'b>(
        &mut self,
        prev_selection: Selection,
        scrolled: bool,
    ) -> EditorRowFlags {
        let cursor_pos = self.editor.get_selection().get_cursor_pos();
        let new_cursor_y = cursor_pos.row;
        let redraw = if scrolled {
            EditorRowFlags::all_rows_starting_at(0)
        } else {
            let flag = match (
                prev_selection.is_range(),
                self.editor.get_selection().is_range(),
            ) {
                (Some((old_first, old_second)), Some((new_first, new_second))) => {
                    let was_single_line_sel = old_first.row == old_second.row;
                    let is_single_line_sel = new_first.row == new_second.row;
                    match (was_single_line_sel, is_single_line_sel) {
                        (true, true) => clear_single_line_selection_rows(new_second),
                        (true, false) => {
                            // was single line selection but expanded to multiline
                            let mut flag = clear_single_line_selection_rows(old_second);
                            flag.merge(EditorRowFlags::range(new_first.row, new_second.row));
                            flag
                        }
                        (false, true) => {
                            // was multi line selection but reduced to singleline
                            let mut flag = clear_single_line_selection_rows(new_second);
                            flag.merge(EditorRowFlags::range(old_first.row, old_second.row));
                            flag
                        }
                        (false, false) => {
                            let from = old_first.row.min(new_cursor_y);
                            let to = old_second.row.max(new_cursor_y);
                            EditorRowFlags::range(from, to)
                        }
                    }
                }
                (Some((old_first, old_second)), None) => {
                    // a selection was cancelled
                    let was_single_line_sel = old_first.row == old_second.row;
                    let mut flag = if was_single_line_sel {
                        clear_single_line_selection_rows(old_second)
                    } else {
                        // multiline selection was cancelled
                        EditorRowFlags::range(old_first.row, old_second.row)
                    };
                    flag.merge(EditorRowFlags::single_row(new_cursor_y));
                    flag
                }
                (None, Some((new_first, new_second))) => {
                    let is_single_line_sel = new_first.row == new_second.row;
                    let flag = if is_single_line_sel {
                        EditorRowFlags::single_row(new_cursor_y)
                    } else {
                        EditorRowFlags::range(new_first.row, new_second.row)
                    };
                    flag
                }
                (None, None) => {
                    let prev_cursor_pos = prev_selection.get_cursor_pos();
                    EditorRowFlags::multiple(&[prev_cursor_pos.row, new_cursor_y])
                }
            };
            flag
        };
        return redraw;
    }

    pub fn handle_input_and_update_tokens_plus_redraw_requirements<'b>(
        &mut self,
        input: EditorInputEvent,
        modifiers: InputModifiers,
        allocator: &'b Arena<char>,
        units: &Units,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        editor_objs: &mut EditorObjects,
    ) -> Option<RowModificationType> {
        struct InputResult {
            modif: Option<RowModificationType>,
            redraw: Option<(EditorRowFlags, RedrawTarget)>,
        };
        let cur_row = self.editor.get_selection().get_cursor_pos().row;
        // TODO EXTRACT alt_handling
        let input_res: InputResult = if self.matrix_editing.is_none() && modifiers.alt {
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
                InputResult {
                    modif: None,
                    redraw: Some((
                        EditorRowFlags::range(start.row, end.row),
                        RedrawTarget::ResultArea,
                    )),
                }
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
                InputResult {
                    modif: None,
                    redraw: Some((
                        EditorRowFlags::range(start.row, end.row),
                        RedrawTarget::ResultArea,
                    )),
                }
            } else if input == EditorInputEvent::Up {
                let cur_pos = self.editor.get_selection().get_cursor_pos();
                let rows = if let Some(selector_row) = self.line_reference_chooser {
                    if selector_row.as_usize() > 0 {
                        Some((selector_row.as_usize(), selector_row.as_usize() - 1))
                    } else {
                        Some((selector_row.as_usize(), selector_row.as_usize()))
                    }
                } else if cur_pos.row > 0 {
                    Some((cur_pos.row - 1, cur_pos.row - 1))
                } else {
                    None
                };
                if let Some((prev_selected_row, new_selected_row)) = rows {
                    self.line_reference_chooser = Some(EditorY::new(new_selected_row));
                    InputResult {
                        modif: None,
                        redraw: Some((
                            EditorRowFlags::range(new_selected_row, prev_selected_row),
                            RedrawTarget::Both,
                        )),
                    }
                } else {
                    InputResult {
                        modif: None,
                        redraw: None,
                    }
                }
            } else if input == EditorInputEvent::Down {
                let cur_pos = self.editor.get_selection().get_cursor_pos();
                let rows = if let Some(selector_row) = self.line_reference_chooser {
                    if selector_row.as_usize() < cur_pos.row - 1 {
                        Some((selector_row.as_usize(), selector_row.as_usize() + 1))
                    } else {
                        Some((selector_row.as_usize(), selector_row.as_usize()))
                    }
                } else {
                    None
                };
                if let Some((prev_selected_row, new_selected_row)) = rows {
                    self.line_reference_chooser = Some(EditorY::new(new_selected_row));
                    InputResult {
                        modif: None,
                        redraw: Some((
                            EditorRowFlags::range(prev_selected_row, new_selected_row),
                            RedrawTarget::Both,
                        )),
                    }
                } else {
                    InputResult {
                        modif: None,
                        redraw: None,
                    }
                }
            } else {
                InputResult {
                    modif: None,
                    redraw: None,
                }
            }
        } else if self.matrix_editing.is_some() {
            let prev_row = cur_row;
            self.handle_matrix_editor_input(input, modifiers);
            if self.matrix_editing.is_none() {
                // user left a matrix
                let new_row = cur_row;

                InputResult {
                    modif: Some(RowModificationType::SingleLine(prev_row)),
                    redraw: Some((EditorRowFlags::single_row(new_row), RedrawTarget::Both)),
                }
            } else {
                let mut redraw_flags = EditorRowFlags::single_row(cur_row);
                if modifiers.alt {
                    let y = EditorY::new(cur_row);
                    let old_h = self.render_data.get_rendered_height(y);
                    let new_h =
                        calc_rendered_height(y, &self.matrix_editing, tokens, results, vars);
                    self.render_data.set_rendered_height(y, new_h);
                    if new_h != old_h {
                        redraw_flags.merge(EditorRowFlags::all_rows_starting_at(cur_row))
                    }
                };
                InputResult {
                    modif: None,
                    redraw: Some((redraw_flags, RedrawTarget::Both)),
                }
            }
        } else {
            if self.handle_completion(&input, editor_objs, vars) {
                InputResult {
                    modif: Some(RowModificationType::SingleLine(cur_row)),
                    redraw: None,
                }
            } else if let Some(modif_type) = self.handle_obj_deletion(&input, editor_objs) {
                InputResult {
                    modif: Some(modif_type),
                    redraw: None,
                }
            } else if self.handle_obj_jump_over(&input, modifiers, editor_objs) {
                InputResult {
                    modif: None,
                    redraw: Some((
                        EditorRowFlags::single_row(cur_row),
                        RedrawTarget::EditorArea,
                    )),
                }
            } else {
                let prev_selection = self.editor.get_selection();
                let prev_cursor_pos = prev_selection.get_cursor_pos();

                let modif_type =
                    self.editor
                        .handle_input(input, modifiers, &mut self.editor_content);

                if modif_type.is_none() {
                    // it is possible to step into a matrix only through navigation
                    self.check_stepping_into_matrix(prev_cursor_pos, editor_objs);
                }

                let cursor_pos = self.editor.get_selection().get_cursor_pos();
                let scrolled = if prev_cursor_pos.row != cursor_pos.row {
                    let scrolled = if cursor_pos.row < self.render_data.scroll_y {
                        // scroll up
                        self.render_data.scroll_y = cursor_pos.row;
                        true
                    } else if (cursor_pos.row - self.render_data.scroll_y)
                        >= self.render_data.client_height
                    {
                        // scroll down
                        self.render_data.scroll_y = (cursor_pos.row
                            - (self.render_data.client_height - 1))
                            .min(self.render_data.client_height - 1);
                        true
                    } else {
                        false
                    };
                    scrolled
                } else {
                    false
                };
                match modif_type {
                    Some(r) => InputResult {
                        modif: Some(r),
                        redraw: if scrolled {
                            Some((EditorRowFlags::all_rows_starting_at(0), RedrawTarget::Both))
                        } else {
                            if let Some((start, end)) = prev_selection.is_range() {
                                if start.row == end.row {
                                    Some((
                                        clear_single_line_selection_rows(start),
                                        RedrawTarget::Both,
                                    ))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        },
                    },
                    None => {
                        let flag = self.calc_rerender_for_cursor_movement(prev_selection, scrolled);
                        InputResult {
                            modif: None,
                            redraw: Some((flag, RedrawTarget::Both)),
                        }
                    }
                }
            }
        };

        if let Some(modif) = input_res.modif {
            self.update_tokens_and_redraw_requirements(
                modif, units, allocator, tokens, results, vars,
            );
        }

        if let Some((flags, target)) = &input_res.redraw {
            self.set_redraw_flag(*flags, *target);
        }

        return input_res.modif;
    }

    pub fn update_tokens_and_redraw_requirements<'b>(
        &mut self,
        input_effect: RowModificationType,
        units: &Units,
        allocator: &'b Arena<char>,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
    ) {
        fn eval_line<'b>(
            editor_content: &EditorContent<LineData>,
            line: &[char],
            units: &Units,
            allocator: &'b Arena<char>,
            tokens_per_lines: &mut AppTokens<'b>,
            results: &mut Results,
            vars: &mut Variables,
            editor_y: EditorY,
        ) -> EditorRowFlags {
            dbg!(&vars);
            // TODO avoid clone
            let prev_var_name = vars[editor_y.as_usize()].as_ref().map(|it| it.name.clone());

            dbg!(&vars);
            tokens_per_lines[editor_y] =
                parse_tokens(line, editor_y.as_usize(), units, &*vars, allocator);
            if let Some(tokens) = &mut tokens_per_lines[editor_y] {
                let result = evaluate_tokens_and_save_result(
                    &mut *vars,
                    editor_y.as_usize(),
                    editor_content,
                    &mut tokens.shunting_output_stack,
                    editor_content.get_line_valid_chars(editor_y.as_usize()),
                );
                let result = result.map(|it| it.map(|it| it.result));
                results[editor_y] = result;
            } else {
                results[editor_y] = Ok(None);
            }

            let mut rows_to_recalc = find_line_ref_dependant_lines(
                editor_content,
                tokens_per_lines,
                editor_y.as_usize(),
            );

            let curr_var_name = vars[editor_y.as_usize()].as_ref().map(|it| &it.name);
            rows_to_recalc.merge(find_variable_dependant_lines(
                curr_var_name,
                prev_var_name,
                tokens_per_lines,
                editor_y.as_usize(),
            ));
            return rows_to_recalc;
        }

        fn find_line_ref_dependant_lines<'b>(
            editor_content: &EditorContent<LineData>,
            tokens_per_lines: &AppTokens<'b>,
            editor_y: usize,
        ) -> EditorRowFlags {
            let mut rows_to_recalc = EditorRowFlags::empty();
            let line_data = editor_content.get_data(editor_y);
            let line_ref_name = &STATIC_LINE_IDS[line_data.line_id];
            for (i, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
                if let Some(tokens) = tokens {
                    for token in &tokens.tokens {
                        match token.typ {
                            TokenType::LineReference { .. } if token.ptr == *line_ref_name => {
                                rows_to_recalc.merge(EditorRowFlags::single_row(editor_y + 1 + i));
                            }
                            _ => {}
                        }
                    }
                }
            }
            return rows_to_recalc;
        }

        fn find_variable_dependant_lines<'b>(
            curr_var_name: Option<&Box<[char]>>,
            prev_var_name: Option<Box<[char]>>,
            tokens_per_lines: &AppTokens<'b>,
            editor_y: usize,
        ) -> EditorRowFlags {
            let mut rows_to_recalc = EditorRowFlags::empty();
            match (prev_var_name, curr_var_name) {
                (None, Some(var_name)) => {
                    // nem volt még, de most van
                    // recalc all the rows which uses this variable name
                    for (i, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
                        if let Some(tokens) = tokens {
                            for token in &tokens.tokens {
                                match token.typ {
                                    TokenType::StringLiteral if *token.ptr == **var_name => {
                                        rows_to_recalc
                                            .merge(EditorRowFlags::single_row(editor_y + 1 + i));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                (Some(old_var_name), None) => {
                    // volt, de most nincs
                    // recalc all the rows which uses the old variable name
                    for (i, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
                        if let Some(tokens) = tokens {
                            for token in &tokens.tokens {
                                match token.typ {
                                    TokenType::Variable { .. } if *token.ptr == *old_var_name => {
                                        rows_to_recalc
                                            .merge(EditorRowFlags::single_row(editor_y + 1 + i));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                (Some(old_var_name), Some(var_name)) if old_var_name != *var_name => {
                    // volt, de most más a neve
                    for (i, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
                        if let Some(tokens) = tokens {
                            for token in &tokens.tokens {
                                let recalc = match token.typ {
                                    TokenType::StringLiteral if *token.ptr == **var_name => true,
                                    TokenType::Variable { .. } if *token.ptr == *old_var_name => {
                                        true
                                    }
                                    _ => false,
                                };
                                if recalc {
                                    rows_to_recalc
                                        .merge(EditorRowFlags::single_row(editor_y + 1 + i));
                                }
                            }
                        }
                    }
                }
                (Some(_old_var_name), Some(var_name)) => {
                    // volt is, van is, a neve is ugyanaz, ugyanaz a neve
                    for (i, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
                        if let Some(tokens) = tokens {
                            for token in &tokens.tokens {
                                let recalc = match token.typ {
                                    TokenType::Variable { .. } if *token.ptr == **var_name => true,
                                    _ => false,
                                };
                                if recalc {
                                    rows_to_recalc
                                        .merge(EditorRowFlags::single_row(editor_y + 1 + i));
                                }
                            }
                        }
                    }
                }
                (None, None) => {}
            }
            return rows_to_recalc;
        }

        let mut sum_is_null = true;
        let mut dependant_rows = EditorRowFlags::empty();
        for editor_y in 0..self.editor_content.line_count() {
            let recalc = match input_effect {
                RowModificationType::SingleLine(to_change_index) if to_change_index == editor_y => {
                    true
                }
                RowModificationType::AllLinesFrom(to_change_index_from)
                    if editor_y >= to_change_index_from =>
                {
                    true
                }
                _ => dependant_rows.need(EditorY::new(editor_y)),
            };
            if recalc {
                if self.editor_content.get_data(editor_y).line_id == 0 {
                    self.editor_content.mut_data(editor_y).line_id = self.line_id_generator;
                    self.line_id_generator += 1;
                }
                let y = EditorY::new(editor_y);
                let rows_to_recalc = eval_line(
                    &self.editor_content,
                    self.editor_content.get_line_valid_chars(editor_y),
                    units,
                    allocator,
                    tokens,
                    results,
                    &mut *vars,
                    y,
                );
                dependant_rows.merge(rows_to_recalc);
                self.set_redraw_flag(rows_to_recalc, RedrawTarget::Both);
                let orig_h = self.render_data.get_rendered_height(y);
                let new_h = calc_rendered_height(y, &self.matrix_editing, tokens, results, vars);
                if new_h != orig_h {
                    // rerender everything below it
                    self.set_redraw_flag(
                        EditorRowFlags::all_rows_starting_at(editor_y),
                        RedrawTarget::Both,
                    );
                }
                self.render_data.set_rendered_height(y, new_h);
            }
            if self
                .editor_content
                .get_line_valid_chars(editor_y)
                .starts_with(&['-', '-'])
            {
                sum_is_null = true;
            }

            match &results[EditorY::new(editor_y)] {
                Ok(Some(result)) => {
                    sum_result(
                        vars[SUM_VARIABLE_INDEX]
                            .as_mut()
                            .expect("SUM always exists"),
                        result,
                        &mut sum_is_null,
                    );
                }
                Err(_) | Ok(None) => {}
            }
        }

        match input_effect {
            RowModificationType::SingleLine(to_change_index) => {
                self.set_redraw_flag(
                    EditorRowFlags::single_row(to_change_index),
                    RedrawTarget::Both,
                );
            }
            RowModificationType::AllLinesFrom(to_change_index_from) => {
                self.set_redraw_flag(
                    EditorRowFlags::all_rows_starting_at(to_change_index_from),
                    RedrawTarget::Both,
                );
            }
        }
    }

    pub fn copy_selected_rows_with_result_to_clipboard<'b>(
        &'b mut self,
        units: &'b Units,
        render_buckets: &'b mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
        token_char_alloator: &'b Arena<char>,
        results: &Results,
    ) -> String {
        let sel = self.editor.get_selection();
        let first_row = sel.get_first().row;
        let second_row = sel.get_second().row;
        let row_nums = second_row - first_row + 1;

        let vars = create_vars();
        let mut tokens = Vec::with_capacity(128);

        let mut gr = GlobalRenderData::new(1024, 1000 /*dummy value*/, 1024 / 2, 0, 2);
        // evaluate all the lines so variables are defined even if they are not selected
        let mut render_height = 0;
        {
            let mut r = PerLineRenderData::new();
            for (i, line) in self.editor_content.lines().enumerate() {
                let i = EditorY::new(i);
                // TODO "--"
                tokens.clear();
                TokenParser::parse_line(
                    line,
                    &vars[..],
                    &mut tokens,
                    &units,
                    i.as_usize(),
                    token_char_alloator,
                );

                let mut shunting_output_stack = Vec::with_capacity(32);
                ShuntingYard::shunting_yard(&mut tokens, &mut shunting_output_stack);

                if i.as_usize() >= first_row && i.as_usize() <= second_row {
                    r.new_line_started();
                    gr.set_render_y(r.editor_y, Some(r.render_y));

                    r.rendered_row_height = PerLineRenderData::calc_rendered_row_height(
                        &results[i],
                        &tokens,
                        &vars[..],
                        None,
                    );
                    // "- 1" so if it is even, it always appear higher
                    r.vert_align_offset = (r.rendered_row_height - 1) / 2;
                    gr.set_rendered_height(r.editor_y, r.rendered_row_height);
                    render_height += r.rendered_row_height;
                    // Todo: refactor the parameters into a struct
                    render_tokens(
                        &tokens,
                        &mut r,
                        &mut gr,
                        render_buckets,
                        // TODO &mut code smell
                        &mut Vec::new(),
                        &self.editor,
                        &self.matrix_editing,
                        // TODO &mut code smell
                        &vars[..],
                        &units,
                        true, // force matrix rendering
                        None,
                    );
                    r.line_render_ended(r.rendered_row_height);
                }
            }
        }

        // TODO what is this 256?
        let mut tmp_canvas: Vec<[char; 256]> = Vec::with_capacity(render_height);
        for _ in 0..render_height {
            tmp_canvas.push([' '; 256]);
        }
        // render all tokens to the tmp canvas, so we can measure the longest row
        render_buckets_into(&render_buckets, &mut tmp_canvas);
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

        //////////////////////////////////////////////////////////////////////////
        //////////////////////////////////////////////////////////////////////////
        render_buckets.clear();
        let result_gutter_x = max_len + 2;
        render_results(
            &units,
            render_buckets,
            &results.as_slice()[first_row..=second_row],
            result_buffer,
            &self.editor_content,
            &gr,
            result_gutter_x,
            EditorRowFlags::all_rows_starting_at(0),
            None,
        );
        for i in 0..render_height {
            render_buckets.draw_char(Layer::AboveText, result_gutter_x, RenderPosY::new(i), '█');
        }
        render_buckets_into(&render_buckets, &mut tmp_canvas);
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

    fn handle_completion<'b>(
        &mut self,
        input: &EditorInputEvent,
        editor_objects: &mut EditorObjects,
        vars: &Variables,
    ) -> bool {
        let cursor_pos = self.editor.get_selection();
        if *input == EditorInputEvent::Tab && cursor_pos.get_cursor_pos().column > 0 {
            // matrix autocompletion 'm' + tab
            let cursor_pos = cursor_pos.get_cursor_pos();
            let line = self.editor_content.get_line_valid_chars(cursor_pos.row);
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
                editor_objects[EditorY::new(cursor_pos.row)].push(EditorObject {
                    typ: EditorObjectType::Matrix {
                        row_count: 1,
                        col_count: 1,
                    },
                    row: EditorY::new(cursor_pos.row),
                    start_x: prev_col - 1,
                    end_x: prev_col + 2,
                    rendered_x: 0,                  // dummy
                    rendered_y: RenderPosY::new(0), // dummy
                    rendered_w: 3,
                    rendered_h: 1,
                });
                self.check_stepping_into_matrix(Pos::from_row_column(0, 0), &editor_objects);
                return true;
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
                let mut matched_var_index = None;
                for (var_i, var) in vars[0..cursor_pos.row].iter().enumerate() {
                    if var.is_none() {
                        continue;
                    }
                    let mut match_len = 0;
                    for (var_ch, actual_ch) in var
                        .as_ref()
                        .unwrap()
                        .name
                        .iter()
                        .zip(&line[begin_index..begin_index + expected_len])
                    {
                        if *var_ch != *actual_ch {
                            break;
                        }
                        match_len += 1;
                    }
                    if expected_len == match_len {
                        if matched_var_index.is_some() {
                            // multiple match, don't autocomplete
                            matched_var_index = None;
                            break;
                        } else {
                            matched_var_index = Some(var_i);
                        }
                    }
                }

                if let Some(matched_var_index) = matched_var_index {
                    for ch in vars[matched_var_index]
                        .as_ref()
                        .unwrap()
                        .name
                        .iter()
                        .skip(expected_len)
                    {
                        self.editor.handle_input(
                            EditorInputEvent::Char(*ch),
                            InputModifiers::none(),
                            &mut self.editor_content,
                        );
                    }
                    return true;
                }
            }
        }
        return false;
    }

    fn handle_obj_deletion<'b>(
        &mut self,
        input: &EditorInputEvent,
        editor_objects: &mut EditorObjects,
    ) -> Option<RowModificationType> {
        let selection = self.editor.get_selection();
        let cursor_pos = selection.get_cursor_pos();
        if *input == EditorInputEvent::Backspace
            && selection.is_range().is_none()
            && selection.start.column > 0
        {
            if let Some(index) =
                self.index_of_matrix_or_lineref_at(cursor_pos.with_prev_col(), editor_objects)
            {
                // remove it
                let obj = editor_objects[EditorY::new(cursor_pos.row)].remove(index);
                let sel = Selection::range(
                    Pos::from_row_column(obj.row.as_usize(), obj.start_x),
                    Pos::from_row_column(obj.row.as_usize(), obj.end_x),
                );
                self.editor.set_selection_save_col(sel);
                self.editor.handle_input(
                    EditorInputEvent::Backspace,
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                return if obj.rendered_h > 1 {
                    Some(RowModificationType::AllLinesFrom(cursor_pos.row))
                } else {
                    Some(RowModificationType::SingleLine(cursor_pos.row))
                };
            }
        } else if *input == EditorInputEvent::Del && selection.is_range().is_none() {
            if let Some(index) = self.index_of_matrix_or_lineref_at(cursor_pos, editor_objects) {
                // remove it
                let obj = editor_objects[EditorY::new(cursor_pos.row)].remove(index);
                let sel = Selection::range(
                    Pos::from_row_column(obj.row.as_usize(), obj.start_x),
                    Pos::from_row_column(obj.row.as_usize(), obj.end_x),
                );
                self.editor.set_selection_save_col(sel);
                self.editor.handle_input(
                    EditorInputEvent::Del,
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                return if obj.rendered_h > 1 {
                    Some(RowModificationType::AllLinesFrom(cursor_pos.row))
                } else {
                    Some(RowModificationType::SingleLine(cursor_pos.row))
                };
            }
        }
        return None;
    }

    fn handle_obj_jump_over<'b>(
        &mut self,
        input: &EditorInputEvent,
        modifiers: InputModifiers,
        editor_objects: &EditorObjects,
    ) -> bool {
        let selection = self.editor.get_selection();
        let cursor_pos = selection.get_cursor_pos();
        if *input == EditorInputEvent::Left
            && selection.is_range().is_none()
            && selection.start.column > 0
            && modifiers.shift == false
        {
            let obj = self
                .find_editor_object_at(cursor_pos.with_prev_col(), editor_objects)
                .map(|it| (it.typ, it.row, it.start_x));
            if let Some((obj_typ, row, start_x)) = obj {
                if obj_typ == EditorObjectType::LineReference {
                    //  jump over it
                    self.editor.set_cursor_pos_r_c(row.as_usize(), start_x);
                    return true;
                }
            }
        } else if *input == EditorInputEvent::Right
            && selection.is_range().is_none()
            && modifiers.shift == false
        {
            let obj = self
                .find_editor_object_at(cursor_pos, editor_objects)
                .map(|it| (it.typ, it.row, it.end_x));

            if let Some((obj_typ, row, end_x)) = obj {
                if obj_typ == EditorObjectType::LineReference {
                    //  jump over it
                    self.editor.set_cursor_pos_r_c(row.as_usize(), end_x);
                    return true;
                }
            }
        }
        return false;
    }

    fn check_stepping_into_matrix<'b>(
        &mut self,
        enter_from_pos: Pos,
        editor_objects: &EditorObjects,
    ) {
        if let Some(editor_obj) =
            is_pos_inside_an_obj(editor_objects, self.editor.get_selection().get_cursor_pos())
        {
            match editor_obj.typ {
                EditorObjectType::Matrix {
                    row_count,
                    col_count,
                } => {
                    if self.matrix_editing.is_none()
                        && self.editor.get_selection().is_range().is_none()
                    {
                        self.matrix_editing = Some(MatrixEditing::new(
                            row_count,
                            col_count,
                            &self
                                .editor_content
                                .get_line_valid_chars(editor_obj.row.as_usize())
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
        editor_objects: &'b EditorObjects,
    ) -> Option<&'b EditorObject> {
        for obj in &editor_objects[EditorY::new(pos.row)] {
            if (obj.start_x..obj.end_x).contains(&pos.column) {
                return Some(obj);
            }
        }
        return None;
    }

    fn index_of_matrix_or_lineref_at<'b>(
        &self,
        pos: Pos,
        editor_objects: &EditorObjects,
    ) -> Option<usize> {
        return editor_objects[EditorY::new(pos.row)]
            .iter()
            .position(|obj| {
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
            end_matrix_editing(
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
                end_matrix_editing(
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
                end_matrix_editing(
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
                end_matrix_editing(
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
                end_matrix_editing(
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
                end_matrix_editing(
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
                end_matrix_editing(
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
                end_matrix_editing(
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
        tokens: &AppTokens<'a>,
        results: &Results,
        vars: &Variables,
        editor_objs: &mut EditorObjects,
    ) {
        NoteCalcApp::renderr(
            &mut self.editor,
            &self.editor_content,
            units,
            &mut self.matrix_editing,
            &mut self.line_reference_chooser,
            render_buckets,
            result_buffer,
            self.editor_area_redraw,
            self.result_area_redraw,
            &mut self.render_data,
            allocator,
            tokens,
            results,
            vars,
            editor_objs,
        );
        self.editor_area_redraw.clear();
        self.result_area_redraw.clear();
    }
}

pub struct ResultLengths {
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

fn draw_cursor(
    render_buckets: &mut RenderBuckets,
    r: &PerLineRenderData,
    gr: &GlobalRenderData,
    editor: &Editor,
    matrix_editing: &Option<MatrixEditing>,
) {
    let cursor_pos = editor.get_selection().get_cursor_pos();
    if cursor_pos.row == r.editor_y.as_usize() {
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
                r.render_y.add(r.vert_align_offset),
                '▏',
            );
        }
    }
}

pub fn clear_rerendered_lines(
    render_buckets: &mut RenderBuckets,
    gr: &mut GlobalRenderData,
    rerendered_lines: &[((RenderPosY, usize), RedrawTarget)],
) {
    for ((render_y, render_height), target) in rerendered_lines {
        let x_w_coords = match *target {
            RedrawTarget::Both => Some((
                LEFT_GUTTER_WIDTH,
                gr.current_result_panel_width
                    + gr.current_editor_width
                    + gr.left_gutter_width
                    + SCROLL_BAR_WIDTH
                    + RIGHT_GUTTER_WIDTH,
            )),
            RedrawTarget::EditorArea => Some((LEFT_GUTTER_WIDTH, gr.current_editor_width)),

            RedrawTarget::ResultArea => Some((
                gr.result_gutter_x,
                gr.current_result_panel_width + RIGHT_GUTTER_WIDTH,
            )),
        };

        if let Some((x, w)) = x_w_coords {
            render_buckets
                .clear_commands
                .push(OutputMessage::RenderRectangle {
                    x,
                    y: *render_y,
                    w,
                    h: *render_height,
                });
        }
    }
}

pub fn pulse_rerendered_lines(
    render_buckets: &mut RenderBuckets,
    gr: &mut GlobalRenderData,
    rerendered_lines: &[((RenderPosY, usize), RedrawTarget)],
) {
    for ((render_y, render_height), target) in rerendered_lines {
        // for DEBUG, pulses the whole line
        // let x_w_coords = match *target {
        //     RedrawTarget::Both => Some((
        //         LEFT_GUTTER_WIDTH,
        //         gr.current_result_panel_width
        //             + gr.current_editor_width
        //             + gr.left_gutter_width
        //             + SCROLL_BAR_WIDTH
        //             + RIGHT_GUTTER_WIDTH,
        //     )),
        //     RedrawTarget::EditorArea => Some((LEFT_GUTTER_WIDTH, gr.current_editor_width)),
        //
        //     RedrawTarget::ResultArea => Some((
        //         gr.result_gutter_x,
        //         gr.current_result_panel_width + RIGHT_GUTTER_WIDTH,
        //     )),
        // };
        // pulses only the result
        let x_w_coords = match *target {
            RedrawTarget::Both | RedrawTarget::ResultArea => Some((
                gr.result_gutter_x,
                gr.current_result_panel_width + RIGHT_GUTTER_WIDTH,
            )),
            _ => None,
        };

        if let Some((x, w)) = x_w_coords {
            render_buckets.custom_commands[Layer::AboveText as usize].push(
                OutputMessage::PulsingRectangle {
                    x,
                    y: *render_y,
                    w,
                    h: *render_height,
                    start_color: 0xFF88FF_11,
                    end_color: 0xFFFFFF_00,
                    animation_time: Duration::from_millis(500),
                },
            );
        }
    }
}

pub fn clear_bottom_due_to_line_shrinking(
    last_row_i: usize,
    render_buckets: &mut RenderBuckets,
    gr: &mut GlobalRenderData,
) {
    let last_editor_y = EditorY::new(last_row_i - 1);
    if let Some(current_bottom) = gr.get_render_y(last_editor_y) {
        if current_bottom < gr.latest_bottom {
            let h = gr.latest_bottom.as_usize() - current_bottom.as_usize();
            // something, e.g. a matrix has shrinked, clear the below area
            let y = current_bottom.add(1);
            render_buckets
                .clear_commands
                .push(OutputMessage::RenderRectangle {
                    x: 0,
                    y,
                    w: gr.current_result_panel_width
                        + gr.current_editor_width
                        + gr.left_gutter_width
                        + SCROLL_BAR_WIDTH
                        + RIGHT_GUTTER_WIDTH,
                    h,
                });
        }
        gr.latest_bottom = current_bottom;
    }
}

pub fn clear_bottom_due_to_line_removal(
    last_row_i: usize,
    render_buckets: &mut RenderBuckets,
    gr: &mut GlobalRenderData,
) {
    let mut clear_box_h = 0;
    let mut clear_box_y = None;
    for i in last_row_i..MAX_LINE_COUNT {
        let editor_y = EditorY::new(i);

        if let Some(render_y) = gr.get_render_y(editor_y) {
            if clear_box_y.is_none() {
                clear_box_y = Some(render_y);
                clear_box_h = gr.get_rendered_height(editor_y);
                gr.set_render_y(editor_y, None);
            } else {
                clear_box_h += gr.get_rendered_height(editor_y);
                gr.set_render_y(editor_y, None);
            }
        } else {
            if clear_box_y.is_some() {
                break;
            }
        }
    }
    if let Some(clear_box_y) = clear_box_y {
        render_buckets
            .clear_commands
            .push(OutputMessage::RenderRectangle {
                x: 0,
                y: clear_box_y,
                w: gr.left_gutter_width
                    + gr.current_editor_width
                    + gr.current_result_panel_width
                    + SCROLL_BAR_WIDTH
                    + RIGHT_GUTTER_WIDTH,
                h: clear_box_h,
            });
    }
}

pub fn parse_tokens<'b>(
    line: &[char],
    editor_y: usize,
    units: &Units,
    vars: &Variables,
    allocator: &'b Arena<char>,
) -> Option<Tokens<'b>> {
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
    gr: &mut GlobalRenderData,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    allocator: &'text_ptr Arena<char>,
) {
    r.set_fix_row_height(1);
    gr.set_rendered_height(r.editor_y, 1);

    let text_len = line.len().min(gr.current_editor_width);

    render_buckets.utf8_texts.push(RenderUtf8TextMsg {
        text: allocator.alloc_extend(line.iter().map(|it| *it).take(text_len)),
        row: r.render_y,
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
    vars: &Variables,
    units: &Units,
    need_matrix_renderer: bool,
    decimal_count: Option<usize>,
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
            token_index = render_matrix(
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
                decimal_count,
            );
        } else if let (TokenType::LineReference { var_index }, true) =
            (&token.typ, need_matrix_renderer)
        {
            let var = vars[*var_index].as_ref().unwrap();

            let (rendered_width, rendered_height) = render_result_inside_editor(
                units,
                render_buckets,
                &var.value,
                r,
                gr,
                decimal_count,
            );

            let var_name_len = var.name.len();
            editor_objects.push(EditorObject {
                typ: EditorObjectType::LineReference,
                row: r.editor_y,
                start_x: r.editor_x,
                end_x: r.editor_x + var_name_len,
                rendered_x: r.render_x,
                rendered_y: r.render_y,
                rendered_w: rendered_width,
                rendered_h: rendered_height,
            });

            token_index += 1;
            r.token_render_done(
                var_name_len,
                rendered_width,
                if cursor_pos.column > r.editor_x {
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
                    row: r.editor_y,
                    start_x: r.editor_x,
                    end_x: r.editor_x + token.ptr.len(),
                    rendered_x: r.render_x,
                    rendered_y: r.render_y,
                    rendered_w: token.ptr.len(),
                    rendered_h: r.rendered_row_height,
                });
            }
            draw_token(
                token,
                r.render_x,
                r.render_y.add(r.vert_align_offset),
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
    if r.render_x > gr.current_editor_width {
        render_buckets.set_color(Layer::Text, 0x000000_FF);
        render_buckets.draw_char(
            Layer::Text,
            gr.current_editor_width + gr.left_gutter_width,
            r.render_y,
            '…',
        );
    }
}

fn draw_line_ref_chooser(
    render_buckets: &mut RenderBuckets,
    r: &PerLineRenderData,
    gr: &GlobalRenderData,
    line_reference_chooser: &Option<EditorY>,
    result_gutter_x: usize,
) {
    if let Some(selection_row) = line_reference_chooser {
        if *selection_row == r.editor_y {
            render_buckets.set_color(Layer::Text, 0xFFCCCC_FF);
            render_buckets.draw_rect(
                Layer::Text,
                0,
                r.render_y,
                result_gutter_x + RIGHT_GUTTER_WIDTH + gr.current_result_panel_width,
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
    match editor_content.get_data(r.editor_y.as_usize()).result_format {
        ResultFormat::Hex => {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['0', 'x'],
                row: r.render_y,
                column: result_gutter_x,
            });
        }
        ResultFormat::Bin => {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['0', 'b'],
                row: r.render_y,
                column: result_gutter_x,
            });
        }
        ResultFormat::Dec => {}
    }
}

fn highlight_current_line(
    render_buckets: &mut RenderBuckets,
    r: &PerLineRenderData,
    editor: &Editor,
    gr: &GlobalRenderData,
    redraw_result_area: EditorRowFlags,
) {
    let cursor_pos = editor.get_selection().get_cursor_pos();
    if cursor_pos.row == r.editor_y.as_usize() {
        render_buckets.set_color(Layer::Text, 0xFFFFCC_55);
        render_buckets.draw_rect(
            Layer::Text,
            0,
            r.render_y,
            gr.result_gutter_x
                + if redraw_result_area.need(r.editor_y) {
                    RIGHT_GUTTER_WIDTH + gr.current_result_panel_width
                } else {
                    0
                },
            r.rendered_row_height,
        );
    }
}

fn evaluate_tokens_and_save_result(
    vars: &mut Variables,
    editor_y: usize,
    editor_content: &EditorContent<LineData>,
    shunting_output_stack: &mut Vec<TokenType>,
    line: &[char],
) -> Result<Option<EvaluationResult>, ()> {
    let result = evaluate_tokens(shunting_output_stack, &vars);
    if let Ok(Some(result)) = &result {
        fn replace_or_insert_var(
            vars: &mut Variables,
            var_name: &[char],
            result: CalcResult,
            editor_y: usize,
        ) {
            if let Some(var) = &mut vars[editor_y] {
                var.name = Box::from(var_name);
                var.value = Ok(result);
            } else {
                vars[editor_y] = Some(Variable {
                    name: Box::from(var_name),
                    value: Ok(result),
                });
            };
        }

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
            replace_or_insert_var(vars, var_name, result.result.clone(), editor_y);
        } else {
            let line_data = editor_content.get_data(editor_y);
            debug_assert!(line_data.line_id > 0);
            let line_id = line_data.line_id;
            replace_or_insert_var(
                vars,
                STATIC_LINE_IDS[line_id],
                result.result.clone(),
                editor_y,
            );
        }
    } else {
        vars[editor_y] = None;
    }
    result
}

fn clear_single_line_selection_rows(pos: Pos) -> EditorRowFlags {
    // single line selection, clear the line above/below to redraw the SUM
    if pos.row == 0 {
        EditorRowFlags::range(0, 1)
    } else {
        EditorRowFlags::range(pos.row - 1, pos.row)
    }
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
    // TODO: why unused?
    _decimal_count: Option<usize>,
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
    let cursor_isnide_matrix: bool = if editor.get_selection().is_range().is_none()
        && cursor_pos.row == r.editor_y.as_usize()
        && cursor_pos.column > r.editor_x
        && cursor_pos.column < r.editor_x + text_width
    {
        // cursor is inside the matrix
        true
    } else {
        false
    };

    let new_render_x = if let (true, Some(mat_editor)) = (cursor_isnide_matrix, matrix_editing) {
        mat_editor.render(
            r.render_x,
            r.render_y,
            gr.current_editor_width,
            gr.left_gutter_width,
            render_buckets,
            r.rendered_row_height,
        )
    } else {
        render_matrix_obj(
            r.render_x,
            r.render_y,
            gr.current_editor_width,
            gr.left_gutter_width,
            row_count,
            col_count,
            &tokens[token_index..],
            render_buckets,
            r.rendered_row_height,
        )
    };

    let rendered_width = new_render_x - r.render_x;
    editor_objects.push(EditorObject {
        typ: EditorObjectType::Matrix {
            row_count,
            col_count,
        },
        row: r.editor_y,
        start_x: r.editor_x,
        end_x: r.editor_x + text_width,
        rendered_x: r.render_x,
        rendered_y: r.render_y,
        rendered_w: rendered_width,
        rendered_h: row_count,
    });

    let x_diff = if cursor_pos.row == r.editor_y.as_usize()
        && cursor_pos.column >= r.editor_x + text_width
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
    vars: &Variables,
    results: &[LineResult],
    allocator: &Arena<char>,
) -> Option<String> {
    let sel = editor.get_selection();
    // TODO optimize vec allocations
    let mut tokens = Vec::with_capacity(128);
    // TODO we should be able to mark the arena allcoator and free it at the end of the function
    if sel.start.row == sel.end.unwrap().row {
        if let Some(selected_text) = Editor::get_selected_text_single_line(sel, &editor_content) {
            if let Ok(Some(result)) = evaluate_text(
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
                        Some(4),
                        true,
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
                Some(4),
                true,
            );
            return Some(result_str);
        }
    }
    return None;
}

fn evaluate_text<'text_ptr>(
    units: &Units,
    text: &[char],
    vars: &Variables,
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
    render_y: RenderPosY,
    current_editor_width: usize,
    left_gutter_width: usize,
    row_count: usize,
    col_count: usize,
    tokens: &[Token<'text_ptr>],
    render_buckets: &mut RenderBuckets<'text_ptr>,
    rendered_row_height: usize,
) -> usize {
    let vert_align_offset = if row_count == 1 {
        (rendered_row_height - 1) / 2
    } else {
        (rendered_row_height - (row_count + MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT)) / 2
    };

    render_matrix_left_brackets(
        render_x + left_gutter_width,
        render_y,
        row_count,
        render_buckets,
        vert_align_offset,
    );
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
            // the content of the matrix starts from the second row
            let matrix_ascii_header_offset = if row_count == 1 { 0 } else { 1 };
            let dst_y = row_i + vert_align_offset + matrix_ascii_header_offset;
            for token in tokens.iter() {
                draw_token(
                    token,
                    render_x + offset_x + local_x,
                    render_y.add(dst_y),
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

    render_matrix_right_brackets(
        render_x + left_gutter_width,
        render_y,
        row_count,
        render_buckets,
        vert_align_offset,
    );
    render_x += 1;

    render_x
}

fn render_matrix_left_brackets(
    x: usize,
    render_y: RenderPosY,
    row_count: usize,
    render_buckets: &mut RenderBuckets,
    vert_align_offset: usize,
) {
    if row_count == 1 {
        render_buckets.operators.push(RenderUtf8TextMsg {
            text: &['['],
            row: render_y.add(vert_align_offset),
            column: x,
        });
    } else {
        render_buckets.operators.push(RenderUtf8TextMsg {
            text: &['┌'],
            row: render_y.add(vert_align_offset),
            column: x,
        });
        for i in 0..row_count {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['│'],
                row: render_y.add(i + vert_align_offset + 1),
                column: x,
            });
        }
        render_buckets.operators.push(RenderUtf8TextMsg {
            text: &['└'],
            row: render_y.add(row_count + vert_align_offset + 1),
            column: x,
        });
    };
}

fn render_matrix_right_brackets(
    x: usize,
    render_y: RenderPosY,
    row_count: usize,
    render_buckets: &mut RenderBuckets,
    vert_align_offset: usize,
) {
    if row_count == 1 {
        render_buckets.operators.push(RenderUtf8TextMsg {
            text: &[']'],
            row: render_y.add(vert_align_offset),
            column: x,
        });
    } else {
        render_buckets.operators.push(RenderUtf8TextMsg {
            text: &['┐'],
            row: render_y.add(vert_align_offset),
            column: x,
        });
        for i in 0..row_count {
            render_buckets.operators.push(RenderUtf8TextMsg {
                text: &['│'],
                row: render_y.add(i + 1 + vert_align_offset),
                column: x,
            });
        }
        render_buckets.operators.push(RenderUtf8TextMsg {
            text: &['┘'],
            row: render_y.add(row_count + 1 + vert_align_offset),
            column: x,
        });
    }
}

fn render_matrix_result<'text_ptr>(
    units: &Units,
    mut render_x: usize,
    render_y: RenderPosY,
    mat: &MatrixData,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    prev_mat_result_lengths: Option<&ResultLengths>,
    rendered_row_height: usize,
    decimal_count: Option<usize>,
) -> usize {
    let start_x = render_x;
    let vert_align_offset = if mat.row_count == 1 {
        (rendered_row_height - 1) / 2
    } else {
        (rendered_row_height - (mat.row_count + MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT)) / 2
    };
    render_matrix_left_brackets(
        start_x,
        render_y,
        mat.row_count,
        render_buckets,
        vert_align_offset,
    );
    render_x += 1;

    let cells_strs = {
        let mut tokens_per_cell: SmallVec<[String; 32]> = SmallVec::with_capacity(32);

        for cell in mat.cells.iter() {
            let result_str =
                render_result(units, cell, &ResultFormat::Dec, false, decimal_count, true);
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
    render_buckets.set_color(Layer::Text, 0x000000_FF);

    for col_i in 0..mat.col_count {
        for row_i in 0..mat.row_count {
            let cell_str = &cells_strs[row_i * mat.col_count + col_i];
            let lengths = get_int_frac_part_len(cell_str);
            // Draw integer part
            let offset_x = max_lengths.int_part_len - lengths.int_part_len;
            // the content of the matrix starts from the second row
            let matrix_ascii_header_offset = if mat.row_count == 1 { 0 } else { 1 };
            let dst_y = render_y.add(row_i + vert_align_offset + matrix_ascii_header_offset);
            render_buckets.draw_string(
                Layer::Text,
                render_x + offset_x,
                dst_y,
                // TOOD nem kell clone, csinálj iter into vhogy
                cell_str[0..lengths.int_part_len].to_owned(),
            );

            let mut frac_offset_x = 0;
            if lengths.frac_part_len > 0 {
                render_buckets.draw_string(
                    Layer::Text,
                    render_x + offset_x + lengths.int_part_len,
                    dst_y,
                    // TOOD nem kell clone, csinálj iter into vhogy
                    cell_str[lengths.int_part_len..lengths.int_part_len + lengths.frac_part_len]
                        .to_owned(),
                )
            } else if max_lengths.frac_part_len > 0 {
                render_buckets.draw_char(
                    Layer::Text,
                    render_x + offset_x + lengths.int_part_len,
                    dst_y,
                    '.',
                );
                frac_offset_x = 1;
            }
            for i in 0..max_lengths.frac_part_len - lengths.frac_part_len - frac_offset_x {
                render_buckets.draw_char(
                    Layer::Text,
                    render_x
                        + offset_x
                        + lengths.int_part_len
                        + lengths.frac_part_len
                        + frac_offset_x
                        + i,
                    dst_y,
                    '0',
                )
            }
            if lengths.unit_part_len > 0 {
                render_buckets.draw_string(
                    Layer::Text,
                    render_x + offset_x + lengths.int_part_len + max_lengths.frac_part_len + 1,
                    dst_y,
                    // TOOD nem kell clone, csinálj iter into vhogy
                    // +1, skip space
                    cell_str[lengths.int_part_len + lengths.frac_part_len + 1..].to_owned(),
                )
            }
        }
        render_x += if col_i + 1 < mat.col_count {
            (max_lengths.int_part_len + max_lengths.frac_part_len + max_lengths.unit_part_len) + 2
        } else {
            max_lengths.int_part_len + max_lengths.frac_part_len + max_lengths.unit_part_len
        };
    }

    render_matrix_right_brackets(
        render_x,
        render_y,
        mat.row_count,
        render_buckets,
        vert_align_offset,
    );
    render_x += 1;
    return render_x - start_x;
}

fn render_result_inside_editor<'text_ptr>(
    units: &Units,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    result: &Result<CalcResult, ()>,
    r: &PerLineRenderData,
    gr: &GlobalRenderData,
    decimal_count: Option<usize>,
) -> (usize, usize) {
    return match &result {
        Ok(CalcResult::Matrix(mat)) => {
            let rendered_width = render_matrix_result(
                units,
                gr.left_gutter_width + r.render_x,
                r.render_y,
                mat,
                render_buckets,
                None,
                r.rendered_row_height,
                decimal_count,
            );
            (rendered_width, mat.row_count)
        }
        Ok(result) => {
            // TODO: optimize string alloc
            let result_str =
                render_result(&units, result, &ResultFormat::Dec, false, Some(2), true);
            let text_len = result_str
                .chars()
                .count()
                .min((gr.current_editor_width as isize - r.render_x as isize).max(0) as usize);
            // TODO avoid String
            render_buckets.line_ref_results.push(RenderStringMsg {
                text: result_str[0..text_len].to_owned(),
                row: r.render_y,
                column: r.render_x + gr.left_gutter_width,
            });
            (text_len, 1)
        }
        Err(_) => {
            render_buckets.line_ref_results.push(RenderStringMsg {
                text: "Err".to_owned(),
                row: r.render_y,
                column: r.render_x + gr.left_gutter_width,
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
    modif_type: EditorRowFlags,
    decimal_count: Option<usize>,
) {
    struct ResultTmp {
        buffer_ptr: Option<Range<usize>>,
        editor_y: EditorY,
        lengths: ResultLengths,
    }
    let (max_lengths, result_ranges) = {
        let mut result_ranges: SmallVec<[ResultTmp; MAX_LINE_COUNT]> =
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
            let editor_y = EditorY::new(editor_y);
            let render_y = if let Some(render_y) = gr.get_render_y(editor_y) {
                render_y
            } else {
                continue;
            };
            if editor_y.as_usize() >= editor_content.line_count() {
                // TODO what is it?
                break;
            }

            if let Err(..) = result {
                result_buffer[result_buffer_index] = b'E';
                result_buffer[result_buffer_index + 1] = b'r';
                result_buffer[result_buffer_index + 2] = b'r';
                result_ranges.push(ResultTmp {
                    buffer_ptr: Some(result_buffer_index..result_buffer_index + 3),
                    editor_y,
                    lengths: ResultLengths {
                        int_part_len: 0,
                        frac_part_len: 0,
                        unit_part_len: 0,
                    },
                });
                result_buffer_index += 3;
                prev_result_matrix_length = None;
            } else if let Ok(Some(result)) = result {
                match &result {
                    CalcResult::Matrix(mat) => {
                        if prev_result_matrix_length.is_none() {
                            prev_result_matrix_length = calc_consecutive_matrices_max_lengths(
                                units,
                                &results[editor_y.as_usize()..],
                            );
                        }
                        if modif_type.need(editor_y) {
                            render_matrix_result(
                                units,
                                result_gutter_x + RIGHT_GUTTER_WIDTH,
                                render_y,
                                mat,
                                render_buckets,
                                prev_result_matrix_length.as_ref(),
                                gr.get_rendered_height(editor_y),
                                decimal_count,
                            );
                        }
                        result_ranges.push(ResultTmp {
                            buffer_ptr: None,
                            editor_y,
                            lengths: ResultLengths {
                                int_part_len: 0,
                                frac_part_len: 0,
                                unit_part_len: 0,
                            },
                        });
                    }
                    _ => {
                        prev_result_matrix_length = None;
                        let start = result_buffer_index;
                        let mut c = Cursor::new(&mut result_buffer[start..]);
                        let lens = render_result_into(
                            &units,
                            &result,
                            &editor_content.get_data(editor_y.as_usize()).result_format,
                            false,
                            &mut c,
                            decimal_count,
                            true,
                        );
                        let len = c.position() as usize;
                        let range = start..start + len;
                        max_lengths.set_max(&lens);
                        result_ranges.push(ResultTmp {
                            buffer_ptr: Some(range),
                            editor_y,
                            lengths: lens,
                        });
                        result_buffer_index += len;
                    }
                };
            } else {
                prev_result_matrix_length = None;
                result_ranges.push(ResultTmp {
                    buffer_ptr: None,
                    editor_y,
                    lengths: ResultLengths {
                        int_part_len: 0,
                        frac_part_len: 0,
                        unit_part_len: 0,
                    },
                });
            }
        }
        (max_lengths, result_ranges)
    };

    // render results from the buffer
    for result_tmp in result_ranges.iter() {
        let rendered_row_height = gr.get_rendered_height(result_tmp.editor_y);
        let render_y = gr.get_render_y(result_tmp.editor_y).expect("");
        if let Some(result_range) = &result_tmp.buffer_ptr {
            if !modif_type.need(result_tmp.editor_y) {
                continue;
            }
            // result background
            render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
            render_buckets.draw_rect(
                Layer::BehindText,
                gr.result_gutter_x + RIGHT_GUTTER_WIDTH,
                render_y,
                gr.current_result_panel_width,
                rendered_row_height,
            );

            let lengths = &result_tmp.lengths;
            let from = result_range.start;
            let vert_align_offset = (rendered_row_height - 1) / 2;
            let row = render_y.add(vert_align_offset);
            enum ResultOffsetX {
                Err,
                Ok(usize),
                TooLong,
            }
            let offset_x = if max_lengths.int_part_len < lengths.int_part_len {
                // it is an "Err"
                ResultOffsetX::Err
            } else {
                let offset_x = max_lengths.int_part_len - lengths.int_part_len;
                let sum_len =
                    lengths.int_part_len + max_lengths.frac_part_len + max_lengths.unit_part_len;
                if offset_x + sum_len > gr.current_result_panel_width {
                    if sum_len > gr.current_result_panel_width {
                        ResultOffsetX::TooLong
                    } else {
                        ResultOffsetX::Ok(gr.current_result_panel_width - sum_len)
                    }
                } else {
                    ResultOffsetX::Ok(offset_x)
                }
            };
            let x = result_gutter_x
                + RIGHT_GUTTER_WIDTH
                + match offset_x {
                    ResultOffsetX::Err => 0,
                    ResultOffsetX::TooLong => 0,
                    ResultOffsetX::Ok(n) => n,
                };
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
                let from = result_range.start + lengths.int_part_len + lengths.frac_part_len + 1;
                render_buckets.ascii_texts.push(RenderAsciiTextMsg {
                    text: &result_buffer[from..result_range.end],
                    row,
                    column: x + lengths.int_part_len + lengths.frac_part_len + 1,
                });
            }
            match offset_x {
                ResultOffsetX::TooLong => {
                    render_buckets.set_color(Layer::AboveText, 0xF2F2F2_FF);
                    render_buckets.draw_char(
                        Layer::AboveText,
                        gr.result_gutter_x + RIGHT_GUTTER_WIDTH + gr.current_result_panel_width - 1,
                        row,
                        '█',
                    );
                    render_buckets.set_color(Layer::AboveText, 0xFF0000_FF);
                    render_buckets.draw_char(
                        Layer::AboveText,
                        gr.result_gutter_x + RIGHT_GUTTER_WIDTH + gr.current_result_panel_width - 1,
                        row,
                        '…',
                    );
                }
                _ => {}
            }
        } else if modif_type.need(result_tmp.editor_y) {
            // no result but need rerender
            // result background
            render_buckets.set_color(Layer::BehindText, 0xF2F2F2_FF);
            render_buckets.draw_rect(
                Layer::BehindText,
                gr.result_gutter_x + RIGHT_GUTTER_WIDTH,
                render_y,
                gr.current_result_panel_width,
                rendered_row_height,
            );
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
                let lengths = calc_matrix_max_lengths(units, mat);
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

fn calc_matrix_max_lengths(units: &Units, mat: &MatrixData) -> ResultLengths {
    let cells_strs = {
        let mut tokens_per_cell: SmallVec<[String; 32]> = SmallVec::with_capacity(32);

        for cell in mat.cells.iter() {
            let result_str = render_result(units, cell, &ResultFormat::Dec, false, Some(4), true);
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

fn draw_token<'text_ptr>(
    token: &Token<'text_ptr>,
    render_x: usize,
    render_y: RenderPosY,
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

fn render_buckets_into(buckets: &RenderBuckets, canvas: &mut [[char; 256]]) {
    fn write_char_slice(canvas: &mut [[char; 256]], row: RenderPosY, col: usize, src: &[char]) {
        let str = &mut canvas[row.as_usize()];
        for (dst_char, src_char) in str[col..].iter_mut().zip(src.iter()) {
            *dst_char = *src_char;
        }
    }

    fn write_str(canvas: &mut [[char; 256]], row: RenderPosY, col: usize, src: &str) {
        let str = &mut canvas[row.as_usize()];
        for (dst_char, src_char) in str[col..].iter_mut().zip(src.chars()) {
            *dst_char = src_char;
        }
    }

    fn write_ascii(canvas: &mut [[char; 256]], row: RenderPosY, col: usize, src: &[u8]) {
        let str = &mut canvas[row.as_usize()];
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

    for command in &buckets.custom_commands[Layer::Text as usize] {
        write_command(canvas, command);
    }

    for command in &buckets.custom_commands[Layer::AboveText as usize] {
        write_command(canvas, command);
    }
}

fn render_selection_and_its_sum<'text_ptr>(
    units: &Units,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    results: &Results,
    editor: &Editor,
    editor_content: &EditorContent<LineData>,
    gr: &GlobalRenderData,
    vars: &Variables,
    allocator: &'text_ptr Arena<char>,
) {
    render_buckets.set_color(Layer::BehindText, 0xA6D2FF_FF);
    if let Some((start, end)) = editor.get_selection().is_range() {
        if end.row > start.row {
            // first line
            if let Some(start_render_y) = gr.get_render_y(EditorY::new(start.row)) {
                let height = gr.get_rendered_height(EditorY::new(start.row));
                render_buckets.draw_rect(
                    Layer::BehindText,
                    start.column + gr.left_gutter_width,
                    start_render_y,
                    editor_content
                        .line_len(start.row)
                        .min(gr.current_editor_width),
                    height,
                );
            }
            // full lines
            for i in start.row + 1..end.row {
                if let Some(render_y) = gr.get_render_y(EditorY::new(i)) {
                    let height = gr.get_rendered_height(EditorY::new(i));
                    render_buckets.draw_rect(
                        Layer::BehindText,
                        gr.left_gutter_width,
                        render_y,
                        editor_content.line_len(i).min(gr.current_editor_width),
                        height,
                    );
                }
            }
            // last line
            if let Some(end_render_y) = gr.get_render_y(EditorY::new(end.row)) {
                let height = gr.get_rendered_height(EditorY::new(end.row));
                render_buckets.draw_rect(
                    Layer::BehindText,
                    gr.left_gutter_width,
                    end_render_y,
                    end.column.min(gr.current_editor_width),
                    height,
                );
            }
        } else if let Some(start_render_y) = gr.get_render_y(EditorY::new(start.row)) {
            let height = gr.get_rendered_height(EditorY::new(start.row));
            render_buckets.draw_rect(
                Layer::BehindText,
                start.column + gr.left_gutter_width,
                start_render_y,
                (end.column - start.column).min(gr.current_editor_width),
                height,
            );
        }
        // evaluated result of selection, selected text
        if let Some(mut partial_result) = evaluate_selection(
            &units,
            editor,
            editor_content,
            &vars,
            results.as_slice(),
            allocator,
        ) {
            if start.row == end.row {
                if let Some(start_render_y) = gr.get_render_y(EditorY::new(start.row)) {
                    let selection_center = start.column + ((end.column - start.column) / 2);
                    partial_result.insert_str(0, "= ");
                    let result_w = partial_result.chars().count();
                    let centered_x =
                        (selection_center as isize - (result_w / 2) as isize).max(0) as usize;
                    render_buckets.set_color(Layer::AboveText, 0xAAFFAA_FF);
                    let rect_y = if start.row == 0 {
                        start_render_y.add(1)
                    } else {
                        start_render_y.sub(1)
                    };
                    render_buckets.draw_rect(
                        Layer::AboveText,
                        gr.left_gutter_width + centered_x,
                        rect_y,
                        result_w,
                        1,
                    );
                    render_buckets.set_color(Layer::AboveText, 0x000000_FF);
                    render_buckets.draw_string(
                        Layer::AboveText,
                        gr.left_gutter_width + centered_x,
                        rect_y,
                        partial_result,
                    );
                }
            } else {
                partial_result.insert_str(0, " ∑ = ");
                let result_w = partial_result.chars().count();
                let x = (start.row..=end.row)
                    .map(|it| editor_content.line_len(it))
                    .max_by(|a, b| a.cmp(b))
                    .unwrap()
                    + 3;
                let frist_visible_row_index = EditorY::new(start.row.max(gr.scroll_y));
                let last_visible_row_index =
                    EditorY::new(end.row.min(gr.scroll_y + gr.client_height - 1));
                let inner_height = gr
                    .get_render_y(last_visible_row_index)
                    .expect("")
                    .as_usize()
                    - gr.get_render_y(frist_visible_row_index)
                        .expect("")
                        .as_usize();
                render_buckets.set_color(Layer::AboveText, 0xAAFFAA_FF);
                render_buckets.draw_rect(
                    Layer::AboveText,
                    gr.left_gutter_width + x,
                    gr.get_render_y(frist_visible_row_index).expect(""),
                    result_w + 1,
                    inner_height + 1,
                );
                // draw the parenthesis
                render_buckets.set_color(Layer::AboveText, 0x000000_FF);

                render_buckets.draw_char(
                    Layer::AboveText,
                    gr.left_gutter_width + x,
                    gr.get_render_y(frist_visible_row_index).expect(""),
                    if frist_visible_row_index.as_usize() == start.row {
                        '⎫'
                    } else {
                        '⎪'
                    },
                );

                render_buckets.draw_char(
                    Layer::AboveText,
                    gr.left_gutter_width + x,
                    gr.get_render_y(last_visible_row_index).expect(""),
                    if last_visible_row_index.as_usize() == end.row {
                        '⎭'
                    } else {
                        '⎪'
                    },
                );

                for i in 1..inner_height {
                    render_buckets.draw_char(
                        Layer::AboveText,
                        gr.left_gutter_width + x,
                        gr.get_render_y(frist_visible_row_index).expect("").add(i),
                        '⎪',
                    );
                }
                // center
                render_buckets.draw_string(
                    Layer::AboveText,
                    gr.left_gutter_width + x,
                    gr.get_render_y(frist_visible_row_index)
                        .expect("")
                        .add(inner_height / 2),
                    partial_result,
                );
            }
        }
    }
}

fn calc_result_gutter_x(current_x: Option<usize>, client_width: usize) -> usize {
    return (if let Some(current_x) = current_x {
        current_x
    } else {
        LEFT_GUTTER_WIDTH + MAX_EDITOR_WIDTH + SCROLL_BAR_WIDTH
    })
    .min(client_width - (RIGHT_GUTTER_WIDTH + MIN_RESULT_PANEL_WIDTH));
}

fn calc_rendered_height<'b>(
    editor_y: EditorY,
    matrix_editing: &Option<MatrixEditing>,
    tokens: &AppTokens,
    results: &Results,
    vars: &Variables,
) -> usize {
    return if let Some(tokens) = &tokens[editor_y] {
        let h = PerLineRenderData::calc_rendered_row_height(
            &results[editor_y],
            &tokens.tokens,
            vars,
            matrix_editing
                .as_ref()
                .filter(|it| it.row_index == editor_y)
                .map(|it| {
                    if it.row_count == 1 {
                        1
                    } else {
                        it.row_count + MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT
                    }
                }),
        );
        h
    } else {
        1
    };
}

fn is_pos_inside_an_obj(editor_objects: &EditorObjects, pos: Pos) -> Option<&EditorObject> {
    for obj in &editor_objects[EditorY::new(pos.row)] {
        if (obj.start_x + 1..obj.end_x).contains(&pos.column) {
            return Some(obj);
        }
    }
    return None;
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
        Pos::from_row_column(mat_editor.row_index.as_usize(), mat_editor.start_text_index),
        Pos::from_row_column(mat_editor.row_index.as_usize(), mat_editor.end_text_index),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::editor::Selection;
    use std::ops::RangeInclusive;

    fn create_holder<'b>() -> (
        AppTokens<'b>,
        Results,
        [Option<Variable>; MAX_LINE_COUNT + 1],
        EditorObjects,
    ) {
        let editor_objects = EditorObjects::new();
        let tokens = AppTokens::new();
        let results = Results::new();
        (tokens, results, create_vars(), editor_objects)
    }

    fn create_app<'a>(
        client_height: usize,
    ) -> (
        NoteCalcApp,
        Units,
        (
            AppTokens<'a>,
            Results,
            [Option<Variable>; MAX_LINE_COUNT + 1],
            EditorObjects,
        ),
    ) {
        let app = NoteCalcApp::new(120, client_height);
        let units = Units::new();
        return (app, units, create_holder());
    }

    #[test]
    fn bug1() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 33));
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Right,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn bug2() {
        let arena = Arena::new();
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        app.handle_paste(
            "[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 1));

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Right,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let arena = Arena::new();
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn bug3() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1\n\
                2+"
            .to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 2));
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn bug4() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1\n\
                "
            .to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 0));
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_eq!(
            "1\n\
             &[1]",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn bug5() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "123\na ".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        assert_eq!(3, tokens[EditorY::new(1)].as_ref().unwrap().tokens.len());
    }

    #[test]
    fn it_is_not_allowed_to_ref_lines_below() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1\n\
                2+\n3\n4"
                .to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 2));
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_eq!(
            "1\n\
                2+\n3\n4",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn it_is_not_allowed_to_ref_lines_below2() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1\n\
                2+\n3\n4"
                .to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 2));
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_eq!(
            "1\n\
                2+&[1]\n3\n4",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn remove_matrix_backspace() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "abcd [1,2,3;4,5,6]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("abcd ", app.editor_content.get_content());
    }

    #[test]
    fn matrix_step_in_dir() {
        // from right
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1,2,3;4,5,6]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('1'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("abcd [1,2,1;4,5,6]", app.editor_content.get_content());
        }
        // from left
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1,2,3;4,5,6]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("abcd [9,2,3;4,5,6]", app.editor_content.get_content());
        }
        // from below
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1,2,3;4,5,6]\naaaaaaaaaaaaaaaaaa".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 7));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!(
                "abcd [1,2,3;9,5,6]\naaaaaaaaaaaaaaaaaa",
                app.editor_content.get_content()
            );
        }
        // from above
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "aaaaaaaaaaaaaaaaaa\nabcd [1,2,3;4,5,6]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 7));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!(
                "aaaaaaaaaaaaaaaaaa\nabcd [9,2,3;4,5,6]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn cursor_is_put_after_the_matrix_after_finished_editing() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "abcd [1,2,3;4,5,6]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('6'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('9'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(app.editor_content.get_content(), "abcd [1,2,6;4,5,6]9");
    }

    #[test]
    fn remove_matrix_del() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "abcd [1,2,3;4,5,6]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 5));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Del,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("abcd ", app.editor_content.get_content());
    }

    #[test]
    fn test_moving_inside_a_matrix() {
        // right to left, cursor at end
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1,2,3;4,5,6]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!("abcd [1,9,3;4,5,6]", app.editor_content.get_content());
        }
        // left to right, cursor at start
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1,2,3;4,5,6]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!("abcd [1,2,9;4,5,6]", app.editor_content.get_content());
        }
        // vertical movement down, cursor tries to keep its position
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1111,22,3;44,55555,666]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            // inside the matrix
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!(
                "abcd [1111,22,3;9,55555,666]",
                app.editor_content.get_content()
            );
        }

        // vertical movement up, cursor tries to keep its position
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1111,22,3;44,55555,666]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            // inside the matrix
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!(
                "abcd [9,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn test_moving_inside_a_matrix_with_tab() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "[1,2,3;4,5,6]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Home,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Right,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('7'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('8'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('9'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('0'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('9'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('4'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_eq!("[1,7,8;9,0,9]4", app.editor_content.get_content());
    }

    #[test]
    fn test_leaving_a_matrix_with_tab() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "[1,2,3;4,5,6]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        // the next tab should leave the matrix
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('7'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_eq!("[1,2,3;4,5,6]7", app.editor_content.get_content());
    }

    #[test]
    fn end_btn_matrix() {
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1111,22,3;44,55555,666] qq".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            // inside the matrix
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::End,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!(
                "abcd [1111,22,9;44,55555,666] qq",
                app.editor_content.get_content()
            );
        }
        // pressing twice, exits the matrix
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1111,22,3;44,55555,666] qq".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(0, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            // inside the matrix
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::End,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::End,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
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
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1111,22,3;44,55555,666]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            // inside the matrix
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Home,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('9'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!(
                "abcd [9,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "abcd [1111,22,3;44,55555,666]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            // inside the matrix
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Home,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Home,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Char('6'),
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!(
                "6abcd [1111,22,3;44,55555,666]",
                app.editor_content.get_content()
            );
        }
    }

    #[test]
    fn bug8() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "16892313\n14 * ".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 5));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        assert_eq!("16892313\n14 * &[1]", app.editor_content.get_content());
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_time(1000);
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("16892313\n14 * ", app.editor_content.get_content());

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('z'),
            InputModifiers::ctrl(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("16892313\n14 * &[1]", app.editor_content.get_content());

        let _input_eff = app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Right,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        ); // end selection
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("16892313\n14 * a&[1]", app.editor_content.get_content());

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char(' '),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Right,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('b'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("16892313\n14 * a &[1]b", app.editor_content.get_content());

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Right,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('c'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("16892313\n14 * a c&[1]b", app.editor_content.get_content());
    }

    #[test]
    fn test_referenced_line_calc() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "2\n3 * ".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 4));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        assert_eq!("2\n3 * &[1]", app.editor_content.get_content());
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        assert_results(&["2", "6"][..], &result_buffer);
    }

    #[test]
    fn test_line_ref_normalization() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(12, 2));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // remove a line
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('x'),
            InputModifiers::ctrl(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        // Move to end
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::PageDown,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::End,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        // ALT
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        assert_eq!(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n12\n13\n&[13]&[13]&[13]",
            &app.editor_content.get_content()
        );
        assert_eq!(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n12\n13\n&[12]&[12]&[12]\n",
            &app.get_normalized_content()
        );
    }

    #[test]
    fn test_line_ref_denormalization() {
        let (mut app, units, (mut tokens, mut results, mut vars, _)) = create_app(35);
        let arena = Arena::new();
        app.set_normalized_content(
            "1111\n2222\n14 * &[2]&[2]&[2]\n",
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        assert_eq!(1, app.editor_content.get_data(0).line_id);
        assert_eq!(2, app.editor_content.get_data(1).line_id);
        assert_eq!(3, app.editor_content.get_data(2).line_id);
    }

    #[test]
    fn test_scroll_y_reset() {
        let (mut app, units, (mut tokens, mut results, mut vars, _)) = create_app(35);
        let arena = Arena::new();
        app.render_data.scroll_y = 1;
        app.set_normalized_content(
            "1111\n2222\n14 * &[2]&[2]&[2]\n",
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        assert_eq!(0, app.render_data.scroll_y);
    }

    #[test]
    fn test_that_set_content_rerenders_everything() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.set_normalized_content(
            "1\n2\n3",
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));
        assert!(!app.result_area_redraw.need(EditorY::new(1)));
        assert!(!app.editor_area_redraw.need(EditorY::new(2)));
        assert!(!app.result_area_redraw.need(EditorY::new(2)));
        app.set_normalized_content(
            "1111\n2222\n14 * &[2]&[2]&[2]\n",
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn test_tab_change_clears_variables() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.set_normalized_content(
            "source: https://rippedbody.com/how-to-calculate-leangains-macros/

weight = 80 kg
height = 190 cm
age = 30

-- Step 1: Calculate your  (Basal Metabolic Rate) (BMR)
men BMR = 66 + (13.7 * weight/1kg) + (5 * height/1cm) - (6.8 * age)

'STEP 2. FIND YOUR TDEE BY ADJUSTING FOR ACTIVITY
Activity 
' Sedentary (little or no exercise) [BMR x 1.15]
' Mostly sedentary (office work), plus 3–6 days of weight lifting [BMR x 1.35]
' Lightly active, plus 3–6 days of weight lifting [BMR x 1.55]
' Highly active, plus 3–6 days of weight lifting [BMR x 1.75]
TDEE = (men BMR * 1.35)

'STEP 3. ADJUST CALORIE INTAKE BASED ON YOUR GOAL
Fat loss
    target weekly fat loss rate = 0.5%
    TDEE - ((weight/1kg) * target weekly fat loss rate * 1100)kcal
Muscle gain
    monthly rates of weight gain = 1%
    TDEE + (weight/1kg * monthly rates of weight gain * 330)kcal

Protein intake
    1.6 g/kg
    2.2 g/kg
    weight * &[27] to g
    weight * &[28] to g
Fat intake
    0.5g/kg or at least 30 %
    1g/kg minimum
    fat calory = 9
    &[24]",
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.set_normalized_content(
            "Valaki elment Horvátba 12000 Ftért
3 éjszakát töltött ott
&[1]*&[2] 
utána vacsorázott egyet 5000ért 


999 + 1
22222
3
4 + 2
2
&[10]
722
alma = 3
alma * 2
alma * &[13] + &[12] 
&[13] km
2222222222222222222722.22222222 km

[1;0] * [1,2]
1 + 2
2


2
23
human brain: 10^16 op/s
so far000 humans lived
avg. human lifespan is 50 years
total human brain activity is &[27] * &[28] * (&[29]/1s)",
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn test_panic_on_pressing_enter() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.set_normalized_content(
            "source: https://rippedbody.com/how-to-calculate-leangains-macros/

weight = 80 kg
height = 190 cm
age = 30

-- Step 1: Calculate your  (Basal Metabolic Rate) (BMR)
men BMR = 66 + (13.7 * weight/1kg) + (5 * height/1cm) - (6.8 * age)

'STEP 2. FIND YOUR TDEE BY ADJUSTING FOR ACTIVITY
Activity 
' Sedentary (little or no exercise) [BMR x 1.15]
' Mostly sedentary (office work), plus 3–6 days of weight lifting [BMR x 1.35]
' Lightly active, plus 3–6 days of weight lifting [BMR x 1.55]
' Highly active, plus 3–6 days of weight lifting [BMR x 1.75]
TDEE = (men BMR * 1.35)

'STEP 3. ADJUST CALORIE INTAKE BASED ON YOUR GOAL
Fat loss
    target weekly fat loss rate = 0.5%
    TDEE - ((weight/1kg) * target weekly fat loss rate * 1100)kcal
Muscle gain
    monthly rates of weight gain = 1%
    TDEE + (weight/1kg * monthly rates of weight gain * 330)kcal

Protein intake
    1.6 g/kg
    2.2 g/kg
    weight * &[27] to g
    weight * &[28] to g
Fat intake
    0.5g/kg or at least 30 %
    1g/kg minimum
    fat calory = 9
    &[24]",
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        assert_eq!(18, vars.len());
        fn assert_var(vars: &Variables, name: &str, defined_at: usize) {
            let var = vars[defined_at].as_ref().unwrap();
            assert!(var.value.is_ok(), "{}", name);
            assert_eq!(name.len(), var.name.len(), "{}", name);
            for (a, b) in name.chars().zip(var.name.iter()) {
                assert_eq!(a, *b, "{}", name);
            }
        }
        assert_var(&vars[..], "weight", 2);
        assert_var(&vars[..], "height", 3);
        assert_var(&vars[..], "age", 4);
        assert_var(&vars[..], "men BMR", 7);
        assert_var(&vars[..], "TDEE", 15);
        assert_var(&vars[..], "target weekly fat loss rate", 19);
        assert_var(&vars[..], "&[21]", 20);
        assert_var(&vars[..], "monthly rates of weight gain", 22);
        assert_var(&vars[..], "&[24]", 23);
        assert_var(&vars[..], "&[27]", 26);
        assert_var(&vars[..], "&[28]", 27);
        assert_var(&vars[..], "&[29]", 28);
        assert_var(&vars[..], "&[30]", 29);
        assert_var(&vars[..], "&[32]", 31);
        assert_var(&vars[..], "&[33]", 32);
        assert_var(&vars[..], "fat calory", 33);
        assert_var(&vars[..], "&[35]", 34);

        app.editor
            .set_selection_save_col(Selection::single_r_c(6, 33));

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_var(&vars[..], "weight", 2);
        assert_var(&vars[..], "height", 3);
        assert_var(&vars[..], "age", 4);
        assert_var(&vars[..], "men BMR", 8);
        assert_var(&vars[..], "TDEE", 16);
        assert_var(&vars[..], "target weekly fat loss rate", 20);
        assert_var(&vars[..], "&[21]", 21);
        assert_var(&vars[..], "monthly rates of weight gain", 23);
        assert_var(&vars[..], "&[24]", 24);
        assert_var(&vars[..], "&[27]", 27);
        assert_var(&vars[..], "&[28]", 28);
        assert_var(&vars[..], "&[29]", 29);
        assert_var(&vars[..], "&[30]", 30);
        assert_var(&vars[..], "&[32]", 32);
        assert_var(&vars[..], "&[33]", 33);
        assert_var(&vars[..], "fat calory", 34);
        assert_var(&vars[..], "&[35]", 35);
    }

    #[test]
    fn no_memory_deallocation_bug_in_line_selection() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(12, 2));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn matrix_deletion() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            " [1,2,3]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Del,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("[1,2,3]", app.editor_content.get_content());
    }

    #[test]
    fn removing_a_tall_matrix_rerenders_below_lines() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "[1;2;3\n4\n5".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 6));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char(']'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));

        let mut result_buffer = [0; 128];
        let mut render_buckets = RenderBuckets::new();
        app.render(
            &units,
            &mut render_buckets,
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        // the 3., and 4. rows must be cleared, since the matrix shrinked back from being 3 rows height to 1
        let mut ok = false;
        for clear_command in &render_buckets.clear_commands {
            match clear_command {
                OutputMessage::RenderRectangle { x, y, w, h } => {
                    if *x == 0 && y.as_usize() == 3 && *h == 4 && *w == 121 {
                        ok = true;
                    }
                }
                _ => {}
            }
        }
        assert!(ok);
    }

    #[test]
    fn matrix_insertion_bug() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "[1,2,3]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("a[1,2,3]", app.editor_content.get_content());
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("a\n[1,2,3]", app.editor_content.get_content());
    }

    #[test]
    fn matrix_insertion_bug2() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "'[X] nth, sum fv".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Del,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
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
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "3m * 2m
--
1
2
sum"
            .to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["6 m^2", "", "1", "2", "3"][..], &result_buffer);
    }

    #[test]
    fn no_sum_value_in_case_of_error() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "3m * 2m\n\
                4\n\
                sum"
            .to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["6 m^2", "4", "Err"][..], &result_buffer);
    }

    #[test]
    fn test_ctrl_c() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "aaaaaaaaa".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('c'),
            InputModifiers::ctrl(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("aaa", &app.editor.clipboard);
    }

    #[test]
    fn test_changing_output_style_for_selected_rows() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "2\n\
                    4\n\
                    5"
            .to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["10", "100", "101"][..], &result_buffer);
    }

    #[test]
    fn test_matrix_sum() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "[1,2,3]\nsum".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // both the first line and the 'sum' line renders a matrix, which leaves the result buffer empty
        assert_results(&["\u{0}"][..], &result_buffer);
    }

    #[test]
    fn test_rich_copy() {
        fn t(content: &str, expected: &str, selected_range: RangeInclusive<usize>) {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                content.to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor.set_selection_save_col(Selection::range(
                Pos::from_row_column(*selected_range.start(), 0),
                Pos::from_row_column(*selected_range.end(), 0),
            ));
            let mut result_buffer = [0; 256];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            fn trimmed_compare(a: &str, b: &str) {
                assert_eq!(a.lines().count(), b.lines().count(), "{} != {}", a, b);

                let a_lines = a.lines();
                let b_lines = b.lines();
                for (a_line, b_line) in a_lines.zip(b_lines) {
                    assert_eq!(a_line.trim_start(), b_line.trim_start());
                }
            }
            trimmed_compare(
                expected,
                &app.copy_selected_rows_with_result_to_clipboard(
                    &units,
                    &mut RenderBuckets::new(),
                    &mut result_buffer,
                    &arena,
                    &results,
                ),
            );
        }
        t("1", "1  █ 1\n", 0..=0);
        t("1 + 2", "1 + 2  █ 3\n", 0..=0);
        t("23", "23  █ 23\n", 0..=0);
        t(
            "1\n\
           23",
            "1   █  1\n\
             23  █ 23\n",
            0..=1,
        );
        t(
            "1\n\
           23\n\
           99999.66666",
            "1   █  1\n\
                 23  █ 23\n",
            0..=1,
        );
        t(
            "1\n\
           23\n\
           99999.66666",
            "1            █      1\n\
             23           █     23\n\
             99999.66666  █ 99 999.66666\n",
            0..=2,
        );
        t("[1]", "[1]  █ [1]\n", 0..=0);
        t(
            "[1]\n\
             [23]",
            "[1]  █ [1]\n",
            0..=0,
        );
        t(
            "[1]\n\
             [23]",
            "[1]   █ [ 1]\n\
             [23]  █ [23]\n",
            0..=1,
        );
        t("[1,2,3]", "[1  2  3]  █ [1  2  3]\n", 0..=0);
        t(
            "[1,2,3]\n[33, 44, 55]",
            "[1  2  3]     █ [ 1   2   3]\n\
             [33  44  55]  █ [33  44  55]\n",
            0..=1,
        );
        t(
            "[1;2;3]",
            "┌ ┐  █ ┌ ┐\n\
             │1│  █ │1│\n\
             │2│  █ │2│\n\
             │3│  █ │3│\n\
             └ ┘  █ └ ┘\n",
            0..=0,
        );
        t(
            "[1, 2, 3] * [1;2;3]",
            "            ┌ ┐  █\n\
                         │1│  █\n\
             [1  2  3] * │2│  █ [14]\n\
                         │3│  █\n\
                         └ ┘  █\n",
            0..=0,
        );
        // test alignment
        t(
            "[1, 2, 3]\n'asd\n[1, 2, 3]\n[10, 20, 30]",
            "[1  2  3]     █ [1  2  3]\n\
             'asd          █\n\
             [1  2  3]     █ [ 1   2   3]\n\
             [10  20  30]  █ [10  20  30]\n",
            0..=3,
        );

        // test alignment + thousand grouping
        t(
            "[1;2.3;2222;4km;50000]",
            // Result
            "┌     ┐  █ ┌           ┐\n\
             │    1│  █ │     1.0   │\n\
             │  2.3│  █ │     2.3   │\n\
             │ 2222│  █ │ 2 222.0   │\n\
             │  4km│  █ │     4.0 km│\n\
             │50000│  █ │50 000.0   │\n\
             └     ┘  █ └           ┘\n",
            0..=0,
        );
        // test selecting only a single line
        t(
            "[1, 2, 3]\n'asd\n[1, 2, 3]\n[10, 20, 30]",
            "[1  2  3]  █ [1  2  3]\n",
            2..=2,
        );
        t(
            "_999
22222
3
4
2
&[4]
722
alma = 3
alma * 2
alma * &[7] + &[6] 
&[7]
2222222222222222222722.22222
^
human brain: 10^16 op/s
so far 100 000 000 000 humans lived
avg. human lifespan is 50 years
total human brain activity is &[14] * &[15] * (&[16]/1s)",
            "2222222222222222222722.22222  █ 2 222 222 222 222 222 222 722.22222\n",
            11..=11,
        );
    }

    #[test]
    fn test_line_ref_selection() {
        // left
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "16892313\n14 * ".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::shift(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Backspace,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("16892313\n14 * &[1", app.editor_content.get_content());
        }
        // right
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "16892313\n14 * ".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.editor
                .set_selection_save_col(Selection::single_r_c(1, 5));
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);

            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::shift(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Del,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("16892313\n14 * [1]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_line_ref_selection_with_mouse() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "16892313\n3\n14 * ".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(2, 5));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_click(
            125,
            RenderPosY::new(0),
            &editor_objects,
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("16892313\n3\n14 * &[1", app.editor_content.get_content());
    }

    #[test]
    fn test_pressing_tab_on_m_char() {
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "m".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Tab,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!("[0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "am".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Tab,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!("am  ", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "a m".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Tab,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_eq!("a [0]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_that_cursor_is_inside_matrix_on_creation() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();
        app.handle_paste(
            "m".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('1'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("[1]", app.editor_content.get_content());
    }

    #[test]
    fn test_matrix_alt_plus_right() {
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1,0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1,0,0,0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1;2]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1,0;2,0]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_matrix_alt_plus_left() {
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1, 2, 3]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1,2]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1, 2, 3]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1, 2, 3; 4,5,6]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1,2;4,5]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_matrix_alt_plus_down() {
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1;0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1;0;0;0]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1,2]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            // this render is important, it tests a bug!
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1,2;0,0]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_that_enlarging_matrix_rerenders_everything_below_it() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "[1]\n1\n2\n3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        // move behind the matrix
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 3));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        for i in 0..3 {
            assert!(app.result_area_redraw.need(EditorY::new(i)));
            assert!(app.editor_area_redraw.need(EditorY::new(i)));
        }
    }

    #[test]
    fn test_matrix_alt_plus_up() {
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1; 2; 3]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1;2]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1; 2; 3]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1]", app.editor_content.get_content());
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "[1, 2, 3; 4,5,6]".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Enter,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            assert_eq!("[1,2,3]", app.editor_content.get_content());
        }
    }

    #[test]
    fn test_autocompletion_single() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 12$".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("apple = 12$\napple", app.editor_content.get_content());
    }

    #[test]
    fn test_autocompletion_var_name_with_space() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "some apples = 12$".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('s'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('o'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(
            "some apples = 12$\nsome apples",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_autocompletion_var_name_with_space2() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "some apples = 12$".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('s'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('o'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('m'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('e'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(
            "some apples = 12$\nsome apples",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_autocompletion_var_name_with_space3() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "men BMR = 12".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('m'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('e'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('n'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char(' '),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("men BMR = 12\nmen BMR ", app.editor_content.get_content());
    }

    #[test]
    fn test_autocompletion_only_above_vars() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 12$".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Home,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("a   \napple = 12$", app.editor_content.get_content());
    }

    #[test]
    fn test_autocompletion_two_vars() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 12$\nbanana = 7$\n".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(
            "apple = 12$\nbanana = 7$\napple",
            app.editor_content.get_content()
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char(' '),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('b'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(
            "apple = 12$\nbanana = 7$\napple banana",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_that_no_autocompletion_for_multiple_results() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 12$\nananas = 7$\n".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Tab,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(
            "apple = 12$\nananas = 7$\na   ",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_click_1() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "'1st row\n[1;2;3] some text\n'3rd row".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // click after the vector in 2nd row
        app.handle_click(
            LEFT_GUTTER_WIDTH + 4,
            RenderPosY::new(2),
            &editor_objects,
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(
            "'1st row\n[1;2;3] Xsome text\n'3rd row",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_click() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "'1st row\nsome text [1;2;3]\n'3rd row".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // click after the vector in 2nd row
        app.handle_click(
            LEFT_GUTTER_WIDTH + 4,
            RenderPosY::new(2),
            &editor_objects,
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(
            "'1st row\nsomeX text [1;2;3]\n'3rd row",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_click_after_eof() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "'1st row\n[1;2;3] some text\n'3rd row".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_click(
            LEFT_GUTTER_WIDTH + 40,
            RenderPosY::new(2),
            &editor_objects,
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(
            "'1st row\n[1;2;3] some textX\n'3rd row",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_click_after_eof2() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "'1st row\n[1;2;3] some text\n'3rd row".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_click(
            LEFT_GUTTER_WIDTH + 40,
            RenderPosY::new(40),
            &editor_objects,
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(
            "'1st row\n[1;2;3] some text\n'3rd rowX",
            app.editor_content.get_content()
        );
    }

    #[test]
    fn test_variable() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 12".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_paste(
            "apple + 2".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["12", "14"][..], &result_buffer);
    }

    #[test]
    fn test_variable_must_be_defined() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 12".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Home,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_paste(
            "apple + 2".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["2", "12"][..], &result_buffer);
    }

    #[test]
    fn test_variables_can_be_defined_afterwards_of_their_usage() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple * 2".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["", "2"][..], &result_buffer);
        // now define the variable 'apple'
        app.handle_paste(
            "apple = 3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["3", "6"][..], &result_buffer);
    }

    #[test]
    fn test_renaming_variable_declaration() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 2\napple * 3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["2", "6"][..], &result_buffer);

        // rename apple to aapple
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["2", "3"][..], &result_buffer);
    }

    #[test]
    fn test_moving_line_does_not_change_its_lineref() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1\n2\n3\n\n\n50year".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        // cursor is in 4th row
        app.editor
            .set_selection_save_col(Selection::single_r_c(3, 0));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["1", "2", "3", "", "", "50 year"][..], &result_buffer);

        // insert linref of 1st line
        for _ in 0..3 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('+'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        // insert linref of 2st line
        for _ in 0..2 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::alt(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('+'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        // insert linref of 3rd line
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        match &tokens[EditorY::new(3)] {
            Some(Tokens {
                tokens,
                shunting_output_stack: _,
            }) => {
                match tokens[0].typ {
                    TokenType::LineReference { var_index } => assert_eq!(var_index, 0),
                    _ => panic!(),
                }
                match tokens[2].typ {
                    TokenType::LineReference { var_index } => assert_eq!(var_index, 1),
                    _ => panic!(),
                }
                match tokens[4].typ {
                    TokenType::LineReference { var_index } => assert_eq!(var_index, 2),
                    _ => panic!(),
                }
            }
            _ => {}
        };

        // insert a newline between the 1st and 2nd row
        for _ in 0..3 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        assert_results(&["1", "", "2", "3", "6", "", "50 year"][..], &result_buffer);

        match &tokens[EditorY::new(4)] {
            Some(Tokens {
                tokens,
                shunting_output_stack: _,
            }) => {
                match tokens[0].typ {
                    TokenType::LineReference { var_index } => assert_eq!(var_index, 0),
                    _ => panic!("{:?}", &tokens[0]),
                }
                match tokens[2].typ {
                    TokenType::LineReference { var_index } => assert_eq!(var_index, 2),
                    _ => panic!("{:?}", &tokens[2]),
                }
                match tokens[4].typ {
                    TokenType::LineReference { var_index } => assert_eq!(var_index, 3),
                    _ => panic!("{:?}", &tokens[4]),
                }
            }
            _ => {}
        };
    }

    #[test]
    fn test_modifying_a_lineref_recalcs_its_dependants() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "2\n * 3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(1, 0));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["2", "3"][..], &result_buffer);

        // insert linref of 1st line
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["2", "6"][..], &result_buffer);

        // now modify the first row
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Home,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('1'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["12", "36"][..], &result_buffer);
    }

    #[test]
    fn test_renaming_variable_declaration2() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 2\naapple * 3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["2", "3"][..], &result_buffer);

        // rename apple to aapple
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["2", "6"][..], &result_buffer);
    }

    #[test]
    fn test_removing_variable_declaration() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 2\napple * 3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["2", "6"][..], &result_buffer);

        // remove the content of the first line
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::End,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));
        assert!(!app.result_area_redraw.need(EditorY::new(1)));
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Del,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["", "3"][..], &result_buffer);
    }

    #[test]
    fn test_that_variable_dependent_rows_are_recalculated() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 2\napple * 3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 9));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["2", "6"][..], &result_buffer);

        // change value of 'apple' from 2 to 24
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('4'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["24", "72"][..], &result_buffer);
    }

    #[test]
    fn test_variable_redefine() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 12".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_paste(
            "apple + 2".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_paste(
            "apple = 0".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_paste(
            "apple + 3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["12", "14", "0", "3"][..], &result_buffer);
    }

    #[test]
    fn test_backspace_bug_editor_obj_deletion_for_simple_tokens() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "asd sad asd asd sX".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!("asd sad asd asd s", app.editor_content.get_content());
    }

    #[test]
    fn test_rendering_while_cursor_move() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "apple = 12$\nasd q".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn stepping_into_a_matrix_renders_it_some_lines_below() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "asdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 2));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));

        assert_eq!(editor_objects[EditorY::new(0)].len(), 1);
        assert_eq!(editor_objects[EditorY::new(1)].len(), 1);

        assert_eq!(app.render_data.get_rendered_height(EditorY::new(0)), 1);
        assert_eq!(app.render_data.get_rendered_height(EditorY::new(1)), 6);
        assert_eq!(
            app.render_data.get_render_y(EditorY::new(0)),
            Some(RenderPosY::new(0))
        );
        assert_eq!(
            app.render_data.get_render_y(EditorY::new(1)),
            Some(RenderPosY::new(1))
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));

        assert_eq!(editor_objects[EditorY::new(0)].len(), 1);
        assert_eq!(editor_objects[EditorY::new(1)].len(), 1);
        assert_eq!(app.render_data.get_rendered_height(EditorY::new(0)), 1);
        assert_eq!(app.render_data.get_rendered_height(EditorY::new(1)), 6);
        assert_eq!(
            app.render_data.get_render_y(EditorY::new(0)),
            Some(RenderPosY::new(0))
        );
        assert_eq!(
            app.render_data.get_render_y(EditorY::new(1)),
            Some(RenderPosY::new(1))
        );
    }

    #[test]
    fn end_matrix_editing_should_rerender_matrix_row_too() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "asdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 2));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        // step into matrix
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));

        // leave matrix
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
    }

    #[test]
    fn clicks_rerender_prev_row_too() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "asdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_click(
            4,
            RenderPosY::new(0),
            &editor_objects,
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
    }

    #[test]
    fn select_only_2_lines_render_bug() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1\n2\n3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        let mut result_buffer = [0; 128];
        let mut render_buckets = RenderBuckets::new();
        app.render(
            &units,
            &mut render_buckets,
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        let mut ok = vec![];
        for command in &render_buckets.custom_commands[Layer::AboveText as usize] {
            match command {
                OutputMessage::RenderChar(x, y, ch) => {
                    if *x == 8 && *y == 1 && *ch == '⎫' {
                        ok.push('a');
                    } else if *x == 8 && *y == 2 && *ch == '⎭' {
                        ok.push('b');
                    }
                }
                OutputMessage::RenderString(RenderStringMsg { text, row, column }) => {
                    if text == " ∑ = 5" && row.as_usize() == 1 && *column == 8 {
                        ok.push('c');
                    }
                }
                _ => {}
            }
        }
        assert_eq!(vec!['a', 'b', 'c'], ok);
    }

    #[test]
    fn moving_single_line_selection_rerenders_both_sum_and_line() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n1+2+3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // select the expression in the last row
        for _ in 0..5 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::shift(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // end the selection, cursor stays in the same row
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Right,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        // the cursor line + the line above which contains the SUM should be rerendered
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        // end the selection, cursor stays in the same row
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        // the cursor line + the line above which contains the SUM should be rerendered
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn test_ending_selection_with_click_clears_sum_too() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n1+2+3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // select the expression in the last row
        for _ in 0..5 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::shift(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // end the selection
        app.handle_click(
            30,
            RenderPosY::new(2),
            &editor_objects,
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        // the cursor line + the line above which contains the SUM should be rerendered
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn single_line_selection_end_rerender() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n1+2+3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // select the expression in the last row
        for _ in 0..5 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Left,
                InputModifiers::shift(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // end the selection, cursor stays in the same row
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Right,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        // the cursor line + the line above which contains the SUM should be rerendered
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn single_line_selection_end_rerender_in_the_first_row() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1+2+3\nasdsad\nasdqweqwe".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // select the expression in the first row
        for _ in 0..5 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::shift(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // end the selection, cursor stays in the same row
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Right,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        // the cursor line + the line below which contains the SUM should be rerendered
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(!app.editor_area_redraw.need(EditorY::new(2)));
        assert!(!app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn test_removing_selected_text_clears_the_sum() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "qweqwe\nasdsad\n1+2+3\n3\n4".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(2, 0));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // select the expression in the 2nd row
        for _ in 0..5 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::shift(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // remove the selected text
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Del,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        // clears the SUM
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        // clears the modified row
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));

        assert!(!app.editor_area_redraw.need(EditorY::new(3)));
        assert!(!app.result_area_redraw.need(EditorY::new(3)));
    }

    #[test]
    fn test_single_to_multi_line_selection_clears() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "qweqwe\nasdsad\n1+2+3\n3\n4".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(2, 0));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // select the expression in the 2nd row
        for _ in 0..5 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::shift(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // expand selection
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        // clears the SUM
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        // clears the selected rows
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
        assert!(app.editor_area_redraw.need(EditorY::new(3)));
        assert!(app.result_area_redraw.need(EditorY::new(3)));
    }

    #[test]
    fn test_multi_to_single_line_selection_clears() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "qweqwe\nasdsad\n1+2+3\n3\n4".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(2, 0));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // select the expression in the 2nd row
        for _ in 0..5 {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Right,
                InputModifiers::shift(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // expand selection
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // reduce selection
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        // 1st row contains the SUM
        // 2nd and 3rd rows contain the selection, all of them must be updated
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
        assert!(app.editor_area_redraw.need(EditorY::new(3)));
        assert!(app.result_area_redraw.need(EditorY::new(3)));
    }

    #[test]
    fn page_down_should_rerender_only_2_rows() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n1+2+3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::PageDown,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));
        assert!(!app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn page_up_should_rerender_only_2_rows() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n1+2+3".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::PageUp,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        // the cursor line + the line above which contains the SUM should be rerendered
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));
        assert!(!app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn all_the_selected_rows_are_rerendered() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn when_selection_shrinks_upward_rerender_prev_row() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(0, 0));
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn when_selection_shrinks_downward_rerender_prev_row() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn all_the_selected_rows_are_rerendered_on_ticking() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));
        assert!(!app.result_area_redraw.need(EditorY::new(1)));
        assert!(!app.editor_area_redraw.need(EditorY::new(2)));
        assert!(!app.result_area_redraw.need(EditorY::new(2)));

        app.handle_time(1000);
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(!app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(!app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn all_the_selected_rows_are_rerendered_on_cancellation() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "r1\nr2 asdsad\nr3 [1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // cancels selection
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn navigating_up_renders() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "r1\nr2 asdsad\nr3 [1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn all_the_selected_rows_are_rerendered_on_cancellation2() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "asdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        // cancels selection
        app.handle_click(
            4,
            RenderPosY::new(0),
            &editor_objects,
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
    }

    #[test]
    fn line_ref_movement_render() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "first\nasdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);
        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));
        assert!(!app.result_area_redraw.need(EditorY::new(1)));
        assert!(!app.editor_area_redraw.need(EditorY::new(2)));
        assert!(!app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn line_ref_movement_render_with_actual_insertion() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "firs 1t\nasdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(!app.editor_area_redraw.need(EditorY::new(2)));
        assert!(!app.result_area_redraw.need(EditorY::new(2)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(!app.editor_area_redraw.need(EditorY::new(2)));
        assert!(!app.result_area_redraw.need(EditorY::new(2)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.alt_key_released(&units, &arena, &mut tokens, &mut results, &mut vars);

        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));
        assert!(!app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn dragging_right_gutter_rerenders_everyhting() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "firs 1t\nasdsad\n[1;2;3;4]".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        app.handle_click(
            app.render_data.result_gutter_x,
            RenderPosY::new(0),
            &editor_objects,
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        assert!(!app.editor_area_redraw.need(EditorY::new(0)));
        assert!(!app.result_area_redraw.need(EditorY::new(0)));
        assert!(!app.editor_area_redraw.need(EditorY::new(1)));
        assert!(!app.result_area_redraw.need(EditorY::new(1)));
        assert!(!app.editor_area_redraw.need(EditorY::new(2)));
        assert!(!app.result_area_redraw.need(EditorY::new(2)));

        assert_eq!(app.render_data.current_result_panel_width, 30);
        app.handle_drag(app.render_data.result_gutter_x - 1, RenderPosY::new(0));
        assert_eq!(app.render_data.current_result_panel_width, 31);

        assert!(app.editor_area_redraw.need(EditorY::new(0)));
        assert!(app.result_area_redraw.need(EditorY::new(0)));
        assert!(app.editor_area_redraw.need(EditorY::new(1)));
        assert!(app.result_area_redraw.need(EditorY::new(1)));
        assert!(app.editor_area_redraw.need(EditorY::new(2)));
        assert!(app.result_area_redraw.need(EditorY::new(2)));
    }

    #[test]
    fn test_sum_rerender() {
        // rust's shitty borrow checker forces me to do this
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "1\n2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "1\n2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "1\n2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "1\n2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "1\n2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
    }

    #[test]
    fn test_sum_rerender_with_ignored_lines() {
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "1\n'2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();
            app.handle_paste(
                "1\n'2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();
            app.handle_paste(
                "1\n'2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();
            app.handle_paste(
                "1\n'2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();
            app.handle_paste(
                "1\n'2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
    }

    #[test]
    fn test_sum_rerender_with_sum_reset() {
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "1\n--2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "3", "3"][..], &result_buffer);
        }
        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            let arena = Arena::new();

            app.handle_paste(
                "1\n--2\n3\nsum".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Up,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "3", "3"][..], &result_buffer);
        }
    }

    #[test]
    fn test_paste_long_text() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "a\nb\na\nb\na\nb\na\nb\na\nb\na\nb\n1".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        for i in 0..12 {
            assert!(app.editor_area_redraw.need(EditorY::new(i)));
            assert!(app.result_area_redraw.need(EditorY::new(i)));
        }

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(
            &["", "", "", "", "", "", "", "", "", "", "", "1"][..],
            &result_buffer,
        );
    }

    #[test]
    fn test_thousand_separator_and_alignment_in_result() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1\n2.3\n2222\n4km\n50000".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single_r_c(2, 0));
        // set result to binary repr
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Left,
            InputModifiers::alt(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        let mut result_buffer = [0; 128];
        let mut render_buckets = RenderBuckets::new();
        app.render(
            &units,
            &mut render_buckets,
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        let base_y = render_buckets.ascii_texts[0].column;
        assert_eq!(render_buckets.ascii_texts[0].text, "1".as_bytes());
        assert_eq!(render_buckets.ascii_texts[0].row, RenderPosY::new(0));

        assert_eq!(render_buckets.ascii_texts[1].text, "2".as_bytes());
        assert_eq!(render_buckets.ascii_texts[1].row, RenderPosY::new(1));
        assert_eq!(render_buckets.ascii_texts[1].column, base_y);

        assert_eq!(render_buckets.ascii_texts[2].text, ".3".as_bytes());
        assert_eq!(render_buckets.ascii_texts[2].row, RenderPosY::new(1));
        assert_eq!(render_buckets.ascii_texts[2].column, base_y + 1);

        assert_eq!(
            render_buckets.ascii_texts[3].text,
            "1000 10101110".as_bytes()
        );
        assert_eq!(render_buckets.ascii_texts[3].row, RenderPosY::new(2));
        assert_eq!(render_buckets.ascii_texts[3].column, base_y - 12);

        assert_eq!(render_buckets.ascii_texts[4].text, "4".as_bytes());
        assert_eq!(render_buckets.ascii_texts[4].row, RenderPosY::new(3));
        assert_eq!(render_buckets.ascii_texts[4].column, base_y);

        assert_eq!(render_buckets.ascii_texts[5].text, "km".as_bytes());
        assert_eq!(render_buckets.ascii_texts[5].row, RenderPosY::new(3));
        assert_eq!(render_buckets.ascii_texts[5].column, base_y + 2);

        assert_eq!(render_buckets.ascii_texts[6].text, "50 000".as_bytes());
        assert_eq!(render_buckets.ascii_texts[6].row, RenderPosY::new(4));
        assert_eq!(render_buckets.ascii_texts[6].column, base_y - 5);
    }

    #[test]
    fn test_ctrl_x() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('x'),
            InputModifiers::ctrl(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        for i in 0..9 {
            assert!(!app.editor_area_redraw.need(EditorY::new(i)));
            assert!(!app.result_area_redraw.need(EditorY::new(i)));
        }
        for i in 9..12 {
            assert!(app.editor_area_redraw.need(EditorY::new(i)));
            assert!(app.result_area_redraw.need(EditorY::new(i)));
        }

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(
            &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"][..],
            &result_buffer,
        );
    }

    #[test]
    fn test_ctrl_x_then_ctrl_z() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "12".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_time(1000);
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["12"][..], &result_buffer);

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('x'),
            InputModifiers::ctrl(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&[""][..], &result_buffer);
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('z'),
            InputModifiers::ctrl(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_results(&["12"][..], &result_buffer);
    }

    #[test]
    fn selection_in_the_first_row_should_not_panic() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "1+1\nasd".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Up,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Home,
            InputModifiers::shift(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn test_removing_height_matrix_rerenders_everything_below_it() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "a\nb\n[1;2;3]\nb\na\nb\na\nb\na\nb\na\nb\n1".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single(Pos::from_row_column(2, 7)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        // step into the vector
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        for i in 0..2 {
            assert!(!app.editor_area_redraw.need(EditorY::new(i)));
            assert!(!app.result_area_redraw.need(EditorY::new(i)));
        }
        for i in 2..12 {
            assert!(app.editor_area_redraw.need(EditorY::new(i)));
            assert!(app.result_area_redraw.need(EditorY::new(i)));
        }
    }

    #[test]
    fn test_that_removed_tail_rows_are_cleared() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "a\nb\n[1;2;3]\nX\na\n1".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single(Pos::from_row_column(3, 0)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_ne!(
            app.render_data.get_render_y(EditorY::new(5)),
            Some(RenderPosY::new(0))
        );

        // removing a line
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        // they must not be 0, otherwise the renderer can't decide if they needed to be cleared,
        assert_ne!(
            app.render_data.get_render_y(EditorY::new(5)),
            Some(RenderPosY::new(0))
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        assert_eq!(app.render_data.get_render_y(EditorY::new(5)), None);
    }

    #[test]
    fn test_that_pressing_enter_eof_moves_scrollbar_down() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        // editor height is 36 in tests, so create a 35 line text
        app.handle_paste(
            "a\n".repeat(35),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single(Pos::from_row_column(3, 0)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
        assert_ne!(
            app.render_data.get_render_y(EditorY::new(5)),
            Some(RenderPosY::new(0))
        );

        // removing a line
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn navigating_to_bottom_no_panic() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        let arena = Arena::new();

        app.handle_paste(
            "aaaaaaaaaaaa\n".repeat(34),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        // removing a line
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::PageDown,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn ctrl_a_plus_typing() {
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(25);
        let arena = Arena::new();

        app.handle_paste(
            "1\n".repeat(34).to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single(Pos::from_row_column(0, 0)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::ctrl(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('1'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );
    }

    #[test]
    fn test_that_scrollbar_stops_at_bottom() {
        let client_height = 25;
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(client_height);
        let arena = Arena::new();

        app.handle_paste(
            "1\n".repeat(client_height * 2).to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single(Pos::from_row_column(0, 0)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::PageDown,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        assert_eq!(app.render_data.scroll_y, client_height - 1);
    }

    #[test]
    fn test_that_no_full_refresh_when_stepping_into_last_line() {
        let client_height = 25;
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(client_height);
        let arena = Arena::new();

        app.handle_paste(
            "1\n".repeat(client_height * 2).to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single(Pos::from_row_column(0, 0)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        // step into last-1 line
        for _i in 0..(client_height - 2) {
            app.handle_input_and_update_tokens_plus_redraw_requirements(
                EditorInputEvent::Down,
                InputModifiers::none(),
                &arena,
                &units,
                &mut tokens,
                &mut results,
                &mut vars,
                &mut editor_objects,
            );
        }
        // rerender so flags are cleared
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        // step into last line
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(app.render_data.scroll_y, 0);
        assert_eq!(app.editor_area_redraw.need(EditorY::new(0)), false);
        assert_eq!(app.result_area_redraw.need(EditorY::new(0)), false);

        // this step scrolls down one
        // step into last line
        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Down,
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );
        assert_eq!(app.render_data.scroll_y, 1);
        for i in 0..client_height {
            assert_eq!(app.editor_area_redraw.need(EditorY::new(i)), true);
            assert_eq!(app.result_area_redraw.need(EditorY::new(i)), true);
        }
    }

    #[test]
    fn test_that_paste_rerenders_the_modified_area_when_scrolled_down() {
        let client_height = 25;
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(client_height);
        let arena = Arena::new();

        app.handle_paste(
            "1\n".repeat(client_height * 2).to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single(Pos::from_row_column(
                client_height - 10,
                0,
            )));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        // paste at (client_height-10, 0)
        app.handle_paste(
            "2\n".repeat(5).to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );

        for i in 0..5 {
            assert_eq!(
                app.editor_area_redraw
                    .need(EditorY::new(client_height - 10 + i)),
                true
            );
            assert_eq!(
                app.result_area_redraw
                    .need(EditorY::new(client_height - 10 + i)),
                true
            );
        }
    }

    #[test]
    fn test_that_removed_lines_are_cleared() {
        let client_height = 25;
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(client_height);
        let arena = Arena::new();

        app.handle_paste(
            "1\n".repeat(client_height * 2).to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        app.editor
            .set_selection_save_col(Selection::single(Pos::from_row_column(0, 0)));

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('a'),
            InputModifiers::ctrl(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_input_and_update_tokens_plus_redraw_requirements(
            EditorInputEvent::Char('1'),
            InputModifiers::none(),
            &arena,
            &units,
            &mut tokens,
            &mut results,
            &mut vars,
            &mut editor_objects,
        );

        for i in 1..(client_height * 2 - 1) {
            assert!(app.editor_area_redraw.need(EditorY::new(i)));
            assert!(app.result_area_redraw.need(EditorY::new(i)));
        }

        let mut result_buffer = [0; 128];
        let mut render_buckets = RenderBuckets::new();
        app.render(
            &units,
            &mut render_buckets,
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        // [0] SetColor
        // [1] clears the scrollbar
        // [2] clears the first row
        match &render_buckets.clear_commands[2] {
            OutputMessage::RenderRectangle { x, y, w, h } => {
                assert_eq!(*x, 4);
                assert_eq!(y.as_usize(), 0);
                assert_eq!(*w, 121);
                assert_eq!(*h, 1);
            }
            _ => assert!(false),
        }

        // [3] clears everything below it
        match &render_buckets.clear_commands[3] {
            OutputMessage::RenderRectangle { x, y, w, h } => {
                assert_eq!(*x, 0);
                assert_eq!(y.as_usize(), 0);
                assert!(*w > app.render_data.current_editor_width);
                assert_eq!(*h, client_height);
            }
            _ => assert!(false),
        }

        assert_eq!(
            None,
            app.render_data
                .get_render_y(EditorY::new(client_height * 2 - 1))
        );
    }

    #[test]
    fn test_that_no_overscrolling() {
        let arena = Arena::new();
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        app.handle_paste(
            "1\n2\n3\n".to_owned(),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_wheel(1);
        assert_eq!(0, app.render_data.scroll_y);
    }

    #[test]
    fn test_that_no_overscrolling2() {
        let arena = Arena::new();
        let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
            create_app(35);
        app.handle_paste(
            "aaaaaaaaaaaa\n".repeat(35),
            &units,
            &arena,
            &mut tokens,
            &mut results,
            &mut vars,
        );
        let mut result_buffer = [0; 128];
        app.render(
            &units,
            &mut RenderBuckets::new(),
            &mut result_buffer,
            &arena,
            &tokens,
            &results,
            &vars,
            &mut editor_objects,
        );

        app.handle_wheel(1);
        assert_eq!(1, app.render_data.scroll_y);
        app.handle_wheel(1);
        assert_eq!(1, app.render_data.scroll_y);
    }

    #[test]
    fn test_that_scrolled_result_is_not_rendered() {
        let arena = Arena::new();

        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            app.handle_paste(
                "1\n2\n3\n".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_paste(
                "aaaaaaaaaaaa\n".repeat(34),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );

            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "2", "3"][..], &result_buffer);
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(0)),
                Some(RenderPosY::new(0))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(1)),
                Some(RenderPosY::new(1))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(2)),
                Some(RenderPosY::new(2))
            );
            assert_eq!(app.render_data.get_render_y(EditorY::new(35)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(36)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(37)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(38)), None);
        }

        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            app.handle_paste(
                "1\n2\n3\n".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_paste(
                "aaaaaaaaaaaa\n".repeat(34),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );

            app.handle_wheel(1);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["2", "3"][..], &result_buffer);
            assert_eq!(app.render_data.get_render_y(EditorY::new(0)), None);
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(1)),
                Some(RenderPosY::new(0))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(2)),
                Some(RenderPosY::new(1))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(35)),
                Some(RenderPosY::new(34))
            );
            assert_eq!(app.render_data.get_render_y(EditorY::new(36)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(37)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(38)), None);
        }

        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            app.handle_paste(
                "1\n2\n3\n".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_paste(
                "aaaaaaaaaaaa\n".repeat(34),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );

            app.handle_wheel(1);
            app.handle_wheel(1);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["3"][..], &result_buffer);
            assert_eq!(app.render_data.get_render_y(EditorY::new(0)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(1)), None);
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(2)),
                Some(RenderPosY::new(0))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(35)),
                Some(RenderPosY::new(33))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(36)),
                Some(RenderPosY::new(34))
            );
            assert_eq!(app.render_data.get_render_y(EditorY::new(37)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(38)), None);
        }

        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            app.handle_paste(
                "1\n2\n3\n".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_paste(
                "aaaaaaaaaaaa\n".repeat(34),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );

            app.handle_wheel(1);
            app.handle_wheel(1);
            app.handle_wheel(1);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&[""][..], &result_buffer);
            assert_eq!(app.render_data.get_render_y(EditorY::new(0)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(1)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(2)), None);
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(35)),
                Some(RenderPosY::new(32))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(36)),
                Some(RenderPosY::new(33))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(37)),
                Some(RenderPosY::new(34))
            );
            assert_eq!(app.render_data.get_render_y(EditorY::new(38)), None);
        }

        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            app.handle_paste(
                "1\n2\n3\n".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_paste(
                "aaaaaaaaaaaa\n".repeat(34),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );

            app.handle_wheel(1);
            app.handle_wheel(1);
            app.handle_wheel(1);
            app.handle_wheel(0);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["3"][..], &result_buffer);
            assert_eq!(app.render_data.get_render_y(EditorY::new(0)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(1)), None);
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(2)),
                Some(RenderPosY::new(0))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(35)),
                Some(RenderPosY::new(33))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(36)),
                Some(RenderPosY::new(34))
            );
            assert_eq!(app.render_data.get_render_y(EditorY::new(37)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(38)), None);
        }

        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            app.handle_paste(
                "1\n2\n3\n".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_paste(
                "aaaaaaaaaaaa\n".repeat(34),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );

            app.handle_wheel(1);
            app.handle_wheel(1);
            app.handle_wheel(1);
            app.handle_wheel(0);
            app.handle_wheel(0);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["2", "3"][..], &result_buffer);
            assert_eq!(app.render_data.get_render_y(EditorY::new(0)), None);
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(1)),
                Some(RenderPosY::new(0))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(2)),
                Some(RenderPosY::new(1))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(35)),
                Some(RenderPosY::new(34))
            );
            assert_eq!(app.render_data.get_render_y(EditorY::new(36)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(37)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(38)), None);
        }

        {
            let (mut app, units, (mut tokens, mut results, mut vars, mut editor_objects)) =
                create_app(35);
            app.handle_paste(
                "1\n2\n3\n".to_owned(),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            app.handle_paste(
                "aaaaaaaaaaaa\n".repeat(34),
                &units,
                &arena,
                &mut tokens,
                &mut results,
                &mut vars,
            );
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );

            app.handle_wheel(1);
            app.handle_wheel(1);
            app.handle_wheel(1);
            app.handle_wheel(0);
            app.handle_wheel(0);
            app.handle_wheel(0);
            let mut result_buffer = [0; 128];
            app.render(
                &units,
                &mut RenderBuckets::new(),
                &mut result_buffer,
                &arena,
                &tokens,
                &results,
                &vars,
                &mut editor_objects,
            );
            assert_results(&["1", "2", "3"][..], &result_buffer);
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(0)),
                Some(RenderPosY::new(0))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(1)),
                Some(RenderPosY::new(1))
            );
            assert_eq!(
                app.render_data.get_render_y(EditorY::new(2)),
                Some(RenderPosY::new(2))
            );
            assert_eq!(app.render_data.get_render_y(EditorY::new(35)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(36)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(37)), None);
            assert_eq!(app.render_data.get_render_y(EditorY::new(38)), None);
        }
    }
}
