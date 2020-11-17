#![feature(const_fn, const_panic, drain_filter)]
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

use std::io::Cursor;
use std::mem::MaybeUninit;
use std::ops::Range;
use std::time::Duration;

use bumpalo::Bump;
use smallvec::SmallVec;
use strum_macros::EnumDiscriminants;

use helper::*;

use crate::calc::{
    add_op, evaluate_tokens, CalcResult, CalcResultType, EvaluationResult, ShuntingYardResult,
};
use crate::consts::{LINE_NUM_CONSTS, LINE_NUM_CONSTS2};
use crate::editor::editor::{
    Editor, EditorInputEvent, InputModifiers, Pos, RowModificationType, Selection,
};
use crate::editor::editor_content::EditorContent;
use crate::matrix::MatrixData;
use crate::renderer::{get_int_frac_part_len, render_result, render_result_into};
use crate::shunting_yard::ShuntingYard;
use crate::token_parser::{OperatorTokenType, Token, TokenParser, TokenType};
use crate::units::units::Units;

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
const LEFT_GUTTER_MIN_WIDTH: usize = 2;
pub const MAX_LINE_COUNT: usize = 64;
const SCROLL_BAR_WIDTH: usize = 1;
const RIGHT_GUTTER_WIDTH: usize = 2;
const CHANGE_RESULT_PULSE_START_COLOR: u32 = 0xFF88FF_AA;
const CHANGE_RESULT_PULSE_END_COLOR: u32 = 0xFFFFFF_55;
const REFERENCE_PULSE_PULSE_START_COLOR: u32 = 0x00FF7F_33;
const MIN_RESULT_PANEL_WIDTH: usize = 7;
const DEFAULT_RESULT_PANEL_WIDTH_PERCENT: usize = 70;
const SUM_VARIABLE_INDEX: usize = MAX_LINE_COUNT;
const MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT: usize = 2;
const ACTIVE_LINE_REF_HIGHLIGHT_COLORS: [u32; 9] = [
    0xFFD300, 0xDE3163, 0x73c2fb, 0xc7ea46, 0x702963, 0x997950, 0x777b73, 0xFC6600, 0xED2939,
];

pub enum Click {
    Simple(Pos),
    Drag(Pos),
}

pub mod helper {
    // so code from the lib module can't access the private parts

    use std::ops::{Index, IndexMut};

    use crate::calc::CalcResultType;
    use crate::{MAX_LINE_COUNT, *};

    pub fn create_vars() -> [Option<Variable>; MAX_LINE_COUNT + 1] {
        let mut vars = [None; MAX_LINE_COUNT + 1];
        vars[SUM_VARIABLE_INDEX] = Some(Variable {
            name: Box::from(&['s', 'u', 'm'][..]),
            value: Err(()),
        });
        return vars;
    }

    #[derive(Debug)]
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

    impl Index<ContentIndex> for EditorObjects {
        type Output = Vec<EditorObject>;

        fn index(&self, index: ContentIndex) -> &Self::Output {
            &self.0[index.0]
        }
    }

    impl IndexMut<ContentIndex> for EditorObjects {
        fn index_mut(&mut self, index: ContentIndex) -> &mut Self::Output {
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

    impl Index<ContentIndex> for Results {
        type Output = LineResult;

        fn index(&self, index: ContentIndex) -> &Self::Output {
            &self.0[index.0]
        }
    }

    impl IndexMut<ContentIndex> for Results {
        fn index_mut(&mut self, index: ContentIndex) -> &mut Self::Output {
            &mut self.0[index.0]
        }
    }

    #[derive(Debug)]
    pub struct AppTokens<'a>([Option<Tokens<'a>>; MAX_LINE_COUNT]);

    impl<'a> AppTokens<'a> {
        pub fn new() -> AppTokens<'a> {
            AppTokens([None; MAX_LINE_COUNT])
        }

        pub fn iter(&self) -> std::slice::Iter<Option<Tokens<'a>>> {
            self.0.iter()
        }
    }

    impl<'a> Index<ContentIndex> for AppTokens<'a> {
        type Output = Option<Tokens<'a>>;

        fn index(&self, index: ContentIndex) -> &Self::Output {
            &self.0[index.0]
        }
    }

    impl<'a> IndexMut<ContentIndex> for AppTokens<'a> {
        fn index_mut(&mut self, index: ContentIndex) -> &mut Self::Output {
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

        pub fn as_u64(&self) -> u64 {
            self.bitset
        }

        pub fn set(&mut self, row_index: usize) {
            self.bitset |= 1u64 << row_index;
        }

        pub fn single_row(row_index: usize) -> EditorRowFlags {
            let bitset = 1u64 << row_index;
            EditorRowFlags { bitset }
        }

        #[inline]
        pub fn clear(&mut self) {
            self.bitset = 0;
        }

        pub fn all_rows_starting_at(row_index: usize) -> EditorRowFlags {
            if row_index >= MAX_LINE_COUNT {
                return EditorRowFlags { bitset: 0 };
            }
            let s = 1u64 << row_index;
            let right_to_s_bits = s - 1;
            let left_to_s_and_s_bits = !right_to_s_bits;
            let bitset = left_to_s_and_s_bits;

            EditorRowFlags { bitset }
        }
        // TODO multiple2(a, b), multiple3(a,b,c) etc, faster
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
            if from >= MAX_LINE_COUNT {
                return EditorRowFlags { bitset: 0 };
            } else if to >= MAX_LINE_COUNT {
                return EditorRowFlags::range(from, MAX_LINE_COUNT - 1);
            }
            let top = 1 << to;
            let right_to_top_bits = top - 1;
            let bottom = 1 << from;
            let right_to_bottom_bits = bottom - 1;
            let bitset = (right_to_top_bits ^ right_to_bottom_bits) | top;

            EditorRowFlags { bitset }
        }

        #[inline]
        pub fn merge(&mut self, other: EditorRowFlags) {
            self.bitset |= other.bitset;
        }

        #[inline]
        pub fn need(&self, line_index: ContentIndex) -> bool {
            ((1 << line_index.0) & self.bitset) != 0
        }

        #[inline]
        pub fn is_true(&self, line_index: usize) -> bool {
            return self.need(content_y(line_index));
        }

        #[inline]
        pub fn is_false(&self, line_index: usize) -> bool {
            return !self.is_true(line_index);
        }
    }

    #[derive(Clone)]
    pub struct GlobalRenderData {
        pub client_height: usize,
        pub scroll_y: usize,
        pub result_gutter_x: usize,
        pub left_gutter_width: usize,
        pub longest_rendered_result_len: usize,

        pub current_editor_width: usize,
        pub current_result_panel_width: usize,
        editor_y_to_render_y: [Option<CanvasY>; MAX_LINE_COUNT],
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
            let min_req_width =
                MIN_RESULT_PANEL_WIDTH + RIGHT_GUTTER_WIDTH + LEFT_GUTTER_MIN_WIDTH + 4;
            if client_width < min_req_width {
                panic!(
                    "client width is too small, it must be at least {} but it is {}",
                    min_req_width, client_width
                );
            }
            let mut r = GlobalRenderData {
                scroll_y: 0,
                longest_rendered_result_len: 0,
                result_gutter_x,
                left_gutter_width,
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

        pub fn set_result_gutter_x(&mut self, client_width: usize, x: usize) {
            self.result_gutter_x = x;
            self.current_editor_width = x - self.left_gutter_width;
            self.current_result_panel_width = client_width - x - RIGHT_GUTTER_WIDTH;
        }

        pub fn calc_bottom_y(&self, content_len: usize) -> CanvasY {
            let bottom_i = content_y(content_len - 1);
            return if let Some(y) = self.get_render_y(bottom_i) {
                y.add(self.get_rendered_height(bottom_i))
            } else {
                canvas_y(self.client_height as isize)
            };
        }

        pub fn clear_editor_y_to_render_y(&mut self) {
            for e in self.editor_y_to_render_y.iter_mut() {
                *e = None;
            }
        }

        pub fn clear(&mut self) {
            for e in self.editor_y_to_render_y.iter_mut() {
                *e = None;
            }
            for e in self.editor_y_to_rendered_height.iter_mut() {
                *e = 0;
            }
            self.scroll_y = 0;
        }

        pub fn is_visible(&self, y: ContentIndex) -> bool {
            let top = match self.get_render_y(content_y(self.scroll_y)) {
                Some(y) => y.as_isize(),
                None => {
                    return false;
                }
            };
            return if let Some(y) = self.get_render_y(y) {
                let y = y.as_isize();
                y >= top && y < (top + self.client_height as isize)
            } else {
                false
            };
        }

        pub fn get_render_y(&self, y: ContentIndex) -> Option<CanvasY> {
            self.editor_y_to_render_y[y.0]
        }

        pub fn set_render_y(&mut self, y: ContentIndex, newy: Option<CanvasY>) {
            self.editor_y_to_render_y[y.0] = newy;
        }

        pub fn editor_y_to_render_y(&self) -> &[Option<CanvasY>] {
            &self.editor_y_to_render_y
        }

        pub fn get_rendered_height(&self, y: ContentIndex) -> usize {
            self.editor_y_to_rendered_height[y.0]
        }

        pub fn set_rendered_height(&mut self, y: ContentIndex, h: usize) {
            self.editor_y_to_rendered_height[y.0] = h;
        }
    }

    pub struct PerLineRenderData {
        pub editor_x: usize,
        pub editor_y: ContentIndex,
        pub render_x: usize,
        pub render_y: CanvasY,
        // contains the y position for each editor line
        pub rendered_row_height: usize,
        pub vert_align_offset: usize,
        pub cursor_render_x_offset: isize,
    }

    impl PerLineRenderData {
        pub fn new() -> PerLineRenderData {
            let r = PerLineRenderData {
                editor_x: 0,
                editor_y: content_y(0),
                render_x: 0,
                render_y: canvas_y(0),
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
            self.render_y.0 += row_height as isize;
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
                    let result_row_height = match &result.typ {
                        CalcResultType::Matrix(mat) => mat.render_height(),
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
                    }) => MatrixData::calc_render_height(row_count),
                    TokenType::LineReference { var_index } => {
                        let var = &vars[var_index];
                        match &var {
                            Some(Variable {
                                value:
                                    Ok(CalcResult {
                                        typ: CalcResultType::Matrix(mat),
                                        ..
                                    }),
                                ..
                            }) => mat.render_height(),
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

    #[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
    pub struct ContentIndex(usize);

    pub fn content_y(y: usize) -> ContentIndex {
        ContentIndex(y)
    }

    impl ContentIndex {
        pub fn new(n: usize) -> ContentIndex {
            ContentIndex(n)
        }

        #[inline]
        pub fn as_usize(self) -> usize {
            self.0
        }

        pub fn add(&self, n: usize) -> ContentIndex {
            ContentIndex(self.0 + n)
        }

        pub fn sub(&self, n: usize) -> ContentIndex {
            ContentIndex(self.0 - n)
        }
    }

    #[derive(Clone, Copy, Eq, PartialEq, Debug, Ord, PartialOrd)]
    pub struct CanvasY(isize);

    pub fn canvas_y(y: isize) -> CanvasY {
        CanvasY(y)
    }

    impl CanvasY {
        pub fn new(n: isize) -> CanvasY {
            CanvasY(n)
        }

        pub fn as_usize(self) -> usize {
            self.0 as usize
        }

        pub fn as_isize(self) -> isize {
            self.0
        }

        pub fn add(&self, n: usize) -> CanvasY {
            CanvasY(self.0 + n as isize)
        }

        pub fn sub(&self, n: usize) -> CanvasY {
            CanvasY(self.0 - n as isize)
        }
    }
}

//, α, Ω, β
// γ - 	Greek Small Letter Gamma[1]
// δ Greek Small Letter Delta
// ε Greek Small Letter Epsilon
// ζ Greek Small Letter Zeta[
// η Greek Small Letter Eta
// θ Greek Small Letter Theta
// λ Greek Small Letter Lamda
// μ Greek Small Letter Mu
// φ Greek Small Letter Phi
// ω Greek Small Letter Omega
// ψ Greek Small Letter Psi
// τ Greek Small Letter Tau
// ϕ Greek Phi Symbol
struct AutoCompletionConst {
    //const PREFIX: char = '.';
    abbrev: &'static [char],
    replace_to: &'static [char],
    relative_new_cursor_pos: Option<usize>,
}

// "0,0,0;0,0,0;0,0,0".split("").map(x => '\'' + x + '\'').join(',')
const AUTOCOMPLETION_CONSTS: [AutoCompletionConst; 5] = [
    AutoCompletionConst {
        abbrev: &['p', 'o', 'w'],
        replace_to: &['^'],
        relative_new_cursor_pos: None,
    },
    AutoCompletionConst {
        abbrev: &['m', 'a', 't', '3'],
        replace_to: &[
            '[', '0', ',', '0', ',', '0', ';', '0', ',', '0', ',', '0', ';', '0', ',', '0', ',',
            '0', ']',
        ],
        relative_new_cursor_pos: Some(1),
    },
    AutoCompletionConst {
        abbrev: &['m', 'a', 't', '4'],
        replace_to: &[
            '[', '0', ',', '0', ',', '0', ',', '0', ';', '0', ',', '0', ',', '0', ',', '0', ';',
            '0', ',', '0', ',', '0', ',', '0', ';', '0', ',', '0', ',', '0', ',', '0', ']',
        ],
        relative_new_cursor_pos: Some(1),
    },
    AutoCompletionConst {
        abbrev: &['m', 'a', 't'],
        replace_to: &['[', '0', ']'],
        relative_new_cursor_pos: Some(1),
    },
    AutoCompletionConst {
        abbrev: &['p', 'i'],
        replace_to: &['π'],
        relative_new_cursor_pos: None,
    },
];

struct ScrollBarRenderInfo {
    scroll_bar_render_y: usize,
    scroll_bar_render_h: usize,
    max_scroll_y: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextStyle {
    Normal,
    Bold,
    Underline,
    Italy,
}

#[derive(Debug, PartialEq)]
pub struct RenderUtf8TextMsg<'a> {
    pub text: &'a [char],
    pub row: CanvasY,
    pub column: usize,
}

#[derive(Debug, PartialEq)]
pub struct RenderAsciiTextMsg<'a> {
    pub text: &'a [u8],
    pub row: CanvasY,
    pub column: usize,
}

#[derive(Debug, PartialEq)]
pub struct RenderStringMsg {
    pub text: String,
    pub row: CanvasY,
    pub column: usize,
}

#[repr(C)]
#[derive(Debug, EnumDiscriminants, PartialEq)]
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
        y: CanvasY,
        w: usize,
        h: usize,
    },
    PulsingRectangle {
        x: usize,
        y: CanvasY,
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

#[derive(Debug, PartialEq)]
pub struct RenderBuckets<'a> {
    pub ascii_texts: Vec<RenderAsciiTextMsg<'a>>,
    pub utf8_texts: Vec<RenderUtf8TextMsg<'a>>,
    pub numbers: Vec<RenderUtf8TextMsg<'a>>,
    pub number_errors: Vec<RenderUtf8TextMsg<'a>>,
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
            number_errors: Vec::with_capacity(32),
            units: Vec::with_capacity(32),
            operators: Vec::with_capacity(32),
            variable: Vec::with_capacity(32),
            line_ref_results: Vec::with_capacity(32),
            clear_commands: Vec::with_capacity(8),
        }
    }

    pub fn custom_commands<'b>(&'b self, layer: Layer) -> &'b Vec<OutputMessage<'a>> {
        &self.custom_commands[layer as usize]
    }

    pub fn clear(&mut self) {
        self.ascii_texts.clear();
        self.utf8_texts.clear();
        self.custom_commands[0].clear();
        self.custom_commands[1].clear();
        self.custom_commands[2].clear();
        self.numbers.clear();
        self.number_errors.clear();
        self.units.clear();
        self.operators.clear();
        self.variable.clear();
        self.line_ref_results.clear();
        self.clear_commands.clear();
    }

    pub fn set_color(&mut self, layer: Layer, color: u32) {
        self.custom_commands[layer as usize].push(OutputMessage::SetColor(color));
    }

    pub fn draw_rect(&mut self, layer: Layer, x: usize, y: CanvasY, w: usize, h: usize) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderRectangle { x, y, w, h });
    }

    pub fn draw_char(&mut self, layer: Layer, x: usize, y: CanvasY, ch: char) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderChar(x, y.as_usize(), ch));
    }

    pub fn draw_text(&mut self, layer: Layer, x: usize, y: CanvasY, text: &'a [char]) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderUtf8Text(
            RenderUtf8TextMsg {
                text,
                row: y,
                column: x,
            },
        ));
    }

    pub fn draw_ascii_text(&mut self, layer: Layer, x: usize, y: CanvasY, text: &'a [u8]) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderAsciiText(
            RenderAsciiTextMsg {
                text,
                row: y,
                column: x,
            },
        ));
    }

    pub fn draw_string(&mut self, layer: Layer, x: usize, y: CanvasY, text: String) {
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
    // has to be pub because of external tests...
    pub line_id: usize,
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
    row_index: ContentIndex,
    cell_strings: Vec<String>,
}

impl MatrixEditing {
    pub fn new<'a>(
        row_count: usize,
        col_count: usize,
        src_canvas: &[char],
        row_index: ContentIndex,
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
        dbg!(src_canvas);
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
            self.move_to_cell(self.current_cell.with_column(self.col_count - 1));
        }
        for row_i in (0..self.row_count).rev() {
            let index = row_i * (self.col_count + 1) + self.col_count;
            self.cell_strings.remove(index);
        }
    }

    fn remove_row(&mut self) {
        self.row_count -= 1;
        if self.current_cell.row >= self.row_count {
            self.move_to_cell(self.current_cell.with_row(self.row_count - 1));
        }
        for _ in 0..self.col_count {
            self.cell_strings.pop();
        }
    }

    fn move_to_cell(&mut self, new_pos: Pos) {
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
        render_y: CanvasY,
        current_editor_width: usize,
        left_gutter_width: usize,
        render_buckets: &mut RenderBuckets<'b>,
        rendered_row_height: usize,
    ) -> usize {
        let vert_align_offset =
            (rendered_row_height - MatrixData::calc_render_height(self.row_count)) / 2;

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

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum EditorObjectType {
    Matrix { row_count: usize, col_count: usize },
    LineReference { var_index: usize },
    Variable { var_index: usize },
    SimpleTokens,
}

#[derive(Clone, Debug)]
pub struct EditorObject {
    // visible for testing
    pub typ: EditorObjectType,
    row: ContentIndex,
    start_x: usize,
    end_x: usize,
    rendered_x: usize,
    rendered_y: CanvasY,
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

#[derive(Debug)]
pub struct Tokens<'a> {
    tokens: Vec<Token<'a>>,
    shunting_output_stack: Vec<ShuntingYardResult>,
}

pub enum MouseState {
    ClickedInEditor,
    ClickedInScrollBar {
        original_click_y: CanvasY,
        original_scroll_y: usize,
    },
    RightGutterIsDragged,
}

#[derive(Debug)]
pub struct EditorObjId {
    content_index: ContentIndex,
    var_index: usize,
}

pub struct NoteCalcApp {
    pub client_width: usize,
    pub editor: Editor,
    pub editor_content: EditorContent<LineData>,
    pub matrix_editing: Option<MatrixEditing>,
    pub line_reference_chooser: Option<ContentIndex>,
    pub line_id_generator: usize,
    pub mouse_state: Option<MouseState>,
    pub updated_line_ref_obj_indices: Vec<EditorObjId>,
    pub editor_objs_referencing_current_line: Vec<EditorObjId>,
    pub render_data: GlobalRenderData,
    // when pressing Ctrl-c without any selection, the result of the current line will be put into this clipboard
    pub clipboard: Option<String>,
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
            updated_line_ref_obj_indices: Vec::with_capacity(16),
            editor_objs_referencing_current_line: Vec::with_capacity(8),
            render_data: GlobalRenderData::new(
                client_width,
                client_height,
                default_result_gutter_x(client_width),
                LEFT_GUTTER_MIN_WIDTH,
                RIGHT_GUTTER_WIDTH,
            ),
            clipboard: None,
        }
    }

    pub fn get_selected_text_and_clear_app_clipboard(&mut self) -> Option<String> {
        // TODO: use fix buffer don't allocate
        let mut str = String::with_capacity(64);
        return if let Some(clipboard) = std::mem::replace(&mut self.clipboard, None) {
            Some(clipboard)
        } else if let Some(matrix_editing) = &self.matrix_editing {
            matrix_editing
                .editor_content
                .write_selection_into(matrix_editing.editor.get_selection(), &mut str);
            Some(str)
        } else if self.editor.get_selection().is_range().is_some() {
            self.editor_content
                .write_selection_into(self.editor.get_selection(), &mut str);
            Some(str)
        } else {
            None
        };
    }

    pub fn set_normalized_content<'b>(
        &mut self,
        mut text: &str,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
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
        self.editor_objs_referencing_current_line.clear();
        self.process_and_render_tokens(
            RowModificationType::AllLinesFrom(0),
            units,
            allocator,
            tokens,
            results,
            vars,
            editor_objs,
            render_buckets,
            result_buffer,
        );
        change_result_panel_size_wrt_result_len(self.client_width, &mut self.render_data);
    }

    pub fn calc_full_content_height(gr: &GlobalRenderData, content_len: usize) -> usize {
        // TODO csak az utolsó sorig iterálj, gr.be asszem letárolom
        let mut h = 0;
        for i in 0..content_len.min(MAX_LINE_COUNT) {
            let editor_y = content_y(i);
            if gr.is_visible(editor_y) {
                h += gr.get_rendered_height(editor_y);
            } else if gr.get_render_y(editor_y).is_some() {
                h += 1;
            }
        }
        h
    }

    pub fn renderr<'b>(
        editor: &mut Editor,
        editor_content: &EditorContent<LineData>,
        units: &Units,
        matrix_editing: &mut Option<MatrixEditing>,
        line_reference_chooser: &mut Option<ContentIndex>,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
        result_change_flag: EditorRowFlags,
        gr: &mut GlobalRenderData,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        editor_objs: &mut EditorObjects,
        updated_line_ref_obj_indices: &[EditorObjId],
        editor_objs_referencing_current_line: &mut Vec<EditorObjId>,
    ) {
        // x, h
        let mut editor_y_to_render_w: [usize; MAX_LINE_COUNT] = [0; MAX_LINE_COUNT];
        {
            let mut r = PerLineRenderData::new();
            r.render_y = canvas_y(-(gr.scroll_y as isize));
            gr.clear_editor_y_to_render_y();
            for line in editor_content.lines().take(MAX_LINE_COUNT) {
                r.new_line_started();
                let editor_y = r.editor_y;
                {
                    if gr.scroll_y > editor_y.as_usize()
                        || editor_y.as_usize() >= gr.scroll_y + gr.client_height
                    {
                        gr.set_render_y(editor_y, Some(r.render_y));
                        r.line_render_ended(1);
                        continue;
                    } else if editor_y.as_usize() > 0 {
                        let prev_editor_y = editor_y.sub(1);

                        if let Some(r_y) = gr.get_render_y(prev_editor_y) {
                            if r_y.as_isize() + (gr.get_rendered_height(prev_editor_y) as isize)
                                >= gr.client_height as isize
                            {
                                for i in editor_y.as_usize()..MAX_LINE_COUNT {
                                    gr.set_render_y(content_y(i), Some(r.render_y));
                                    r.line_render_ended(1);
                                }
                                break;
                            }
                        }
                    }
                }

                let render_y = r.render_y;
                gr.set_render_y(editor_y, Some(render_y));
                r.rendered_row_height = gr.get_rendered_height(editor_y);
                // "- 1" so if it is even, it always appear higher
                r.vert_align_offset = (r.rendered_row_height - 1) / 2;

                highlight_current_line(render_buckets, &r, editor, &gr);

                if let Some(tokens) = &tokens[editor_y] {
                    // TODO: choose a better name
                    // it means that either we use the nice token rendering (e.g. for matrix it is the multiline matrix stuff),
                    // or render simply the backend content (e.g. for matrix it is [1;2;3]
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
                    // don't highlight refs in the current row as they will be pulsing in different colors
                    if editor.get_selection().get_cursor_pos().row != r.editor_y.as_usize() {
                        highlight_line_refs(&editor_objs[editor_y], render_buckets, &r, gr);
                    } else {
                        highlight_active_line_refs(&editor_objs[editor_y], render_buckets, &r, gr);
                    }
                } else {
                    r.rendered_row_height = 1;
                    render_simple_text_line(line, &mut r, gr, render_buckets, allocator);
                }
                render_wrap_dots(render_buckets, &r, &gr);

                editor_y_to_render_w[r.editor_y.as_usize()] = r.render_x;

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
                    let line_num_str = if r.editor_y.as_usize() < 9 {
                        &(LINE_NUM_CONSTS[editor_y.as_usize()][..])
                    } else {
                        &(LINE_NUM_CONSTS2[(editor_y.as_usize()) - 9][..])
                    };
                    render_buckets.draw_text(
                        Layer::Text,
                        0,
                        render_y.add(vert_align_offset),
                        line_num_str,
                    );
                }
                // } else if redraw_result_area.need(editor_y) {
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
                r.line_render_ended(gr.get_rendered_height(editor_y));
            }
        }

        draw_line_refs_and_vars_referenced_from_cur_row(
            &editor_objs[content_y(editor.get_selection().get_cursor_pos().row)],
            gr,
            render_buckets,
            &editor_y_to_render_w,
        );

        NoteCalcApp::fill_editor_objs_referencing_current_line(
            content_y(editor.get_selection().get_cursor_pos().row),
            tokens,
            vars,
            editor_objs_referencing_current_line,
            editor_content,
        );

        render_buckets
            .clear_commands
            .push(OutputMessage::SetColor(0xFFFFFF_FF));

        // clear the whole scrollbar area
        render_buckets
            .clear_commands
            .push(OutputMessage::RenderRectangle {
                x: gr.result_gutter_x - SCROLL_BAR_WIDTH,
                y: canvas_y(0),
                w: SCROLL_BAR_WIDTH,
                h: gr.client_height,
            });

        // TODO calc it once on content change (scroll_bar_h as well) (it is used in handle_drag)
        if let Some(scrollbar_info) =
            NoteCalcApp::get_scrollbar_info(gr, editor_content.line_count())
        {
            render_buckets.set_color(Layer::BehindText, 0xFFCCCC_FF);
            render_buckets.draw_rect(
                Layer::BehindText,
                gr.result_gutter_x - SCROLL_BAR_WIDTH,
                canvas_y(scrollbar_info.scroll_bar_render_y as isize),
                SCROLL_BAR_WIDTH,
                scrollbar_info.scroll_bar_render_h,
            );
        }

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

        let mut tmp = ResultRender::new(SmallVec::with_capacity(MAX_LINE_COUNT));

        render_results_into_buf_and_calc_len(
            &units,
            results.as_slice(),
            result_buffer,
            &mut tmp,
            &editor_content,
            gr,
            Some(4),
        );
        tmp.max_len = create_render_commands_for_results_and_render_matrices(
            &tmp,
            units,
            results.as_slice(),
            render_buckets,
            result_buffer,
            gr,
            Some(4),
        )
        .max(tmp.max_len);
        gr.longest_rendered_result_len = tmp.max_len;

        pulse_changed_results(
            render_buckets,
            gr,
            gr.longest_rendered_result_len,
            &result_change_flag,
        );

        pulse_modified_line_references(
            render_buckets,
            gr,
            updated_line_ref_obj_indices,
            editor_objs,
        );

        pulse_editor_objs_referencing_current_line(
            render_buckets,
            gr,
            editor_objs_referencing_current_line,
            editor_objs,
        );
    }

    pub fn handle_wheel<'b>(
        &mut self,
        dir: usize,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) -> bool {
        let has_moved = if dir == 0 && self.render_data.scroll_y > 0 {
            self.render_data.scroll_y -= 1;
            true
        } else if dir == 1 {
            let content_height = NoteCalcApp::calc_full_content_height(
                &self.render_data,
                self.editor_content.line_count(),
            );
            if (self.render_data.scroll_y + self.render_data.client_height) < content_height {
                self.render_data.scroll_y += 1;
                true
            } else {
                false
            }
        } else {
            false
        };
        if has_moved {
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                result_buffer,
                allocator,
                tokens,
                results,
                vars,
                editor_objs,
                EditorRowFlags::empty(),
            );
        }
        return has_moved;
    }

    pub fn handle_click<'b>(
        &mut self,
        x: usize,
        clicked_y: CanvasY,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) {
        let scroll_bar_x = self.render_data.result_gutter_x - SCROLL_BAR_WIDTH;
        if x < self.render_data.left_gutter_width {
            // clicked on left gutter
        } else if x < scroll_bar_x {
            self.handle_editor_area_click(
                x,
                clicked_y,
                editor_objs,
                units,
                allocator,
                tokens,
                results,
                vars,
                render_buckets,
                result_buffer,
            );
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
                    self.insert_line_ref(
                        units,
                        allocator,
                        tokens,
                        results,
                        vars,
                        editor_y,
                        editor_objs,
                        render_buckets,
                        result_buffer,
                    );
                }
                None
            };
        }
    }

    pub fn handle_mouse_up(&mut self) {
        match self.mouse_state {
            Some(MouseState::RightGutterIsDragged) => {}
            Some(MouseState::ClickedInEditor) => {}
            Some(MouseState::ClickedInScrollBar {
                original_click_y, ..
            }) => {
                let gr = &mut self.render_data;
                if let Some(scrollbar_render_info) =
                    NoteCalcApp::get_scrollbar_info(gr, self.editor_content.line_count())
                {
                    if original_click_y.as_usize() < scrollbar_render_info.scroll_bar_render_y {
                        // scroll up
                        gr.scroll_y -= 1;
                    } else if original_click_y.as_usize()
                        > scrollbar_render_info.scroll_bar_render_y
                            + scrollbar_render_info.scroll_bar_render_h
                    {
                        // scroll down
                        gr.scroll_y += 1;
                    }
                }
            }
            None => {}
        }
        self.mouse_state = None;
    }

    fn handle_editor_area_click<'b>(
        &mut self,
        x: usize,
        clicked_y: CanvasY,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) {
        let clicked_x = x - self.render_data.left_gutter_width;
        let clicked_row = self.get_clicked_row_clamped(clicked_y);

        let matrix_row_index = if self.matrix_editing.is_some() {
            let matrix_row_index = self.matrix_editing.as_ref().unwrap().row_index;
            end_matrix_editing(
                &mut self.matrix_editing,
                &mut self.editor,
                &mut self.editor_content,
                None,
            );
            Some(matrix_row_index)
        } else {
            None
        };

        let editor_click_pos = if let Some(editor_obj) =
            self.get_obj_at(clicked_x, clicked_row, clicked_y, editor_objs)
        {
            match editor_obj.typ {
                EditorObjectType::LineReference { .. } => {
                    Pos::from_row_column(editor_obj.row.as_usize(), editor_obj.end_x)
                }
                EditorObjectType::Matrix {
                    row_count,
                    col_count,
                } => {
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
                    ));
                    Pos::from_row_column(editor_obj.row.as_usize(), editor_obj.start_x + 1)
                }
                EditorObjectType::SimpleTokens | EditorObjectType::Variable { .. } => {
                    let x_pos_within = clicked_x - editor_obj.rendered_x;
                    Pos::from_row_column(
                        editor_obj.row.as_usize(),
                        editor_obj.start_x + x_pos_within,
                    )
                }
            }
        } else {
            let eol = self.editor_content.line_len(clicked_row.as_usize());
            Pos::from_row_column(clicked_row.as_usize(), eol)
        };

        self.editor.handle_click(
            editor_click_pos.column,
            editor_click_pos.row,
            &self.editor_content,
        );

        self.editor.blink_cursor();

        if self.mouse_state.is_none() {
            self.mouse_state = Some(MouseState::ClickedInEditor);
        }

        if let Some(matrix_row_index) = matrix_row_index {
            self.process_and_render_tokens(
                RowModificationType::SingleLine(matrix_row_index.as_usize()),
                units,
                allocator,
                tokens,
                results,
                vars,
                editor_objs,
                render_buckets,
                result_buffer,
            );
        } else {
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                result_buffer,
                allocator,
                tokens,
                results,
                vars,
                editor_objs,
                EditorRowFlags::empty(),
            );
        }
    }

    pub fn rendered_y_to_editor_y(&self, clicked_y: CanvasY) -> Option<ContentIndex> {
        let editor_y_to_render_y = self.render_data.editor_y_to_render_y();
        let mut was_visible_row = false;
        for (ed_y, r_y) in editor_y_to_render_y.iter().enumerate() {
            if let Some(r_y) = r_y {
                was_visible_row = true;
                if *r_y == clicked_y {
                    return Some(content_y(ed_y));
                } else if *r_y > clicked_y {
                    return Some(content_y(ed_y - 1));
                }
            } else if was_visible_row {
                return Some(content_y(ed_y - 1));
            }
        }
        return None;
    }

    pub fn get_clicked_row_clamped<'a>(&self, render_y: CanvasY) -> ContentIndex {
        let latest_bottom_i = self
            .render_data
            .calc_bottom_y(self.editor_content.line_count().min(MAX_LINE_COUNT - 1));
        return if render_y >= latest_bottom_i {
            content_y(self.editor_content.line_count() - 1)
        } else if let Some(editor_y) = self.rendered_y_to_editor_y(render_y) {
            editor_y
        } else {
            panic!();
        };
    }

    pub fn get_obj_at_rendered_pos<'a>(
        &self,
        x: usize,
        render_y: CanvasY,
        editor_objects: &'a EditorObjects,
    ) -> Option<&'a EditorObject> {
        let editor_y = if render_y
            >= self
                .render_data
                .calc_bottom_y(self.editor_content.line_count())
        {
            content_y(self.editor_content.line_count() - 1)
        } else if let Some(editor_y) = self.rendered_y_to_editor_y(render_y) {
            editor_y
        } else {
            return None;
        };
        return editor_objects[editor_y].iter().find(|editor_obj| {
            (editor_obj.rendered_x..editor_obj.rendered_x + editor_obj.rendered_w).contains(&x)
                && (editor_obj.rendered_y.as_usize()
                    ..editor_obj.rendered_y.as_usize() + editor_obj.rendered_h)
                    .contains(&render_y.as_usize())
        });
    }

    pub fn get_obj_at<'a>(
        &self,
        x: usize,
        editor_y: ContentIndex,
        render_y: CanvasY,
        editor_objects: &'a EditorObjects,
    ) -> Option<&'a EditorObject> {
        return editor_objects[editor_y].iter().find(|editor_obj| {
            (editor_obj.rendered_x..editor_obj.rendered_x + editor_obj.rendered_w).contains(&x)
                && (editor_obj.rendered_y.as_usize()
                    ..editor_obj.rendered_y.as_usize() + editor_obj.rendered_h)
                    .contains(&render_y.as_usize())
        });
    }

    pub fn get_obj_at_inside<'a>(
        &self,
        x: usize,
        editor_y: ContentIndex,
        editor_objects: &'a EditorObjects,
    ) -> Option<&'a EditorObject> {
        return editor_objects[editor_y]
            .iter()
            .find(|editor_obj| (editor_obj.start_x + 1..editor_obj.end_x).contains(&x));
    }

    pub fn handle_drag<'b>(
        &mut self,
        x: usize,
        y: CanvasY,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) -> bool {
        let need_render = match self.mouse_state {
            Some(MouseState::RightGutterIsDragged) => {
                self.render_data.set_result_gutter_x(
                    self.client_width,
                    calc_result_gutter_x(x, self.client_width, self.render_data.left_gutter_width),
                );
                true
            }
            Some(MouseState::ClickedInEditor) => {
                if let Some(y) = self.rendered_y_to_editor_y(y) {
                    self.editor.handle_drag(
                        (x as isize - self.render_data.left_gutter_width as isize).max(0) as usize,
                        y.as_usize(),
                        &self.editor_content,
                    );
                    self.editor.blink_cursor();
                    true
                } else {
                    false
                }
            }
            Some(MouseState::ClickedInScrollBar {
                original_click_y,
                original_scroll_y,
            }) => {
                let gr = &mut self.render_data;
                if let Some(scrollbar_info) =
                    NoteCalcApp::get_scrollbar_info(gr, self.editor_content.line_count())
                {
                    let delta_y = y.as_isize() - original_click_y.as_isize();
                    gr.scroll_y = ((original_scroll_y as isize + delta_y).max(0) as usize)
                        .min(scrollbar_info.max_scroll_y);

                    true
                } else {
                    false
                }
            }
            None => false,
        };
        if need_render {
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                result_buffer,
                allocator,
                tokens,
                results,
                vars,
                editor_objs,
                EditorRowFlags::empty(),
            );
        }
        return need_render;
    }

    fn get_scrollbar_info(
        gr: &GlobalRenderData,
        content_len: usize,
    ) -> Option<ScrollBarRenderInfo> {
        let content_height = NoteCalcApp::calc_full_content_height(gr, content_len);
        let max_scroll_y = content_height as isize - gr.client_height as isize;
        if max_scroll_y > 0 {
            let max_scroll_y = max_scroll_y as usize;
            let scroll_bar_h = (gr.client_height as isize - max_scroll_y as isize).max(1) as usize;
            let scroll_bar_y = if scroll_bar_h > 1 {
                gr.scroll_y
            } else {
                ((gr.scroll_y as f64 / (max_scroll_y + 1) as f64) * gr.client_height as f64)
                    as usize
            };
            Some(ScrollBarRenderInfo {
                scroll_bar_render_y: scroll_bar_y,
                scroll_bar_render_h: scroll_bar_h,
                max_scroll_y,
            })
        } else {
            None
        }
    }

    pub fn handle_resize<'b>(
        &mut self,
        new_client_width: usize,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) {
        if new_client_width
            < LEFT_GUTTER_MIN_WIDTH + RIGHT_GUTTER_WIDTH + MIN_RESULT_PANEL_WIDTH + SCROLL_BAR_WIDTH
        {
            return;
        }
        self.client_width = new_client_width;
        change_result_panel_size_wrt_result_len(self.client_width, &mut self.render_data);
        self.generate_render_commands_and_fill_editor_objs(
            units,
            render_buckets,
            result_buffer,
            allocator,
            tokens,
            results,
            vars,
            editor_objs,
            EditorRowFlags::empty(),
        );
    }

    pub fn handle_time<'b>(
        &mut self,
        now: u32,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) -> bool {
        let need_rerender = if let Some(mat_editor) = &mut self.matrix_editing {
            mat_editor.editor.handle_tick(now)
        } else {
            self.editor.handle_tick(now)
        };
        if need_rerender {
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                result_buffer,
                allocator,
                tokens,
                results,
                vars,
                editor_objs,
                EditorRowFlags::empty(),
            );
        }
        need_rerender
    }

    pub fn get_line_ref_normalized_content(&self) -> String {
        // TODO: no alloc
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
                            let referenced_row_index = self
                                .editor_content
                                .data()
                                .iter()
                                .position(|it| it.line_id == num as usize)
                                .unwrap_or(0)
                                + 1; // '+1' line id cannot be 0
                            result.push('&');
                            result.push('[');
                            {
                                // TODO: change this code if 64/99/etc line count limit is removed
                                let mut tmp_arr = ['0', '0', '0'];
                                let mut tmp_rev_index = 3;
                                let mut line_id = referenced_row_index;
                                while line_id > 0 {
                                    tmp_rev_index -= 1;
                                    let to_insert = line_id % 10;
                                    tmp_arr[tmp_rev_index] = (48 + to_insert as u8) as char;
                                    line_id /= 10;
                                }
                                for i in tmp_rev_index..=2 {
                                    result.push(tmp_arr[i]);
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

    pub fn normalize_line_refs_in_place(&mut self) {
        let mut original_selection = self.editor.get_selection();
        for line_i in 0..self.editor_content.line_count() {
            let mut i = 0;
            'i: while i < self.editor_content.line_len(line_i) {
                //self.editor_content.get_line_valid_chars(line_i)
                let start = i;
                if i + 3 < self.editor_content.line_len(line_i)
                    && self.editor_content.get_char(line_i, i) == '&'
                    && self.editor_content.get_char(line_i, i + 1) == '['
                {
                    let mut end = i + 2;
                    let mut num_inside_lineref: u32 = 0;
                    while end < self.editor_content.line_len(line_i) {
                        if self.editor_content.get_char(line_i, end) == ']'
                            && num_inside_lineref > 0
                        {
                            let num_len = end - (start + 2); // start --> &[num] <- end

                            // remove the number from the original line_ref text '&[x]' (remove only x)
                            {
                                let start_pos = Pos::from_row_column(line_i, start + 2);
                                let end_pos = start_pos.with_column(end);
                                self.editor.set_cursor_range(start_pos, end_pos);
                                self.editor.handle_input(
                                    EditorInputEvent::Del,
                                    InputModifiers::none(),
                                    &mut self.editor_content,
                                );
                            }
                            {
                                // which row has the id of 'num_inside_lineref'?
                                let referenced_row_index = self
                                    .editor_content
                                    .data()
                                    .iter()
                                    .position(|it| it.line_id == num_inside_lineref as usize)
                                    .unwrap_or(0)
                                    + 1; // '+1' line id cannot be 0
                                         // TODO: change this code if 64/99/etc line count limit is removed
                                let mut tmp_arr = ['0', '0', '0'];
                                let mut tmp_rev_index = 3;
                                let mut line_id = referenced_row_index;
                                while line_id > 0 {
                                    tmp_rev_index -= 1;

                                    let to_insert = line_id % 10;
                                    tmp_arr[tmp_rev_index] = (48 + to_insert as u8) as char;
                                    line_id /= 10;
                                }

                                i = start + 2;
                                let mut align_selection = 0;
                                if line_i == original_selection.start.row
                                    && original_selection.start.column >= i
                                {
                                    original_selection.start.column -= num_len;
                                    align_selection |= 1;
                                }
                                if let Some(end) = original_selection.end {
                                    if line_i == end.row && end.column >= i {
                                        original_selection.end.as_mut().expect("must").column -=
                                            num_len;
                                        align_selection |= 2;
                                    }
                                };
                                for tmp_arr_i in tmp_rev_index..=2 {
                                    self.editor.handle_input(
                                        EditorInputEvent::Char(tmp_arr[tmp_arr_i]),
                                        InputModifiers::none(),
                                        &mut self.editor_content,
                                    );
                                    i += 1;
                                    if (align_selection & 1) > 0 {
                                        original_selection.start.column += 1;
                                    }
                                    if (align_selection & 2) > 0 {
                                        original_selection.end.as_mut().expect("must").column += 1;
                                    }
                                }
                                i += 1; // skip ']'
                            }
                            continue 'i;
                        } else if let Some(digit) =
                            self.editor_content.get_char(line_i, end).to_digit(10)
                        {
                            num_inside_lineref = if num_inside_lineref == 0 {
                                digit
                            } else {
                                num_inside_lineref * 10 + digit
                            };
                        } else {
                            break;
                        }
                        end += 1;
                    }
                }
                i += 1;
            }
        }
        for line_i in 0..self.editor_content.line_count() {
            self.editor_content.mut_data(line_i).line_id = line_i + 1;
        }
        self.line_id_generator = self.editor_content.line_count() + 1;

        self.editor.set_selection_save_col(original_selection);
    }

    pub fn alt_key_released<'b>(
        &mut self,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) {
        if let Some(line_ref_row) = self.line_reference_chooser {
            self.line_reference_chooser = None;
            self.insert_line_ref(
                units,
                allocator,
                tokens,
                results,
                vars,
                line_ref_row,
                editor_objs,
                render_buckets,
                result_buffer,
            );
        } else {
            return;
        }
    }

    fn insert_line_ref<'b>(
        &mut self,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        line_ref_row: ContentIndex,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) {
        let cursor_row = self.editor.get_selection().get_cursor_pos().row;
        if cursor_row == line_ref_row.as_usize()
            || matches!(&results[line_ref_row], Err(_) | Ok(None))
        {
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                result_buffer,
                allocator,
                tokens,
                results,
                vars,
                editor_objs,
                EditorRowFlags::empty(),
            );
            return;
        }
        if let Some(var) = &vars[line_ref_row.as_usize()] {
            for ch in var.name.iter() {
                self.editor.handle_input(
                    EditorInputEvent::Char(*ch),
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
            }
        } else {
            let line_id = {
                let line_data = self.editor_content.get_data(line_ref_row.as_usize());
                line_data.line_id
            };

            let inserting_text = format!("&[{}]", line_id);
            self.editor
                .insert_text(&inserting_text, &mut self.editor_content);
        }

        self.process_and_render_tokens(
            RowModificationType::SingleLine(cursor_row),
            units,
            allocator,
            tokens,
            results,
            vars,
            editor_objs,
            render_buckets,
            result_buffer,
        );
    }

    pub fn handle_paste<'b>(
        &mut self,
        text: String,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) {
        match self.editor.insert_text(&text, &mut self.editor_content) {
            Some(modif) => {
                if self.editor.get_selection().get_cursor_pos().row >= MAX_LINE_COUNT {
                    self.editor.set_cursor_pos_r_c(MAX_LINE_COUNT - 1, 0);
                }
                self.process_and_render_tokens(
                    modif,
                    units,
                    allocator,
                    tokens,
                    results,
                    vars,
                    editor_objs,
                    render_buckets,
                    result_buffer,
                );
            }
            None => {}
        };
    }

    pub fn reparse_everything<'b, 'q>(
        &'q mut self,
        allocator: &'b Bump,
        units: &Units,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) {
        self.process_and_render_tokens(
            RowModificationType::AllLinesFrom(0),
            units,
            allocator,
            tokens,
            results,
            vars,
            editor_objs,
            render_buckets,
            result_buffer,
        );
    }

    pub fn handle_input<'b, 'q>(
        &'q mut self,
        input: EditorInputEvent,
        modifiers: InputModifiers,
        allocator: &'b Bump,
        units: &Units,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) -> Option<RowModificationType> {
        fn handle_input_with_alt<'b>(
            app: &mut NoteCalcApp,
            input: EditorInputEvent,
        ) -> Option<RowModificationType> {
            if input == EditorInputEvent::Left {
                let selection = app.editor.get_selection();
                let (start, end) = selection.get_range();
                for row_i in start.row..=end.row {
                    let new_format = match &app.editor_content.get_data(row_i).result_format {
                        ResultFormat::Bin => ResultFormat::Hex,
                        ResultFormat::Dec => ResultFormat::Bin,
                        ResultFormat::Hex => ResultFormat::Dec,
                    };
                    app.editor_content.mut_data(row_i).result_format = new_format;
                }
                None
            } else if input == EditorInputEvent::Right {
                let selection = app.editor.get_selection();
                let (start, end) = selection.get_range();
                for row_i in start.row..=end.row {
                    let new_format = match &app.editor_content.get_data(row_i).result_format {
                        ResultFormat::Bin => ResultFormat::Dec,
                        ResultFormat::Dec => ResultFormat::Hex,
                        ResultFormat::Hex => ResultFormat::Bin,
                    };
                    app.editor_content.mut_data(row_i).result_format = new_format;
                }
                None
            } else if input == EditorInputEvent::Up {
                let cur_pos = app.editor.get_selection().get_cursor_pos();
                let rows = if let Some(selector_row) = app.line_reference_chooser {
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
                if let Some((_, new_selected_row)) = rows {
                    app.line_reference_chooser = Some(content_y(new_selected_row));
                    None
                } else {
                    None
                }
            } else if input == EditorInputEvent::Down {
                let cur_pos = app.editor.get_selection().get_cursor_pos();
                let rows = if let Some(selector_row) = app.line_reference_chooser {
                    if selector_row.as_usize() < cur_pos.row - 1 {
                        Some((selector_row.as_usize(), selector_row.as_usize() + 1))
                    } else {
                        Some((selector_row.as_usize(), selector_row.as_usize()))
                    }
                } else {
                    None
                };
                if let Some((_prev_selected_row, new_selected_row)) = rows {
                    app.line_reference_chooser = Some(content_y(new_selected_row));
                    None
                } else {
                    None
                }
            } else {
                None
            }
        }

        ////////////////////////////////////////////////////
        ////////////////////////////////////////////////////
        ////////////////////////////////////////////////////
        let prev_row = self.editor.get_selection().get_cursor_pos().row;
        let modif = if self.matrix_editing.is_none() && modifiers.alt {
            handle_input_with_alt(&mut *self, input)
        } else if self.matrix_editing.is_some() {
            self.handle_matrix_editor_input(input, modifiers);
            if self.matrix_editing.is_none() {
                // user left a matrix
                Some(RowModificationType::SingleLine(prev_row))
            } else {
                if modifiers.alt {
                    let y = content_y(prev_row);
                    let new_h =
                        calc_rendered_height(y, &self.matrix_editing, tokens, results, vars);
                    self.render_data.set_rendered_height(y, new_h);
                };
                None
            }
        } else {
            if self.handle_completion(&input, editor_objs, vars) {
                Some(RowModificationType::SingleLine(prev_row))
            } else if let Some(modif_type) = self.handle_obj_deletion(&input, editor_objs) {
                Some(modif_type)
            } else if input == EditorInputEvent::Char('c')
                && modifiers.ctrl
                && self.editor.get_selection().is_range().is_none()
            {
                let row = self.editor.get_selection().get_cursor_pos().row;
                if let Ok(Some(result)) = &results[content_y(row)] {
                    self.clipboard = Some(render_result(
                        &units,
                        &result,
                        &self.editor_content.get_data(row).result_format,
                        false,
                        Some(4),
                        true,
                    ));
                }
                None
            } else if input == EditorInputEvent::Char('b') && modifiers.ctrl {
                self.handle_jump_to_definition(&input, modifiers, editor_objs);
                None
            } else if self.handle_obj_jump_over(&input, modifiers, editor_objs) {
                None
            } else {
                let prev_selection = self.editor.get_selection();
                let prev_cursor_pos = prev_selection.get_cursor_pos();

                // if the cursor is inside a matrix, put it afterwards
                if let Some(obj) = self.get_obj_at_inside(
                    prev_cursor_pos.column,
                    content_y(prev_cursor_pos.row),
                    editor_objs,
                ) {
                    match obj.typ {
                        EditorObjectType::Matrix { .. } => self
                            .editor
                            .set_cursor_pos_r_c(obj.row.as_usize(), obj.end_x),
                        _ => {}
                    }
                }
                let modif_type =
                    self.editor
                        .handle_input(input, modifiers, &mut self.editor_content);

                if self.editor.get_selection().get_cursor_pos().row >= MAX_LINE_COUNT {
                    if let Some((start, _end)) = self.editor.get_selection().is_range() {
                        self.editor.set_selection_save_col(Selection::range(
                            start,
                            Pos::from_row_column(MAX_LINE_COUNT - 1, 0),
                        ));
                    } else {
                        self.editor
                            .set_selection_save_col(Selection::single_r_c(MAX_LINE_COUNT - 1, 0));
                    }
                }

                if modif_type.is_none() {
                    // it is possible to step into a matrix only through navigation
                    self.check_stepping_into_matrix(prev_cursor_pos, editor_objs);
                    // if there was no selection but now there is
                    if prev_selection.is_range().is_none()
                        && self.editor.get_selection().is_range().is_some()
                    {
                        self.normalize_line_refs_in_place();
                        Some(RowModificationType::AllLinesFrom(0))
                    } else {
                        modif_type
                    }
                } else {
                    modif_type
                }
            }
        };

        let cursor_pos = self.editor.get_selection().get_cursor_pos();
        let scroll_y =
            get_scroll_y_after_cursor_movement(prev_row, cursor_pos.row, &self.render_data);
        if let Some(scroll_y) = scroll_y {
            self.render_data.scroll_y = scroll_y;
        }

        if let Some(modif) = modif {
            self.process_and_render_tokens(
                modif,
                units,
                allocator,
                tokens,
                results,
                vars,
                editor_objs,
                render_buckets,
                result_buffer,
            );
        } else {
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                result_buffer,
                allocator,
                tokens,
                results,
                vars,
                editor_objs,
                EditorRowFlags::empty(),
            );
            // because of ALT+left or ALT+right, result len can change
            change_result_panel_size_wrt_result_len(self.client_width, &mut self.render_data);
        }

        return modif;
    }

    pub fn process_and_render_tokens<'b>(
        &mut self,
        input_effect: RowModificationType,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) {
        fn eval_line<'a>(
            editor_content: &EditorContent<LineData>,
            line: &[char],
            units: &Units,
            allocator: &'a Bump,
            tokens_per_lines: &mut AppTokens<'a>,
            results: &mut Results,
            vars: &mut Variables,
            editor_y: ContentIndex,
            updated_line_ref_obj_indices: &mut Vec<EditorObjId>,
        ) -> (bool, EditorRowFlags) {
            // TODO avoid clone
            let prev_var_name = vars[editor_y.as_usize()].as_ref().map(|it| it.name.clone());

            tokens_per_lines[editor_y] = Some(parse_tokens(
                line,
                editor_y.as_usize(),
                units,
                &*vars,
                allocator,
            ));
            let new_result = if let Some(tokens) = &mut tokens_per_lines[editor_y] {
                let result = evaluate_tokens_and_save_result(
                    &mut *vars,
                    editor_y.as_usize(),
                    editor_content,
                    &mut tokens.tokens,
                    &mut tokens.shunting_output_stack,
                    editor_content.get_line_valid_chars(editor_y.as_usize()),
                );
                let result = result.map(|it| it.map(|it| it.result));
                result
            } else {
                Ok(None)
            };
            let vars: &Variables = vars;

            let prev_result = std::mem::replace(&mut results[editor_y], new_result);
            let result_has_changed = {
                let new_result = &results[editor_y];
                match (&prev_result, new_result) {
                    (Ok(Some(_)), Err(_)) => true,
                    (Ok(Some(_)), Ok(None)) => true,
                    (Ok(Some(prev_r)), Ok(Some(new_r))) => prev_r.typ != new_r.typ,
                    (Err(_), Err(_)) => false,
                    (Err(_), Ok(None)) => true,
                    (Err(_), Ok(Some(_))) => true,
                    (Ok(None), Ok(Some(_))) => true,
                    (Ok(None), Ok(None)) => false,
                    (Ok(None), Err(_)) => true,
                }
            };

            let mut rows_to_recalc = EditorRowFlags::empty();
            if result_has_changed {
                let line_ref_name =
                    NoteCalcApp::get_line_ref_name(&editor_content, editor_y.as_usize());
                rows_to_recalc.merge(NoteCalcApp::find_line_ref_dependant_lines(
                    &line_ref_name,
                    tokens_per_lines,
                    editor_y.as_usize(),
                    updated_line_ref_obj_indices,
                ));
            }

            let curr_var_name = vars[editor_y.as_usize()].as_ref().map(|it| &it.name);
            rows_to_recalc.merge(find_lines_that_affected_by_var_change(
                result_has_changed,
                curr_var_name,
                prev_var_name,
                tokens_per_lines,
                editor_y.as_usize(),
            ));

            rows_to_recalc.merge(find_sum_variable_name(
                tokens_per_lines,
                editor_y.as_usize(),
            ));
            return (result_has_changed, rows_to_recalc);
        }

        fn find_sum_variable_name(tokens_per_lines: &AppTokens, editor_y: usize) -> EditorRowFlags {
            let mut rows_to_recalc = EditorRowFlags::empty();
            'outer: for (line_index, tokens) in
                tokens_per_lines.iter().skip(editor_y + 1).enumerate()
            {
                if let Some(tokens) = tokens {
                    for token in &tokens.tokens {
                        match token.typ {
                            TokenType::StringLiteral if token.ptr.starts_with(&['-', '-']) => {
                                break 'outer;
                            }
                            TokenType::Variable { var_index }
                                if var_index == SUM_VARIABLE_INDEX =>
                            {
                                rows_to_recalc
                                    .merge(EditorRowFlags::single_row(editor_y + 1 + line_index));
                                break 'outer;
                            }
                            _ => {}
                        }
                    }
                }
            }
            return rows_to_recalc;
        }

        fn find_lines_that_affected_by_var_change<'b>(
            needs_dependency_check: bool,
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
                                    TokenType::StringLiteral => var_name.starts_with(token.ptr),
                                    TokenType::Variable { .. } => *token.ptr == *old_var_name,
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
                    if !needs_dependency_check {
                        return EditorRowFlags::empty();
                    }
                    // volt is, van is, a neve is ugyanaz
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
        let mut result_change_flag = EditorRowFlags::empty();
        for editor_y in 0..self.editor_content.line_count().min(MAX_LINE_COUNT) {
            let recalc = match input_effect {
                RowModificationType::SingleLine(to_change_index) if to_change_index == editor_y => {
                    true
                }
                RowModificationType::AllLinesFrom(to_change_index_from)
                    if editor_y >= to_change_index_from =>
                {
                    true
                }
                _ => dependant_rows.need(content_y(editor_y)),
            };
            if recalc {
                if self.editor_content.get_data(editor_y).line_id == 0 {
                    self.editor_content.mut_data(editor_y).line_id = self.line_id_generator;
                    self.line_id_generator += 1;
                }
                let y = content_y(editor_y);

                let (result_has_changed, rows_to_recalc) = eval_line(
                    &self.editor_content,
                    self.editor_content.get_line_valid_chars(editor_y),
                    units,
                    allocator,
                    tokens,
                    results,
                    &mut *vars,
                    y,
                    &mut self.updated_line_ref_obj_indices,
                );
                if result_has_changed {
                    result_change_flag.merge(EditorRowFlags::single_row(editor_y));
                }
                dependant_rows.merge(rows_to_recalc);
                let new_h = calc_rendered_height(y, &self.matrix_editing, tokens, results, vars);
                self.render_data.set_rendered_height(y, new_h);
            }
            if self
                .editor_content
                .get_line_valid_chars(editor_y)
                .starts_with(&['-', '-'])
            {
                sum_is_null = true;
            }

            match &results[content_y(editor_y)] {
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

        if self.editor_content.line_count() > 9 {
            self.render_data.left_gutter_width = LEFT_GUTTER_MIN_WIDTH + 1;
        } else {
            self.render_data.left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
        }

        self.generate_render_commands_and_fill_editor_objs(
            units,
            render_buckets,
            result_buffer,
            allocator,
            tokens,
            results,
            vars,
            editor_objs,
            result_change_flag,
        );

        // is there any change
        if result_change_flag.as_u64() > 0 {
            change_result_panel_size_wrt_result_len(self.client_width, &mut self.render_data);
        }
    }

    fn get_line_ref_name(editor_content: &EditorContent<LineData>, y: usize) -> Vec<char> {
        let line_data = editor_content.get_data(y);
        // TODO opt
        let line_ref_name: Vec<char> = format!("&[{}]", line_data.line_id).chars().collect();
        return line_ref_name;
    }

    fn fill_editor_objs_referencing_current_line<'b>(
        current_y: ContentIndex,
        tokens: &AppTokens<'b>,
        vars: &Variables,
        editor_objs_referencing_current_line: &mut Vec<EditorObjId>,
        editor_content: &EditorContent<LineData>,
    ) {
        editor_objs_referencing_current_line.clear();
        if let Some(var) = &vars[current_y.as_usize()] {
            NoteCalcApp::find_line_ref_dependant_lines(
                &var.name,
                tokens,
                current_y.as_usize(),
                editor_objs_referencing_current_line,
            );
        } else {
            let line_ref_name =
                NoteCalcApp::get_line_ref_name(&editor_content, current_y.as_usize());
            NoteCalcApp::find_line_ref_dependant_lines(
                &line_ref_name,
                tokens,
                current_y.as_usize(),
                editor_objs_referencing_current_line,
            );
        };
    }

    fn find_line_ref_dependant_lines<'b>(
        editor_obj_name: &[char],
        tokens_per_lines: &AppTokens<'b>,
        editor_y: usize,
        updated_line_ref_obj_indices: &mut Vec<EditorObjId>,
    ) -> EditorRowFlags {
        let mut rows_to_recalc = EditorRowFlags::empty();
        for (token_line_index, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
            if let Some(tokens) = tokens {
                let mut already_added = EditorRowFlags::empty();
                for token in &tokens.tokens {
                    let var_index = match token.typ {
                        TokenType::LineReference { var_index }
                            if already_added.is_false(var_index)
                                && token.ptr == editor_obj_name =>
                        {
                            var_index
                        }
                        TokenType::Variable { var_index }
                            if var_index != SUM_VARIABLE_INDEX
                                && already_added.is_false(var_index)
                                && token.ptr == editor_obj_name =>
                        {
                            var_index
                        }

                        _ => {
                            continue;
                        }
                    };
                    let index = editor_y + 1 + token_line_index;
                    updated_line_ref_obj_indices.push(EditorObjId {
                        content_index: content_y(index),
                        var_index,
                    });
                    rows_to_recalc.merge(EditorRowFlags::single_row(index));
                    already_added.set(var_index);
                }
            } else {
                break;
            }
        }
        return rows_to_recalc;
    }

    pub fn copy_selected_rows_with_result_to_clipboard<'b>(
        &'b mut self,
        units: &'b Units,
        render_buckets: &'b mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
        tokens: &AppTokens<'b>,
        vars: &Variables,
        results: &Results,
    ) -> String {
        render_buckets.clear();

        let (first_row, second_row) =
            if let Some((start, end)) = self.editor.get_selection().is_range() {
                (start.row, end.row)
            } else {
                (0, self.editor_content.line_count() - 1)
            };
        let row_nums = second_row - first_row + 1;

        //let vars = create_vars();
        //let mut tokens = Vec::with_capacity(128);

        let mut gr = GlobalRenderData::new(1024, 1000 /*dummy value*/, 1024 / 2, 0, 2);
        // evaluate all the lines so variables are defined even if they are not selected
        let mut render_height = 0;
        {
            let mut r = PerLineRenderData::new();
            for i in first_row..=second_row {
                let i = content_y(i);
                // TODO "--"
                // tokens.clear();
                // TokenParser::parse_line(
                //     line,
                //     &vars[..],
                //     &mut tokens,
                //     &units,
                //     i.as_usize(),
                //     token_char_alloator,
                // );
                //
                // let mut shunting_output_stack = Vec::with_capacity(32);
                // ShuntingYard::shunting_yard(&mut tokens, &mut shunting_output_stack);

                // tokens must be evaluated to register variables for line reference inlining in the output text

                if let Some(tokens) = &tokens[i] {
                    r.new_line_started();
                    gr.set_render_y(r.editor_y, Some(r.render_y));

                    r.rendered_row_height = PerLineRenderData::calc_rendered_row_height(
                        &results[i],
                        &tokens.tokens,
                        &vars[..],
                        None,
                    );
                    // "- 1" so if it is even, it always appear higher
                    r.vert_align_offset = (r.rendered_row_height - 1) / 2;
                    gr.set_rendered_height(r.editor_y, r.rendered_row_height);
                    render_height += r.rendered_row_height;
                    // Todo: refactor the parameters into a struct
                    render_tokens(
                        &tokens.tokens,
                        &mut r,
                        &mut gr,
                        render_buckets,
                        // TODO &mut code smell
                        &mut Vec::new(),
                        &self.editor,
                        &self.matrix_editing,
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

        let mut tmp = ResultRender::new(SmallVec::with_capacity(MAX_LINE_COUNT));

        gr.result_gutter_x = max_len + 2;
        render_results_into_buf_and_calc_len(
            &units,
            &results.as_slice()[first_row..=second_row],
            result_buffer,
            &mut tmp,
            &self.editor_content,
            &gr,
            None,
        );
        gr.longest_rendered_result_len = tmp.max_len;

        create_render_commands_for_results_and_render_matrices(
            &tmp,
            units,
            &results.as_slice()[first_row..=second_row],
            render_buckets,
            result_buffer,
            &gr,
            None,
        );

        for i in 0..render_height {
            render_buckets.draw_char(
                Layer::AboveText,
                gr.result_gutter_x,
                canvas_y(i as isize),
                '█',
            );
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
        if *input != EditorInputEvent::Tab || cursor_pos.get_cursor_pos().column == 0 {
            return false;
        }

        // matrix autocompletion 'm' + tab
        let cursor_pos = cursor_pos.get_cursor_pos();

        for autocompl_const in &AUTOCOMPLETION_CONSTS {
            let len = autocompl_const.abbrev.len();
            if cursor_pos.column <= len {
                continue;
            }
            let start_x = cursor_pos.column - (len + 1);
            let autocompletion_match = {
                let line = self.editor_content.get_line_valid_chars(cursor_pos.row);
                line[start_x] == '.'
                    && &line[start_x + 1..cursor_pos.column] == autocompl_const.abbrev
            };
            if autocompletion_match {
                let start = cursor_pos.with_column(start_x);
                self.editor
                    .set_selection_save_col(Selection::range(start, cursor_pos));
                self.editor.handle_input(
                    EditorInputEvent::Backspace,
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                for ch in autocompl_const.replace_to {
                    self.editor.handle_input(
                        EditorInputEvent::Char(*ch),
                        InputModifiers::none(),
                        &mut self.editor_content,
                    );
                }
                if let Some(relative_new_cursor_pos) = autocompl_const.relative_new_cursor_pos {
                    self.editor.set_selection_save_col(Selection::single(
                        start.add_column(relative_new_cursor_pos),
                    ));
                }
                if autocompl_const.abbrev.len() >= 3
                    && &autocompl_const.abbrev[0..3] == &['m', 'a', 't']
                {
                    // remove the SimpleToken of the .mat string
                    let size = if autocompl_const.abbrev.len() == 3 {
                        1 // .mat
                    } else if autocompl_const.abbrev[3] == '3' {
                        3
                    } else {
                        4
                    };
                    editor_objects[content_y(cursor_pos.row)].pop();
                    editor_objects[content_y(cursor_pos.row)].push(EditorObject {
                        typ: EditorObjectType::Matrix {
                            row_count: size,
                            col_count: size,
                        },
                        row: content_y(cursor_pos.row),
                        start_x,
                        end_x: start_x + autocompl_const.replace_to.len(),
                        rendered_x: 0,           // dummy
                        rendered_y: canvas_y(0), // dummy
                        rendered_w: size + 2,
                        rendered_h: 1,
                    });
                    self.check_stepping_into_matrix(Pos::from_row_column(0, 0), &editor_objects);
                }
                return true;
            }
        }

        let line = self.editor_content.get_line_valid_chars(cursor_pos.row);
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
                let obj = editor_objects[content_y(cursor_pos.row)].remove(index);
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
                let obj = editor_objects[content_y(cursor_pos.row)].remove(index);
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
                .find_editor_object_at_including_end_of_word(cursor_pos, editor_objects)
                .map(|it| (it.typ, it.row, it.start_x));
            if let Some((EditorObjectType::LineReference { .. }, row, start_x)) = obj {
                //  jump over it
                self.editor.set_cursor_pos_r_c(row.as_usize(), start_x);
                return true;
            }
        } else if *input == EditorInputEvent::Right
            && selection.is_range().is_none()
            && modifiers.shift == false
        {
            let obj = self
                .find_editor_object_at_excluding_end_of_word(cursor_pos, editor_objects)
                .map(|it| (it.typ, it.row, it.end_x));

            if let Some((EditorObjectType::LineReference { .. }, row, end_x)) = obj {
                //  jump over it
                self.editor.set_cursor_pos_r_c(row.as_usize(), end_x);
                return true;
            }
        }
        return false;
    }

    fn handle_jump_to_definition<'b>(
        &mut self,
        input: &EditorInputEvent,
        modifiers: InputModifiers,
        editor_objects: &EditorObjects,
    ) -> bool {
        let selection = self.editor.get_selection();
        let cursor_pos = selection.get_cursor_pos();
        if *input == EditorInputEvent::Char('b') && modifiers.ctrl {
            if let Some(var_index) =
                self.find_var_index_of_var_or_lineref_at(cursor_pos, editor_objects)
            {
                self.editor.set_cursor_pos_r_c(var_index, 0);
                return true;
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
                EditorObjectType::SimpleTokens
                | EditorObjectType::LineReference { .. }
                | EditorObjectType::Variable { .. } => {}
            }
        }
    }

    fn find_editor_object_at_including_end_of_word<'b>(
        &self,
        pos: Pos,
        editor_objects: &'b EditorObjects,
    ) -> Option<&'b EditorObject> {
        for obj in &editor_objects[content_y(pos.row)] {
            if (obj.start_x..=obj.end_x).contains(&pos.column) {
                return Some(obj);
            }
        }
        return None;
    }

    // "asd |var"
    // here, the cursor is at the edge of the first and second tokens as well, but the simpletoken
    // comes first so the variable won't be found, that's why we need this specific find method for
    // linerefs and vars
    fn find_var_index_of_var_or_lineref_at(
        &self,
        pos: Pos,
        editor_objects: &EditorObjects,
    ) -> Option<usize> {
        for obj in &editor_objects[content_y(pos.row)] {
            match obj.typ {
                EditorObjectType::Variable { var_index }
                | EditorObjectType::LineReference { var_index }
                    if (obj.start_x..=obj.end_x).contains(&pos.column) =>
                {
                    return Some(var_index);
                }
                _ => {}
            }
        }
        return None;
    }

    fn find_editor_object_at_excluding_end_of_word<'b>(
        &self,
        pos: Pos,
        editor_objects: &'b EditorObjects,
    ) -> Option<&'b EditorObject> {
        for obj in &editor_objects[content_y(pos.row)] {
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
        return editor_objects[content_y(pos.row)].iter().position(|obj| {
            matches!(obj.typ, EditorObjectType::LineReference{..} | EditorObjectType::Matrix {..})
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
                mat_edit.move_to_cell(mat_edit.current_cell.with_next_col());
            } else if mat_edit.current_cell.row + 1 < mat_edit.row_count {
                mat_edit.move_to_cell(mat_edit.current_cell.with_next_row().with_column(0));
            } else {
                end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    None,
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
                mat_edit.move_to_cell(mat_edit.current_cell.with_prev_col());
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
            if let Some((_from, to)) = mat_edit.editor.get_selection().is_range() {
                mat_edit.editor.set_cursor_pos_r_c(0, to.column);
            } else {
                if mat_edit.current_cell.column + 1 < mat_edit.col_count {
                    mat_edit.move_to_cell(mat_edit.current_cell.with_next_col());
                } else {
                    end_matrix_editing(
                        &mut self.matrix_editing,
                        &mut self.editor,
                        &mut self.editor_content,
                        None,
                    );
                }
            }
        } else if simple && input == EditorInputEvent::Up {
            if mat_edit.current_cell.row > 0 {
                mat_edit.move_to_cell(mat_edit.current_cell.with_prev_row());
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
                mat_edit.move_to_cell(mat_edit.current_cell.with_next_row());
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
                mat_edit.move_to_cell(mat_edit.current_cell.with_column(mat_edit.col_count - 1));
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
        } else if simple && input == EditorInputEvent::Home {
            if mat_edit.current_cell.column != 0 {
                mat_edit.move_to_cell(mat_edit.current_cell.with_column(0));
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

    pub fn generate_render_commands_and_fill_editor_objs<'b>(
        &mut self,
        units: &Units,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        editor_objs: &mut EditorObjects,
        result_change_flag: EditorRowFlags,
    ) {
        render_buckets.clear();
        NoteCalcApp::renderr(
            &mut self.editor,
            &self.editor_content,
            units,
            &mut self.matrix_editing,
            &mut self.line_reference_chooser,
            render_buckets,
            result_buffer,
            result_change_flag,
            &mut self.render_data,
            allocator,
            tokens,
            results,
            vars,
            editor_objs,
            &self.updated_line_ref_obj_indices,
            &mut self.editor_objs_referencing_current_line,
        );
        self.updated_line_ref_obj_indices.clear();
    }
}

#[derive(Debug)]
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

pub fn pulse_modified_line_references(
    render_buckets: &mut RenderBuckets,
    gr: &GlobalRenderData,
    updated_line_ref_obj_indices: &[EditorObjId],
    editor_objects: &EditorObjects,
) {
    // Pulsing changed line references
    for id in updated_line_ref_obj_indices {
        for ed_obj in &editor_objects[id.content_index] {
            match ed_obj {
                EditorObject {
                    typ: EditorObjectType::LineReference { var_index },
                    rendered_x,
                    rendered_y,
                    rendered_w,
                    rendered_h,
                    ..
                } if *var_index == id.var_index => {
                    render_buckets.custom_commands[Layer::AboveText as usize].push(
                        OutputMessage::PulsingRectangle {
                            x: gr.left_gutter_width + *rendered_x,
                            y: *rendered_y,
                            w: *rendered_w,
                            h: *rendered_h,
                            start_color: CHANGE_RESULT_PULSE_START_COLOR,
                            end_color: CHANGE_RESULT_PULSE_END_COLOR,
                            animation_time: Duration::from_millis(2000),
                        },
                    );
                }
                _ => {}
            }
        }
    }
}

pub fn pulse_editor_objs_referencing_current_line(
    render_buckets: &mut RenderBuckets,
    gr: &GlobalRenderData,
    editor_objs_referencing_current_line: &[EditorObjId],
    editor_objects: &EditorObjects,
) {
    for id in editor_objs_referencing_current_line {
        for ed_obj in &editor_objects[id.content_index] {
            match ed_obj {
                EditorObject {
                    typ: EditorObjectType::LineReference { var_index },
                    ..
                }
                | EditorObject {
                    typ: EditorObjectType::Variable { var_index },
                    ..
                } if *var_index == id.var_index => {
                    if gr.is_visible(ed_obj.row) {
                        let rendered_row_height = gr.get_rendered_height(ed_obj.row);
                        let vert_align_offset = (rendered_row_height - ed_obj.rendered_h) / 2;
                        render_buckets.custom_commands[Layer::AboveText as usize].push(
                            OutputMessage::PulsingRectangle {
                                x: gr.left_gutter_width + ed_obj.rendered_x,
                                y: ed_obj.rendered_y.add(vert_align_offset),
                                w: ed_obj.rendered_w,
                                h: ed_obj.rendered_h,
                                start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                                end_color: 0x00FF7F_00,
                                animation_time: Duration::from_millis(1000),
                            },
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

pub fn pulse_changed_results(
    render_buckets: &mut RenderBuckets,
    gr: &GlobalRenderData,
    longest_rendered_result_len: usize,
    result_change_flag: &EditorRowFlags,
) {
    if gr.get_render_y(content_y(0)).is_none() {
        // there were no render yet
        return;
    }
    // TODO iter through only visible rows
    // Pulsing changed results
    for i in 0..MAX_LINE_COUNT {
        if result_change_flag.is_true(i) {
            if let Some(render_y) = gr.get_render_y(content_y(i)) {
                render_buckets.custom_commands[Layer::AboveText as usize].push(
                    OutputMessage::PulsingRectangle {
                        x: gr.result_gutter_x + RIGHT_GUTTER_WIDTH,
                        y: render_y,
                        w: longest_rendered_result_len,
                        h: gr.get_rendered_height(content_y(i)),
                        start_color: CHANGE_RESULT_PULSE_START_COLOR,
                        end_color: CHANGE_RESULT_PULSE_END_COLOR,
                        animation_time: Duration::from_millis(1000),
                    },
                );
            }
        }
    }
}

pub fn parse_tokens<'b>(
    line: &[char],
    editor_y: usize,
    units: &Units,
    vars: &Variables,
    allocator: &'b Bump,
) -> Tokens<'b> {
    // TODO optimize vec allocations
    let mut tokens = Vec::with_capacity(128);
    TokenParser::parse_line(line, &vars, &mut tokens, &units, editor_y, allocator);

    // TODO: measure is 128 necessary?
    // and remove allocation
    let mut shunting_output_stack = Vec::with_capacity(128);
    ShuntingYard::shunting_yard(&mut tokens, &mut shunting_output_stack);
    Tokens {
        tokens,
        shunting_output_stack,
    }
}

fn render_simple_text_line<'text_ptr>(
    line: &[char],
    r: &mut PerLineRenderData,
    gr: &mut GlobalRenderData,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    allocator: &'text_ptr Bump,
) {
    r.set_fix_row_height(1);
    gr.set_rendered_height(r.editor_y, 1);

    let text_len = line.len().min(gr.current_editor_width);

    render_buckets.utf8_texts.push(RenderUtf8TextMsg {
        text: allocator.alloc_slice_fill_iter(line.iter().map(|it| *it).take(text_len)),
        row: r.render_y,
        column: gr.left_gutter_width,
    });

    r.token_render_done(text_len, text_len, 0);
}

#[inline]
fn highlight_line_refs<'text_ptr>(
    editor_objs: &Vec<EditorObject>,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    r: &PerLineRenderData,
    gr: &GlobalRenderData,
) {
    for editor_obj in editor_objs.iter() {
        if matches!(editor_obj.typ, EditorObjectType::LineReference{..}) {
            let vert_align_offset = (r.rendered_row_height - editor_obj.rendered_h) / 2;
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
}

#[inline]
fn highlight_active_line_refs<'text_ptr>(
    editor_objs: &[EditorObject],
    render_buckets: &mut RenderBuckets<'text_ptr>,
    r: &PerLineRenderData,
    gr: &GlobalRenderData,
) {
    let mut color_index = 0;
    let mut colors: [Option<u32>; MAX_LINE_COUNT] = [None; MAX_LINE_COUNT];
    for editor_obj in editor_objs.iter() {
        match editor_obj.typ {
            EditorObjectType::LineReference { var_index }
            | EditorObjectType::Variable { var_index }
                if var_index != SUM_VARIABLE_INDEX =>
            {
                let color = if let Some(color) = colors[var_index] {
                    color
                } else {
                    let color = ACTIVE_LINE_REF_HIGHLIGHT_COLORS[color_index] << 8 | 0x55;
                    colors[var_index] = Some(color);
                    color_index = if color_index < 8 { color_index + 1 } else { 0 };
                    color
                };

                let vert_align_offset = (r.rendered_row_height - editor_obj.rendered_h) / 2;
                render_buckets.set_color(Layer::BehindText, color);
                render_buckets.draw_rect(
                    Layer::BehindText,
                    gr.left_gutter_width + editor_obj.rendered_x,
                    editor_obj.rendered_y.add(vert_align_offset),
                    editor_obj.rendered_w,
                    editor_obj.rendered_h,
                );
            }
            _ => {}
        }
    }
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
        } else if let (TokenType::Variable { var_index }, true) = (&token.typ, need_matrix_renderer)
        {
            editor_objects.push(EditorObject {
                typ: EditorObjectType::Variable {
                    var_index: *var_index,
                },
                row: r.editor_y,
                start_x: r.editor_x,
                end_x: r.editor_x + token.ptr.len(),
                rendered_x: r.render_x,
                rendered_y: r.render_y,
                rendered_w: token.ptr.len(),
                rendered_h: r.rendered_row_height,
            });
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
                typ: EditorObjectType::LineReference {
                    var_index: *var_index,
                },
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
            if let Some(EditorObject {
                typ: EditorObjectType::SimpleTokens,
                end_x,
                rendered_w,
                ..
            }) = editor_objects.last_mut()
            {
                // last token was a simple token too, extend it
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
    line_reference_chooser: &Option<ContentIndex>,
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
) {
    let cursor_pos = editor.get_selection().get_cursor_pos();
    if cursor_pos.row == r.editor_y.as_usize() {
        render_buckets.set_color(Layer::Text, 0xFFFFCC_55);
        render_buckets.draw_rect(
            Layer::Text,
            0,
            r.render_y,
            gr.result_gutter_x + RIGHT_GUTTER_WIDTH + gr.current_result_panel_width,
            r.rendered_row_height,
        );
    }
}

fn evaluate_tokens_and_save_result<'text_ptr>(
    vars: &mut Variables,
    editor_y: usize,
    editor_content: &EditorContent<LineData>,
    tokens: &mut [Token<'text_ptr>],
    shunting_output_stack: &mut Vec<ShuntingYardResult>,
    line: &[char],
) -> Result<Option<EvaluationResult>, ()> {
    let result = evaluate_tokens(tokens, shunting_output_stack, &vars);
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
                if line[0] == '=' {
                    // it might happen that there are more '=' in a line.
                    // To avoid panic, start the index from 1, so if the first char is
                    // '=', it will be ignored.
                    i += 1;
                }
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
            // TODO opt
            let var_name: Vec<char> = format!("&[{}]", line_id).chars().collect();
            replace_or_insert_var(vars, &var_name, result.result.clone(), editor_y);
        }
    } else {
        if let Some(var) = &mut vars[editor_y] {
            let line_data = editor_content.get_data(editor_y);
            debug_assert!(line_data.line_id > 0);
            let line_id = line_data.line_id;
            // TODO opt
            let var_name: Vec<char> = format!("&[{}]", line_id).chars().collect();
            var.name = Box::from(var_name);
            var.value = Err(());
        } else {
            vars[editor_y] = None;
        }
    }
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
    let cursor_inside_matrix: bool = if editor.get_selection().is_range().is_none()
        && cursor_pos.row == r.editor_y.as_usize()
        && cursor_pos.column > r.editor_x
        && cursor_pos.column < r.editor_x + text_width
    {
        // cursor is inside the matrix
        true
    } else {
        false
    };

    let new_render_x = if let (true, Some(mat_editor)) = (cursor_inside_matrix, matrix_editing) {
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
        rendered_h: MatrixData::calc_render_height(row_count),
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
    allocator: &Bump,
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
    allocator: &'text_ptr Bump,
) -> Result<Option<EvaluationResult>, ()> {
    TokenParser::parse_line(text, vars, tokens, &units, editor_y, allocator);
    let mut shunting_output_stack = Vec::with_capacity(4);
    ShuntingYard::shunting_yard(tokens, &mut shunting_output_stack);
    return evaluate_tokens(tokens, &mut shunting_output_stack, &vars);
}

fn render_matrix_obj<'text_ptr>(
    mut render_x: usize,
    render_y: CanvasY,
    current_editor_width: usize,
    left_gutter_width: usize,
    row_count: usize,
    col_count: usize,
    tokens: &[Token<'text_ptr>],
    render_buckets: &mut RenderBuckets<'text_ptr>,
    rendered_row_height: usize,
) -> usize {
    let vert_align_offset = (rendered_row_height - MatrixData::calc_render_height(row_count)) / 2;

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
    render_y: CanvasY,
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
    render_y: CanvasY,
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
    render_y: CanvasY,
    mat: &MatrixData,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    prev_mat_result_lengths: Option<&ResultLengths>,
    rendered_row_height: usize,
    decimal_count: Option<usize>,
) -> usize {
    let start_x = render_x;

    let vert_align_offset = (rendered_row_height - mat.render_height()) / 2;
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
        Ok(CalcResult {
            typ: CalcResultType::Matrix(mat),
            ..
        }) => {
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
            (rendered_width, mat.render_height())
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

struct ResultTmp {
    buffer_ptr: Option<Range<usize>>,
    editor_y: ContentIndex,
    lengths: ResultLengths,
}
struct ResultRender {
    result_ranges: SmallVec<[ResultTmp; MAX_LINE_COUNT]>,
    max_len: usize,
    max_lengths: ResultLengths,
}

impl ResultRender {
    pub fn new(vec: SmallVec<[ResultTmp; MAX_LINE_COUNT]>) -> ResultRender {
        return ResultRender {
            result_ranges: vec,
            max_len: 0,
            max_lengths: ResultLengths {
                int_part_len: 0,
                frac_part_len: 0,
                unit_part_len: 0,
            },
        };
    }
}

fn render_results_into_buf_and_calc_len<'text_ptr>(
    units: &Units,
    results: &[LineResult],
    result_buffer: &'text_ptr mut [u8],
    tmp: &mut ResultRender,
    editor_content: &EditorContent<LineData>,
    gr: &GlobalRenderData,
    decimal_count: Option<usize>,
) {
    let mut result_buffer_index = 0;
    // calc max length and render results into buffer
    for (editor_y, result) in results.iter().enumerate() {
        let editor_y = content_y(editor_y);
        if gr.get_render_y(editor_y).is_none() {
            continue;
        };
        if !gr.is_visible(editor_y) {
            continue;
        }

        if let Err(..) = result {
            result_buffer[result_buffer_index] = b'E';
            result_buffer[result_buffer_index + 1] = b'r';
            result_buffer[result_buffer_index + 2] = b'r';
            tmp.result_ranges.push(ResultTmp {
                buffer_ptr: Some(result_buffer_index..result_buffer_index + 3),
                editor_y,
                lengths: ResultLengths {
                    int_part_len: 999,
                    frac_part_len: 0,
                    unit_part_len: 0,
                },
            });
            result_buffer_index += 3;
        } else if let Ok(Some(result)) = result {
            match &result.typ {
                CalcResultType::Matrix(_mat) => {
                    tmp.result_ranges.push(ResultTmp {
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
                    tmp.max_lengths.set_max(&lens);
                    tmp.result_ranges.push(ResultTmp {
                        buffer_ptr: Some(range),
                        editor_y,
                        lengths: lens,
                    });
                    result_buffer_index += len;
                }
            };
        } else {
            tmp.result_ranges.push(ResultTmp {
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
    tmp.max_len = tmp.max_lengths.int_part_len
        + tmp.max_lengths.frac_part_len
        + if tmp.max_lengths.unit_part_len > 0 {
            tmp.max_lengths.unit_part_len + 1
        } else {
            0
        };
}

fn create_render_commands_for_results_and_render_matrices<'text_ptr>(
    tmp: &ResultRender,
    units: &Units,
    results: &[LineResult],
    render_buckets: &mut RenderBuckets<'text_ptr>,
    result_buffer: &'text_ptr [u8],
    gr: &GlobalRenderData,
    decimal_count: Option<usize>,
) -> usize {
    let mut prev_result_matrix_length = None;
    let mut matrix_len = 0;
    for result_tmp in tmp.result_ranges.iter() {
        let rendered_row_height = gr.get_rendered_height(result_tmp.editor_y);
        let render_y = gr.get_render_y(result_tmp.editor_y).expect("");
        if let Some(result_range) = &result_tmp.buffer_ptr {
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
            let offset_x = if tmp.max_lengths.int_part_len < lengths.int_part_len {
                // it is an "Err"
                ResultOffsetX::Err
            } else {
                let offset_x = tmp.max_lengths.int_part_len - lengths.int_part_len;
                let sum_len = lengths.int_part_len
                    + tmp.max_lengths.frac_part_len
                    + tmp.max_lengths.unit_part_len;
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
            let x = gr.result_gutter_x
                + RIGHT_GUTTER_WIDTH
                + match offset_x {
                    ResultOffsetX::Err => 0,
                    ResultOffsetX::TooLong => 0,
                    ResultOffsetX::Ok(n) => n,
                };
            let int_w = match offset_x {
                ResultOffsetX::Err => 3,
                _ => lengths.int_part_len,
            };
            render_buckets.ascii_texts.push(RenderAsciiTextMsg {
                text: &result_buffer[from..from + int_w],
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
                // e.g. in case of 2 units mm and m, m should be 1 coordinates right
                let offset_x = tmp.max_lengths.unit_part_len - lengths.unit_part_len;
                render_buckets.ascii_texts.push(RenderAsciiTextMsg {
                    text: &result_buffer[from..result_range.end],
                    row,
                    column: gr.result_gutter_x
                        + RIGHT_GUTTER_WIDTH
                        + tmp.max_lengths.int_part_len
                        + tmp.max_lengths.frac_part_len
                        + 1
                        + offset_x,
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
            prev_result_matrix_length = None;
        } else {
            match &results[result_tmp.editor_y.as_usize()] {
                Ok(Some(CalcResult {
                    typ: CalcResultType::Matrix(mat),
                    ..
                })) => {
                    // TODO: why it is called "prev.."?
                    if prev_result_matrix_length.is_none() {
                        prev_result_matrix_length = calc_consecutive_matrices_max_lengths(
                            units,
                            &results[result_tmp.editor_y.as_usize()..],
                        );
                    }
                    let width = render_matrix_result(
                        units,
                        gr.result_gutter_x + RIGHT_GUTTER_WIDTH,
                        render_y,
                        mat,
                        render_buckets,
                        prev_result_matrix_length.as_ref(),
                        gr.get_rendered_height(result_tmp.editor_y),
                        decimal_count,
                    );
                    if width > matrix_len {
                        matrix_len = width;
                    }
                }
                _ => {
                    // no result but need rerender
                    // result background
                    prev_result_matrix_length = None;
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
    }
    return matrix_len;
}

fn calc_consecutive_matrices_max_lengths(
    units: &Units,
    results: &[LineResult],
) -> Option<ResultLengths> {
    let mut max_lengths: Option<ResultLengths> = None;
    for result in results.iter() {
        match result {
            Ok(Some(CalcResult {
                typ: CalcResultType::Matrix(mat),
                ..
            })) => {
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

fn draw_line_refs_and_vars_referenced_from_cur_row<'b>(
    editor_objs: &[EditorObject],
    gr: &GlobalRenderData,
    render_buckets: &mut RenderBuckets<'b>,
    editor_y_to_render_w: &[usize; MAX_LINE_COUNT],
) {
    let mut color_index = 0;
    let mut highlighted = EditorRowFlags::empty();
    for editor_obj in editor_objs {
        match editor_obj.typ {
            EditorObjectType::LineReference { var_index }
            | EditorObjectType::Variable { var_index } => {
                if var_index == SUM_VARIABLE_INDEX {
                    continue;
                }
                let color = if highlighted.is_true(var_index) {
                    continue;
                } else {
                    highlighted.set(var_index);
                    let color = ACTIVE_LINE_REF_HIGHLIGHT_COLORS[color_index];
                    color_index = if color_index < 8 { color_index + 1 } else { 0 };
                    color
                };
                let defined_at = content_y(var_index);
                if let Some(render_y) = gr.get_render_y(defined_at) {
                    render_buckets.custom_commands[Layer::AboveText as usize]
                        .push(OutputMessage::SetColor(color << 8 | 0x33));
                    render_buckets.custom_commands[Layer::AboveText as usize].push(
                        OutputMessage::RenderRectangle {
                            x: gr.left_gutter_width,
                            y: render_y,
                            w: editor_y_to_render_w[defined_at.as_usize()],
                            h: gr.get_rendered_height(defined_at),
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

fn draw_token<'text_ptr>(
    token: &Token<'text_ptr>,
    render_x: usize,
    render_y: CanvasY,
    current_editor_width: usize,
    left_gutter_width: usize,
    render_buckets: &mut RenderBuckets<'text_ptr>,
) {
    let dst = if token.has_error() {
        &mut render_buckets.number_errors
    } else {
        match &token.typ {
            TokenType::StringLiteral => &mut render_buckets.utf8_texts,
            TokenType::Variable { .. } => &mut render_buckets.variable,
            TokenType::LineReference { .. } => &mut render_buckets.variable,
            TokenType::NumberLiteral(_) => &mut render_buckets.numbers,
            TokenType::NumberErr => &mut render_buckets.number_errors,
            TokenType::Operator(OperatorTokenType::ApplyUnit(_)) => &mut render_buckets.units,
            TokenType::Unit(_) => &mut render_buckets.units,
            TokenType::Operator(_) => &mut render_buckets.operators,
        }
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
    fn write_char_slice(canvas: &mut [[char; 256]], row: CanvasY, col: usize, src: &[char]) {
        let str = &mut canvas[row.as_usize()];
        for (dst_char, src_char) in str[col..].iter_mut().zip(src.iter()) {
            *dst_char = *src_char;
        }
    }

    fn write_str(canvas: &mut [[char; 256]], row: CanvasY, col: usize, src: &str) {
        let str = &mut canvas[row.as_usize()];
        for (dst_char, src_char) in str[col..].iter_mut().zip(src.chars()) {
            *dst_char = src_char;
        }
    }

    fn write_ascii(canvas: &mut [[char; 256]], row: CanvasY, col: usize, src: &[u8]) {
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

    for command in &buckets.number_errors {
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
    allocator: &'text_ptr Bump,
) {
    render_buckets.set_color(Layer::BehindText, 0xA6D2FF_FF);
    if let Some((start, end)) = editor.get_selection().is_range() {
        if end.row > start.row {
            // first line
            if let Some(start_render_y) = gr.get_render_y(content_y(start.row)) {
                let height = gr.get_rendered_height(content_y(start.row));
                render_buckets.draw_rect(
                    Layer::BehindText,
                    start.column + gr.left_gutter_width,
                    start_render_y,
                    (editor_content.line_len(start.row) - start.column)
                        .min(gr.current_editor_width),
                    height,
                );
            }
            // full lines
            for i in start.row + 1..end.row {
                if let Some(render_y) = gr.get_render_y(content_y(i)) {
                    let height = gr.get_rendered_height(content_y(i));
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
            if let Some(end_render_y) = gr.get_render_y(content_y(end.row)) {
                let height = gr.get_rendered_height(content_y(end.row));
                render_buckets.draw_rect(
                    Layer::BehindText,
                    gr.left_gutter_width,
                    end_render_y,
                    end.column.min(gr.current_editor_width),
                    height,
                );
            }
        } else if let Some(start_render_y) = gr.get_render_y(content_y(start.row)) {
            let height = gr.get_rendered_height(content_y(start.row));
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
                if let Some(start_render_y) = gr.get_render_y(content_y(start.row)) {
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
                let frist_visible_row_index = content_y(start.row.max(gr.scroll_y));
                let last_visible_row_index =
                    content_y(end.row.min(gr.scroll_y + gr.client_height - 1));
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

fn calc_result_gutter_x(current_x: usize, client_width: usize, left_gutter_width: usize) -> usize {
    let default_x = default_result_gutter_x(client_width);
    current_x.min(default_x).max(left_gutter_width + 4)
}

fn calc_result_gutter_x_wrt_result_lengths(
    longest_result_len: usize,
    client_width: usize,
    left_gutter_width: usize,
) -> usize {
    let x_wrt_results =
        client_width - (RIGHT_GUTTER_WIDTH + longest_result_len.max(MIN_RESULT_PANEL_WIDTH));
    calc_result_gutter_x(x_wrt_results, client_width, left_gutter_width)
}

fn default_result_gutter_x(client_width: usize) -> usize {
    (client_width * DEFAULT_RESULT_PANEL_WIDTH_PERCENT / 100)
        .max(LEFT_GUTTER_MIN_WIDTH + SCROLL_BAR_WIDTH)
}

fn calc_rendered_height<'b>(
    editor_y: ContentIndex,
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
                .map(|it| MatrixData::calc_render_height(it.row_count)),
        );
        h
    } else {
        1
    };
}

fn is_pos_inside_an_obj(editor_objects: &EditorObjects, pos: Pos) -> Option<&EditorObject> {
    for obj in &editor_objects[content_y(pos.row)] {
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

fn get_scroll_y_after_cursor_movement(
    prev_row: usize,
    current_row: usize,
    render_data: &GlobalRenderData,
) -> Option<usize> {
    if prev_row != current_row {
        if current_row < render_data.scroll_y {
            // scroll up
            Some(current_row)
        } else if render_data
            .get_render_y(content_y(current_row))
            .map(|render_y| render_y.as_isize() >= (render_data.client_height as isize))
            .unwrap_or(false)
        {
            // scroll down
            Some(
                render_data
                    .get_render_y(content_y(current_row))
                    .expect("must")
                    .as_usize()
                    + render_data.scroll_y
                    - (render_data.client_height - 1),
            )
        } else {
            None
        }
    } else {
        None
    }
}

fn change_result_panel_size_wrt_result_len(client_width: usize, gr: &mut GlobalRenderData) {
    let new_x = calc_result_gutter_x_wrt_result_lengths(
        gr.longest_rendered_result_len,
        client_width,
        gr.left_gutter_width,
    );
    if new_x < gr.result_gutter_x
        || new_x >= (client_width * DEFAULT_RESULT_PANEL_WIDTH_PERCENT / 100)
    {
        // there might be a new result which requires more space to render

        // TODO: this change will have an effect only in the next render cycle
        gr.set_result_gutter_x(client_width, new_x);
    }
}

#[cfg(test)]
mod main_tests {

    use super::*;

    static mut RESULT_BUFFER: [u8; 2048] = [0; 2048];

    const fn result_panel_w(client_width: usize) -> usize {
        client_width * DEFAULT_RESULT_PANEL_WIDTH_PERCENT / 100
    }

    struct RustIsShit {
        app_ptr: u64,
        units_ptr: u64,
        render_bucket_ptr: u64,
        tokens_ptr: u64,
        results_ptr: u64,
        vars_ptr: u64,
        editor_objects_ptr: u64,
        allocator: u64,
    }

    impl RustIsShit {
        fn mut_app<'a>(&self) -> &'a mut NoteCalcApp {
            unsafe { &mut *(self.app_ptr as *mut NoteCalcApp) }
        }

        fn app<'a>(&self) -> &'a NoteCalcApp {
            unsafe { &*(self.app_ptr as *const NoteCalcApp) }
        }

        fn units<'a>(&self) -> &'a mut Units {
            unsafe { &mut *(self.units_ptr as *mut Units) }
        }

        fn mut_render_bucket<'a>(&self) -> &'a mut RenderBuckets<'a> {
            unsafe { &mut *(self.render_bucket_ptr as *mut RenderBuckets) }
        }

        fn tokens<'a>(&self) -> &'a AppTokens<'a> {
            unsafe { &*(self.tokens_ptr as *const AppTokens) }
        }

        fn mut_tokens<'a>(&self) -> &'a mut AppTokens<'a> {
            unsafe { &mut *(self.tokens_ptr as *mut AppTokens) }
        }

        fn mut_results<'a>(&self) -> &'a mut Results {
            unsafe { &mut *(self.results_ptr as *mut Results) }
        }

        fn mut_editor_objects<'a>(&self) -> &'a mut EditorObjects {
            unsafe { &mut *(self.editor_objects_ptr as *mut EditorObjects) }
        }

        fn editor_objects<'a>(&self) -> &'a EditorObjects {
            unsafe { &*(self.editor_objects_ptr as *const EditorObjects) }
        }

        fn mut_vars<'a>(&self) -> &'a mut [Option<Variable>] {
            unsafe {
                &mut (&mut *(self.vars_ptr as *mut [Option<Variable>; MAX_LINE_COUNT + 1]))[..]
            }
        }

        fn allocator<'a>(&self) -> &'a Bump {
            unsafe { &*(self.allocator as *const Bump) }
        }

        fn render(&self) {
            self.mut_app()
                .generate_render_commands_and_fill_editor_objs(
                    self.units(),
                    self.mut_render_bucket(),
                    unsafe { &mut RESULT_BUFFER },
                    self.allocator(),
                    self.mut_tokens(),
                    self.mut_results(),
                    self.mut_vars(),
                    self.mut_editor_objects(),
                    EditorRowFlags::empty(),
                );
        }

        fn paste(&self, str: &str) {
            self.mut_app().handle_paste(
                str.to_owned(),
                self.units(),
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_editor_objects(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn render_get_result_buf(&self, result_buffer: &mut [u8]) {
            self.mut_app()
                .generate_render_commands_and_fill_editor_objs(
                    self.units(),
                    self.mut_render_bucket(),
                    result_buffer,
                    self.allocator(),
                    self.mut_tokens(),
                    self.mut_results(),
                    self.mut_vars(),
                    self.mut_editor_objects(),
                    EditorRowFlags::empty(),
                );
        }

        fn render_get_result_commands<'b>(
            &'b self,
            render_buckets: &mut RenderBuckets<'b>,
            result_buffer: &'b mut [u8],
        ) {
            self.mut_app()
                .generate_render_commands_and_fill_editor_objs(
                    self.units(),
                    render_buckets,
                    result_buffer,
                    self.allocator(),
                    self.mut_tokens(),
                    self.mut_results(),
                    self.mut_vars(),
                    self.mut_editor_objects(),
                    EditorRowFlags::empty(),
                );
        }

        fn contains<'b>(
            &'b self,
            render_bucket: &[OutputMessage<'b>],
            expected_count: usize,
            expected_command: OutputMessage,
        ) {
            let mut count = 0;
            for command in render_bucket {
                if *command == expected_command {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times, expected {}.\n{:?}\nin\n{:?}",
                count, expected_count, expected_command, render_bucket
            );
        }

        fn set_normalized_content(&self, str: &str) {
            self.mut_app().set_normalized_content(
                str,
                self.units(),
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_editor_objects(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn repeated_paste(&self, str: &str, times: usize) {
            self.mut_app().handle_paste(
                str.repeat(times),
                self.units(),
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_editor_objects(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn click(&self, x: usize, y: isize) {
            self.mut_app().handle_click(
                x,
                canvas_y(y),
                self.mut_editor_objects(),
                self.units(),
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn handle_resize(&self, new_client_width: usize) {
            self.mut_app().handle_resize(
                new_client_width,
                self.mut_editor_objects(),
                self.units(),
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn handle_wheel(&self, dir: usize) {
            self.mut_app().handle_wheel(
                dir,
                self.mut_editor_objects(),
                self.units(),
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn handle_drag(&self, x: usize, y: isize) {
            self.mut_app().handle_drag(
                x,
                canvas_y(y),
                self.mut_editor_objects(),
                self.units(),
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn alt_key_released(&self) {
            self.mut_app().alt_key_released(
                self.units(),
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_editor_objects(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn handle_time(&self, tick: u32) {
            self.mut_app().handle_time(
                tick,
                self.units(),
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_editor_objects(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn input(&self, event: EditorInputEvent, modif: InputModifiers) {
            self.mut_app().handle_input(
                event,
                modif,
                self.allocator(),
                self.units(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_editor_objects(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
            );
        }

        fn handle_mouse_up(&self) {
            self.mut_app().handle_mouse_up();
        }

        fn get_render_data(&self) -> GlobalRenderData {
            return self.mut_app().render_data.clone();
        }

        fn get_editor_content(&self) -> String {
            return self.mut_app().editor_content.get_content();
        }

        fn get_cursor_pos(&self) -> Pos {
            return self.mut_app().editor.get_selection().get_cursor_pos();
        }

        fn get_selection(&self) -> Selection {
            return self.mut_app().editor.get_selection();
        }

        fn set_selection(&self, selection: Selection) {
            let app = &mut self.mut_app();
            app.editor.set_selection_save_col(selection);
        }

        fn set_cursor_row_col(&self, row: usize, col: usize) {
            self.set_selection(Selection::single_r_c(row, col));
        }
    }

    fn create_app3<'a>(client_width: usize, client_height: usize) -> RustIsShit {
        let app = NoteCalcApp::new(client_width, client_height);
        let editor_objects = EditorObjects::new();
        let tokens = AppTokens::new();
        let results = Results::new();
        let vars = create_vars();
        fn to_box_ptr<T>(t: T) -> u64 {
            let ptr = Box::into_raw(Box::new(t)) as u64;
            ptr
        }
        return RustIsShit {
            app_ptr: to_box_ptr(app),
            units_ptr: to_box_ptr(Units::new()),
            render_bucket_ptr: to_box_ptr(RenderBuckets::new()),
            tokens_ptr: to_box_ptr(tokens),
            results_ptr: to_box_ptr(results),
            vars_ptr: to_box_ptr(vars),
            editor_objects_ptr: to_box_ptr(editor_objects),
            allocator: to_box_ptr(Bump::with_capacity(MAX_LINE_COUNT * 120)),
        };
    }

    fn create_app2<'a>(client_height: usize) -> RustIsShit {
        create_app3(120, client_height)
    }

    #[test]
    fn bug1() {
        let test = create_app2(35);
        test.paste("[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]");

        test.set_cursor_row_col(0, 33);
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.render();
    }

    #[test]
    fn bug2() {
        let test = create_app2(35);
        test.paste("[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]");
        test.set_cursor_row_col(0, 1);

        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.render();
        test.input(EditorInputEvent::Down, InputModifiers::none());
        test.render();
    }

    #[test]
    fn bug3() {
        let test = create_app2(35);
        test.paste(
            "1\n\
                    2+",
        );
        test.set_cursor_row_col(1, 2);
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        test.render();
    }

    #[test]
    fn test_that_variable_name_is_inserted_when_referenced_a_var_line() {
        let test = create_app2(35);
        test.paste(
            "var_name = 1\n\
                    2+",
        );
        test.set_cursor_row_col(1, 2);
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        test.render();
        assert_eq!(
            "var_name = 1\n\
                 2+var_name",
            test.get_editor_content()
        );
    }

    #[test]
    fn bug4() {
        let test = create_app2(35);
        test.paste(
            "1\n\
                    ",
        );
        test.render();
        test.set_cursor_row_col(1, 0);
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        test.render();
        assert_eq!(
            "1\n\
                 &[1]",
            test.get_editor_content()
        );
    }

    #[test]
    fn bug5() {
        let test = create_app2(35);
        test.paste("123\na ");

        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        assert_eq!(
            3,
            test.tokens()[content_y(1)].as_ref().unwrap().tokens.len()
        );
    }

    #[test]
    fn it_is_not_allowed_to_ref_lines_below() {
        let test = create_app2(35);
        test.paste(
            "1\n\
                    2+\n3\n4",
        );
        test.render();
        test.set_cursor_row_col(1, 2);
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.alt_key_released();
        test.render();
        assert_eq!(
            "1\n\
                    2+\n3\n4",
            test.get_editor_content()
        );
    }

    #[test]
    fn it_is_not_allowed_to_ref_lines_below2() {
        let test = create_app2(35);
        test.paste(
            "1\n\
                    2+\n3\n4",
        );
        test.render();
        test.set_cursor_row_col(1, 2);
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.alt_key_released();
        test.render();
        assert_eq!(
            "1\n\
                    2+&[1]\n3\n4",
            test.get_editor_content()
        );
    }

    #[test]
    fn remove_matrix_backspace() {
        let test = create_app2(35);
        test.paste("abcd [1,2,3;4,5,6]");
        test.render();
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        assert_eq!("abcd ", test.get_editor_content());
    }

    #[test]
    fn matrix_step_in_dir() {
        // from right
        {
            let test = create_app2(35);
            test.paste("abcd [1,2,3;4,5,6]");
            test.render();
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("abcd [1,2,1;4,5,6]", test.get_editor_content());
        }
        // from left
        {
            let test = create_app2(35);
            test.paste("abcd [1,2,3;4,5,6]");
            test.set_cursor_row_col(0, 5);
            test.render();
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!("abcd [9,2,3;4,5,6]", test.get_editor_content());
        }
        // from below
        {
            let test = create_app2(35);
            test.paste("abcd [1,2,3;4,5,6]\naaaaaaaaaaaaaaaaaa");
            test.set_cursor_row_col(1, 7);
            test.render();
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!(
                "abcd [1,2,3;9,5,6]\naaaaaaaaaaaaaaaaaa",
                test.get_editor_content()
            );
        }
        // from above
        {
            let test = create_app2(35);
            test.paste("aaaaaaaaaaaaaaaaaa\nabcd [1,2,3;4,5,6]");
            test.set_cursor_row_col(0, 7);
            test.render();
            test.input(EditorInputEvent::Down, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            assert_eq!(
                "aaaaaaaaaaaaaaaaaa\nabcd [9,2,3;4,5,6]",
                test.get_editor_content()
            );
        }
    }

    #[test]
    fn cursor_is_put_after_the_matrix_after_finished_editing() {
        let test = create_app2(35);
        test.paste("abcd [1,2,3;4,5,6]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Char('6'), InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Char('9'), InputModifiers::none());
        assert_eq!(test.get_editor_content(), "abcd [1,2,6;4,5,6]9");
    }

    #[test]
    fn remove_matrix_del() {
        let test = create_app2(35);
        test.paste("abcd [1,2,3;4,5,6]");
        test.set_cursor_row_col(0, 5);
        test.render();
        test.input(EditorInputEvent::Del, InputModifiers::none());
        assert_eq!("abcd ", test.get_editor_content());
    }

    #[test]
    fn test_that_selected_matrix_content_is_copied_on_ctrl_c() {
        let test = create_app2(35);
        test.paste("abcd [69,2,3;4,5,6]");
        test.set_cursor_row_col(0, 5);
        test.render();
        test.input(EditorInputEvent::Right, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Char('c'), InputModifiers::ctrl());
        assert_eq!(
            test.mut_app()
                .get_selected_text_and_clear_app_clipboard()
                .as_ref()
                .map(|it| it.as_str()),
            Some("69")
        );
    }

    #[test]
    fn test_insert_matrix_line_ref_panic() {
        let test = create_app2(35);
        test.paste("[1,2,3;4,5,6]\n[1;2;3]\n");
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        assert_eq!(test.get_render_data().get_rendered_height(content_y(2)), 5);
    }

    #[test]
    fn test_matrix_rendering_parameters_single_row() {
        let test = create_app2(35);
        test.paste("[1]");
        assert_eq!(test.editor_objects()[content_y(0)][0].rendered_x, 0);
        assert_eq!(
            test.editor_objects()[content_y(0)][0].rendered_y,
            canvas_y(0)
        );
        assert_eq!(test.editor_objects()[content_y(0)][0].rendered_h, 1);
        assert_eq!(test.editor_objects()[content_y(0)][0].rendered_w, 3);
    }

    #[test]
    fn test_matrix_rendering_parameters_multiple_rows() {
        let test = create_app2(35);
        test.paste("[1;2;3]");
        assert_eq!(test.editor_objects()[content_y(0)][0].rendered_x, 0);
        assert_eq!(
            test.editor_objects()[content_y(0)][0].rendered_y,
            canvas_y(0)
        );
        assert_eq!(test.editor_objects()[content_y(0)][0].rendered_h, 5);
        assert_eq!(test.editor_objects()[content_y(0)][0].rendered_w, 3);
    }

    #[test]
    fn test_referencing_matrix_size_correct2() {
        let test = create_app2(35);
        test.paste("[6]\n&[1]");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        assert_eq!(test.editor_objects()[content_y(1)][0].rendered_h, 1);
    }

    #[test]
    fn test_referencing_matrix_size_correct2_vert_align() {
        let test = create_app2(35);
        test.paste("[1;2;3]\n[4]\n&[1]  &[2]");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        let first_line_h = 5;
        let second_line_half = (5 / 2) + 1;
        let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
        test.contains(
            &test.mut_render_bucket().custom_commands[Layer::AboveText as usize],
            1,
            OutputMessage::PulsingRectangle {
                x: left_gutter_width + 5,
                y: canvas_y(first_line_h + second_line_half),
                w: 3,
                h: 1,
                start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                end_color: 0x00FF7F_00,
                animation_time: Duration::from_millis(1000),
            },
        )
    }

    #[test]
    fn test_referencing_matrix_size_correct() {
        let test = create_app2(35);
        test.paste("[1;2;3]\n&[1]");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        assert_eq!(test.editor_objects()[content_y(1)][0].rendered_h, 5);
    }

    #[test]
    fn test_moving_inside_a_matrix() {
        // right to left, cursor at end
        {
            let test = create_app2(35);
            test.paste("abcd [1,2,3;4,5,6]");
            test.render();
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.render();
            assert_eq!("abcd [1,9,3;4,5,6]", test.get_editor_content());
        }
        // pressing right while there is a selection, just cancels the selection and put the cursor
        // at the end of it
        {
            let test = create_app2(35);
            test.paste("abcd [1,2,3;4,5,6]");
            test.set_cursor_row_col(0, 5);
            test.render();
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.render();
            assert_eq!("abcd [19,2,3;4,5,6]", test.get_editor_content());
        }
        // left to right, cursor at start
        {
            let test = create_app2(35);
            test.paste("abcd [1,2,3;4,5,6]");
            test.set_cursor_row_col(0, 5);
            test.render();
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.render();
            assert_eq!("abcd [1,2,9;4,5,6]", test.get_editor_content());
        }
        // vertical movement down, cursor tries to keep its position
        {
            let test = create_app2(35);
            test.paste("abcd [1111,22,3;44,55555,666]");
            test.set_cursor_row_col(0, 5);
            test.render();
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.render();
            // inside the matrix
            test.input(EditorInputEvent::Down, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.render();
            assert_eq!("abcd [1111,22,3;9,55555,666]", test.get_editor_content());
        }

        // vertical movement up, cursor tries to keep its position
        {
            let test = create_app2(35);
            test.paste("abcd [1111,22,3;44,55555,666]");
            test.set_cursor_row_col(0, 5);
            test.render();
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.render();
            // inside the matrix
            test.input(EditorInputEvent::Down, InputModifiers::none());
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.render();
            assert_eq!("abcd [9,22,3;44,55555,666]", test.get_editor_content());
        }
    }

    #[test]
    fn test_moving_inside_a_matrix_with_tab() {
        let test = create_app2(35);
        test.paste("[1,2,3;4,5,6]");
        test.render();
        test.input(EditorInputEvent::Home, InputModifiers::none());
        test.input(EditorInputEvent::Right, InputModifiers::none());

        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('8'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('9'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('0'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('9'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('4'), InputModifiers::none());
        test.render();
        assert_eq!("[1,7,8;9,0,9]4", test.get_editor_content());
    }

    #[test]
    fn test_leaving_a_matrix_with_tab() {
        let test = create_app2(35);
        test.paste("[1,2,3;4,5,6]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        // the next tab should leave the matrix
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.render();
        assert_eq!("[1,2,3;4,5,6]7", test.get_editor_content());
    }

    #[test]
    fn end_btn_matrix() {
        {
            let test = create_app2(35);
            test.paste("abcd [1111,22,3;44,55555,666] qq");
            test.set_cursor_row_col(0, 5);
            test.render();
            test.input(EditorInputEvent::Right, InputModifiers::none());
            test.render();
            // inside the matrix
            test.input(EditorInputEvent::End, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.render();
            assert_eq!(
                "abcd [1111,22,9;44,55555,666] qq",
                test.get_editor_content()
            );
        }
        // pressing twice, exits the matrix
        {
            let test = create_app2(35);
            test.paste("abcd [1111,22,3;44,55555,666] qq");
            test.set_cursor_row_col(0, 5);
            test.render();
            test.input(EditorInputEvent::Right, InputModifiers::none());
            // inside the matrix
            test.input(EditorInputEvent::End, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::End, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.render();
            assert_eq!(
                "abcd [1111,22,3;44,55555,666] qq9",
                test.get_editor_content()
            );
        }
    }

    #[test]
    fn home_btn_matrix() {
        {
            let test = create_app2(35);
            test.paste("abcd [1111,22,3;44,55555,666]");
            test.render();
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.render();
            // inside the matrix
            test.input(EditorInputEvent::Home, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('9'), InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.render();
            assert_eq!("abcd [9,22,3;44,55555,666]", test.get_editor_content());
        }
        {
            let test = create_app2(35);
            test.paste("abcd [1111,22,3;44,55555,666]");
            test.render();
            test.input(EditorInputEvent::Left, InputModifiers::none());
            // inside the matrix
            test.input(EditorInputEvent::Home, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Home, InputModifiers::none());
            test.render();
            test.input(EditorInputEvent::Char('6'), InputModifiers::none());
            test.render();
            assert_eq!("6abcd [1111,22,3;44,55555,666]", test.get_editor_content());
        }
    }

    #[test]
    fn bug8() {
        let test = create_app2(35);
        test.paste("16892313\n14 * ");
        test.set_cursor_row_col(1, 5);
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        assert_eq!("16892313\n14 * &[1]", test.get_editor_content());
        test.render();
        test.handle_time(1000);
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        assert_eq!("16892313\n14 * ", test.get_editor_content());

        test.input(EditorInputEvent::Char('z'), InputModifiers::ctrl());
        assert_eq!("16892313\n14 * &[1]", test.get_editor_content());

        let _input_eff = test.input(EditorInputEvent::Right, InputModifiers::none()); // end selection
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        assert_eq!("16892313\n14 * a&[1]", test.get_editor_content());

        test.input(EditorInputEvent::Char(' '), InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Right, InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        assert_eq!("16892313\n14 * a &[1]b", test.get_editor_content());

        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        assert_eq!("16892313\n14 * a c&[1]b", test.get_editor_content());
    }

    #[test]
    fn test_referenced_line_calc() {
        let test = create_app2(35);
        test.paste("2\n3 * ");
        test.set_cursor_row_col(1, 4);
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        assert_eq!("2\n3 * &[1]", test.get_editor_content());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        assert_results(&["2", "6"][..], &result_buffer);
    }

    #[test]
    fn test_empty_right_gutter_min_len() {
        let test = create_app2(35);
        test.set_normalized_content("");
        assert_eq!(test.get_render_data().result_gutter_x, result_panel_w(120));
    }

    #[test]
    fn right_gutter_is_moving_if_there_would_be_enough_space_for_result() {
        let test = create_app3(40, 35);
        test.paste("1\n");
        assert_eq!(test.get_render_data().result_gutter_x, result_panel_w(40));

        test.paste("999 999 999 999");
        assert_eq!(
            test.get_render_data().result_gutter_x,
            40 - ("999 999 999 999".len() + RIGHT_GUTTER_WIDTH)
        );
    }

    #[test]
    fn right_gutter_is_moving_if_there_would_be_enough_space_for_binary_result() {
        let test = create_app3(40, 35);
        test.paste("9999");
        assert_eq!(test.get_render_data().result_gutter_x, result_panel_w(40),);

        test.input(EditorInputEvent::Left, InputModifiers::alt());
        assert_eq!(
            test.get_render_data().result_gutter_x,
            40 - ("100111 00001111".len() + RIGHT_GUTTER_WIDTH)
        );
    }

    #[test]
    fn right_gutter_calc_panic() {
        let test = create_app3(176, 35);
        test.paste("ok");
    }

    #[test]
    fn test_that_alignment_is_considered_for_longest_result_len() {
        let test = create_app2(35);
        test.set_normalized_content("80kg\n190cm\n0.0016\n0.128 kg");
        assert_eq!(test.get_render_data().longest_rendered_result_len, 11);
    }

    #[test]
    fn test_resize_keeps_result_width() {
        let test = create_app3(60, 35);
        test.set_normalized_content("80kg\n190cm\n0.0016\n0.128 kg");
        assert_eq!(test.get_render_data().longest_rendered_result_len, 11);
        assert_eq!(
            test.get_render_data().result_gutter_x,
            default_result_gutter_x(60)
        );

        test.handle_resize(50);
        assert_eq!(test.get_render_data().longest_rendered_result_len, 11);
        assert_eq!(
            test.get_render_data().result_gutter_x,
            default_result_gutter_x(50)
        );

        test.handle_resize(60);
        assert_eq!(test.get_render_data().longest_rendered_result_len, 11);
        assert_eq!(
            test.get_render_data().result_gutter_x,
            default_result_gutter_x(60)
        );

        test.handle_resize(40);
        assert_eq!(test.get_render_data().longest_rendered_result_len, 11);
        assert_eq!(
            test.get_render_data().result_gutter_x,
            40 - (11 + RIGHT_GUTTER_WIDTH)
        );

        test.handle_resize(30);
        assert_eq!(test.get_render_data().longest_rendered_result_len, 11);
        assert_eq!(
            test.get_render_data().result_gutter_x,
            30 - (11 + RIGHT_GUTTER_WIDTH)
        );

        test.handle_resize(20);
        assert_eq!(test.get_render_data().longest_rendered_result_len, 11);
        assert_eq!(
            test.get_render_data().result_gutter_x,
            20 - (11 + RIGHT_GUTTER_WIDTH)
        );

        // too small
        test.handle_resize(10);
        assert_eq!(test.get_render_data().longest_rendered_result_len, 11);
        assert_eq!(test.get_render_data().result_gutter_x, 7);
    }

    #[test]
    fn test_scroll_y_reset() {
        let test = create_app2(35);
        test.mut_app().render_data.scroll_y = 1;
        test.set_normalized_content("1111\n2222\n14 * &[2]&[2]&[2]\n");
        assert_eq!(0, test.get_render_data().scroll_y);
    }

    #[test]
    fn test_tab_change_clears_variables() {
        let test = create_app2(35);
        test.set_normalized_content(
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
        );

        test.render();

        test.set_normalized_content(
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
        );

        test.render();
    }

    #[test]
    fn test_panic_on_pressing_enter() {
        let test = create_app2(35);
        test.set_normalized_content(
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
    (TDEE - ((weight/1kg) * target weekly fat loss rate * 1100))kcal
Muscle gain
    monthly rates of weight gain = 1%
    (TDEE + (weight/1kg * monthly rates of weight gain * 330))kcal

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
        );

        fn assert_var(vars: &Variables, name: &str, defined_at: usize) {
            let var = vars[defined_at].as_ref().unwrap();
            assert!(var.value.is_ok(), "{}", name);
            assert_eq!(name.len(), var.name.len(), "{}", name);
            for (a, b) in name.chars().zip(var.name.iter()) {
                assert_eq!(a, *b, "{}", name);
            }
        }
        {
            let vars = &test.mut_vars();
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
        }

        test.set_cursor_row_col(6, 33);

        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.render();

        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        test.render();

        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.render();
        let vars = &test.mut_vars();
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
        let test = create_app2(35);
        test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n");
        test.set_cursor_row_col(12, 2);
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::shift());
        test.render();
    }

    #[test]
    fn matrix_deletion() {
        let test = create_app2(35);
        test.paste(" [1,2,3]");
        test.set_cursor_row_col(0, 0);
        test.render();
        test.input(EditorInputEvent::Del, InputModifiers::none());
        assert_eq!("[1,2,3]", test.get_editor_content());
    }

    #[test]
    fn matrix_insertion_bug() {
        let test = create_app2(35);
        test.paste("[1,2,3]");
        test.set_cursor_row_col(0, 0);
        test.render();
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        assert_eq!("a[1,2,3]", test.get_editor_content());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("a\n[1,2,3]", test.get_editor_content());
    }

    #[test]
    fn matrix_insertion_bug2() {
        let test = create_app2(35);
        test.paste("'[X] nth, sum fv");
        test.render();
        test.set_cursor_row_col(0, 0);
        test.input(EditorInputEvent::Del, InputModifiers::none());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["Err"][..], &result_buffer);
    }

    #[test]
    fn test_err_result_rendering() {
        let test = create_app2(35);
        test.paste("'[X] nth, sum fv");
        test.render();
        test.set_cursor_row_col(0, 0);
        test.input(EditorInputEvent::Del, InputModifiers::none());
        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer = [0; 128];
        test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
        match &render_buckets.ascii_texts[0] {
            RenderAsciiTextMsg { text, row, column } => {
                assert_eq!(text, &[b'E', b'r', b'r']);
                assert_eq!(*row, canvas_y(0));
                assert_eq!(*column, result_panel_w(120) + RIGHT_GUTTER_WIDTH);
            }
        }
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
        assert_eq!(
            result_buffer[i], 0,
            "more results than expected at char {}.",
            i
        );
    }

    #[test]
    fn sum_can_be_nullified() {
        let test = create_app2(35);
        test.paste(
            "3m * 2m
--
1
2
sum",
        );
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["6 m^2", "", "1", "2", "3"][..], &result_buffer);
    }

    #[test]
    fn no_sum_value_in_case_of_error() {
        let test = create_app2(35);
        test.paste(
            "3m * 2m\n\
                    4\n\
                    sum",
        );
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["6 m^2", "4", "Err"][..], &result_buffer);
    }

    #[test]
    fn test_ctrl_c() {
        let test = create_app2(35);
        test.paste("aaaaaaaaa");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::shift());
        test.input(EditorInputEvent::Left, InputModifiers::shift());
        test.input(EditorInputEvent::Left, InputModifiers::shift());
        test.input(EditorInputEvent::Char('c'), InputModifiers::ctrl());
        assert_eq!("aaa", &test.app().editor.clipboard);
        assert_eq!(&None, &test.app().clipboard);
    }

    #[test]
    fn test_ctrl_c_without_selection() {
        let test = create_app2(35);
        test.paste("12*3");
        test.input(EditorInputEvent::Char('c'), InputModifiers::ctrl());
        assert_eq!(&Some("36".to_owned()), &test.app().clipboard);
        assert!(test.app().editor.clipboard.is_empty());
    }

    #[test]
    fn test_ctrl_c_without_selection2() {
        let test = create_app2(35);
        test.paste("12*3");
        test.input(EditorInputEvent::Char('c'), InputModifiers::ctrl());
        assert_eq!(
            Some("36".to_owned()),
            test.mut_app().get_selected_text_and_clear_app_clipboard()
        );
        assert_eq!(
            None,
            test.mut_app().get_selected_text_and_clear_app_clipboard()
        );
    }

    #[test]
    fn test_changing_output_style_for_selected_rows() {
        let test = create_app2(35);
        test.paste(
            "2\n\
                        4\n\
                        5",
        );
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::shift());
        test.input(EditorInputEvent::Up, InputModifiers::shift());
        test.input(EditorInputEvent::Left, InputModifiers::alt());

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["10", "100", "101"][..], &result_buffer);
    }

    #[test]
    fn test_matrix_sum() {
        let test = create_app2(35);
        test.paste("[1,2,3]\nsum");
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        // both the first line and the 'sum' line renders a matrix, which leaves the result buffer empty
        assert_results(&["\u{0}"][..], &result_buffer);
    }

    #[test]
    fn test_line_ref_selection() {
        // left
        {
            let test = create_app2(35);
            test.paste("16892313\n14 * ");
            test.set_cursor_row_col(1, 5);
            test.render();
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            test.render();
            test.input(EditorInputEvent::Left, InputModifiers::shift());
            test.input(EditorInputEvent::Backspace, InputModifiers::none());
            assert_eq!("16892313\n14 * &[1", test.get_editor_content());
        }
        // right
        {
            let test = create_app2(35);
            test.paste("16892313\n14 * ");
            test.set_cursor_row_col(1, 5);
            test.render();
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();

            test.render();
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.input(EditorInputEvent::Right, InputModifiers::shift());
            test.input(EditorInputEvent::Del, InputModifiers::none());
            assert_eq!("16892313\n14 * [1]", test.get_editor_content());
        }
    }

    #[test]
    fn test_line_ref_selection_with_mouse() {
        let test = create_app2(35);
        test.paste("16892313\n3\n14 * ");
        test.set_cursor_row_col(2, 5);
        test.render();
        test.click(125, 0);

        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::shift());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        assert_eq!("16892313\n3\n14 * &[1", test.get_editor_content());
    }

    #[test]
    fn test_click_1() {
        let test = create_app2(35);
        test.paste("'1st row\n[1;2;3] some text\n'3rd row");
        test.render();
        // click after the vector in 2nd row
        let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
        test.click(left_gutter_width + 4, 2);
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        assert_eq!(
            "'1st row\n[1;2;3] Xsome text\n'3rd row",
            test.get_editor_content()
        );
    }

    #[test]
    fn test_click() {
        let test = create_app2(35);
        test.paste("'1st row\nsome text [1;2;3]\n'3rd row");
        test.render();
        // click after the vector in 2nd row
        let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
        test.click(left_gutter_width + 4, 2);
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        assert_eq!(
            test.get_editor_content(),
            "'1st row\nsomeX text [1;2;3]\n'3rd row"
        );
    }

    #[test]
    fn test_click_after_eof() {
        let test = create_app2(35);
        test.paste("'1st row\n[1;2;3] some text\n'3rd row");
        test.render();
        let left_gutter_width = 1;
        test.click(left_gutter_width + 40, 2);
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        assert_eq!(
            "'1st row\n[1;2;3] some textX\n'3rd row",
            test.get_editor_content()
        );
    }

    #[test]
    fn test_click_after_eof2() {
        let test = create_app2(35);
        test.paste("'1st row\n[1;2;3] some text\n'3rd row");
        test.render();
        let left_gutter_width = 1;
        test.click(left_gutter_width + 40, 40);
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        assert_eq!(
            "'1st row\n[1;2;3] some text\n'3rd rowX",
            test.get_editor_content()
        );
    }

    #[test]
    fn test_variable() {
        let test = create_app2(35);
        test.paste("apple = 12");
        test.render();
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.paste("apple + 2");
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["12", "14"][..], &result_buffer);
    }

    #[test]
    fn test_variable_must_be_defined() {
        let test = create_app2(35);
        test.paste("apple = 12");
        test.render();
        test.input(EditorInputEvent::Home, InputModifiers::none());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.paste("apple + 2");

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["2", "12"][..], &result_buffer);
    }

    #[test]
    fn test_variables_can_be_defined_afterwards_of_their_usage() {
        let test = create_app2(35);
        test.paste("apple * 2");
        test.set_cursor_row_col(0, 0);

        test.render();
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::none());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["", "2"][..], &result_buffer);
        // now define the variable 'apple'
        test.paste("apple = 3");

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["3", "6"][..], &result_buffer);
    }

    #[test]
    fn test_variables_can_be_defined_afterwards_of_their_usage2() {
        let test = create_app2(35);
        test.paste("apple asd * 2");
        test.set_cursor_row_col(0, 0);

        test.render();
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::none());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["", "2"][..], &result_buffer);
        // now define the variable 'apple'
        test.paste("apple asd = 3");

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["3", "6"][..], &result_buffer);
    }

    #[test]
    fn test_renaming_variable_declaration() {
        let test = create_app2(35);
        test.paste("apple = 2\napple * 3");
        test.set_cursor_row_col(0, 0);

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["2", "6"][..], &result_buffer);

        // rename apple to aapple
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["2", "3"][..], &result_buffer);
    }

    #[test]
    fn test_moving_line_does_not_change_its_lineref() {
        let test = create_app2(35);
        test.paste("1\n2\n3\n\n\n50year");
        // cursor is in 4th row
        test.set_cursor_row_col(3, 0);

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["1", "2", "3", "", "", "50 year"][..], &result_buffer);

        // insert linref of 1st line
        for _ in 0..3 {
            test.input(EditorInputEvent::Up, InputModifiers::alt());
        }
        test.alt_key_released();
        test.render();
        test.input(EditorInputEvent::Char('+'), InputModifiers::none());

        // insert linref of 2st line
        for _ in 0..2 {
            test.input(EditorInputEvent::Up, InputModifiers::alt());
        }
        test.alt_key_released();
        test.render();
        test.input(EditorInputEvent::Char('+'), InputModifiers::none());

        // insert linref of 3rd line
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        match &test.tokens()[content_y(3)] {
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
            test.input(EditorInputEvent::Up, InputModifiers::none());
        }

        test.input(EditorInputEvent::Enter, InputModifiers::none());

        assert_results(&["1", "", "2", "3", "6", "", "50 year"][..], &result_buffer);

        match &test.tokens()[content_y(4)] {
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

    mod test_line_dependency_and_pulsing_on_change {
        use super::super::*;
        use super::*;
        #[test]
        fn test_modifying_a_lineref_recalcs_its_dependants() {
            let test = create_app2(35);
            test.paste("2\n * 3");
            test.set_cursor_row_col(1, 0);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "3"][..], &result_buffer);

            // insert linref of 1st line
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "6"][..], &result_buffer);

            // now modify the first row
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Home, InputModifiers::none());
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());

            let render_commands =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            assert_eq!(
                render_commands[2],
                OutputMessage::PulsingRectangle {
                    x: default_result_gutter_x(120) + RIGHT_GUTTER_WIDTH,
                    y: canvas_y(0),
                    w: 2,
                    h: 1,
                    start_color: 0xFF88FF_AA,
                    end_color: CHANGE_RESULT_PULSE_END_COLOR,
                    animation_time: Duration::from_millis(1000),
                }
            );
            assert_eq!(
                render_commands[3],
                OutputMessage::PulsingRectangle {
                    x: default_result_gutter_x(120) + RIGHT_GUTTER_WIDTH,
                    y: canvas_y(1),
                    w: 2,
                    h: 1,
                    start_color: 0xFF88FF_AA,
                    end_color: CHANGE_RESULT_PULSE_END_COLOR,
                    animation_time: Duration::from_millis(1000),
                }
            );

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["12", "36"][..], &result_buffer);
        }

        #[test]
        fn test_that_dependant_line_refs_are_pulsed_on_change() {
            let test = create_app2(35);
            test.paste("2\n * 3");
            test.set_cursor_row_col(1, 0);
            test.render();

            // insert linref of 1st line
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            test.render();

            // now modify the first row
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Home, InputModifiers::none());
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());

            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            // SetColor(
            // RenderChar(
            // PulsingRectangle {
            // PulsingRectangle {
            // PulsingRectangle {
            assert_eq!(
                render_bucket[4],
                OutputMessage::PulsingRectangle {
                    x: LEFT_GUTTER_MIN_WIDTH,
                    y: canvas_y(1),
                    w: 2,
                    h: 1,
                    start_color: 0xFF88FF_AA,
                    end_color: CHANGE_RESULT_PULSE_END_COLOR,
                    animation_time: Duration::from_millis(2000),
                }
            );
        }

        #[test]
        fn test_that_all_dependant_line_refs_in_same_row_are_pulsed_only_once_on_change() {
            let test = create_app2(35);
            test.paste("2\n * 3");
            test.set_cursor_row_col(1, 0);
            test.render();

            // insert linref of 1st line
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            test.render();

            test.input(EditorInputEvent::Char(' '), InputModifiers::none());
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            test.render();

            // now modify the first row
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Home, InputModifiers::none());
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());

            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            assert_eq!(render_bucket.len(), 8);

            let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
            assert_eq!(
                render_bucket[4],
                OutputMessage::PulsingRectangle {
                    x: left_gutter_width,
                    y: canvas_y(1),
                    w: 2,
                    h: 1,
                    start_color: CHANGE_RESULT_PULSE_START_COLOR,
                    end_color: CHANGE_RESULT_PULSE_END_COLOR,
                    animation_time: Duration::from_millis(2000),
                }
            );
            assert_eq!(
                render_bucket[5],
                OutputMessage::PulsingRectangle {
                    x: left_gutter_width + 3,
                    y: canvas_y(1),
                    w: 2,
                    h: 1,
                    start_color: CHANGE_RESULT_PULSE_START_COLOR,
                    end_color: CHANGE_RESULT_PULSE_END_COLOR,
                    animation_time: Duration::from_millis(2000),
                }
            );
            // the last 2 command is for pulsing references for the active row
            assert_eq!(
                render_bucket[6],
                OutputMessage::PulsingRectangle {
                    x: left_gutter_width,
                    y: canvas_y(1),
                    w: 2,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
            assert_eq!(
                render_bucket[7],
                OutputMessage::PulsingRectangle {
                    x: left_gutter_width + 3,
                    y: canvas_y(1),
                    w: 2,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
        }

        #[test]
        fn test_that_all_dependant_line_refs_in_different_rows_are_pulsed_on_change() {
            let test = create_app2(35);
            test.paste("2\n * 3");
            test.set_cursor_row_col(1, 0);

            test.render();

            // insert linref of 1st line
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            test.render();

            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            test.render();

            // now modify the first row
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Home, InputModifiers::none());

            test.input(EditorInputEvent::Char('1'), InputModifiers::none());

            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            assert_eq!(render_bucket.len(), 9);

            let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
            assert_eq!(
                render_bucket[5],
                OutputMessage::PulsingRectangle {
                    x: left_gutter_width,
                    y: canvas_y(1),
                    w: 2,
                    h: 1,
                    start_color: CHANGE_RESULT_PULSE_START_COLOR,
                    end_color: CHANGE_RESULT_PULSE_END_COLOR,
                    animation_time: Duration::from_millis(2000),
                }
            );
            assert_eq!(
                render_bucket[6],
                OutputMessage::PulsingRectangle {
                    x: left_gutter_width,
                    y: canvas_y(2),
                    w: 2,
                    h: 1,
                    start_color: CHANGE_RESULT_PULSE_START_COLOR,
                    end_color: CHANGE_RESULT_PULSE_END_COLOR,
                    animation_time: Duration::from_millis(2000),
                }
            );
            // the last 2 command is for pulsing references for the active row
            assert_eq!(
                render_bucket[7],
                OutputMessage::PulsingRectangle {
                    x: left_gutter_width,
                    y: canvas_y(1),
                    w: 2,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
            assert_eq!(
                render_bucket[8],
                OutputMessage::PulsingRectangle {
                    x: left_gutter_width,
                    y: canvas_y(2),
                    w: 2,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
        }

        #[test]
        fn test_that_dependant_line_refs_are_pulsing_when_the_cursor_is_on_the_referenced_line() {
            let test = create_app2(35);
            test.paste("2\n * 3");
            test.set_cursor_row_col(1, 0);
            test.render();

            // insert linref of 1st line
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            test.render();

            // there should not be pulsing here yet
            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            for command in render_bucket {
                assert!(!matches!(command, OutputMessage::PulsingRectangle {..}));
            }

            // step into the first row
            test.input(EditorInputEvent::Up, InputModifiers::none());

            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            dbg!(&render_bucket);
            assert_eq!(render_bucket.len(), 3);
            // SetColor(
            // RenderChar(
            // PulsingRectangle {
            assert_eq!(
                render_bucket[2],
                OutputMessage::PulsingRectangle {
                    x: LEFT_GUTTER_MIN_WIDTH,
                    y: canvas_y(1),
                    w: 1,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
        }

        #[test]
        fn test_that_multiple_dependant_line_refs_are_pulsed_when_the_cursor_is_on_the_referenced_line(
        ) {
            let test = create_app2(35);
            test.paste("2\n * 3");
            test.set_cursor_row_col(1, 0);
            test.render();

            // insert linref of 1st line
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            test.render();
            test.input(EditorInputEvent::Char(' '), InputModifiers::alt());
            test.render();
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            test.render();

            // there should not be pulsing here yet
            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            for command in render_bucket {
                assert!(!matches!(command, OutputMessage::PulsingRectangle {..}));
            }

            // step into the first row
            test.input(EditorInputEvent::Up, InputModifiers::none());

            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            assert_eq!(render_bucket.len(), 4);
            // SetColor(
            // RenderChar(
            // PulsingRectangle {
            assert_eq!(
                render_bucket[2],
                OutputMessage::PulsingRectangle {
                    x: LEFT_GUTTER_MIN_WIDTH,
                    y: canvas_y(1),
                    w: 1,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
            assert_eq!(
                render_bucket[3],
                OutputMessage::PulsingRectangle {
                    x: LEFT_GUTTER_MIN_WIDTH + 1,
                    y: canvas_y(1),
                    w: 1,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
        }

        #[test]
        fn test_that_multiple_dependant_vars_are_pulsed_when_the_cursor_is_on_the_definition_line()
        {
            let test = create_app2(35);
            test.paste("var = 2\nvar * 3\n12 * var");
            test.set_cursor_row_col(1, 0);
            test.render();

            // there should not be pulsing here yet
            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            for command in render_bucket {
                assert!(!matches!(command, OutputMessage::PulsingRectangle {..}));
            }

            // step into the first row
            test.input(EditorInputEvent::Up, InputModifiers::none());

            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            assert_eq!(render_bucket.len(), 4);
            // SetColor(
            // RenderChar(
            assert_eq!(
                render_bucket[2],
                OutputMessage::PulsingRectangle {
                    x: LEFT_GUTTER_MIN_WIDTH,
                    y: canvas_y(1),
                    w: 3,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
            assert_eq!(
                render_bucket[3],
                OutputMessage::PulsingRectangle {
                    x: LEFT_GUTTER_MIN_WIDTH + 5,
                    y: canvas_y(2),
                    w: 3,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
        }

        #[test]
        fn test_that_dependant_vars_are_pulsed_when_the_cursor_is_on_the_definition_line() {
            let test = create_app2(35);
            test.paste("var = 2\nvar * 3");
            test.set_cursor_row_col(1, 0);
            test.render();

            // there should not be pulsing here yet
            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            for command in render_bucket {
                assert!(!matches!(command, OutputMessage::PulsingRectangle {..}));
            }

            // step into the first row
            test.input(EditorInputEvent::Up, InputModifiers::none());

            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            assert_eq!(render_bucket.len(), 3);
            // SetColor(
            // RenderChar(
            assert_eq!(
                render_bucket[2],
                OutputMessage::PulsingRectangle {
                    x: LEFT_GUTTER_MIN_WIDTH,
                    y: canvas_y(1),
                    w: 3,
                    h: 1,
                    start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                    end_color: 0x00FF7F_00,
                    animation_time: Duration::from_millis(1000),
                }
            );
        }
    }

    #[test]
    fn test_modifying_a_lineref_does_not_change_the_line_id() {
        let test = create_app2(35);
        test.paste("2\n3\n");
        test.set_cursor_row_col(2, 0);
        test.render();
        // insert linref of 1st line
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        test.render();

        test.input(EditorInputEvent::Char(' '), InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Char('*'), InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Char(' '), InputModifiers::none());
        test.render();

        // insert linref of 2st line
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["2", "3", "6"][..], &result_buffer);

        // now modify the 2nd row
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Home, InputModifiers::none());
        test.input(EditorInputEvent::Del, InputModifiers::none());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["2", "Err", ""][..], &result_buffer);

        test.input(EditorInputEvent::Char('4'), InputModifiers::none());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["2", "4", "8"][..], &result_buffer);
    }

    mod dependent_lines_recalculation_tests {
        use super::super::*;
        use super::*;

        #[test]
        fn test_modifying_a_lineref_recalcs_its_dependants_only_if_its_value_has_changed() {
            let test = create_app2(35);
            test.paste("2\n * 3");
            test.set_cursor_row_col(1, 0);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "3"][..], &result_buffer);

            // insert linref of 1st line
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "6"][..], &result_buffer);

            // now modify the first row
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::End, InputModifiers::none());
            test.render();
            // inserting a '.' does not modify the result of the line
            test.input(EditorInputEvent::Char('.'), InputModifiers::none());

            let render_commands =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            // expect no pulsing since there were no value change
            test.contains(
                render_commands,
                0,
                OutputMessage::PulsingRectangle {
                    x: 90,
                    y: canvas_y(0),
                    w: 2,
                    h: 1,
                    start_color: 0xFF88FF_AA,
                    end_color: CHANGE_RESULT_PULSE_END_COLOR,
                    animation_time: Duration::from_millis(2000),
                },
            );
            test.contains(
                render_commands,
                0,
                OutputMessage::PulsingRectangle {
                    x: 90,
                    y: canvas_y(1),
                    w: 2,
                    h: 1,
                    start_color: 0xFF88FF_AA,
                    end_color: CHANGE_RESULT_PULSE_END_COLOR,
                    animation_time: Duration::from_millis(2000),
                },
            );

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "6"][..], &result_buffer);
        }

        #[test]
        fn test_renaming_variable_declaration2() {
            let test = create_app2(35);
            test.paste("apple = 2\naapple * 3");
            test.set_cursor_row_col(0, 0);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "3"][..], &result_buffer);

            // rename apple to aapple
            test.input(EditorInputEvent::Char('a'), InputModifiers::none());

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "6"][..], &result_buffer);
        }

        #[test]
        fn test_removing_variable_declaration() {
            let test = create_app2(35);
            test.paste("apple = 2\napple * 3");
            test.set_cursor_row_col(0, 0);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "6"][..], &result_buffer);

            // remove the content of the first line
            test.input(EditorInputEvent::End, InputModifiers::shift());

            test.input(EditorInputEvent::Del, InputModifiers::none());

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["", "3"][..], &result_buffer);
        }

        #[test]
        fn test_that_variable_dependent_rows_are_recalculated() {
            let test = create_app2(35);
            test.paste("apple = 2\napple * 3");
            test.set_cursor_row_col(0, 9);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "6"][..], &result_buffer);

            // change value of 'apple' from 2 to 24
            test.input(EditorInputEvent::Char('4'), InputModifiers::none());

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["24", "72"][..], &result_buffer);
        }

        #[test]
        fn test_that_sum_is_recalculated_if_anything_changes_above() {
            let test = create_app2(35);
            test.paste("2\n3\nsum");
            test.set_cursor_row_col(0, 1);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "3", "5"][..], &result_buffer);

            // change value from 2 to 21
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["21", "3", "24"][..], &result_buffer);
        }

        #[test]
        fn test_that_sum_is_recalculated_if_anything_changes_above2() {
            let test = create_app2(35);
            test.paste("2\n3\n4 * sum");
            test.set_cursor_row_col(0, 1);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "3", "20"][..], &result_buffer);

            // change value from 2 to 21
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["21", "3", "96"][..], &result_buffer);
        }

        #[test]
        fn test_that_sum_is_not_recalculated_if_there_is_separator() {
            let test = create_app2(35);
            test.paste("2\n3\n--\n5\nsum");
            test.set_cursor_row_col(0, 1);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "3", "", "5", "5"][..], &result_buffer);

            // change value from 2 to 12
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["21", "3", "", "5", "5"][..], &result_buffer);
        }

        #[test]
        fn test_that_sum_is_not_recalculated_if_there_is_separator_with_comment() {
            let test = create_app2(35);
            test.paste("2\n3\n-- some comment\n5\nsum");
            test.set_cursor_row_col(0, 1);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "3", "", "5", "5"][..], &result_buffer);

            // change value from 2 to 12
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["21", "3", "", "5", "5"][..], &result_buffer);
        }

        #[test]
        fn test_adding_sum_updates_lower_sums() {
            let test = create_app2(35);
            test.paste("2\n3\n\n4\n5\nsum\n-- some comment\n24\n25\nsum");
            test.set_cursor_row_col(2, 0);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(
                &["2", "3", "", "4", "5", "14", "", "24", "25", "49"][..],
                &result_buffer,
            );

            test.paste("sum");

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(
                &["2", "3", "5", "4", "5", "19", "", "24", "25", "49"][..],
                &result_buffer,
            );
        }

        #[test]
        fn test_updating_two_sums() {
            let test = create_app2(35);
            test.paste("2\n3\nsum\n4\n5\nsum\n-- some comment\n24\n25\nsum");
            test.set_cursor_row_col(0, 1);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(
                &["2", "3", "5", "4", "5", "19", "", "24", "25", "49"][..],
                &result_buffer,
            );

            // change value from 2 to 21
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(
                &["21", "3", "24", "4", "5", "57", "", "24", "25", "49"][..],
                &result_buffer,
            );
        }
    }

    #[test]
    fn test_that_result_is_not_changing_if_tokens_change_before_it() {
        let test = create_app2(35);
        test.paste("111");

        test.input(EditorInputEvent::Home, InputModifiers::none());
        test.input(EditorInputEvent::Char(' '), InputModifiers::none());

        let render_commands = &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
        // expect no pulsing since there were no value change
        for command in render_commands {
            match command {
                OutputMessage::PulsingRectangle { .. } => assert!(false, "{:?}", render_commands),
                _ => {}
            }
        }
    }

    #[test]
    fn test_variable_redefine() {
        let test = create_app2(35);
        test.paste("apple = 12");
        test.render();
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.paste("apple + 2");
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.paste("apple = 0");
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.paste("apple + 3");
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["12", "14", "0", "3"][..], &result_buffer);
    }

    #[test]
    fn test_backspace_bug_editor_obj_deletion_for_simple_tokens() {
        let test = create_app2(35);
        test.paste("asd sad asd asd sX");
        test.render();
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        assert_eq!("asd sad asd asd s", test.get_editor_content());
    }

    #[test]
    fn test_rendering_while_cursor_move() {
        let test = create_app2(35);
        test.paste("apple = 12$\nasd q");
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.render();
    }

    #[test]
    fn stepping_into_a_matrix_renders_it_some_lines_below() {
        let test = create_app2(35);
        test.paste("asdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 2);
        test.render();

        test.input(EditorInputEvent::Down, InputModifiers::none());

        {
            let editor_objects = test.editor_objects();
            assert_eq!(editor_objects[content_y(0)].len(), 1);
            assert_eq!(editor_objects[content_y(1)].len(), 1);

            assert_eq!(test.app().render_data.get_rendered_height(content_y(0)), 1);
            assert_eq!(test.app().render_data.get_rendered_height(content_y(1)), 6);
            assert_eq!(
                test.get_render_data().get_render_y(content_y(0)),
                Some(canvas_y(0))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(1)),
                Some(canvas_y(1))
            );
        }

        test.render();

        let editor_objects = test.editor_objects();
        assert_eq!(editor_objects[content_y(0)].len(), 1);
        assert_eq!(editor_objects[content_y(1)].len(), 1);
        assert_eq!(test.app().render_data.get_rendered_height(content_y(0)), 1);
        assert_eq!(test.app().render_data.get_rendered_height(content_y(1)), 6);
        assert_eq!(
            test.get_render_data().get_render_y(content_y(0)),
            Some(canvas_y(0))
        );
        assert_eq!(
            test.get_render_data().get_render_y(content_y(1)),
            Some(canvas_y(1))
        );
    }

    #[test]
    fn select_only_2_lines_render_bug() {
        let test = create_app2(35);
        test.paste("1\n2\n3");
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::shift());

        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer = [0; 128];

        test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
        let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
        let expected_x = left_gutter_width + 4;
        let commands = &render_buckets.custom_commands[Layer::AboveText as usize];
        dbg!(commands);
        test.contains(commands, 1, OutputMessage::RenderChar(expected_x, 1, '⎫'));
        test.contains(commands, 1, OutputMessage::RenderChar(expected_x, 2, '⎭'));
        test.contains(
            commands,
            1,
            OutputMessage::RenderString(RenderStringMsg {
                text: " ∑ = 5".to_owned(),
                row: canvas_y(1),
                column: expected_x,
            }),
        );
    }

    #[test]
    fn scroll_dragging_limit() {
        let test = create_app2(35);
        test.repeated_paste("1\n", 39);
        test.render();
        test.click(test.get_render_data().result_gutter_x - SCROLL_BAR_WIDTH, 0);

        assert_eq!(test.get_render_data().scroll_y, 0);

        for i in 0..5 {
            test.handle_drag(
                test.get_render_data().result_gutter_x - SCROLL_BAR_WIDTH,
                1 + i,
            );
            assert_eq!(test.get_render_data().scroll_y, 1 + i as usize);
        }
        test.handle_drag(test.get_render_data().result_gutter_x - SCROLL_BAR_WIDTH, 6);
        // the scrollbar reached its bottom position, it won't go further down
        assert_eq!(test.get_render_data().scroll_y, 5);
    }

    #[test]
    fn scroll_dragging_upwards() {
        let test = create_app2(35);
        test.repeated_paste("1\n", 39);

        test.render();
        test.click(test.get_render_data().result_gutter_x - SCROLL_BAR_WIDTH, 0);

        assert_eq!(test.get_render_data().scroll_y, 0);

        for i in 0..5 {
            test.handle_drag(
                test.get_render_data().result_gutter_x - SCROLL_BAR_WIDTH,
                1 + i,
            );
            assert_eq!(test.get_render_data().scroll_y, 1 + i as usize);
        }
        for i in 0..5 {
            test.handle_drag(
                test.get_render_data().result_gutter_x - SCROLL_BAR_WIDTH,
                4 - i,
            );
            assert_eq!(test.get_render_data().scroll_y, 4 - i as usize);
        }
    }

    #[test]
    fn test_scrolling_by_single_click_in_scrollbar() {
        let test = create_app2(30);
        test.repeated_paste("1\n", 60);
        test.render();
        assert_eq!(test.get_render_data().scroll_y, 0);

        for i in 0..4 {
            let mouse_x = test.get_render_data().result_gutter_x - SCROLL_BAR_WIDTH;
            test.click(mouse_x, 20 + i);
            assert_eq!(test.get_render_data().scroll_y, i as usize);
            test.handle_mouse_up();
            assert_eq!(test.get_render_data().scroll_y, 1 + i as usize);
        }
        for i in 0..3 {
            let mouse_x = test.get_render_data().result_gutter_x - SCROLL_BAR_WIDTH;
            test.click(mouse_x, 0);
            assert_eq!(test.get_render_data().scroll_y, 4 - i as usize);
            test.handle_mouse_up();
            assert_eq!(test.get_render_data().scroll_y, 3 - i as usize);
        }
    }

    #[test]
    fn clicking_behind_matrix_should_move_the_cursor_there() {
        let test = create_app2(35);

        test.paste("firs 1t\nasdsad\n[1;2;3;4]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();
        assert_eq!(test.get_cursor_pos().row, 0);
        let left_gutter_width = 1;
        test.click(left_gutter_width + 50, 13);
        assert_eq!(test.get_cursor_pos().row, 5);
    }

    #[test]
    fn clicking_inside_matrix_while_selected_should_put_cursor_after_matrix() {
        let test = create_app2(35);
        test.paste("firs 1t\nasdsad\n[1;2;3;4]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();
        // select all
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.render();

        // click inside the real matrix repr
        // the problem is that this click is inside a SimpleToken (as the matrix is rendered as
        // SimpleToken if it is selected), so the cursor is set accordingly,
        // but as soon as the selection is cancelled by the click, we render a matrix,
        // and the cursor is inside the matrix, which is not OK.
        let left_gutter_width = 1;
        test.click(left_gutter_width + 7, 2);

        // typing should append after the matrix
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        assert_eq!(
            "firs 1t\nasdsad\n[1;2;3;4]X\nfirs 1t\nasdsad\n[1;2;3;4]",
            test.get_editor_content()
        );
    }

    #[test]
    fn limiting_cursor_does_not_kill_selection() {
        let test = create_app2(35);

        test.repeated_paste("1\n", 65);
        test.set_cursor_row_col(0, 0);
        test.render();
        test.input(EditorInputEvent::PageDown, InputModifiers::shift());
        test.render();
        assert_eq!(
            test.get_selection().is_range(),
            Some((
                Pos::from_row_column(0, 0),
                Pos::from_row_column(MAX_LINE_COUNT - 1, 0)
            ))
        );
    }

    #[test]
    fn deleting_all_selected_lines_no_panic() {
        let test = create_app2(35);
        test.repeated_paste("1\n", 80);
        test.set_cursor_row_col(0, 0);
        test.render();
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.render();
        test.input(EditorInputEvent::Del, InputModifiers::ctrl());
    }

    #[test]
    fn click_into_a_row_with_matrix_put_the_cursor_after_the_rendered_matrix() {
        let test = create_app2(35);
        test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();
        assert_eq!(test.get_cursor_pos().column, 0);

        let left_gutter_width = 1;
        for i in 0..5 {
            test.click(left_gutter_width + 13 + i, 5);
            assert_eq!(test.get_cursor_pos().column, 25);
        }
    }

    #[test]
    fn clicking_into_matrices_panic() {
        let test = create_app2(35);
        test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();
        // click into 1st matrix to edit it
        let left_gutter_width = 1;
        test.click(left_gutter_width + 1, 5);
        test.render();
        // write 333 into the first cell
        for _ in 0..3 {
            test.input(EditorInputEvent::Char('3'), InputModifiers::none());
        }
        test.render();
        // click into 2nd matrix
        test.click(left_gutter_width + 1, 15);
        test.render();
        // click back into 1nd matrix
        test.click(left_gutter_width + 1, 5);
        test.render();
    }

    #[test]
    fn leaving_matrix_by_clicking_should_trigger_reevaluation() {
        let test = create_app2(35);
        test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();
        // click into 1st matrix to edit it
        let left_gutter_width = 1;
        test.click(left_gutter_width + 1, 5);
        test.render();
        // write 333 into the first cell
        for _ in 0..3 {
            test.input(EditorInputEvent::Char('3'), InputModifiers::none());
        }
        test.render();
        // click into 2nd matrix
        test.click(left_gutter_width + 1, 15);
        test.render();
        assert_eq!(test.editor_objects()[content_y(2)][0].rendered_w, 8);
    }

    #[test]
    fn click_into_a_matrix_start_mat_editing() {
        let test = create_app2(35);
        test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();
        let left_gutter_width = 1;
        test.click(left_gutter_width + 1, 5);
        assert!(test.app().matrix_editing.is_some());
    }

    #[test]
    fn mouse_selecting_moving_mouse_out_of_editor() {
        let test = create_app2(35);
        test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();
        let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
        test.click(left_gutter_width + 7, 0);
        test.handle_drag(0, 0);
        assert_eq!(
            test.get_selection().is_range(),
            Some((Pos::from_row_column(0, 0), Pos::from_row_column(0, 7)))
        );
    }

    #[test]
    fn test_dragging_right_gutter_panic() {
        let test = create_app2(35);
        test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();

        let orig_x = test.get_render_data().result_gutter_x;
        test.click(test.get_render_data().result_gutter_x, 0);

        for i in 1..=orig_x {
            test.handle_drag(orig_x - i, 0);
        }
    }

    #[test]
    fn test_small_right_gutter_panic() {
        let test = create_app3(20, 35);
        test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();

        let orig_x = test.get_render_data().result_gutter_x;
        test.click(test.get_render_data().result_gutter_x, 0);

        for i in 1..=orig_x {
            test.handle_drag(orig_x - i, 0);
        }
    }

    #[test]
    fn bug_selection_rectangle_is_longer_than_the_selected_row() {
        let test = create_app2(35);
        test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
        test.set_cursor_row_col(0, 0);
        test.render();
        let left_gutter_width = 2;
        test.click(left_gutter_width + 4, 0);
        test.render();

        test.handle_drag(left_gutter_width + 0, 1);

        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer = [0; 128];

        test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);

        let pos = render_buckets
            .custom_commands(Layer::BehindText)
            .iter()
            .position(|it| matches!(it, OutputMessage::SetColor(0xA6D2FF_FF)))
            .expect("there is no selection box drawing");
        assert_eq!(
            render_buckets.custom_commands(Layer::BehindText)[pos + 1],
            OutputMessage::RenderRectangle {
                x: left_gutter_width + 4,
                y: canvas_y(0),
                w: 3,
                h: 1
            }
        );
        assert_eq!(
            render_buckets.custom_commands(Layer::BehindText)[pos + 2],
            OutputMessage::RenderRectangle {
                x: left_gutter_width,
                y: canvas_y(1),
                w: 0,
                h: 1
            }
        );
    }

    #[test]
    fn test_handling_too_much_rows_no_panic() {
        let test = create_app2(35);
        test.paste(&("1\n".repeat(MAX_LINE_COUNT - 1).to_owned()));
        test.set_cursor_row_col(MAX_LINE_COUNT - 2, 1);

        test.render();
        test.input(EditorInputEvent::Enter, InputModifiers::none());
    }

    #[test]
    fn inserting_too_many_rows_no_panic() {
        let test = create_app2(35);
        test.paste("");
        test.set_cursor_row_col(0, 0);

        for _ in 0..20 {
            test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n0");
            test.render();
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.render();
        }
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::PageDown, InputModifiers::none());
    }

    #[test]
    fn stepping_down_to_unrendered_line_scrolls_down_the_screen() {
        let test = create_app2(35);
        test.repeated_paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n0", 6);
        test.set_cursor_row_col(0, 0);
        test.render();

        assert_eq!(test.get_render_data().scroll_y, 0);
        test.input(EditorInputEvent::PageDown, InputModifiers::none());
        assert_ne!(test.get_render_data().scroll_y, 0);
    }

    #[test]
    fn test_scrolling_by_keyboard() {
        let test = create_app2(35);
        test.paste(
            "0
1
2
[4;5;6;7]
9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
--
1
2
3
4
5
6
7
8
10",
        );
        test.set_cursor_row_col(34, 0);
        test.render();
        test.input(EditorInputEvent::PageUp, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 0);
        // in this setup (35 canvas height) only 30 line is visible, so the client
        // has to press DOWN 29 times
        let matrix_height = 6;
        for _ in 0..(35 - matrix_height) {
            test.input(EditorInputEvent::Down, InputModifiers::none());
        }
        assert_eq!(test.get_render_data().scroll_y, 0);
        for i in 0..3 {
            test.input(EditorInputEvent::Down, InputModifiers::none());
            test.render();
            assert_eq!(test.get_render_data().scroll_y, 1 + i);
            assert_eq!(
                test.app().render_data.get_render_y(content_y(30 + i)),
                Some(canvas_y(34)),
            );
        }
        // This step moves the matrix out of vision, so 6 line will appear instead of it at the bottom
        test.input(EditorInputEvent::Down, InputModifiers::none());
        test.render();
        assert_eq!(test.get_render_data().scroll_y, 4);
        assert_eq!(
            test.get_render_data().get_render_y(content_y(33)),
            Some(canvas_y(29)),
        );
    }

    #[test]
    fn test_sum_rerender() {
        // rust's borrow checker forces me to do this
        {
            let test = create_app2(35);
            test.paste("1\n2\n3\nsum");
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let test = create_app2(35);
            test.paste("1\n2\n3\nsum");
            test.input(EditorInputEvent::Up, InputModifiers::none());
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let test = create_app2(35);
            test.paste("1\n2\n3\nsum");
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Up, InputModifiers::none());
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let test = create_app2(35);
            test.paste("1\n2\n3\nsum");
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Down, InputModifiers::none());
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
        {
            let test = create_app2(35);
            test.paste("1\n2\n3\nsum");
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Down, InputModifiers::none());
            test.input(EditorInputEvent::Down, InputModifiers::none());
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "2", "3", "6"][..], &result_buffer);
        }
    }

    #[test]
    fn test_sum_rerender_with_ignored_lines() {
        {
            let test = create_app2(35);
            test.paste("1\n'2\n3\nsum");
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let test = create_app2(35);
            test.paste("1\n'2\n3\nsum");
            test.input(EditorInputEvent::Up, InputModifiers::none());
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let test = create_app2(35);
            test.paste("1\n'2\n3\nsum");
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Up, InputModifiers::none());
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let test = create_app2(35);
            test.paste("1\n'2\n3\nsum");
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Down, InputModifiers::none());
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
        {
            let test = create_app2(35);
            test.paste("1\n'2\n3\nsum");
            test.input(EditorInputEvent::Up, InputModifiers::none());
            test.input(EditorInputEvent::Down, InputModifiers::none());
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "3", "4"][..], &result_buffer);
        }
    }

    #[test]
    fn test_sum_rerender_with_sum_reset() {
        {
            let test = create_app2(35);
            test.paste("1\n--2\n3\nsum");
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "3", "3"][..], &result_buffer);
        }
        {
            let test = create_app2(35);
            test.paste("1\n--2\n3\nsum");
            test.input(EditorInputEvent::Up, InputModifiers::none());
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "3", "3"][..], &result_buffer);
        }
    }

    #[test]
    fn test_paste_long_text() {
        let test = create_app2(35);
        test.paste("a\nb\na\nb\na\nb\na\nb\na\nb\na\nb\n1");

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(
            &["", "", "", "", "", "", "", "", "", "", "", "1"][..],
            &result_buffer,
        );
    }

    #[test]
    fn test_thousand_separator_and_alignment_in_result() {
        let test = create_app2(35);
        test.paste("1\n2.3\n2222\n4km\n50000");
        test.set_cursor_row_col(2, 0);
        // set result to binary repr
        test.input(EditorInputEvent::Left, InputModifiers::alt());

        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer = [0; 128];

        test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);

        let base_x = render_buckets.ascii_texts[0].column;
        assert_eq!(render_buckets.ascii_texts[0].text, "1".as_bytes());
        assert_eq!(render_buckets.ascii_texts[0].row, canvas_y(0));

        assert_eq!(render_buckets.ascii_texts[1].text, "2".as_bytes());
        assert_eq!(render_buckets.ascii_texts[1].row, canvas_y(1));
        assert_eq!(render_buckets.ascii_texts[1].column, base_x);

        assert_eq!(render_buckets.ascii_texts[2].text, ".3".as_bytes());
        assert_eq!(render_buckets.ascii_texts[2].row, canvas_y(1));
        assert_eq!(render_buckets.ascii_texts[2].column, base_x + 1);

        assert_eq!(
            render_buckets.ascii_texts[3].text,
            "1000 10101110".as_bytes()
        );
        assert_eq!(render_buckets.ascii_texts[3].row, canvas_y(2));
        assert_eq!(render_buckets.ascii_texts[3].column, base_x - 12);

        assert_eq!(render_buckets.ascii_texts[4].text, "4".as_bytes());
        assert_eq!(render_buckets.ascii_texts[4].row, canvas_y(3));
        assert_eq!(render_buckets.ascii_texts[4].column, base_x);

        assert_eq!(render_buckets.ascii_texts[5].text, "km".as_bytes());
        assert_eq!(render_buckets.ascii_texts[5].row, canvas_y(3));
        assert_eq!(render_buckets.ascii_texts[5].column, base_x + 4);

        assert_eq!(render_buckets.ascii_texts[6].text, "50 000".as_bytes());
        assert_eq!(render_buckets.ascii_texts[6].row, canvas_y(4));
        assert_eq!(render_buckets.ascii_texts[6].column, base_x - 5);
    }

    #[test]
    fn test_units_are_aligned_as_well() {
        let test = create_app2(35);
        test.paste("1cm\n2.3m\n2222.33 km\n4km\n50000 mm");
        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer = [0; 128];

        test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);

        dbg!(&render_buckets.ascii_texts);
        let base_x = render_buckets.ascii_texts[1].column; // 1 cm

        assert_eq!(render_buckets.ascii_texts[1].text, "cm".as_bytes());
        assert_eq!(render_buckets.ascii_texts[1].row, canvas_y(0));
        assert_eq!(render_buckets.ascii_texts[1].column, base_x);

        assert_eq!(render_buckets.ascii_texts[4].text, "m".as_bytes());
        assert_eq!(render_buckets.ascii_texts[4].row, canvas_y(1));
        assert_eq!(render_buckets.ascii_texts[4].column, base_x + 1);

        assert_eq!(render_buckets.ascii_texts[7].text, "km".as_bytes());
        assert_eq!(render_buckets.ascii_texts[7].row, canvas_y(2));
        assert_eq!(render_buckets.ascii_texts[7].column, base_x);

        assert_eq!(render_buckets.ascii_texts[9].text, "km".as_bytes());
        assert_eq!(render_buckets.ascii_texts[9].row, canvas_y(3));
        assert_eq!(render_buckets.ascii_texts[9].column, base_x);

        assert_eq!(render_buckets.ascii_texts[11].text, "mm".as_bytes());
        assert_eq!(render_buckets.ascii_texts[11].row, canvas_y(4));
        assert_eq!(render_buckets.ascii_texts[11].column, base_x);
    }

    #[test]
    fn test_that_alignment_changes_trigger_rerendering_of_results() {
        let test = create_app2(35);
        test.paste("1\n");
        test.set_cursor_row_col(1, 0);

        test.render();
        test.paste("4km");

        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer = [0; 128];

        test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);

        let base_x = render_buckets.ascii_texts[0].column;
        assert_eq!(render_buckets.ascii_texts[0].text, "1".as_bytes());
        assert_eq!(render_buckets.ascii_texts[0].row, canvas_y(0));

        assert_eq!(render_buckets.ascii_texts[1].text, "4".as_bytes());
        assert_eq!(render_buckets.ascii_texts[1].row, canvas_y(1));
        assert_eq!(render_buckets.ascii_texts[1].column, base_x);

        assert_eq!(render_buckets.ascii_texts[2].text, "km".as_bytes());
        assert_eq!(render_buckets.ascii_texts[2].row, canvas_y(1));
        assert_eq!(render_buckets.ascii_texts[2].column, base_x + 2);
    }

    #[test]
    fn test_ctrl_x() {
        let test = create_app2(35);
        test.paste("0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12");
        test.render();

        test.input(EditorInputEvent::Up, InputModifiers::shift());
        test.input(EditorInputEvent::Up, InputModifiers::shift());
        test.input(EditorInputEvent::Up, InputModifiers::shift());
        test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(
            &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"][..],
            &result_buffer,
        );
    }

    #[test]
    fn test_ctrl_x_then_ctrl_z() {
        let test = create_app2(35);
        test.paste("12");
        test.handle_time(1000);
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["12"][..], &result_buffer);

        test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&[""][..], &result_buffer);

        test.input(EditorInputEvent::Char('z'), InputModifiers::ctrl());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["12"][..], &result_buffer);
    }

    #[test]
    fn selection_in_the_first_row_should_not_panic() {
        let test = create_app2(35);
        test.paste("1+1\nasd");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Home, InputModifiers::shift());

        test.render();
    }

    #[test]
    fn test_that_removed_tail_rows_are_cleared() {
        let test = create_app2(35);
        test.paste("a\nb\n[1;2;3]\nX\na\n1");
        test.set_cursor_row_col(3, 0);

        test.render();
        assert_ne!(
            test.get_render_data().get_render_y(content_y(5)),
            Some(canvas_y(0))
        );

        // removing a line
        test.input(EditorInputEvent::Backspace, InputModifiers::none());

        // they must not be 0, otherwise the renderer can't decide if they needed to be cleared,
        assert_ne!(
            test.get_render_data().get_render_y(content_y(5)),
            Some(canvas_y(0))
        );

        test.render();

        assert_eq!(test.get_render_data().get_render_y(content_y(5)), None);
    }

    #[test]
    fn test_that_pressing_enter_eof_moves_scrollbar_down() {
        let test = create_app2(35);
        // editor height is 36 in tests, so create a 35 line text
        test.repeated_paste("a\n", 35);
        test.set_cursor_row_col(3, 0);

        test.render();
        assert_ne!(
            test.get_render_data().get_render_y(content_y(5)),
            Some(canvas_y(0))
        );

        // removing a line
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
    }

    #[test]
    fn test_that_multiline_matrix_is_considered_when_scrolling() {
        let test = create_app2(35);
        // editor height is 36 in tests, so create a 35 line text
        test.repeated_paste("a\n", 40);
        test.set_cursor_row_col(3, 0);

        test.render();
        assert_eq!(
            test.get_render_data().get_render_y(content_y(34)),
            Some(canvas_y(34))
        );
        assert_eq!(
            test.get_render_data().get_render_y(content_y(35)),
            Some(canvas_y(35))
        );
        assert_eq!(
            test.get_render_data().get_render_y(content_y(39)),
            Some(canvas_y(39))
        );
        assert!(test.get_render_data().is_visible(content_y(30)));
        assert!(test.get_render_data().is_visible(content_y(31)));
        assert!(!test.get_render_data().is_visible(content_y(39)));

        test.paste("[1;2;3;4]");
        test.render();
        assert_eq!(
            test.get_render_data().get_render_y(content_y(29)),
            Some(canvas_y(34))
        );
        assert_eq!(
            test.get_render_data().get_render_y(content_y(30)),
            Some(canvas_y(35))
        );
        assert_eq!(
            test.get_render_data().get_render_y(content_y(31)),
            Some(canvas_y(36))
        );
        assert_eq!(
            test.get_render_data().get_render_y(content_y(39)),
            Some(canvas_y(44))
        );
        assert!(!test.get_render_data().is_visible(content_y(30)));
        assert!(!test.get_render_data().is_visible(content_y(31)));
        assert!(!test.get_render_data().is_visible(content_y(39)));

        assert_eq!(
            test.get_render_data().get_render_y(content_y(1)),
            Some(canvas_y(1))
        );
        assert_eq!(test.get_render_data().scroll_y, 0);

        // move to the last visible line
        test.set_cursor_row_col(29, 0);
        // Since the matrix takes up 6 lines, a scroll should occur when pressing down
        test.input(EditorInputEvent::Down, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 1);

        test.render();
        assert_eq!(
            test.get_render_data().get_render_y(content_y(1)),
            Some(canvas_y(0))
        );
    }

    #[test]
    fn navigating_to_bottom_no_panic() {
        let test = create_app2(35);
        test.repeated_paste("aaaaaaaaaaaa\n", 34);

        test.render();

        test.input(EditorInputEvent::PageDown, InputModifiers::none());
    }

    #[test]
    fn ctrl_a_plus_typing() {
        let test = create_app2(25);
        test.repeated_paste("1\n", 34);
        test.set_cursor_row_col(0, 0);

        test.render();

        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());

        test.render();

        test.input(EditorInputEvent::Char('1'), InputModifiers::none());

        test.render();
    }

    #[test]
    fn test_that_scrollbar_stops_at_bottom() {
        let client_height = 25;
        let test = create_app2(client_height);
        test.repeated_paste("1\n", client_height * 2);
        test.set_cursor_row_col(0, 0);

        test.render();

        test.input(EditorInputEvent::PageDown, InputModifiers::none());

        assert_eq!(test.get_render_data().scroll_y, 26);
    }

    #[test]
    fn test_that_no_full_refresh_when_stepping_into_last_line() {
        let client_height = 25;
        let test = create_app2(client_height);
        test.repeated_paste("1\n", client_height * 2);
        test.set_cursor_row_col(0, 0);

        test.render();

        // step into last-1 line
        for _i in 0..(client_height - 2) {
            test.input(EditorInputEvent::Down, InputModifiers::none());
        }
        // rerender so flags are cleared
        test.render();

        // step into last visible line
        test.input(EditorInputEvent::Down, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 0);

        // this step scrolls down one
        // step into last line
        test.input(EditorInputEvent::Down, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 1);
    }

    #[test]
    fn test_that_removed_lines_are_cleared() {
        let client_height = 25;
        let test = create_app2(client_height);
        test.repeated_paste("1\n", client_height * 2);
        test.set_cursor_row_col(0, 0);

        test.render();

        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());

        test.render();

        test.input(EditorInputEvent::Char('1'), InputModifiers::none());

        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer = [0; 128];

        test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);

        assert_eq!(
            None,
            test.app()
                .render_data
                .get_render_y(content_y(client_height * 2 - 1))
        );
    }

    #[test]
    fn test_that_no_overscrolling() {
        let test = create_app2(35);
        test.paste("1\n2\n3\n");
        test.render();

        test.handle_wheel(1);
        assert_eq!(0, test.get_render_data().scroll_y);
    }

    #[test]
    fn test_that_unvisible_rows_have_height_1() {
        let test = create_app2(25);
        test.repeated_paste("1\n2\n\n[1;2;3;4]", 10);

        test.render();

        for _ in 0..3 {
            test.handle_wheel(1);
        }
        test.handle_wheel(1);
        test.render();
        assert_eq!(
            test.get_render_data().get_render_y(content_y(3)),
            Some(canvas_y(-1))
        );
        assert_eq!(test.app().render_data.get_rendered_height(content_y(3)), 6);
        assert_eq!(
            test.get_render_data().get_render_y(content_y(4)),
            Some(canvas_y(0))
        );
        assert_eq!(test.app().render_data.get_rendered_height(content_y(4)), 1);
    }

    #[test]
    fn test_that_unvisible_rows_contribute_with_only_1_height_to_calc_content_height() {
        let test = create_app2(25);
        test.repeated_paste("1\n2\n\n[1;2;3;4]", 10);

        test.render();

        for _ in 0..4 {
            test.handle_wheel(1);
        }
        test.render();
        assert_eq!(
            46,
            NoteCalcApp::calc_full_content_height(
                &test.get_render_data(),
                test.app().editor_content.line_count()
            )
        );
    }

    #[test]
    fn test_stepping_into_scrolled_matrix_panic() {
        let test = create_app2(25);
        test.repeated_paste("1\n2\n\n[1;2;3;4]", 10);

        test.render();

        test.set_cursor_row_col(0, 0);

        for _ in 0..2 {
            test.input(EditorInputEvent::Down, InputModifiers::none());
            test.render();
        }
        test.handle_wheel(1);
        test.handle_wheel(1);
        test.render();

        test.input(EditorInputEvent::Down, InputModifiers::none());
        test.render();
    }

    #[test]
    fn tall_rows_are_considered_in_scrollbar_height_calc() {
        const CANVAS_HEIGHT: usize = 25;
        let test = create_app2(CANVAS_HEIGHT);
        test.repeated_paste("1\n2\n\n[1;2;3;4]", 5);

        let mut render_buckets = RenderBuckets::new();
        let mut result_buffer = [0; 128];
        test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);

        test.contains(
            &render_buckets.custom_commands[Layer::BehindText as usize],
            1,
            OutputMessage::RenderRectangle {
                x: result_panel_w(120) - SCROLL_BAR_WIDTH,
                y: canvas_y(0),
                w: 1,
                h: 19,
            },
        );
    }

    #[test]
    fn test_no_scrolling_in_empty_document() {
        let test = create_app2(25);
        test.paste("1");

        test.render();

        test.handle_wheel(1);

        test.render();

        assert_eq!(0, test.get_render_data().scroll_y);
    }

    #[test]
    fn test_that_no_overscrolling2() {
        let test = create_app2(35);
        test.repeated_paste("aaaaaaaaaaaa\n", 35);
        test.render();

        test.handle_wheel(1);
        assert_eq!(1, test.get_render_data().scroll_y);
        test.handle_wheel(1);
        assert_eq!(1, test.get_render_data().scroll_y);
    }

    #[test]
    fn test_that_scrolled_result_is_not_rendered() {
        {
            let test = create_app2(35);
            test.paste("1\n2\n3\n");
            test.repeated_paste("aaaaaaaaaaaa\n", 34);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "2", "3"][..], &result_buffer);
            assert_eq!(
                test.get_render_data().get_render_y(content_y(0)),
                Some(canvas_y(0))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(1)),
                Some(canvas_y(1))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(2)),
                Some(canvas_y(2))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(35)),
                Some(canvas_y(35))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(36)),
                Some(canvas_y(36))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(37)),
                Some(canvas_y(37))
            );
            assert_eq!(test.get_render_data().get_render_y(content_y(38)), None,);
            assert_eq!(test.get_render_data().is_visible(content_y(35)), false);
            assert_eq!(test.get_render_data().is_visible(content_y(36)), false);
            assert_eq!(test.get_render_data().is_visible(content_y(37)), false);
            assert_eq!(test.get_render_data().is_visible(content_y(38)), false);
        }

        {
            let test = create_app2(35);
            test.paste("1\n2\n3\n");
            test.repeated_paste("aaaaaaaaaaaa\n", 34);
            test.render();

            test.handle_wheel(1);
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_eq!(
                test.get_render_data().get_render_y(content_y(0)),
                Some(canvas_y(-1))
            );
            assert!(!test.get_render_data().is_visible(content_y(0)));
            assert_eq!(
                test.get_render_data().get_render_y(content_y(1)),
                Some(canvas_y(0))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(2)),
                Some(canvas_y(1))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(35)),
                Some(canvas_y(34))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(36)),
                Some(canvas_y(35))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(37)),
                Some(canvas_y(36))
            );
            assert_eq!(test.get_render_data().get_render_y(content_y(38)), None);
            assert_results(&["2", "3"][..], &result_buffer);
        }

        {
            let test = create_app2(35);
            test.paste("1\n2\n3\n");
            test.repeated_paste("aaaaaaaaaaaa\n", 34);
            test.render();

            test.handle_wheel(1);
            test.handle_wheel(1);
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["3"][..], &result_buffer);
            assert_eq!(
                test.get_render_data().get_render_y(content_y(0)),
                Some(canvas_y(-2))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(1)),
                Some(canvas_y(-1))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(2)),
                Some(canvas_y(0))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(35)),
                Some(canvas_y(33))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(36)),
                Some(canvas_y(34))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(37)),
                Some(canvas_y(35))
            );
            assert_eq!(test.get_render_data().get_render_y(content_y(38)), None);
        }

        {
            let test = create_app2(35);
            test.paste("1\n2\n3\n");
            test.repeated_paste("aaaaaaaaaaaa\n", 34);
            test.render();

            test.handle_wheel(1);
            test.handle_wheel(1);
            test.handle_wheel(1);
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&[""][..], &result_buffer);
            assert_eq!(
                test.get_render_data().get_render_y(content_y(0)),
                Some(canvas_y(-3))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(1)),
                Some(canvas_y(-2))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(2)),
                Some(canvas_y(-1))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(35)),
                Some(canvas_y(32))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(36)),
                Some(canvas_y(33))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(37)),
                Some(canvas_y(34))
            );
            assert_eq!(test.get_render_data().get_render_y(content_y(38)), None);
        }

        {
            let test = create_app2(35);
            test.paste("1\n2\n3\n");
            test.repeated_paste("aaaaaaaaaaaa\n", 34);
            test.render();

            test.handle_wheel(1);
            test.handle_wheel(1);
            test.handle_wheel(1);
            test.handle_wheel(0);
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["3"][..], &result_buffer);
            assert_eq!(
                test.get_render_data().get_render_y(content_y(0)),
                Some(canvas_y(-2))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(1)),
                Some(canvas_y(-1))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(2)),
                Some(canvas_y(0))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(35)),
                Some(canvas_y(33))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(36)),
                Some(canvas_y(34))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(37)),
                Some(canvas_y(35))
            );
            assert_eq!(test.get_render_data().get_render_y(content_y(38)), None);
        }

        {
            let test = create_app2(35);
            test.paste("1\n2\n3\n");
            test.repeated_paste("aaaaaaaaaaaa\n", 34);
            test.render();

            test.handle_wheel(1);
            test.handle_wheel(1);
            test.handle_wheel(1);
            test.handle_wheel(0);
            test.handle_wheel(0);

            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["2", "3"][..], &result_buffer);
            assert_eq!(
                test.get_render_data().get_render_y(content_y(0)),
                Some(canvas_y(-1))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(1)),
                Some(canvas_y(0))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(2)),
                Some(canvas_y(1))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(35)),
                Some(canvas_y(34))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(36)),
                Some(canvas_y(35))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(37)),
                Some(canvas_y(36))
            );
            assert_eq!(test.get_render_data().get_render_y(content_y(38)), None);
        }

        {
            let test = create_app2(35);
            test.paste("1\n2\n3\n");
            test.repeated_paste("aaaaaaaaaaaa\n", 34);
            test.render();

            test.handle_wheel(1);
            test.handle_wheel(1);
            test.handle_wheel(1);
            test.handle_wheel(0);
            test.handle_wheel(0);
            test.handle_wheel(0);
            let mut result_buffer = [0; 128];
            test.render_get_result_buf(&mut result_buffer[..]);
            assert_results(&["1", "2", "3"][..], &result_buffer);
            assert_eq!(
                test.get_render_data().get_render_y(content_y(0)),
                Some(canvas_y(0))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(1)),
                Some(canvas_y(1))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(2)),
                Some(canvas_y(2))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(35)),
                Some(canvas_y(35))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(36)),
                Some(canvas_y(36))
            );
            assert_eq!(
                test.get_render_data().get_render_y(content_y(37)),
                Some(canvas_y(37))
            );
            assert_eq!(test.get_render_data().get_render_y(content_y(38)), None);
        }
    }

    #[test]
    fn test_ctrl_b_jumps_to_var_def() {
        for i in 0..=3 {
            let test = create_app2(35);
            test.paste("some text\nvar = 2\nvar * 3");
            test.set_cursor_row_col(2, i);
            test.render();

            test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());
            let cursor_pos = test.app().editor.get_selection().get_cursor_pos();
            assert_eq!(cursor_pos.row, 1);
            assert_eq!(cursor_pos.column, 0);
            assert_eq!("some text\nvar = 2\nvar * 3", &test.get_editor_content());
        }
    }

    #[test]
    fn test_ctrl_b_jumps_to_var_def_and_moves_the_scrollbar() {
        let test = create_app2(35);
        test.paste("var = 2\n");
        test.repeated_paste("asd\n", 40);
        test.paste("var");
        test.set_cursor_row_col(0, 0);
        assert_eq!(test.get_render_data().scroll_y, 0);
        test.input(EditorInputEvent::PageDown, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 7 /*42 - 35*/);
        test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());
        assert_eq!(test.get_render_data().scroll_y, 0);
    }

    #[test]
    fn test_ctrl_b_jumps_to_var_def_negative() {
        let test = create_app2(35);
        test.paste("some text\nvar = 2\nvar * 3");
        for i in 0..=9 {
            test.set_cursor_row_col(0, i);
            test.render();
            test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());
            let cursor_pos = test.app().editor.get_selection().get_cursor_pos();
            assert_eq!(cursor_pos.row, 0);
            assert_eq!(cursor_pos.column, i);
            assert_eq!(
                "some text",
                test.get_editor_content().lines().next().unwrap()
            );
        }
        for i in 0..=7 {
            test.set_cursor_row_col(1, i);
            test.render();
            test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());
            let cursor_pos = test.app().editor.get_selection().get_cursor_pos();
            assert_eq!(cursor_pos.row, 1);
            assert_eq!(cursor_pos.column, i);
            let content = test.get_editor_content();
            let mut lines = content.lines();
            lines.next();
            assert_eq!("var = 2", lines.next().unwrap());
        }
        for i in 0..=4 {
            test.set_cursor_row_col(2, 4 + i);
            test.render();
            test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());
            let cursor_pos = test.app().editor.get_selection().get_cursor_pos();
            assert_eq!(cursor_pos.row, 2);
            assert_eq!(cursor_pos.column, 4 + i);
            let content = test.get_editor_content();
            let mut lines = content.lines();
            lines.next();
            lines.next();
            assert_eq!("var * 3", lines.next().unwrap());
        }
    }

    #[test]
    fn test_ctrl_b_jumps_to_line_ref() {
        let test = create_app2(35);
        test.paste("2\n3\nasd &[2] * 4");
        test.set_cursor_row_col(2, 3);

        test.input(EditorInputEvent::Right, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());
        let cursor_pos = test.app().editor.get_selection().get_cursor_pos();
        assert_eq!(cursor_pos.row, 1);
        assert_eq!(cursor_pos.column, 0);

        test.input(EditorInputEvent::Down, InputModifiers::none());
        test.set_cursor_row_col(2, 3);
        test.input(EditorInputEvent::Right, InputModifiers::none());
        test.input(EditorInputEvent::Right, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());
        let cursor_pos = test.app().editor.get_selection().get_cursor_pos();
        assert_eq!(cursor_pos.row, 1);
        assert_eq!(cursor_pos.column, 0);
    }

    #[test]
    fn test_that_dependant_vars_are_pulsed_when_the_cursor_gets_there_by_ctrl_b() {
        let test = create_app2(35);
        test.paste("var = 2\nvar * 3");
        test.set_cursor_row_col(1, 0);

        test.render();
        let render_bucket = &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
        assert_eq!(render_bucket.len(), 4);

        // step into the first row
        test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());

        let render_bucket = &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
        assert_eq!(render_bucket.len(), 3);
        // SetColor(
        // RenderChar(
        let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
        assert_eq!(
            render_bucket[2],
            OutputMessage::PulsingRectangle {
                x: left_gutter_width,
                y: canvas_y(1),
                w: 3,
                h: 1,
                start_color: REFERENCE_PULSE_PULSE_START_COLOR,
                end_color: 0x00FF7F_00,
                animation_time: Duration::from_millis(1000)
            }
        );
    }

    mod highlighting_referenced_lines_tests {
        use super::super::*;
        use super::*;

        #[test]
        fn test_referenced_lineref_of_active_line_are_highlighted() {
            let test = create_app2(35);
            test.paste("223456\nasd &[1] * 2");
            test.set_cursor_row_col(0, 0);
            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render();
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
            assert_eq!(
                render_buckets.custom_commands[Layer::AboveText as usize].len(),
                3 /*SetColor and render cursor and 1 pulsing*/
            );

            test.input(EditorInputEvent::Down, InputModifiers::none());

            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);

            let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
            let render_commands = &render_buckets.custom_commands[Layer::AboveText as usize];
            assert_eq!(render_commands.len(), 4);
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x33),
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(0),
                    w: "223456".len(),
                    h: 1,
                },
            );

            // the line_refs itself have different bacground color
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x55),
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w + 4,
                    y: canvas_y(1),
                    w: "223 456".len(),
                    h: 1,
                },
            );
        }

        #[test]
        fn test_multiple_referenced_linerefs_in_different_rows_of_active_line_are_highlighted() {
            let test = create_app2(35);
            test.paste("234\n356789\nasd &[1] * &[2] * 2");
            test.set_cursor_row_col(1, 0);
            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render();
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
            dbg!(&render_buckets.custom_commands[Layer::AboveText as usize]);
            assert_eq!(
                render_buckets.custom_commands[Layer::AboveText as usize].len(),
                3 /*SetColor, render cursor and pulsing the 2nd editor obj in 3rd line*/
            );

            test.input(EditorInputEvent::Down, InputModifiers::none());

            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
            let render_commands = &render_buckets.custom_commands[Layer::AboveText as usize];
            assert_eq!(render_commands.len(), 6);
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x33),
            );
            let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(0),
                    w: "234".len(),
                    h: 1,
                },
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1] << 8 | 0x33),
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(1),
                    w: "356789".len(),
                    h: 1,
                },
            );

            // the line_refs itself have different bacground color
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x55),
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w + 4,
                    y: canvas_y(2),
                    w: "234".len(),
                    h: 1,
                },
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1] << 8 | 0x55),
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w + 10,
                    y: canvas_y(2),
                    w: "356 789".len(),
                    h: 1,
                },
            );
        }

        #[test]
        fn test_same_lineref_referenced_multiple_times_is_highlighted() {
            let test = create_app2(35);
            test.paste("2345\nasd &[1] * &[1] * 2");
            test.set_cursor_row_col(0, 0);
            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render();
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
            assert_eq!(
                render_buckets.custom_commands[Layer::AboveText as usize].len(),
                4 /*SetColor and render cursor and 2 pulsings*/
            );

            test.input(EditorInputEvent::Down, InputModifiers::none());

            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
            let render_commands = &render_buckets.custom_commands[Layer::AboveText as usize];
            assert_eq!(render_commands.len(), 4);
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x33),
            );
            let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(0),
                    w: "2345".len(),
                    h: 1,
                },
            );

            // the line_refs have to have same background colors
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                2,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x55),
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w + 4,
                    y: canvas_y(1),
                    w: 5,
                    h: 1,
                },
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w + 12,
                    y: canvas_y(1),
                    w: 5,
                    h: 1,
                },
            );
        }

        #[test]
        fn test_same_lineref_referenced_multiple_times_plus_another_in_diff_row_is_highlighted() {
            let test = create_app2(35);
            test.paste("2345\n123\nasd &[1] * &[1] * &[2] * 2");
            test.set_cursor_row_col(1, 0);
            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render();
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
            assert_eq!(
                render_buckets.custom_commands[Layer::AboveText as usize].len(),
                3 /*SetColor and render cursor and 1 pulsing*/
            );

            test.input(EditorInputEvent::Down, InputModifiers::none());

            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
            let render_commands = &render_buckets.custom_commands[Layer::AboveText as usize];
            assert_eq!(render_commands.len(), 6);
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x33),
            );
            let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(0),
                    w: "2345".len(),
                    h: 1,
                },
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1] << 8 | 0x33),
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(1),
                    w: "123".len(),
                    h: 1,
                },
            );

            // the line_refs have to have same background colors
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                2,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x55),
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w + 4,
                    y: canvas_y(2),
                    w: 5,
                    h: 1,
                },
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w + 12,
                    y: canvas_y(2),
                    w: 5,
                    h: 1,
                },
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1] << 8 | 0x55),
            );
            test.contains(
                &render_buckets.custom_commands[Layer::BehindText as usize],
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w + 20,
                    y: canvas_y(2),
                    w: 3,
                    h: 1,
                },
            );
        }

        #[test]
        fn test_out_of_screen_pulsing_var() {
            let test = create_app2(20);
            test.paste("var = 4");
            test.repeated_paste("asd\n", 30);
            test.paste("var");
            test.set_cursor_row_col(0, 0);
            test.render();
            test.input(EditorInputEvent::PageDown, InputModifiers::none());
            test.input(EditorInputEvent::PageUp, InputModifiers::none());
            let render_bucket =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            // no pulsing should happen since the referencing line is out of view
            for command in render_bucket {
                assert!(!matches!(command, OutputMessage::PulsingRectangle {..}));
            }
        }

        #[test]
        fn test_referenced_vars_and_linerefs_of_active_lines_are_pulsing() {
            let test = create_app2(35);
            test.paste("2\n3\nvar = 4\nasd &[1] * &[2] * var");
            test.set_cursor_row_col(2, 0);
            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render();
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
            assert_eq!(
                render_buckets.custom_commands[Layer::AboveText as usize].len(),
                3 /*SetColor and render cursor and 1 pulsing*/
            );

            test.input(EditorInputEvent::Down, InputModifiers::none());

            let mut render_buckets = RenderBuckets::new();
            let mut result_buffer = [0; 128];
            test.render_get_result_commands(&mut render_buckets, &mut result_buffer[..]);
            let render_commands = &render_buckets.custom_commands[Layer::AboveText as usize];
            let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
            assert_eq!(render_commands.len(), 8);
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x33),
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(0),
                    w: "2".len(),
                    h: 1,
                },
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1] << 8 | 0x33),
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(1),
                    w: "3".len(),
                    h: 1,
                },
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[2] << 8 | 0x33),
            );
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(2),
                    w: "var = 4".len(),
                    h: 1,
                },
            );
        }

        #[test]
        fn test_bug_wrong_referenced_line_is_highlighted() {
            let test = create_app2(35);
            test.paste(
                "pi() * 3
nth([1,2,3], 2)

a = -9.8m/s^2
v0 = 100m/s
x0 = 490m
t = 30s

1/2*a*(t^2) + (v0*t) + x0


price = 350k$
down payment = 20% * price
finance amount = price - down payment

interest rate = 0.037 (1/year)
term = 30 years
-- n = term * 12 (1/year)
n = 360
r = interest rate / (12 (1/year))

monthly payment = r/(1 - (1 + r)^(-n)) *finance amount",
            );
            test.set_cursor_row_col(11, 0);
            test.render();

            test.input(EditorInputEvent::Backspace, InputModifiers::none());
            test.input(EditorInputEvent::Down, InputModifiers::none());

            let render_commands =
                &test.mut_render_bucket().custom_commands[Layer::AboveText as usize];
            test.contains(
                render_commands,
                1,
                OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0] << 8 | 0x33),
            );
            let left_gutter_w = LEFT_GUTTER_MIN_WIDTH + 1;
            test.contains(
                render_commands,
                1,
                OutputMessage::RenderRectangle {
                    x: left_gutter_w,
                    y: canvas_y(10),
                    w: "price = 350k$".len(),
                    h: 1,
                },
            );
        }
    }

    // let say a 3rd row references a var from the 2nd row.
    // Then I remove the first row, then, when the parser parses the new
    // 2nd row (which was the 3rd one), in its vars[1] there is the variable,
    // since in the previous parse it was defined at index 1.
    // this test guarantee that when parsing row index 1, var index 1
    // is not considered.
    #[test]
    fn test_that_var_from_prev_frame_in_the_current_line_is_not_considered_during_parsing() {
        let test = create_app2(35);
        test.paste(
            "
a = 10
b = a * 20",
        );
        test.set_cursor_row_col(0, 0);
        test.input(EditorInputEvent::Del, InputModifiers::none());
        dbg!(&test.editor_objects()[content_y(1)][1]);
        assert!(matches!(
            &test.editor_objects()[content_y(1)][1].typ,
            EditorObjectType::Variable { var_index: 0 }
        ))
    }

    #[test]
    fn converting_unit_of_line_ref() {
        let test = create_app2(35);
        test.paste("573 390 s");
        test.set_cursor_row_col(0, 9);
        test.render();

        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        test.input(EditorInputEvent::Char(' '), InputModifiers::none());
        test.input(EditorInputEvent::Char('i'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char(' '), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());

        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);
        assert_results(&["573 390 s", "159.275 h"][..], &result_buffer);
    }

    #[test]
    fn calc_pow() {
        let test = create_app2(35);
        test.paste(
            "price = 350k$
down payment = 20% * price
finance amount = price - down payment

interest rate = 0.037 (1/year)
term = 30 years
-- n = term * 12 (1/year)
n = 360
r = interest rate / (12 (1/year))

monthly payment = r/(1 - (1 + r)^(-n)) *finance amount",
        );
    }

    #[test]
    fn no_panic_on_huge_input() {
        let test = create_app2(35);
        test.paste("3^300");
    }

    #[test]
    fn no_panic_on_huge_input2() {
        let test = create_app2(35);
        test.paste("300^300");
    }

    #[test]
    fn test_error_related_to_variable_precedence() {
        let test = create_app2(35);
        test.paste(
            "v0=2m/s
t=4s
0m+v0*t",
        );
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        assert_results(&["2 m / s", "4 s", "8 m"][..], &result_buffer);
    }

    #[test]
    fn test_error_related_to_variable_precedence2() {
        let test = create_app2(35);
        test.paste(
            "a = -9.8m/s^2
v0 = 100m/s
x0 = 490m
t = 2s
1/2*a*t^2 + v0*t + x0",
        );
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        assert_results(
            &["-9.8 m / s^2", "100 m / s", "490 m", "2 s", "670.4 m"][..],
            &result_buffer,
        );
    }

    #[test]
    fn test_no_panic_on_too_big_number() {
        let test = create_app2(35);
        test.paste(
            "pi() * 3
nth([1,2,3], 2)

a = -9.8m/s^2
v0 = 100m/s
x0 = 490m
t = 30s

1/2*a*(t^2) + (v0*t) + x0

price = 350k$
down payment = 20% * price
finance amount = price - down payment

interest rate = 0.037 (1/year)
term = 30 years
-- n = term * 12 (1/year)
n = 36000
r = interest rate / (12 (1/year))

monthly payment = r/(1 - (1 + r)^(-n)) *finance amount",
        );
        test.set_cursor_row_col(17, 9);
        test.input(EditorInputEvent::Char('0'), InputModifiers::none());
    }

    #[test]
    fn test_itself_unit_rendering() {
        let test = create_app2(35);
        test.paste("a = /year");
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        assert_results(&[""][..], &result_buffer);
    }

    #[test]
    fn test_itself_unit_rendering2() {
        let test = create_app2(35);
        test.paste("a = 2/year");
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        assert_results(&["2 year^-1"][..], &result_buffer);
    }

    #[test]
    fn test_editor_panic() {
        let test = create_app2(35);
        test.paste(
            "
a",
        );
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
    }

    #[test]
    fn test_wrong_selection_removal() {
        let test = create_app2(35);
        test.paste(
            "
interest rate = 3.7%/year
term = 30 years
n = term * 12/year
interest rate / (12 (1/year))

2m^4kg/s^3
946728000 *1246728000 *12",
        );
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        assert_eq!("p", test.get_editor_content());
    }

    #[test]
    fn integration_test() {
        let test = create_app2(35);
        test.paste(
            "price = 350 000$
down payment = price * 20%
finance amount = price - down payment

interest rate = 3.7%/year
term = 30year

n = term * 12/year
r = interest rate / (12/year)

monthly payment = r/(1 - (1+r)^(-n)) * finance amount",
        );
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        assert_results(
            &[
                "350 000 $",
                "70 000 $",
                "280 000 $",
                "",
                "0.037 year^-1",
                "30 year",
                "",
                "360",
                "0.0031",
                "",
                "1 288.7924 $",
            ][..],
            &result_buffer,
        );
    }

    #[test]
    fn test_if_number_is_too_big_for_binary_repr_show_err() {
        let test = create_app2(35);
        test.paste("10e24");
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        assert_results(&["Err"][..], &result_buffer);
    }

    #[test]
    fn test_if_number_is_too_big_for_hex_repr_show_err() {
        let test = create_app2(35);
        test.paste("10e24");
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        assert_results(&["Err"][..], &result_buffer);
    }

    #[test]
    fn integration_test_for_rich_copy() {
        let test = create_app2(35);
        test.paste(
            "price = 350 000$
down payment = price * 20%
finance amount = price - down payment

interest rate = 3.7%/year
term = 30year

n = term * 12/year
r = interest rate / (12/year)

monthly payment = r/(1 - (1+r)^(-n)) * finance amount",
        );
        test.input(EditorInputEvent::Char('c'), InputModifiers::ctrl_shift());
    }

    #[test]
    fn test_percentage_output() {
        let test = create_app2(35);
        test.paste("20%");
        let mut result_buffer = [0; 128];
        test.render_get_result_buf(&mut result_buffer[..]);

        assert_results(&["20 %"][..], &result_buffer);
    }

    #[test]
    fn test_parsing_panic_20201116() {
        let test = create_app2(35);
        test.paste("2^63-1\n6*13\nennyi staging entity lehet &[1] / 50\n\nnaponta ennyit kell beszurni, hogy \'1 év alatt megteljen: &[1] / 365\n\nennyi évig üzemel, ha napi ezer sor szurodik be: &[1] / (365*1000)\n120 * 100 = \n1.23e20\n\n500$ / 20$/hour in hour\n1km + 1000m\n3 kg * 3 liter\n3 hours + 5minutes + 10 seconds in seconds\n20%\n\n1t in kg\nmass of earth = 5.972e18 Gg\n\n20%\n");
    }
}
