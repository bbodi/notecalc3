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
use strum_macros::EnumDiscriminants;

use helper::*;

use crate::calc::{
    add_op, evaluate_tokens, get_var_name_from_assignment, process_variable_assignment_or_line_ref,
    CalcResult, CalcResultType, EvalErr, EvaluationResult, ShuntingYardResult,
};
use crate::consts::{LINE_NUM_CONSTS, LINE_NUM_CONSTS2, LINE_NUM_CONSTS3};
use crate::editor::editor::{
    Editor, EditorInputEvent, InputModifiers, Pos, RowModificationType, Selection,
};
use crate::editor::editor_content::EditorContent;
use crate::functions::FnType;
use crate::matrix::MatrixData;
use crate::renderer::{get_int_frac_part_len, render_result, render_result_into};
use crate::shunting_yard::ShuntingYard;
use crate::token_parser::{debug_print, OperatorTokenType, Token, TokenParser, TokenType};
use crate::units::units::Units;
use tinyvec::ArrayVec;

pub mod functions;
pub mod matrix;
pub mod shunting_yard;
pub mod test_common;
pub mod token_parser;
pub mod units;

pub mod borrow_checker_fighter;
pub mod calc;
pub mod consts;
pub mod editor;
pub mod renderer;

#[inline]
fn _readonly_<T: ?Sized>(e: &mut T) -> &T {
    return e;
}

#[inline]
#[cfg(feature = "tracy")]
fn tracy_span(name: &str, file: &str, line: u32) -> tracy_client::Span {
    return tracy_client::Span::new(name, name, file, line, 100);
}

#[inline]
#[cfg(not(feature = "tracy"))]
fn tracy_span(_name: &str, _file: &str, _line: u32) -> () {}

pub const SCROLLBAR_WIDTH: usize = 1;

pub const RENDERED_RESULT_PRECISION: usize = 28;
pub const MAX_EDITOR_WIDTH: usize = 120;
pub const LEFT_GUTTER_MIN_WIDTH: usize = 2;

// Currently y coords are transmitted as u8 to the frontend, if you raise this value,
// don't forget to update  the communication layer as well
pub const MAX_LINE_COUNT: usize = 256;
pub const MAX_TOKEN_COUNT_PER_LINE: usize = MAX_LINE_COUNT;
pub const RIGHT_GUTTER_WIDTH: usize = 2;
pub const MIN_RESULT_PANEL_WIDTH: usize = 7;

// There are some optimizationts (stack allocated arrays etc), where we have to know
// the maximum lines rendered at once, so it is limited to 64
pub const MAX_CLIENT_HEIGHT: usize = 64;
pub const DEFAULT_RESULT_PANEL_WIDTH_PERCENT: usize = 30;
pub const SUM_VARIABLE_INDEX: usize = MAX_LINE_COUNT;
#[allow(dead_code)]
pub const FIRST_FUNC_PARAM_VAR_INDEX: usize = SUM_VARIABLE_INDEX + 1;
pub const VARIABLE_ARR_SIZE: usize = MAX_LINE_COUNT + 1 + MAX_FUNCTION_PARAM_COUNT;
pub const MATRIX_ASCII_HEADER_FOOTER_LINE_COUNT: usize = 2;
pub const ACTIVE_LINE_REF_HIGHLIGHT_COLORS: [u32; 9] = [
    0xFFD300FF, 0xDE3163FF, 0x73c2fbFF, 0xc7ea46FF, 0x702963FF, 0x997950FF, 0x777b73FF, 0xFC6600FF,
    0xED2939FF,
];

// I hate Rust's borrow checker
// Double rendering was not possible in a single method since there is no
// way to express my intentions and tell that it is safe to reuse this buffer
// because the references to it are removed -_-'.
// Because of this global var, only single thread test running is possible.
static mut RESULT_BUFFER: [u8; 2048] = [0; 2048];

#[allow(dead_code)]
pub struct Theme {
    pub bg: u32,
    pub func_bg: u32,
    pub result_bg_color: u32,
    pub selection_color: u32,
    pub sum_bg_color: u32,
    pub sum_text_color: u32,
    pub reference_pulse_start: u32,
    pub reference_pulse_end: u32,
    pub number: u32,
    pub number_error: u32,
    pub operator: u32,
    pub unit: u32,
    pub variable: u32,
    pub result_text: u32,
    pub header: u32,
    pub text: u32,
    pub cursor: u32,
    pub matrix_edit_active_bg: u32,
    pub matrix_edit_active_text: u32,
    pub matrix_edit_inactive_text: u32,
    pub result_gutter_bg: u32,
    pub left_gutter_bg: u32,
    pub line_num_active: u32,
    pub line_num_simple: u32,
    pub scrollbar_hovered: u32,
    pub scrollbar_normal: u32,
    pub line_ref_bg: u32,
    pub line_ref_text: u32,
    pub line_ref_selector: u32,
    pub referenced_matrix_text: u32,
    pub change_result_pulse_start: u32,
    pub change_result_pulse_end: u32,
    pub current_line_bg: u32,
    pub parenthesis: u32,
}

#[allow(dead_code)]
impl Theme {
    const DRACULA_BG: u32 = 0x282a36_FF;
    const DRACULA_CURRENT_LINE: u32 = 0x44475a_FF;
    const DRACULA_FG: u32 = 0xf8f8f2_FF;
    const DRACULA_CYAN: u32 = 0x8be9fd_FF;
    const DRACULA_COMMENT: u32 = 0x6272a4_FF;
    const DRACULA_GREEN: u32 = 0x50fa7b_FF;
    const DRACULA_ORANGE: u32 = 0xffb86c_FF;
    const DRACULA_PINK: u32 = 0xff79c6_FF;
    const DRACULA_PURPLE: u32 = 0xbd93f9_FF;
    const DRACULA_RED: u32 = 0xff5555_FF;
    const DRACULA_YELLOW: u32 = 0xf1fa8c_FF;
}

pub const THEMES: [Theme; 2] = [
    // LIGHT
    Theme {
        bg: 0xFFFFFF_FF,
        func_bg: 0xEFEFEF_FF,
        result_bg_color: 0xF2F2F2_FF,
        result_gutter_bg: 0xD2D2D2_FF,
        selection_color: 0xA6D2FF_FF,
        sum_bg_color: 0x008a0d_FF,
        sum_text_color: 0x000000_FF,
        reference_pulse_start: 0x00FF7F_33,
        reference_pulse_end: 0x00FF7F_00,
        number: 0x8963c4_FF,
        number_error: 0xde353d_FF,
        operator: 0x3a88e8_FF,
        unit: 0x048395_FF,
        variable: 0xc26406_FF,
        result_text: 0x000000_FF,
        header: 0x000000_FF,
        text: 0x8393c7_FF,
        cursor: 0x000000_FF,
        matrix_edit_active_bg: 0xBBBBBB_55,
        matrix_edit_active_text: 0x000000_FF,
        matrix_edit_inactive_text: 0x000000_FF,
        left_gutter_bg: 0xF2F2F2_FF,
        line_num_active: 0x000000_FF,
        line_num_simple: 0xADADAD_FF,
        scrollbar_hovered: 0xFFBBBB_FF,
        scrollbar_normal: 0xFFCCCC_FF,
        line_ref_text: 0x000000_FF,
        line_ref_bg: 0xDCE2F7_FF,
        line_ref_selector: 0xDCE2F7_FF,
        referenced_matrix_text: 0x000000_FF,
        change_result_pulse_start: 0xFF88FF_AA,
        change_result_pulse_end: 0xFFFFFF_55,
        current_line_bg: 0xFFFFCC_FF,
        parenthesis: 0x565869_FF,
    },
    // DARK
    Theme {
        bg: Theme::DRACULA_BG,
        func_bg: 0x292B37_FF,
        result_bg_color: 0x3c3f41_FF,
        result_gutter_bg: 0x313335_FF,
        selection_color: 0x214283_FF,
        sum_bg_color: Theme::DRACULA_GREEN,
        sum_text_color: 0x000000_FF,
        reference_pulse_start: 0x00FF7F_33,
        reference_pulse_end: 0x00FF7F_00,
        number: Theme::DRACULA_PURPLE,
        number_error: Theme::DRACULA_RED,
        operator: 0x5bb0ff_FF, // Theme::DRACULA_YELLOW,
        unit: Theme::DRACULA_CYAN,
        variable: Theme::DRACULA_ORANGE,
        result_text: Theme::DRACULA_FG,
        header: Theme::DRACULA_FG,
        text: Theme::DRACULA_COMMENT + 0x444444_00,
        cursor: Theme::DRACULA_FG,
        matrix_edit_active_bg: 0xBBBBBB_55,
        matrix_edit_active_text: 0x000000_FF,
        matrix_edit_inactive_text: 0x000000_FF,
        left_gutter_bg: 0x3c3f41_FF,
        line_num_active: 0xa3a2a0_FF,
        line_num_simple: 0x4e6164_FF,
        scrollbar_hovered: 0x4f4f4f_FF,
        scrollbar_normal: 0x4b4b4b_FF,
        line_ref_bg: 0x7C92A7_FF,
        line_ref_text: 0x000000_FF,
        line_ref_selector: Theme::DRACULA_BG + 0x333300_00,
        referenced_matrix_text: 0x000000_FF,
        change_result_pulse_start: 0xFF88FF_AA,
        change_result_pulse_end: Theme::DRACULA_BG - 0xFF,
        current_line_bg: Theme::DRACULA_CURRENT_LINE,
        parenthesis: Theme::DRACULA_PINK,
    },
];

#[allow(non_snake_case)]
#[inline]
pub fn NOT(a: bool) -> bool {
    !a
}

pub enum Click {
    Simple(Pos),
    Drag(Pos),
}

pub const MAX_FUNCTION_PARAM_COUNT: usize = 6;
pub const MAX_VAR_NAME_LEN: usize = 32;

fn get_function_index_for_line(
    line_index: usize,
    func_defs: &FunctionDefinitions,
) -> Option<usize> {
    for investigated_line_i in (0..=line_index).rev() {
        if let Some(fd) = func_defs[investigated_line_i].as_ref() {
            let func_end_index = fd.last_row_index.as_usize();
            let line_index_is_part_of_that_func = line_index <= func_end_index;
            return if line_index_is_part_of_that_func {
                Some(investigated_line_i)
            } else {
                None
            };
        }
    }
    None
}

#[derive(Debug)]
pub struct FunctionDef<'a> {
    // dont wanna fight with rust bc
    pub func_name: &'a [char],
    pub param_names: [&'a [char]; MAX_FUNCTION_PARAM_COUNT],
    pub param_count: usize,
    pub first_row_index: ContentIndex,
    pub last_row_index: ContentIndex,
}

pub fn try_extract_function_def<'b>(
    parsed_tokens: &mut [Token<'b>],
    allocator: &'b Bump,
) -> Option<FunctionDef<'b>> {
    if parsed_tokens.len() < 4
        || (!parsed_tokens[0].ptr[0].is_alphabetic() && parsed_tokens[0].ptr[0] != '_')
        || parsed_tokens[1].typ != TokenType::Operator(OperatorTokenType::ParenOpen)
        || parsed_tokens.last().unwrap().ptr != &[':']
    {
        return None;
    }
    let mut fd = FunctionDef {
        func_name: parsed_tokens[0].ptr,
        param_names: [&[]; MAX_FUNCTION_PARAM_COUNT],
        param_count: 0,
        first_row_index: content_y(0),
        last_row_index: content_y(0),
    };
    fn skip_whitespace_tokens(parsed_tokens: &[Token], token_index: &mut usize) {
        while *token_index < parsed_tokens.len()
            && parsed_tokens[*token_index].ptr[0].is_whitespace()
        {
            *token_index += 1;
        }
    }
    fn close_var_name_parsing<'b>(
        param_index: &mut usize,
        fd: &mut FunctionDef<'b>,
        var_name: &[char],
        allocator: &'b Bump,
    ) {
        fd.param_names[*param_index] =
            allocator.alloc_slice_fill_iter(var_name.iter().map(|it| *it));
        *param_index += 1;
    }
    let mut param_index = 0;
    let mut token_index = 2;
    // TODO Bitflag u16 or u32 is enough
    let mut token_indices_for_params = BitFlag256::empty();
    let mut tmp_var_name: ArrayVec<[char; MAX_VAR_NAME_LEN]> = ArrayVec::new();
    loop {
        if token_index == parsed_tokens.len() - 2
            && parsed_tokens[token_index].typ == TokenType::Operator(OperatorTokenType::ParenClose)
            && parsed_tokens[token_index + 1].ptr == &[':']
        {
            if !tmp_var_name.is_empty() {
                // there is a last parameter
                close_var_name_parsing(&mut param_index, &mut fd, &tmp_var_name, allocator);
            }
            break;
        } else if parsed_tokens[token_index].typ == TokenType::Operator(OperatorTokenType::Comma) {
            close_var_name_parsing(&mut param_index, &mut fd, &tmp_var_name, allocator);
            tmp_var_name.clear();

            token_index += 1; // skip ','
            skip_whitespace_tokens(parsed_tokens, &mut token_index);
        } else if matches!(parsed_tokens[token_index].typ, TokenType::StringLiteral | TokenType::Variable {..})
        {
            if tmp_var_name.len() + parsed_tokens[token_index].ptr.len() > MAX_VAR_NAME_LEN {
                return None;
            }
            tmp_var_name.extend_from_slice(parsed_tokens[token_index].ptr);
            token_indices_for_params.set(token_index);
            token_index += 1;
        } else {
            return None;
        }
    }

    fd.param_count = param_index;

    parsed_tokens[0].typ = TokenType::Operator(OperatorTokenType::Fn {
        arg_count: fd.param_count,
        typ: FnType::UserDefined(0),
    });
    // set ':' to operator
    parsed_tokens.last_mut().unwrap().typ = TokenType::Operator(OperatorTokenType::Add);
    // set param names to variables
    for i in 0..parsed_tokens.len() {
        if token_indices_for_params.is_true(i) {
            parsed_tokens[i].typ = TokenType::Variable {
                var_index: FIRST_FUNC_PARAM_VAR_INDEX + i,
            };
        }
    }

    return Some(fd);
}

pub mod helper {
    // so code from the lib module can't access the private parts

    use std::ops::{Index, IndexMut};

    use crate::calc::CalcResultType;
    pub use crate::{MAX_LINE_COUNT, *};

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
    pub struct BitFlag256 {
        pub bitset: [u128; 2],
    }

    impl BitFlag256 {
        pub fn empty() -> BitFlag256 {
            BitFlag256 { bitset: [0; 2] }
        }

        fn get_index(row_index: usize) -> (usize, usize) {
            let array_index = (row_index & (!127) > 0) as usize;
            let index_inside_u128 = row_index & 127;
            return (array_index, index_inside_u128);
        }

        pub fn set(&mut self, row_index: usize) {
            let (array_index, index_inside_u128) = BitFlag256::get_index(row_index);
            self.bitset[array_index] |= 1u128 << index_inside_u128;
        }

        pub fn single_row(row_index: usize) -> BitFlag256 {
            let (array_index, index_inside_u128) = BitFlag256::get_index(row_index);
            let mut bitset = [0; 2];
            bitset[array_index] = 1u128 << index_inside_u128;
            BitFlag256 { bitset }
        }

        #[inline]
        pub fn clear(&mut self) {
            self.bitset[0] = 0;
            self.bitset[1] = 0;
        }

        pub fn all_rows_starting_at(row_index: usize) -> BitFlag256 {
            if row_index >= MAX_LINE_COUNT {
                return BitFlag256::empty();
            }
            let mut bitset = [0; 2];

            let (array_index, index_inside_u128) = BitFlag256::get_index(row_index);
            let s = 1u128 << index_inside_u128;
            let right_to_s_bits = s - 1;
            let left_to_s_and_s_bits = !right_to_s_bits;
            bitset[array_index] = left_to_s_and_s_bits;
            // the other int is either fully 1s (if the array_index is 0) or 0s (if array_index is 1)
            let other_index = array_index ^ 1;
            bitset[other_index] = std::u128::MAX * other_index as u128;

            BitFlag256 { bitset }
        }
        // TODO multiple2(a, b), multiple3(a,b,c) etc, faster
        pub fn multiple(indices: &[usize]) -> BitFlag256 {
            let mut b = [0; 2];
            for i in indices {
                let (array_index, index_inside_u128) = BitFlag256::get_index(*i);
                b[array_index] |= 1 << index_inside_u128;
            }
            let bitset = b;

            BitFlag256 { bitset }
        }

        pub fn range_incl(from: usize, to: usize) -> BitFlag256 {
            debug_assert!(to >= from);
            if from >= MAX_LINE_COUNT {
                return BitFlag256::empty();
            } else if to >= MAX_LINE_COUNT {
                return BitFlag256::range_incl(from, MAX_LINE_COUNT - 1);
            }
            fn set_range_u128(from: usize, to: usize) -> u128 {
                let top = 1 << to;
                let right_to_top_bits = top - 1;
                let bottom = 1 << from;
                let right_to_bottom_bits = bottom - 1;
                return (right_to_top_bits ^ right_to_bottom_bits) | top;
            }
            let mut b = BitFlag256::empty();
            if from < 128 {
                b.bitset[0] = set_range_u128(from, to.min(127));
            }
            if to >= 128 {
                b.bitset[1] = set_range_u128(((from as isize) - 128).max(0) as usize, to - 128);
            }

            return b;
        }

        #[inline]
        pub fn merge(&mut self, other: BitFlag256) {
            self.bitset[0] |= other.bitset[0];
            self.bitset[1] |= other.bitset[1];
        }

        #[inline]
        pub fn need(&self, line_index: ContentIndex) -> bool {
            let (array_index, index_inside_u128) = BitFlag256::get_index(line_index.0);
            ((1 << index_inside_u128) & self.bitset[array_index]) != 0
        }

        #[inline]
        pub fn is_true(&self, line_index: usize) -> bool {
            return self.need(content_y(line_index));
        }

        #[inline]
        pub fn is_false(&self, line_index: usize) -> bool {
            return !self.is_true(line_index);
        }

        #[inline]
        pub fn is_non_zero(&self) -> bool {
            (self.bitset[0] | self.bitset[1]) != 0
        }
    }

    #[derive(Clone, Debug)]
    pub struct GlobalRenderData {
        pub client_height: usize,
        pub client_width: usize,
        pub scroll_y: usize,
        pub result_gutter_x: usize,
        pub left_gutter_width: usize,
        pub longest_visible_result_len: usize,
        pub longest_visible_editor_line_len: usize,
        pub current_editor_width: usize,
        pub current_result_panel_width: usize,
        editor_y_to_render_y: [Option<CanvasY>; MAX_LINE_COUNT],
        editor_y_to_rendered_height: [usize; MAX_LINE_COUNT],
        pub theme_index: usize,
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
                longest_visible_result_len: 0,
                longest_visible_editor_line_len: 0,
                result_gutter_x,
                left_gutter_width,
                current_editor_width: 0,
                current_result_panel_width: 0,
                editor_y_to_render_y: [None; MAX_LINE_COUNT],
                editor_y_to_rendered_height: [0; MAX_LINE_COUNT],
                client_height: client_height.min(MAX_CLIENT_HEIGHT),
                client_width,
                theme_index: 0,
            };
            r.current_editor_width = (result_gutter_x - left_gutter_width) - 1;
            r.current_result_panel_width = client_width - result_gutter_x - right_gutter_width;
            // so tests without calling "paste" work
            r.editor_y_to_rendered_height[0] = 1;
            r
        }

        pub fn set_result_gutter_x(&mut self, x: usize) {
            self.result_gutter_x = x;
            // - 1 so that the last visible character in the editor is '…' if the content is to long
            self.current_editor_width = (x - self.left_gutter_width) - 1;
            self.current_result_panel_width = self.client_width - x - RIGHT_GUTTER_WIDTH;
        }

        pub fn set_left_gutter_width(&mut self, new_width: usize) {
            self.left_gutter_width = new_width;
            // - 1 so that the last visible character in the editor is '…' if the content is to long
            self.current_editor_width = (self.result_gutter_x - new_width) - 1;
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

    #[derive(Debug)]
    pub struct PerLineRenderData {
        pub editor_x: usize,
        pub editor_y: ContentIndex,
        pub render_x: usize,
        pub render_y: CanvasY,
        // contains the y position for each editor line
        pub rendered_row_height: usize,
        pub vert_align_offset: usize,
        pub cursor_render_x_offset: isize,
        // for rendering line number
        pub line_num_digit_0: u8,
        pub line_num_digit_1: u8,
        pub line_num_digit_2: u8,
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
                line_num_digit_0: 0,
                line_num_digit_1: 0,
                line_num_digit_2: 0,
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

    #[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
    pub struct ContentIndex(usize);

    #[inline]
    pub fn content_y(y: usize) -> ContentIndex {
        ContentIndex(y)
    }

    impl ContentIndex {
        #[inline]
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

#[derive(Debug, PartialEq, Clone)]
pub struct RenderUtf8TextMsg<'a> {
    pub text: &'a [char],
    pub row: CanvasY,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderChar {
    pub col: usize,
    pub row: CanvasY,
    pub char: char,
}

#[derive(Debug, PartialEq, Clone)]
pub struct RenderAsciiTextMsg<'a> {
    pub text: &'a [u8],
    pub row: CanvasY,
    pub column: usize,
}

#[derive(Debug, PartialEq, Clone)]
pub struct RenderStringMsg {
    pub text: String,
    pub row: CanvasY,
    pub column: usize,
}

#[derive(Debug, PartialEq)]
pub struct PulsingRectangle {
    pub x: usize,
    pub y: CanvasY,
    pub w: usize,
    pub h: usize,
    pub start_color: u32,
    pub end_color: u32,
    pub animation_time: Duration,
    pub repeat: bool,
}

#[repr(C)]
#[derive(Debug, Clone, EnumDiscriminants, PartialEq)]
#[strum_discriminants(name(OutputMessageCommandId))]
pub enum OutputMessage<'a> {
    SetStyle(TextStyle),
    SetColor(u32),
    RenderChar(RenderChar),
    RenderUtf8Text(RenderUtf8TextMsg<'a>),
    RenderAsciiText(RenderAsciiTextMsg<'a>),
    RenderString(RenderStringMsg),
    RenderRectangle {
        x: usize,
        y: CanvasY,
        w: usize,
        h: usize,
    },
    FollowingTextCommandsAreHeaders(bool),
    RenderUnderline {
        x: usize,
        y: CanvasY,
        w: usize,
    },
    UpdatePulses,
}

#[repr(C)]
pub enum Layer {
    // function background
    BehindTextBehindCursor,
    // cursor
    BehindTextCursor,
    // highlighting words, matrix editor bg
    BehindTextAboveCursor,
    Text,
    AboveText,
}

#[derive(Debug, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

#[derive(Debug)]
pub struct RenderBuckets<'a> {
    pub left_gutter_bg: Rect,
    pub right_gutter_bg: Rect,
    pub result_panel_bg: Rect,
    pub scroll_bar: Option<(u32, Rect)>,
    pub ascii_texts: Vec<RenderAsciiTextMsg<'a>>,
    pub utf8_texts: Vec<RenderUtf8TextMsg<'a>>,
    pub headers: Vec<RenderUtf8TextMsg<'a>>,
    pub numbers: Vec<RenderUtf8TextMsg<'a>>,
    pub number_errors: Vec<RenderUtf8TextMsg<'a>>,
    pub units: Vec<RenderUtf8TextMsg<'a>>,
    pub operators: Vec<RenderUtf8TextMsg<'a>>,
    pub parenthesis: Vec<RenderChar>,
    pub variable: Vec<RenderUtf8TextMsg<'a>>,
    pub line_ref_results: Vec<RenderStringMsg>,
    pub custom_commands: [Vec<OutputMessage<'a>>; 5],
    pub pulses: Vec<PulsingRectangle>,
    pub clear_pulses: bool,
}

impl<'a> RenderBuckets<'a> {
    pub fn new() -> RenderBuckets<'a> {
        RenderBuckets {
            left_gutter_bg: Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            right_gutter_bg: Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            result_panel_bg: Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            scroll_bar: None,
            ascii_texts: Vec::with_capacity(128),
            utf8_texts: Vec::with_capacity(128),
            headers: Vec::with_capacity(16),
            custom_commands: [
                Vec::with_capacity(128),
                Vec::with_capacity(128),
                Vec::with_capacity(128),
                Vec::with_capacity(128),
                Vec::with_capacity(128),
            ],
            numbers: Vec::with_capacity(32),
            number_errors: Vec::with_capacity(32),
            units: Vec::with_capacity(32),
            operators: Vec::with_capacity(32),
            parenthesis: Vec::with_capacity(32),
            variable: Vec::with_capacity(32),
            line_ref_results: Vec::with_capacity(32),
            pulses: Vec::with_capacity(8),
            clear_pulses: false,
        }
    }

    pub fn custom_commands<'b>(&'b self, layer: Layer) -> &'b Vec<OutputMessage<'a>> {
        &self.custom_commands[layer as usize]
    }

    pub fn clear(&mut self) {
        self.ascii_texts.clear();
        self.utf8_texts.clear();
        self.headers.clear();
        for bucket in self.custom_commands.iter_mut() {
            bucket.clear();
        }
        self.numbers.clear();
        self.number_errors.clear();
        self.units.clear();
        self.operators.clear();
        self.variable.clear();
        self.line_ref_results.clear();
        self.pulses.clear();
        self.parenthesis.clear();
        self.clear_pulses = false;
    }

    pub fn set_color(&mut self, layer: Layer, color: u32) {
        self.custom_commands[layer as usize].push(OutputMessage::SetColor(color));
    }

    pub fn draw_rect(&mut self, layer: Layer, x: usize, y: CanvasY, w: usize, h: usize) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderRectangle { x, y, w, h });
    }

    pub fn draw_char(&mut self, layer: Layer, col: usize, row: CanvasY, char: char) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderChar(RenderChar {
            col,
            row,
            char,
        }));
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

    pub fn draw_underline(&mut self, layer: Layer, x: usize, y: CanvasY, w: usize) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderUnderline { x, y, w });
    }

    pub fn draw_string(&mut self, layer: Layer, x: usize, y: CanvasY, text: String) {
        self.custom_commands[layer as usize].push(OutputMessage::RenderString(RenderStringMsg {
            text: text.clone(),
            row: y,
            column: x,
        }));
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum ResultFormat {
    Bin,
    Dec,
    Hex,
}

#[derive(Clone, Debug)]
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

#[derive(Debug)]
pub struct MatrixEditing {
    pub editor_content: EditorContent<LineData>,
    pub editor: Editor<LineData>,
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

        let mut editor_content = EditorContent::new(32, 1);
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
        mat_edit.cell_strings.push(str);

        let cell_index = mat_edit.current_cell.row * col_count + mat_edit.current_cell.column;
        mat_edit
            .editor_content
            .init_with(&mat_edit.cell_strings[cell_index]);
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
        self.editor_content.init_with(new_content);

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
        theme: &Theme,
    ) -> usize {
        let vert_align_offset =
            (rendered_row_height - MatrixData::calc_render_height(self.row_count)) / 2;

        render_matrix_left_brackets(
            render_x + left_gutter_width,
            render_y,
            self.row_count,
            render_buckets,
            vert_align_offset,
            false,
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
                    render_buckets
                        .set_color(Layer::BehindTextAboveCursor, theme.matrix_edit_active_bg);
                    render_buckets.draw_rect(
                        Layer::BehindTextAboveCursor,
                        render_x + padding_x + left_gutter_width,
                        dst_y,
                        text_len,
                        1,
                    );
                    let chars = &self.editor_content.lines().next().unwrap();
                    render_buckets.set_color(Layer::Text, theme.matrix_edit_active_text);
                    for (i, char) in chars.iter().enumerate() {
                        render_buckets.draw_char(
                            Layer::Text,
                            render_x + padding_x + left_gutter_width + i,
                            dst_y,
                            *char,
                        );
                    }
                    let sel = self.editor.get_selection();
                    if let Some((first, second)) = sel.is_range_ordered() {
                        let len = second.column - first.column;
                        render_buckets
                            .set_color(Layer::BehindTextAboveCursor, theme.selection_color);
                        render_buckets.draw_rect(
                            Layer::BehindTextAboveCursor,
                            render_x + padding_x + left_gutter_width + first.column,
                            dst_y,
                            len,
                            1,
                        );
                    }
                } else {
                    let chars = &self.cell_strings[row_i * self.col_count + col_i];
                    render_buckets.set_color(Layer::Text, theme.matrix_edit_inactive_text);
                    render_buckets.draw_string(
                        Layer::Text,
                        render_x + padding_x + left_gutter_width,
                        dst_y,
                        (&chars[0..text_len]).to_owned(),
                    );
                }

                if self.current_cell == Pos::from_row_column(row_i, col_i)
                    && self.editor.is_cursor_shown()
                {
                    render_buckets.set_color(Layer::Text, theme.cursor);
                    render_buckets.draw_char(
                        Layer::Text,
                        (self.editor.get_selection().get_cursor_pos().column + left_gutter_width)
                            + render_x
                            + padding_x,
                        dst_y,
                        '▏',
                    );
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
            false,
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
    pub row: ContentIndex,
    pub start_x: usize,
    pub end_x: usize,
    pub rendered_x: usize,
    pub rendered_y: CanvasY,
    pub rendered_w: usize,
    pub rendered_h: usize,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: Box<[char]>,
    pub value: Result<CalcResult, ()>,
}

pub type LineResult = Result<Option<CalcResult>, EvalErr>;
pub type Variables = [Option<Variable>];
pub type FunctionDefinitions<'a> = [Option<FunctionDef<'a>>];

#[derive(Debug)]
pub struct Tokens<'a> {
    pub tokens: Vec<Token<'a>>,
    pub shunting_output_stack: Vec<ShuntingYardResult>,
}

pub enum MouseClickType {
    ClickedInEditor,
    ClickedInScrollBar {
        original_click_y: CanvasY,
        original_scroll_y: usize,
    },
    RightGutterIsDragged,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MouseHoverType {
    Normal,
    Scrollbar,
    RightGutter,
    Result,
}

#[derive(Debug)]
pub struct EditorObjId {
    content_index: ContentIndex,
    var_index: usize,
}

pub struct NoteCalcApp {
    pub result_panel_width_percent: usize,
    pub editor: Editor<LineData>,
    pub editor_content: EditorContent<LineData>,
    pub matrix_editing: Option<MatrixEditing>,
    pub line_reference_chooser: Option<ContentIndex>,
    pub line_id_generator: usize,
    pub mouse_state: Option<MouseClickType>,
    pub mouse_hover_type: MouseHoverType,
    pub updated_line_ref_obj_indices: Vec<EditorObjId>,
    pub render_data: GlobalRenderData,
    // when pressing Ctrl-c without any selection, the result of the current line will be put into this clipboard
    pub clipboard: Option<String>,
}

pub const EMPTY_FILE_DEFUALT_CONTENT: &str = "\n\n\n\n\n\n\n\n\n\n";

impl NoteCalcApp {
    pub fn new(client_width: usize, client_height: usize) -> NoteCalcApp {
        let mut editor_content = EditorContent::new(MAX_EDITOR_WIDTH, MAX_LINE_COUNT);
        NoteCalcApp {
            line_reference_chooser: None,
            result_panel_width_percent: DEFAULT_RESULT_PANEL_WIDTH_PERCENT,
            editor: Editor::new(&mut editor_content),
            editor_content,
            matrix_editing: None,
            line_id_generator: 1,
            mouse_state: None,
            mouse_hover_type: MouseHoverType::Normal,
            updated_line_ref_obj_indices: Vec::with_capacity(16),
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

    pub fn reset(&mut self) {
        self.line_reference_chooser = None;
        self.matrix_editing = None;
        self.line_id_generator = 1;
        self.mouse_hover_type = MouseHoverType::Normal;
        self.updated_line_ref_obj_indices.clear();
        self.clipboard = None;
        self.render_data = GlobalRenderData::new(
            self.render_data.client_width,
            self.render_data.client_height,
            default_result_gutter_x(self.render_data.client_width),
            LEFT_GUTTER_MIN_WIDTH,
            RIGHT_GUTTER_WIDTH,
        );
        self.editor.reset();
        self.editor_content.init_with("");
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
        } else if self.editor.get_selection().is_range() {
            self.editor_content
                .write_selection_into(self.editor.get_selection(), &mut str);
            Some(str)
        } else if !self.editor.clipboard.is_empty() {
            Some(std::mem::replace(&mut self.editor.clipboard, String::new()))
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
        func_defs: &mut FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        let content_is_empty = text.is_empty();
        if content_is_empty {
            text = EMPTY_FILE_DEFUALT_CONTENT;
        }
        let prev_line_count = self.editor_content.line_count();
        self.editor_content.init_with(text);
        self.editor.set_cursor_pos_r_c(0, 0);
        for (i, data) in self.editor_content.data_mut().iter_mut().enumerate() {
            data.line_id = i + 1;
        }
        self.line_id_generator = self.editor_content.line_count() + 1;

        self.matrix_editing = None;
        self.line_reference_chooser = None;
        self.mouse_state = None;
        self.mouse_hover_type = MouseHoverType::Normal;
        self.updated_line_ref_obj_indices.clear();
        self.clipboard = None;

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
        self.process_and_render_tokens(
            RowModificationType::AllLinesFrom(0),
            units,
            allocator,
            tokens,
            results,
            vars,
            func_defs,
            editor_objs,
            render_buckets,
            prev_line_count,
        );
        if !content_is_empty {
            self.set_editor_and_result_panel_widths_wrt_editor_and_rerender_if_necessary(
                units,
                render_buckets,
                allocator,
                _readonly_(tokens),
                _readonly_(results),
                _readonly_(vars),
                _readonly_(func_defs),
                editor_objs,
            );
        }
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
        editor: &mut Editor<LineData>,
        editor_content: &EditorContent<LineData>,
        units: &Units,
        matrix_editing: &mut Option<MatrixEditing>,
        line_reference_chooser: &mut Option<ContentIndex>,
        render_buckets: &mut RenderBuckets<'b>,
        result_change_flag: BitFlag256,
        gr: &mut GlobalRenderData,
        allocator: &'b Bump,
        apptokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        updated_line_ref_obj_indices: &[EditorObjId],
        mouse_hover_type: MouseHoverType,
    ) {
        let theme = &THEMES[gr.theme_index];
        gr.longest_visible_editor_line_len = 0;

        // result background
        render_buckets.result_panel_bg = Rect {
            x: (gr.result_gutter_x + RIGHT_GUTTER_WIDTH) as u16,
            y: 0,
            w: gr.current_result_panel_width as u16,
            h: gr.client_height as u16,
        };
        // result gutter
        render_buckets.right_gutter_bg = Rect {
            x: gr.result_gutter_x as u16,
            y: 0,
            w: RIGHT_GUTTER_WIDTH as u16,
            h: gr.client_height as u16,
        };
        // left gutter
        render_buckets.left_gutter_bg = Rect {
            x: 0,
            y: 0,
            w: gr.left_gutter_width as u16,
            h: gr.client_height as u16,
        };

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

                if let Some(tokens) = &apptokens[editor_y] {
                    // TODO: choose a better name
                    // it means that either we use the nice token rendering (e.g. for matrix it is the multiline matrix stuff),
                    // or render simply the backend content (e.g. for matrix it is [1;2;3]
                    let need_matrix_renderer =
                        if let Some((first, second)) = editor.get_selection().is_range_ordered() {
                            NOT((first.row..=second.row).contains(&(editor_y.as_usize())))
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
                        Some(RENDERED_RESULT_PRECISION),
                        theme,
                    );
                    highlight_line_ref_background(
                        &editor_objs[editor_y],
                        render_buckets,
                        &r,
                        gr,
                        theme,
                    );
                    if editor.get_selection().get_cursor_pos().row == r.editor_y.as_usize() {
                        underline_active_line_refs(&editor_objs[editor_y], render_buckets, gr);
                    }
                } else {
                    r.rendered_row_height = 1;
                    render_simple_text_line(line, &mut r, gr, render_buckets, allocator);
                }

                editor_y_to_render_w[r.editor_y.as_usize()] = r.render_x;

                draw_line_ref_chooser(
                    render_buckets,
                    &r,
                    &gr,
                    &line_reference_chooser,
                    gr.result_gutter_x,
                    theme,
                );

                draw_cursor(render_buckets, &r, &gr, &editor, &matrix_editing, theme);

                draw_right_gutter_num_prefixes(
                    render_buckets,
                    gr.result_gutter_x,
                    &editor_content,
                    &r,
                    theme,
                );

                render_wrap_dots(render_buckets, &r, &gr, theme);

                draw_right_gutter_num_prefixes(
                    render_buckets,
                    gr.result_gutter_x,
                    &editor_content,
                    &r,
                    theme,
                );
                r.line_render_ended(gr.get_rendered_height(editor_y));
                gr.longest_visible_editor_line_len =
                    gr.longest_visible_editor_line_len.max(r.render_x);
            }
            #[cfg(debug_assertions)]
            {
                let chars = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
                for i in 0..(gr.left_gutter_width
                    + (gr.current_editor_width + 1)
                    + RIGHT_GUTTER_WIDTH
                    + gr.current_result_panel_width)
                {
                    let d = i + 1;
                    render_buckets.draw_char(Layer::AboveText, i, canvas_y(0), chars[d % 10]);
                    if d % 10 == 0 && d < 100 {
                        render_buckets.draw_char(Layer::AboveText, i, canvas_y(1), chars[d / 10]);
                    }
                }
            }
        }

        let mut cursor_inside_function_body = false;
        // render function background fn bg
        {
            let cursor_row = editor.get_selection().get_cursor_pos().row;
            render_buckets.set_color(Layer::BehindTextBehindCursor, theme.func_bg);
            let mut i = gr.scroll_y;
            let to = (i + gr.client_height).min(MAX_LINE_COUNT);
            while i < to {
                if let Some(fd) = func_defs[i].as_ref() {
                    let mut rendered_h = 1;
                    for j in fd.first_row_index.add(1).as_usize()..=fd.last_row_index.as_usize() {
                        rendered_h += gr.get_rendered_height(content_y(j));
                    }
                    render_buckets.draw_rect(
                        Layer::BehindTextBehindCursor,
                        gr.left_gutter_width,
                        gr.get_render_y(content_y(i - gr.scroll_y)).expect("must"),
                        gr.current_editor_width,
                        rendered_h,
                    );

                    cursor_inside_function_body |= cursor_row >= fd.first_row_index.as_usize()
                        && cursor_row <= fd.last_row_index.as_usize();

                    i = fd.last_row_index.as_usize() + 1;
                } else {
                    i += 1;
                }
            }
        }

        highlight_current_line(
            render_buckets,
            editor,
            &gr,
            theme,
            cursor_inside_function_body,
        );

        // line numbers
        {
            let mut target_y = canvas_y(0);
            render_buckets.set_color(Layer::Text, theme.line_num_simple);
            for i in 0..gr.client_height {
                let y = gr.scroll_y + i;
                if y >= MAX_LINE_COUNT {
                    break;
                }
                if y == editor.get_selection().get_cursor_pos().row {
                    render_buckets.set_color(Layer::Text, theme.line_num_active);
                }
                let row_height = gr.get_rendered_height(content_y(y)).max(1);
                let vert_align_offset = (row_height - 1) / 2;
                let line_num_str = if y < 9 {
                    &(LINE_NUM_CONSTS[y][..])
                } else if y < 99 {
                    &(LINE_NUM_CONSTS2[y - 9][..])
                } else {
                    &(LINE_NUM_CONSTS3[y - 99][..])
                };
                render_buckets.draw_text(
                    Layer::Text,
                    0,
                    target_y.add(vert_align_offset),
                    line_num_str,
                );
                target_y = target_y.add(row_height);
                if y == editor.get_selection().get_cursor_pos().row {
                    render_buckets.set_color(Layer::Text, theme.line_num_simple);
                }
            }
        }

        draw_line_refs_and_vars_referenced_from_cur_row(
            &editor_objs[content_y(editor.get_selection().get_cursor_pos().row)],
            gr,
            render_buckets,
        );

        // TODO calc it once on content change (scroll_bar_h as well) (it is used in handle_drag)
        render_buckets.scroll_bar = if let Some(scrollbar_info) =
            NoteCalcApp::get_scrollbar_info(gr, editor_content.line_count())
        {
            let color = if mouse_hover_type == MouseHoverType::Scrollbar {
                theme.scrollbar_hovered
            } else {
                theme.scrollbar_normal
            };
            Some((
                color,
                Rect {
                    x: (gr.result_gutter_x - SCROLLBAR_WIDTH) as u16,
                    y: scrollbar_info.scroll_bar_render_y as u16,
                    w: SCROLLBAR_WIDTH as u16,
                    h: scrollbar_info.scroll_bar_render_h as u16,
                },
            ))
        } else {
            None
        };

        render_selection_and_its_sum(
            &units,
            render_buckets,
            results,
            &editor,
            &editor_content,
            &gr,
            vars,
            func_defs,
            allocator,
            theme,
            apptokens,
        );

        let mut tmp = ResultRender::new(ArrayVec::new());

        render_results_into_buf_and_calc_len(
            &units,
            results.as_slice(),
            &mut tmp,
            &editor_content,
            gr,
            Some(RENDERED_RESULT_PRECISION),
        );
        tmp.max_len = create_render_commands_for_results_and_render_matrices(
            &tmp,
            units,
            results.as_slice(),
            render_buckets,
            gr,
            Some(RENDERED_RESULT_PRECISION),
            theme,
        )
        .max(tmp.max_len);
        gr.longest_visible_result_len = tmp.max_len;

        pulse_changed_results(
            render_buckets,
            gr,
            gr.longest_visible_result_len,
            &result_change_flag,
            theme,
        );

        pulse_modified_line_references(
            render_buckets,
            gr,
            updated_line_ref_obj_indices,
            editor_objs,
            theme,
        );
    }

    pub fn handle_wheel<'b>(
        &mut self,
        dir: usize,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        render_buckets: &mut RenderBuckets<'b>,
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
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                editor_objs,
                BitFlag256::empty(),
            );
            self.set_editor_and_result_panel_widths_and_rerender_if_necessary(
                units,
                render_buckets,
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                editor_objs,
                BitFlag256::empty(),
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
        func_defs: &mut FunctionDefinitions<'b>,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        let scroll_bar_x = self.render_data.result_gutter_x - SCROLLBAR_WIDTH;
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
                func_defs,
                render_buckets,
                0,
            );
        } else if self.mouse_state.is_none() {
            self.mouse_state = if x - scroll_bar_x < SCROLLBAR_WIDTH {
                Some(MouseClickType::ClickedInScrollBar {
                    original_click_y: clicked_y,
                    original_scroll_y: self.render_data.scroll_y,
                })
            } else if x - self.render_data.result_gutter_x < RIGHT_GUTTER_WIDTH {
                Some(MouseClickType::RightGutterIsDragged)
            } else {
                // clicked in result
                if let Some(editor_y) = self.rendered_y_to_editor_y(clicked_y) {
                    self.insert_line_ref(
                        units,
                        allocator,
                        tokens,
                        results,
                        vars,
                        func_defs,
                        editor_y,
                        editor_objs,
                        render_buckets,
                    );
                }
                None
            };
        }
    }

    pub fn handle_mouse_up(&mut self) {
        match self.mouse_state {
            Some(MouseClickType::RightGutterIsDragged) => {}
            Some(MouseClickType::ClickedInEditor) => {}
            Some(MouseClickType::ClickedInScrollBar {
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
        func_defs: &mut FunctionDefinitions<'b>,
        render_buckets: &mut RenderBuckets<'b>,
        deep: usize,
    ) {
        if deep > 1 {
            return;
        }
        let clicked_x = x - self.render_data.left_gutter_width;
        let clicked_row = self.get_clicked_row_clamped(clicked_y);

        let previously_editing_matrix_row_index = if self.matrix_editing.is_some() {
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
            self.mouse_state = Some(MouseClickType::ClickedInEditor);
        }

        if let Some(matrix_row_index) = previously_editing_matrix_row_index {
            self.process_and_render_tokens(
                RowModificationType::SingleLine(matrix_row_index.as_usize()),
                units,
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                editor_objs,
                render_buckets,
                self.editor_content.line_count(),
            );
        } else {
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                editor_objs,
                BitFlag256::empty(),
            );
            // HACK
            // if there is a selection in a row which contains a matrix, the matrix is
            // rendered as simpletext, and clicking inside it put the cursor at the clicked position.
            // To identify that there is a matrix at that pos, we first have to render everything (above),
            // which fills the editor_objs, then call the click function again so now this click
            // will be registered as click into a matrix, and the mat_edit will be Some
            self.handle_editor_area_click(
                x,
                clicked_y,
                editor_objs,
                units,
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                render_buckets,
                deep + 1,
            );
        }
        self.editor_objs_referencing_current_line_might_changed(
            tokens,
            vars,
            editor_objs,
            render_buckets,
        );
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

    pub fn handle_mouse_move<'b>(
        &mut self,
        x: usize,
        _y: CanvasY,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        render_buckets: &mut RenderBuckets<'b>,
    ) -> usize {
        let scroll_bar_x = self.render_data.result_gutter_x - SCROLLBAR_WIDTH;
        let new_mouse_state = if x < self.render_data.left_gutter_width {
            MouseHoverType::Normal
        } else if x < scroll_bar_x {
            // editor
            MouseHoverType::Normal
        } else if (x as isize - scroll_bar_x as isize) < (SCROLLBAR_WIDTH as isize) {
            MouseHoverType::Scrollbar
        } else if (x as isize - self.render_data.result_gutter_x as isize)
            < (RIGHT_GUTTER_WIDTH as isize)
        {
            MouseHoverType::RightGutter
        } else {
            // result
            return MouseHoverType::Result as usize;
        };
        if self.mouse_hover_type != new_mouse_state {
            self.mouse_hover_type = new_mouse_state;
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                editor_objs,
                BitFlag256::empty(),
            );
        }
        return self.mouse_hover_type as usize;
    }

    pub fn handle_drag<'b>(
        &mut self,
        x: usize,
        y: CanvasY,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        render_buckets: &mut RenderBuckets<'b>,
    ) -> bool {
        let need_render = match self.mouse_state {
            Some(MouseClickType::RightGutterIsDragged) => {
                let x_bounded = x.max(self.render_data.left_gutter_width + 4);
                let client_width = self.render_data.client_width;
                let new_result_panel_width_percent =
                    (client_width - x_bounded) * 100 / client_width;
                self.result_panel_width_percent = new_result_panel_width_percent;
                set_editor_and_result_panel_widths(
                    client_width,
                    self.result_panel_width_percent,
                    &mut self.render_data,
                );
                true
            }
            Some(MouseClickType::ClickedInEditor) => {
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
            Some(MouseClickType::ClickedInScrollBar {
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
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                editor_objs,
                BitFlag256::empty(),
            );
            self.editor_objs_referencing_current_line_might_changed(
                tokens,
                vars,
                editor_objs,
                render_buckets,
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

    pub fn set_theme<'b>(
        &mut self,
        new_theme_index: usize,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        self.render_data.theme_index = new_theme_index;
        self.generate_render_commands_and_fill_editor_objs(
            units,
            render_buckets,
            allocator,
            tokens,
            results,
            vars,
            func_defs,
            editor_objs,
            BitFlag256::empty(),
        );
    }

    pub fn handle_resize<'b>(
        &mut self,
        new_client_width: usize,
        editor_objs: &mut EditorObjects,
        units: &Units,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        if new_client_width
            < LEFT_GUTTER_MIN_WIDTH + RIGHT_GUTTER_WIDTH + MIN_RESULT_PANEL_WIDTH + SCROLLBAR_WIDTH
        {
            return;
        }
        self.render_data.client_width = new_client_width;
        set_editor_and_result_panel_widths(
            new_client_width,
            self.result_panel_width_percent,
            &mut self.render_data,
        );
        self.generate_render_commands_and_fill_editor_objs(
            units,
            render_buckets,
            allocator,
            tokens,
            results,
            vars,
            func_defs,
            editor_objs,
            BitFlag256::empty(),
        );
    }

    pub fn handle_time<'b>(
        &mut self,
        now: u32,
        units: &Units,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
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
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                editor_objs,
                BitFlag256::empty(),
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
        let editor_content = &mut self.editor_content;
        for line_i in 0..editor_content.line_count() {
            let mut i = 0;
            'i: while i < editor_content.line_len(line_i) {
                let start = i;
                if i + 3 < editor_content.line_len(line_i)
                    && editor_content.get_char(line_i, i) == '&'
                    && editor_content.get_char(line_i, i + 1) == '['
                {
                    let mut end = i + 2;
                    let mut num_inside_lineref: u32 = 0;
                    while end < editor_content.line_len(line_i) {
                        if editor_content.get_char(line_i, end) == ']' && num_inside_lineref > 0 {
                            let num_len = end - (start + 2); // start --> &[num] <- end

                            // remove the number from the original line_ref text '&[x]' (remove only x)
                            {
                                let start_pos = Pos::from_row_column(line_i, start + 2);
                                let end_pos = start_pos.with_column(end);
                                self.editor.set_cursor_range(start_pos, end_pos);
                                self.editor.handle_input_no_undo(
                                    EditorInputEvent::Del,
                                    InputModifiers::none(),
                                    editor_content,
                                );
                            }
                            {
                                // which row has the id of 'num_inside_lineref'?
                                let referenced_row_index = editor_content
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
                                    self.editor.handle_input_no_undo(
                                        EditorInputEvent::Char(tmp_arr[tmp_arr_i]),
                                        InputModifiers::none(),
                                        editor_content,
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
                            editor_content.get_char(line_i, end).to_digit(10)
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
        for line_i in 0..editor_content.line_count() {
            editor_content.mut_data(line_i).line_id = line_i + 1;
        }
        self.line_id_generator = editor_content.line_count() + 1;

        self.editor.set_selection_save_col(original_selection);
    }

    pub fn alt_key_released<'b>(
        &mut self,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        func_defs: &mut FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        if let Some(line_ref_row) = self.line_reference_chooser {
            self.line_reference_chooser = None;
            self.insert_line_ref(
                units,
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                line_ref_row,
                editor_objs,
                render_buckets,
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
        func_defs: &mut FunctionDefinitions<'b>,
        line_ref_row: ContentIndex,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        let cursor_row = self.editor.get_selection().get_cursor_pos().row;
        let allowed_from_func_perspective = match (
            get_function_index_for_line(cursor_row, _readonly_(func_defs)),
            get_function_index_for_line(line_ref_row.as_usize(), _readonly_(func_defs)),
        ) {
            (Some(i), Some(j)) if i == j => {
                // they are in same func
                true
            }
            (Some(_), None) => {
                // insert lineref from global scope to function
                true
            }
            (None, None) => {
                // insert lineref from global scope to global scope
                true
            }
            _ => false,
        };
        if cursor_row == line_ref_row.as_usize()
            || matches!(&results[line_ref_row], Err(_) | Ok(None))
            || NOT(allowed_from_func_perspective)
        {
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                allocator,
                _readonly_(tokens),
                _readonly_(results),
                _readonly_(vars),
                _readonly_(func_defs),
                editor_objs,
                BitFlag256::empty(),
            );
            return;
        }
        if let Some(var) = &vars[line_ref_row.as_usize()] {
            let pos = self.editor.get_selection().get_cursor_pos();
            if pos.column > 0 {
                let prev_ch = self.editor_content.get_char(pos.row, pos.column - 1);
                let prev_token_is_lineref = pos.column > 3 /* smallest lineref is 4 char long '&[1]'*/
                        && self.editor_content.get_char(pos.row, pos.column - 1) == ']' && {
                        let mut i = (pos.column - 2) as isize;
                        while i > 1 {
                            if NOT(self.editor_content.get_char(pos.row, i as usize).is_ascii_digit()) {
                                break;
                            }
                            i -= 1;
                        }
                        i > 0
                            && self.editor_content.get_char(pos.row, i as usize) == '['
                            && self.editor_content.get_char(pos.row, (i - 1) as usize) == '&'
                    };
                if prev_ch.is_alphanumeric() || prev_ch == '_' || prev_token_is_lineref {
                    self.editor.handle_input_undoable(
                        EditorInputEvent::Char(' '),
                        InputModifiers::none(),
                        &mut self.editor_content,
                    );
                }
            }
            for ch in var.name.iter() {
                self.editor.handle_input_undoable(
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
                .insert_text_undoable(&inserting_text, &mut self.editor_content);
        }

        self.process_and_render_tokens(
            RowModificationType::SingleLine(cursor_row),
            units,
            allocator,
            tokens,
            results,
            vars,
            func_defs,
            editor_objs,
            render_buckets,
            self.editor_content.line_count(),
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
        func_defs: &mut FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        let prev_row = self.editor.get_selection().get_cursor_pos().row;
        let prev_line_count = self.editor_content.line_count();
        match self
            .editor
            .insert_text_undoable(&text, &mut self.editor_content)
        {
            Some(modif) => {
                if self.editor.get_selection().get_cursor_pos().row >= MAX_LINE_COUNT {
                    self.editor.set_cursor_pos_r_c(MAX_LINE_COUNT - 1, 0);
                }
                let cursor_pos = self.editor.get_selection().get_cursor_pos();
                let scroll_y =
                    get_scroll_y_after_cursor_movement(prev_row, cursor_pos.row, &self.render_data);
                if let Some(scroll_y) = scroll_y {
                    self.render_data.scroll_y = scroll_y;
                }
                self.process_and_render_tokens(
                    modif,
                    units,
                    allocator,
                    tokens,
                    results,
                    vars,
                    func_defs,
                    editor_objs,
                    render_buckets,
                    prev_line_count,
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
        func_defs: &mut FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        self.process_and_render_tokens(
            RowModificationType::AllLinesFrom(0),
            units,
            allocator,
            tokens,
            results,
            vars,
            func_defs,
            editor_objs,
            render_buckets,
            self.editor_content.line_count(),
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
        func_defs: &mut FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
    ) -> Option<RowModificationType> {
        let _span = tracy_span("handle_input", file!(), line!());
        fn handle_input_with_alt<'b>(
            app: &mut NoteCalcApp,
            input: EditorInputEvent,
        ) -> Option<RowModificationType> {
            if input == EditorInputEvent::Left {
                let selection = app.editor.get_selection();
                let (start, end) = selection.get_range_ordered();
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
                let (start, end) = selection.get_range_ordered();
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
        //
        let prev_line_count = self.editor_content.line_count();
        let prev_selection = self.editor.get_selection();
        let prev_row = self.editor.get_selection().get_cursor_pos().row;
        let mut refactor_me = false;
        let modif = if self.matrix_editing.is_none() && modifiers.alt {
            handle_input_with_alt(&mut *self, input)
        } else if self.is_matrix_editing_or_need_to_create_one(
            input,
            _readonly_(editor_objs),
            &mut refactor_me,
        ) {
            self.handle_matrix_editor_input(input, modifiers);
            if self.matrix_editing.is_none() {
                // user left a matrix
                // TODO sometimes leaving a matrix inserts too many chars in a row
                // so it overflows, so we have to reparse the consecutive lines as well.
                // The real solution for it is to allow "unlimited" line width
                //Some(RowModificationType::SingleLine(prev_row))
                Some(RowModificationType::AllLinesFrom(prev_row))
            } else {
                if modifiers.alt {
                    let y = content_y(prev_row);
                    let new_h =
                        calc_rendered_height(y, &self.matrix_editing, tokens, results, vars);
                    self.render_data.set_rendered_height(y, new_h);
                };
                None
            }
        } else if self.handle_completion(&input, editor_objs, _readonly_(vars)) {
            Some(RowModificationType::SingleLine(prev_row))
        } else if let Some(modif_type) = self.handle_obj_deletion(&input, editor_objs, modifiers) {
            Some(modif_type)
        } else if input == EditorInputEvent::Char('c')
            && modifiers.ctrl
            && !self.editor.get_selection().is_range()
        {
            let row = self.editor.get_selection().get_cursor_pos().row;
            if let Ok(Some(result)) = &results[content_y(row)] {
                self.clipboard = Some(render_result(
                    &units,
                    &result,
                    &self.editor_content.get_data(row).result_format,
                    false,
                    Some(RENDERED_RESULT_PRECISION),
                    true,
                ));
            }
            None
        } else if input == EditorInputEvent::Char('b') && modifiers.ctrl {
            self.handle_jump_to_definition(&input, modifiers, editor_objs);
            None
        } else if self.handle_obj_jump_over(&input, modifiers, editor_objs) {
            None
        } else if self.handle_parenthesis_wrapping(&input) {
            Some(RowModificationType::AllLinesFrom(
                prev_selection.get_first().row,
            ))
        } else if self.handle_parenthesis_removal(&input) {
            Some(RowModificationType::SingleLine(
                prev_selection.get_first().row,
            ))
        } else if self.handle_parenthesis_completion(&input) {
            Some(RowModificationType::SingleLine(
                prev_selection.get_first().row,
            ))
        } else {
            let prev_cursor_pos = prev_selection.get_cursor_pos();

            let modif_type =
                self.editor
                    .handle_input_undoable(input, modifiers, &mut self.editor_content);

            if self.editor.get_selection().get_cursor_pos().row >= MAX_LINE_COUNT {
                if let Some((start, _end)) = self.editor.get_selection().is_range_ordered() {
                    self.editor.set_selection_save_col(Selection::range(
                        start,
                        Pos::from_row_column(MAX_LINE_COUNT - 1, 0),
                    ));
                } else {
                    self.editor
                        .set_selection_save_col(Selection::single_r_c(MAX_LINE_COUNT - 1, 0));
                }
            }

            let modif_type = if refactor_me {
                Some(RowModificationType::AllLinesFrom(0))
            } else {
                modif_type
            };

            if modif_type.is_none() {
                // it is possible to step into a matrix only through navigation
                self.check_stepping_into_matrix(prev_cursor_pos, editor_objs);
                // if there was no selection but now there is
                if !prev_selection.is_range() && self.editor.get_selection().is_range() {
                    // since the content is modified, it is considered as modification
                    self.normalize_line_refs_in_place();
                    Some(RowModificationType::AllLinesFrom(0))
                } else {
                    modif_type
                }
            } else {
                modif_type
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
                func_defs,
                editor_objs,
                render_buckets,
                prev_line_count,
            );
        } else {
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                allocator,
                _readonly_(tokens),
                _readonly_(results),
                _readonly_(vars),
                _readonly_(func_defs),
                editor_objs,
                BitFlag256::empty(),
            );
            self.set_editor_and_result_panel_widths_and_rerender_if_necessary(
                units,
                render_buckets,
                allocator,
                _readonly_(tokens),
                _readonly_(results),
                _readonly_(vars),
                _readonly_(func_defs),
                editor_objs,
                BitFlag256::empty(),
            );
        }

        // TODO vec alloc
        if prev_row != cursor_pos.row || modif.is_some() {
            self.editor_objs_referencing_current_line_might_changed(
                tokens,
                vars,
                editor_objs,
                render_buckets,
            );
        }

        return modif;
    }

    pub fn editor_objs_referencing_current_line_might_changed<'b>(
        &mut self,
        tokens: &AppTokens<'b>,
        vars: &Variables,
        editor_objs: &EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
    ) {
        let mut editor_objs_referencing_current_line: Vec<EditorObjId> = Vec::with_capacity(8);
        render_buckets.clear_pulses = true;
        NoteCalcApp::fill_editor_objs_referencing_current_line(
            content_y(self.editor.get_selection().get_cursor_pos().row),
            tokens,
            vars,
            &mut editor_objs_referencing_current_line,
            &self.editor_content,
        );
        pulse_editor_objs_referencing_current_line(
            render_buckets,
            &self.render_data,
            &editor_objs_referencing_current_line,
            editor_objs,
            &THEMES[self.render_data.theme_index],
        );
    }

    pub fn process_and_render_tokens<'b>(
        &mut self,
        input_effect: RowModificationType,
        units: &Units,
        allocator: &'b Bump,
        tokens: &mut AppTokens<'b>,
        results: &mut Results,
        vars: &mut Variables,
        func_defs: &mut FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        render_buckets: &mut RenderBuckets<'b>,
        prev_line_count: usize,
    ) {
        let _span = tracy_span("process_and_render_tokens", file!(), line!());
        fn eval_line<'a>(
            editor_content: &EditorContent<LineData>,
            line: &[char],
            units: &Units,
            allocator: &'a Bump,
            apptokens: &mut AppTokens<'a>,
            results: &mut Results,
            vars: &mut Variables,
            func_defs: &FunctionDefinitions<'a>,
            editor_y: ContentIndex,
            updated_line_ref_obj_indices: &mut Vec<EditorObjId>,
            function_def_index: &Option<usize>,
            argument_dependend_lines: &mut BitFlag256,
        ) -> (bool, BitFlag256, Option<FunctionDef<'a>>) {
            let _span = tracy_span("eval_line", file!(), line!());

            debug_print(&format!("eval> {:?}", line));

            let func_param_count = if let Some(i) = function_def_index {
                func_defs[*i].as_ref().unwrap().param_count
            } else {
                0
            };

            // TODO optimize vec allocations
            let mut parsed_tokens = Vec::with_capacity(128);
            TokenParser::parse_line(
                line,
                &vars,
                &mut parsed_tokens,
                &units,
                editor_y.as_usize(),
                allocator,
                func_param_count,
                func_defs,
            );

            if let Some(mut fd) = try_extract_function_def(&mut parsed_tokens, allocator) {
                fd.first_row_index = editor_y;
                fd.last_row_index = editor_y;
                apptokens[editor_y] = Some(Tokens {
                    tokens: parsed_tokens,
                    shunting_output_stack: Vec::with_capacity(0),
                });
                for i in fd.param_count..MAX_FUNCTION_PARAM_COUNT {
                    vars[FIRST_FUNC_PARAM_VAR_INDEX + i] = None;
                }
                results[editor_y] = Ok(None);
                return (
                    true,
                    BitFlag256::all_rows_starting_at(editor_y.as_usize()),
                    Some(fd),
                );
            }

            // TODO: measure is 128 necessary? and remove allocation
            let mut shunting_output_stack = Vec::with_capacity(128);
            ShuntingYard::shunting_yard(
                &mut parsed_tokens,
                &mut shunting_output_stack,
                units,
                &func_defs[0..editor_y.as_usize()],
            );

            // TODO avoid clone
            let prev_var_name = vars[editor_y.as_usize()].as_ref().map(|it| it.name.clone());
            apptokens[editor_y] = Some(Tokens {
                tokens: parsed_tokens,
                shunting_output_stack,
            });

            let new_result = if apptokens[editor_y].is_some() {
                let result_depends_on_argument =
                    if let Some(function_def_index) = function_def_index {
                        fn determine_argument_dependend_lines<'a>(
                            fd: &FunctionDef<'a>,
                            y: usize,
                            all_lines_tokens: &AppTokens<'a>,
                            argument_dependend_lines: &mut BitFlag256,
                        ) -> bool {
                            if let Some(tokens) = &all_lines_tokens[content_y(y)] {
                                for t in &tokens.shunting_output_stack {
                                    match t.typ {
                                        TokenType::Variable { var_index } => {
                                            // the result of this line depends on the argument of the function, don't calc it now
                                            if var_index >= SUM_VARIABLE_INDEX
                                                || argument_dependend_lines.is_true(var_index)
                                            {
                                                argument_dependend_lines.set(y);
                                                return true;
                                            } else {
                                                // check if the variable is inside our function, and if its line
                                                // depends on the parameter (even indirectly)
                                                if var_index <= fd.last_row_index.as_usize()
                                                    && var_index > fd.first_row_index.as_usize()
                                                {
                                                    if determine_argument_dependend_lines(
                                                        fd,
                                                        var_index,
                                                        &all_lines_tokens,
                                                        argument_dependend_lines,
                                                    ) {
                                                        argument_dependend_lines.set(y);
                                                        return true;
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            return false;
                        }
                        let fd = func_defs[*function_def_index].as_ref().unwrap();
                        determine_argument_dependend_lines(
                            fd,
                            editor_y.as_usize(),
                            &apptokens,
                            argument_dependend_lines,
                        )
                    } else {
                        false
                    };
                if result_depends_on_argument {
                    let var_name =
                        get_var_name_from_assignment(editor_y.as_usize(), editor_content);
                    if !var_name.is_empty() {
                        debug_print(&format!(
                            "eval> register variable {:?} at {}",
                            var_name,
                            editor_y.as_usize()
                        ));
                        // just a Dummy value to register the variable name so following line
                        // in the function body can refer to it
                        vars[editor_y.as_usize()] = Some(Variable {
                            name: Box::from(var_name),
                            value: Err(()),
                        });
                    }
                    Ok(None)
                } else {
                    let (wrong_type_token_indices, result) = evaluate_tokens(
                        editor_y.as_usize(),
                        apptokens,
                        &vars,
                        &func_defs,
                        units,
                        editor_content,
                        0,
                        None,
                    );
                    for (i, token) in apptokens[editor_y]
                        .as_mut()
                        .unwrap()
                        .tokens
                        .iter_mut()
                        .enumerate()
                    {
                        if wrong_type_token_indices.is_true(i) {
                            debug_print(&format!(" calc> {:?} --> String", token));
                            token.typ = TokenType::StringLiteral;
                        }
                    }
                    if let Err(err) = result.as_ref() {
                        Token::set_token_error_flag_by_index(
                            err.token_index,
                            &mut apptokens[editor_y].as_mut().unwrap().tokens,
                        );
                        for i in &[
                            err.token_index_lhs_1,
                            err.token_index_lhs_2,
                            err.token_index_rhs_1,
                            err.token_index_rhs_2,
                        ] {
                            if let Some(i) = i {
                                Token::set_token_error_flag_by_index(
                                    *i,
                                    &mut apptokens[editor_y].as_mut().unwrap().tokens,
                                );
                            }
                        }
                    }

                    process_variable_assignment_or_line_ref(
                        &result,
                        vars,
                        editor_y.as_usize(),
                        editor_content,
                    );
                    let result = result.map(|it| it.map(|it| it.result));
                    result
                }
            } else {
                Ok(None)
            };
            let vars: &Variables = vars;

            let prev_result = std::mem::replace(&mut results[editor_y], new_result);
            let result_has_changed = {
                let new_result = &results[editor_y];
                function_def_index.is_some()
                    || match (&prev_result, new_result) {
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

            let mut rows_to_recalc = BitFlag256::empty();
            if result_has_changed {
                let line_ref_name =
                    NoteCalcApp::get_line_ref_name(&editor_content, editor_y.as_usize());
                rows_to_recalc.merge(NoteCalcApp::find_line_ref_dependant_lines(
                    &line_ref_name,
                    apptokens,
                    editor_y.as_usize(),
                    updated_line_ref_obj_indices,
                ));
            }

            let curr_var_name = vars[editor_y.as_usize()].as_ref().map(|it| &it.name);
            rows_to_recalc.merge(find_lines_that_affected_by_var_change(
                result_has_changed,
                curr_var_name,
                prev_var_name,
                apptokens,
                editor_y.as_usize(),
            ));

            rows_to_recalc.merge(find_sum_variable_name(apptokens, editor_y.as_usize()));
            return (result_has_changed, rows_to_recalc, None);
        }

        fn find_sum_variable_name(tokens_per_lines: &AppTokens, editor_y: usize) -> BitFlag256 {
            let mut rows_to_recalc = BitFlag256::empty();
            'outer: for (line_index, tokens) in
                tokens_per_lines.iter().skip(editor_y + 1).enumerate()
            {
                if let Some(tokens) = tokens {
                    for token in &tokens.tokens {
                        match token.typ {
                            TokenType::Header => {
                                break 'outer;
                            }
                            TokenType::Variable { var_index }
                                if var_index == SUM_VARIABLE_INDEX =>
                            {
                                rows_to_recalc
                                    .merge(BitFlag256::single_row(editor_y + 1 + line_index));
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
        ) -> BitFlag256 {
            let mut rows_to_recalc = BitFlag256::empty();
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
                                            .merge(BitFlag256::single_row(editor_y + 1 + i));
                                        break;
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
                                            .merge(BitFlag256::single_row(editor_y + 1 + i));
                                        break;
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
                                    rows_to_recalc.merge(BitFlag256::single_row(editor_y + 1 + i));
                                    break;
                                }
                            }
                        }
                    }
                }
                (Some(_old_var_name), Some(var_name)) => {
                    if !needs_dependency_check {
                        return BitFlag256::empty();
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
                                    rows_to_recalc.merge(BitFlag256::single_row(editor_y + 1 + i));
                                    break;
                                }
                            }
                        }
                    }
                }
                (None, None) => {}
            }
            return rows_to_recalc;
        }

        fn find_lines_that_affected_by_fn_change<'b>(
            needs_dependency_check: bool,
            curr_var_name: Option<&[char]>,
            prev_var_name: Option<&[char]>,
            tokens_per_lines: &AppTokens<'b>,
            editor_y: usize,
        ) -> BitFlag256 {
            let mut rows_to_recalc = BitFlag256::empty();
            match (prev_var_name, curr_var_name) {
                (None, Some(var_name)) => {
                    // nem volt még, de most van
                    // recalc all the rows which uses this variable name
                    for (i, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
                        if let Some(tokens) = tokens {
                            for token in &tokens.tokens {
                                match token.typ {
                                    TokenType::StringLiteral if *token.ptr == *var_name => {
                                        rows_to_recalc
                                            .merge(BitFlag256::single_row(editor_y + 1 + i));
                                        break;
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
                                    TokenType::Operator(OperatorTokenType::Fn {
                                        typ: FnType::UserDefined(_),
                                        ..
                                    }) if *token.ptr == *old_var_name => {
                                        rows_to_recalc
                                            .merge(BitFlag256::single_row(editor_y + 1 + i));
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                (Some(old_var_name), Some(var_name)) if old_var_name != var_name => {
                    // volt, de most más a neve
                    for (i, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
                        if let Some(tokens) = tokens {
                            for token in &tokens.tokens {
                                let recalc = match token.typ {
                                    TokenType::StringLiteral => var_name.starts_with(token.ptr),
                                    TokenType::Operator(OperatorTokenType::Fn {
                                        typ: FnType::UserDefined(_),
                                        ..
                                    }) => *token.ptr == *old_var_name,
                                    _ => false,
                                };
                                if recalc {
                                    rows_to_recalc.merge(BitFlag256::single_row(editor_y + 1 + i));
                                    break;
                                }
                            }
                        }
                    }
                }
                (Some(_old_var_name), Some(var_name)) => {
                    if !needs_dependency_check {
                        return BitFlag256::empty();
                    }
                    // volt is, van is, a neve is ugyanaz
                    for (i, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
                        if let Some(tokens) = tokens {
                            for token in &tokens.tokens {
                                let recalc = match token.typ {
                                    TokenType::Operator(OperatorTokenType::Fn {
                                        typ: FnType::UserDefined(_),
                                        ..
                                    }) if *token.ptr == *var_name => true,
                                    _ => false,
                                };
                                if recalc {
                                    rows_to_recalc.merge(BitFlag256::single_row(editor_y + 1 + i));
                                    break;
                                }
                            }
                        }
                    }
                }
                (None, None) => {}
            }
            return rows_to_recalc;
        }

        if matches!(input_effect, RowModificationType::AllLinesFrom(_)) {
            let curr_line_count = self.editor_content.line_count();
            for i in curr_line_count..prev_line_count.min(MAX_LINE_COUNT) {
                func_defs[i] = None;
            }
        }

        let mut sum_is_null = true;
        let mut dependant_rows = BitFlag256::empty();
        let mut result_change_flag = BitFlag256::empty();
        // HACK: currently this flag is here, but it makes the parsing
        // depends on previous parsing results, which works now
        // because a change in a function causes the whole function
        // to be reparsed,
        let mut argument_dependend_lines = BitFlag256::empty();

        // the index of the FunctionDef whose body is currently processed
        let mut function_def_index: Option<usize> = None;

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
                if let Some(fd_index) = function_def_index {
                    let fd = func_defs[fd_index].as_mut().unwrap();

                    let line_is_empty = self.editor_content.line_len(editor_y) == 0;
                    let can_be_func_body = line_is_empty
                        || self
                            .editor_content
                            .get_char(editor_y, 0)
                            .is_ascii_whitespace();
                    if (fd.last_row_index.as_usize() >= editor_y && !can_be_func_body)
                        || (fd.last_row_index.as_usize() == editor_y && line_is_empty)
                    {
                        // this line was part of the function but it is not anymore
                        fd.last_row_index = content_y(editor_y - 1);
                        function_def_index = None;
                        {
                            let fd = func_defs[fd_index].as_ref().unwrap();
                            dependant_rows.merge(find_lines_that_affected_by_fn_change(
                                true,
                                Some(fd.func_name), // TODO
                                Some(fd.func_name),
                                _readonly_(tokens),
                                editor_y,
                            ));
                        }
                    } else if fd.last_row_index.as_usize() < editor_y
                        && can_be_func_body
                        && !line_is_empty
                    {
                        // was not part of the function but it is now
                        fd.last_row_index = content_y(editor_y);
                        // recalc everything since this change could connect the next lines to the
                        // function header
                        // TODO: investigate if you could optimize it (e.g. avoid unnecessary func-call dependency checks later"
                        dependant_rows.merge(BitFlag256::all_rows_starting_at(editor_y + 1));
                    } else if can_be_func_body {
                        // was part of the function and still it is
                    } else {
                        function_def_index = None;
                        // it was not part of the function neither now it is
                    }
                }

                if self.editor_content.get_data(editor_y).line_id == 0 {
                    self.editor_content.mut_data(editor_y).line_id = self.line_id_generator;
                    self.line_id_generator += 1;
                }
                let y = content_y(editor_y);

                let (result_has_changed, rows_to_recalc, func_def) = eval_line(
                    &self.editor_content,
                    self.editor_content.get_line_valid_chars(editor_y),
                    units,
                    allocator,
                    tokens,
                    results,
                    &mut *vars,
                    func_defs,
                    y,
                    &mut self.updated_line_ref_obj_indices,
                    &function_def_index,
                    &mut argument_dependend_lines,
                );
                if let Some(fd) = func_def {
                    // a new function has been defined in the current row
                    func_defs[editor_y] = Some(fd);

                    // close the previous function if any
                    if let Some(prev_fd_index) = function_def_index {
                        func_defs[prev_fd_index].as_mut().unwrap().last_row_index =
                            content_y(editor_y - 1);
                    }
                } else {
                    func_defs[editor_y] = None;
                }
                if result_has_changed {
                    result_change_flag.merge(BitFlag256::single_row(editor_y));
                    if let Some(fd_i) = function_def_index {
                        let fd = func_defs[fd_i].as_ref().unwrap();
                        dependant_rows.merge(find_lines_that_affected_by_fn_change(
                            true,
                            Some(fd.func_name), // TODO
                            Some(fd.func_name),
                            _readonly_(tokens),
                            editor_y,
                        ));
                    }
                }
                dependant_rows.merge(rows_to_recalc);
                let new_h = calc_rendered_height(y, &self.matrix_editing, tokens, results, vars);
                self.render_data.set_rendered_height(y, new_h);
            }

            if let Some(fd) = &func_defs[editor_y] {
                function_def_index = Some(editor_y);
                // set function parameters as local Variables
                for i in 0..fd.param_count {
                    // TODO: absolutely not, it would mean an alloc in hot path
                    vars[FIRST_FUNC_PARAM_VAR_INDEX + i] = Some(Variable {
                        name: Box::from(fd.param_names[i]),
                        value: Err(()),
                    })
                }
            } else if let Some(fdi) = function_def_index {
                if func_defs[fdi].as_ref().expect("must").last_row_index < content_y(editor_y) {
                    function_def_index = None;
                }
            }

            let function_def_index = function_def_index;
            if self
                .editor_content
                .get_line_valid_chars(editor_y)
                .starts_with(&['#'])
            {
                vars[SUM_VARIABLE_INDEX] = Some(Variable {
                    name: Box::from(&['s', 'u', 'm'][..]),
                    value: Err(()),
                });
                sum_is_null = true;
            } else if function_def_index.is_none() {
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
        }

        if self.editor_content.line_count() > 99 {
            self.render_data
                .set_left_gutter_width(LEFT_GUTTER_MIN_WIDTH + 2);
        } else if self.editor_content.line_count() > 9 {
            self.render_data
                .set_left_gutter_width(LEFT_GUTTER_MIN_WIDTH + 1);
        } else {
            self.render_data
                .set_left_gutter_width(LEFT_GUTTER_MIN_WIDTH);
        }

        self.generate_render_commands_and_fill_editor_objs(
            units,
            render_buckets,
            allocator,
            _readonly_(tokens),
            _readonly_(results),
            _readonly_(vars),
            _readonly_(func_defs),
            editor_objs,
            result_change_flag,
        );
        self.set_editor_and_result_panel_widths_and_rerender_if_necessary(
            units,
            render_buckets,
            allocator,
            _readonly_(tokens),
            _readonly_(results),
            _readonly_(vars),
            _readonly_(func_defs),
            editor_objs,
            result_change_flag,
        );
    }

    fn set_editor_and_result_panel_widths_wrt_editor_and_rerender_if_necessary<'b>(
        &mut self,
        units: &Units,
        render_buckets: &mut RenderBuckets<'b>,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
    ) {
        let minimum_required_space_for_editor =
            self.render_data.longest_visible_editor_line_len.max(20);

        let desired_gutter_x = self.render_data.left_gutter_width +
                minimum_required_space_for_editor + 1 /*scrollbar*/;
        if desired_gutter_x < self.render_data.result_gutter_x {
            let client_width = self.render_data.client_width;
            self.result_panel_width_percent =
                (client_width - desired_gutter_x) * 100 / client_width;
            self.set_editor_and_result_panel_widths_and_rerender_if_necessary(
                units,
                render_buckets,
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                editor_objs,
                BitFlag256::empty(),
            );
        }
    }

    fn set_editor_and_result_panel_widths_and_rerender_if_necessary<'b>(
        &mut self,
        units: &Units,
        render_buckets: &mut RenderBuckets<'b>,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        result_change_flag: BitFlag256,
    ) {
        let current_result_g_x = self.render_data.result_gutter_x;
        set_editor_and_result_panel_widths(
            self.render_data.client_width,
            self.result_panel_width_percent,
            &mut self.render_data,
        );
        if self.render_data.result_gutter_x != current_result_g_x {
            // HACKY
            // we know the length of each line and results only after rendering them,
            // so we can set the right gutter position only after a full render pass.
            // If it turns out that the gutter has to/and can be moved,
            // we rerender everything with the new gutter position.
            self.generate_render_commands_and_fill_editor_objs(
                units,
                render_buckets,
                allocator,
                tokens,
                results,
                vars,
                func_defs,
                editor_objs,
                result_change_flag,
            );
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
    ) -> BitFlag256 {
        let mut rows_to_recalc = BitFlag256::empty();
        for (token_line_index, tokens) in tokens_per_lines.iter().skip(editor_y + 1).enumerate() {
            if let Some(tokens) = tokens {
                let mut already_added = BitFlag256::empty();
                for token in &tokens.tokens {
                    let var_index = match token.typ {
                        TokenType::LineReference { var_index }
                            if already_added.is_false(var_index)
                                && token.ptr == editor_obj_name =>
                        {
                            var_index
                        }
                        TokenType::Variable { var_index }
                            if var_index <= SUM_VARIABLE_INDEX
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
                    rows_to_recalc.merge(BitFlag256::single_row(index));
                    already_added.set(var_index);
                }
            } else {
                break;
            }
        }
        return rows_to_recalc;
    }

    // export
    pub fn copy_selected_rows_with_result_to_clipboard<'b>(
        &'b mut self,
        units: &'b Units,
        render_buckets: &'b mut RenderBuckets<'b>,
        tokens: &AppTokens<'b>,
        vars: &Variables,
        results: &Results,
    ) -> String {
        render_buckets.clear();
        let theme = &THEMES[self.render_data.theme_index];
        let (first_row, second_row) =
            if let Some((start, end)) = self.editor.get_selection().is_range_ordered() {
                (start.row, end.row)
            } else {
                (0, self.editor_content.line_count() - 1)
            };
        let row_nums = second_row - first_row + 1;

        let mut gr = GlobalRenderData::new(1024, 1000 /*dummy value*/, 1024 / 2, 0, 2);
        // evaluate all the lines so variables are defined even if they are not selected
        let mut render_height = 0;
        {
            let mut r = PerLineRenderData::new();
            for i in first_row..=second_row {
                let i = content_y(i);
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
                        theme,
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

        let mut tmp = ResultRender::new(ArrayVec::new());

        gr.result_gutter_x = max_len + 2;
        render_results_into_buf_and_calc_len(
            &units,
            &results.as_slice()[first_row..=second_row],
            &mut tmp,
            &self.editor_content,
            &gr,
            None,
        );
        gr.longest_visible_result_len = tmp.max_len;

        create_render_commands_for_results_and_render_matrices(
            &tmp,
            units,
            &results.as_slice()[first_row..=second_row],
            render_buckets,
            &gr,
            None,
            theme,
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

    fn is_matrix_editing_or_need_to_create_one<'b>(
        &mut self,
        input: EditorInputEvent,
        editor_objs: &EditorObjects,
        refactor_me: &mut bool,
    ) -> bool {
        // either there is an active matrix editing, or the cursor is inside a matrix and
        // we have to create the active matrix editing then
        let prev_selection = self.editor.get_selection();
        let prev_cursor_pos = prev_selection.get_cursor_pos();
        if let Some(matrix_edit) = &self.matrix_editing {
            if matrix_edit.row_count == 1
                && !matrix_edit.editor.get_selection().is_range()
                && matrix_edit.editor.is_cursor_at_beginning()
                && matrix_edit.current_cell.column == 0
                && input == EditorInputEvent::Backspace
            {
                let row = matrix_edit.row_index.as_usize();
                let col = matrix_edit.start_text_index;
                end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    Some(Pos::from_row_column(row, col + 1)),
                );
                // let the outer code remove the '['
                *refactor_me = true;
                return false;
            } else if matrix_edit.row_count == 1
                && !matrix_edit.editor.get_selection().is_range()
                && matrix_edit
                    .editor
                    .is_cursor_at_eol(&matrix_edit.editor_content)
                && input == EditorInputEvent::Del
            {
                end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    None,
                );
                self.editor.set_selection_save_col(Selection::single(
                    self.editor.get_selection().get_cursor_pos().with_prev_col(),
                ));
                // let the outer code remove the ']'
                *refactor_me = true;
                return false;
            }
            return true;
        } else if let Some(editor_obj) = self.get_obj_at_inside(
            prev_cursor_pos.column,
            content_y(prev_cursor_pos.row),
            editor_objs,
        ) {
            match editor_obj.typ {
                EditorObjectType::Matrix {
                    col_count,
                    row_count,
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
                    return true;
                }
                _ => {}
            }
        }
        return false;
    }

    fn handle_parenthesis_completion<'b>(&mut self, input: &EditorInputEvent) -> bool {
        let closing_char = match input {
            EditorInputEvent::Char('(') => Some(')'),
            EditorInputEvent::Char('[') => Some(']'),
            EditorInputEvent::Char('{') => Some('}'),
            EditorInputEvent::Char('\"') => Some('\"'),
            _ => None,
        };
        if let Some(closing_char) = closing_char {
            if self
                .editor
                .handle_input_undoable(*input, InputModifiers::none(), &mut self.editor_content)
                .is_some()
            {
                let cursor = self.editor.get_selection().get_cursor_pos();
                let next_char = self.editor_content.get_char(cursor.row, cursor.column);
                let closing_paren_allowed = next_char.is_whitespace()
                    || next_char == ')'
                    || next_char == ']'
                    || next_char == '}'
                    || self.editor.is_cursor_at_eol(&self.editor_content);
                if !closing_paren_allowed {
                    return true;
                }

                if self
                    .editor
                    .handle_input_undoable(
                        EditorInputEvent::Char(closing_char),
                        InputModifiers::none(),
                        &mut self.editor_content,
                    )
                    .is_some()
                {
                    self.editor.handle_input_undoable(
                        EditorInputEvent::Left,
                        InputModifiers::none(),
                        &mut self.editor_content,
                    );
                    if closing_char == ']' {
                        self.matrix_editing = Some(MatrixEditing::new(
                            1,
                            1,
                            &[],
                            content_y(cursor.row),
                            cursor.column - 1,
                            cursor.column + 1,
                            Pos::from_row_column(0, 0),
                        ));
                    }
                }
                return true;
            }
        }
        return false;
    }

    fn handle_parenthesis_removal<'b>(&mut self, input: &EditorInputEvent) -> bool {
        let prev_selection = self.editor.get_selection();
        if prev_selection.is_range() {
            return false;
        }
        let prev_cursor_pos = prev_selection.get_cursor_pos();
        let char_in_front_of_cursur = if prev_cursor_pos.column > 0 {
            Some(
                self.editor_content
                    .get_char(prev_cursor_pos.row, prev_cursor_pos.column - 1),
            )
        } else {
            None
        };
        if *input == EditorInputEvent::Backspace {
            let closing_char = match char_in_front_of_cursur {
                Some('(') => Some(')'),
                Some('[') => Some(']'),
                Some('{') => Some('}'),
                Some('\"') => Some('\"'),
                _ => None,
            };
            if let Some(closing_char) = closing_char {
                let cursor_pos = self.editor.get_selection().get_cursor_pos();
                if !self.editor.is_cursor_at_eol(&self.editor_content)
                    && self
                        .editor_content
                        .get_char(cursor_pos.row, cursor_pos.column)
                        == closing_char
                {
                    if self
                        .editor
                        .handle_input_undoable(
                            EditorInputEvent::Backspace,
                            InputModifiers::none(),
                            &mut self.editor_content,
                        )
                        .is_some()
                    {
                        self.editor.handle_input_undoable(
                            EditorInputEvent::Del,
                            InputModifiers::none(),
                            &mut self.editor_content,
                        );
                        return true;
                    }
                }
            }
        }
        return false;
    }

    fn handle_parenthesis_wrapping<'b>(&mut self, input: &EditorInputEvent) -> bool {
        let prev_selection = self.editor.get_selection();
        if let Some((start, end)) = prev_selection.is_range_ordered() {
            let single_line = start.row == end.row;
            let closing_char = match input {
                EditorInputEvent::Char('(') => Some(')'),
                EditorInputEvent::Char('[') => Some(']'),
                EditorInputEvent::Char('{') => Some('}'),
                EditorInputEvent::Char('\"') => Some('\"'),
                _ => None,
            };
            if let Some(closing_char) = closing_char {
                // end selection and insert closing char
                self.editor.set_cursor_pos(end);
                if self
                    .editor
                    .handle_input_undoable(
                        EditorInputEvent::Char(closing_char),
                        InputModifiers::none(),
                        &mut self.editor_content,
                    )
                    .is_some()
                {
                    // insert opening char
                    self.editor.set_cursor_pos(start);
                    if self
                        .editor
                        .handle_input_undoable(
                            *input,
                            InputModifiers::none(),
                            &mut self.editor_content,
                        )
                        .is_some()
                    {
                        // restore selection
                        let (real_start, real_end) = prev_selection.get_range();
                        if start != real_start {
                            // the selection is backwards
                            self.editor.set_selection_save_col(Selection::range(
                                if single_line {
                                    real_start.with_next_col()
                                } else {
                                    real_start
                                },
                                real_end.with_next_col(),
                            ));
                        } else {
                            self.editor.set_selection_save_col(Selection::range(
                                start.with_next_col(),
                                if single_line {
                                    end.with_next_col()
                                } else {
                                    end
                                },
                            ));
                        }
                    }
                    return true;
                }
            }
        }
        return false;
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
                self.editor.handle_input_undoable(
                    EditorInputEvent::Backspace,
                    InputModifiers::none(),
                    &mut self.editor_content,
                );
                for ch in autocompl_const.replace_to {
                    self.editor.handle_input_undoable(
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
                    self.check_stepping_into_matrix(
                        Pos::from_row_column(0, 0),
                        _readonly_(editor_objects),
                    );
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
                self.editor.handle_input_undoable(
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
        modifiers: InputModifiers,
    ) -> Option<RowModificationType> {
        let selection = self.editor.get_selection();
        let cursor_pos = selection.get_cursor_pos();
        if *input == EditorInputEvent::Backspace
            && !selection.is_range()
            && selection.start.column > 0
        {
            if let Some(index) = if modifiers.ctrl {
                self.index_of_matrix_or_lineref_at(
                    cursor_pos.with_prev_col(),
                    _readonly_(editor_objects),
                )
            } else {
                self.index_of_lineref_at(cursor_pos.with_prev_col(), _readonly_(editor_objects))
            } {
                // remove it
                let obj = editor_objects[content_y(cursor_pos.row)].remove(index);
                let sel = Selection::range(
                    Pos::from_row_column(obj.row.as_usize(), obj.start_x),
                    Pos::from_row_column(obj.row.as_usize(), obj.end_x),
                );
                self.editor.set_selection_save_col(sel);
                self.editor.handle_input_undoable(
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
        } else if *input == EditorInputEvent::Del && !selection.is_range() {
            if let Some(index) = if modifiers.ctrl {
                self.index_of_matrix_or_lineref_at(cursor_pos, _readonly_(editor_objects))
            } else {
                self.index_of_lineref_at(cursor_pos, _readonly_(editor_objects))
            } {
                // remove it
                let obj = editor_objects[content_y(cursor_pos.row)].remove(index);
                let sel = Selection::range(
                    Pos::from_row_column(obj.row.as_usize(), obj.start_x),
                    Pos::from_row_column(obj.row.as_usize(), obj.end_x),
                );
                self.editor.set_selection_save_col(sel);
                self.editor.handle_input_undoable(
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
            && !selection.is_range()
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
            && !selection.is_range()
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
                    if self.matrix_editing.is_none() && !self.editor.get_selection().is_range() {
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

    #[allow(dead_code)]
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

    fn index_of_lineref_at<'b>(&self, pos: Pos, editor_objects: &EditorObjects) -> Option<usize> {
        return editor_objects[content_y(pos.row)].iter().position(|obj| {
            matches!(obj.typ, EditorObjectType::LineReference{..})
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
            if let Some((_from, to)) = mat_edit.editor.get_selection().is_range_ordered() {
                mat_edit.editor.set_cursor_pos_r_c(0, to.column);
            } else if mat_edit.current_cell.column + 1 < mat_edit.col_count {
                mat_edit.move_to_cell(mat_edit.current_cell.with_next_col());
            } else {
                end_matrix_editing(
                    &mut self.matrix_editing,
                    &mut self.editor,
                    &mut self.editor_content,
                    None,
                );
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
                    .handle_input_undoable(input, modifiers, &mut self.editor_content);
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
                    .handle_input_undoable(input, modifiers, &mut self.editor_content);
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
                    .handle_input_undoable(input, modifiers, &mut self.editor_content);
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
                    .handle_input_undoable(input, modifiers, &mut self.editor_content);
            }
        } else {
            mat_edit
                .editor
                .handle_input_undoable(input, modifiers, &mut mat_edit.editor_content);
        }
    }

    pub fn generate_render_commands_and_fill_editor_objs<'b>(
        &mut self,
        units: &Units,
        render_buckets: &mut RenderBuckets<'b>,
        allocator: &'b Bump,
        tokens: &AppTokens<'b>,
        results: &Results,
        vars: &Variables,
        func_defs: &FunctionDefinitions<'b>,
        editor_objs: &mut EditorObjects,
        result_change_flag: BitFlag256,
    ) {
        let _span = tracy_span(
            "generate_render_commands_and_fill_editor_objs",
            file!(),
            line!(),
        );
        render_buckets.clear();
        NoteCalcApp::renderr(
            &mut self.editor,
            &self.editor_content,
            units,
            &mut self.matrix_editing,
            &mut self.line_reference_chooser,
            render_buckets,
            result_change_flag,
            &mut self.render_data,
            allocator,
            tokens,
            results,
            vars,
            func_defs,
            editor_objs,
            &self.updated_line_ref_obj_indices,
            self.mouse_hover_type,
        );
        self.updated_line_ref_obj_indices.clear();
    }
}

#[derive(Debug, Default)]
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
    editor: &Editor<LineData>,
    matrix_editing: &Option<MatrixEditing>,
    theme: &Theme,
) {
    let cursor_pos = editor.get_selection().get_cursor_pos();
    if cursor_pos.row == r.editor_y.as_usize() {
        render_buckets.set_color(Layer::AboveText, theme.cursor);
        if editor.is_cursor_shown()
            && matrix_editing.is_none()
            && ((cursor_pos.column as isize + r.cursor_render_x_offset) as usize)
                <= gr.current_editor_width
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
    theme: &Theme,
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
                    render_buckets.pulses.push(PulsingRectangle {
                        x: gr.left_gutter_width + *rendered_x,
                        y: *rendered_y,
                        w: *rendered_w,
                        h: *rendered_h,
                        start_color: theme.change_result_pulse_start,
                        end_color: theme.change_result_pulse_end,
                        animation_time: Duration::from_millis(2000),
                        repeat: false,
                    });
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
    theme: &Theme,
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
                    let end_editor_x = gr.current_editor_width + gr.left_gutter_width + 1;
                    if gr.is_visible(ed_obj.row) {
                        let rendered_row_height = gr.get_rendered_height(ed_obj.row);
                        let vert_align_offset = (rendered_row_height - ed_obj.rendered_h) / 2;
                        let obj_start_x =
                            (gr.left_gutter_width + ed_obj.rendered_x).min(end_editor_x - 1);
                        let obj_end_x = (obj_start_x + ed_obj.rendered_w).min(end_editor_x);
                        render_buckets.pulses.push(PulsingRectangle {
                            x: obj_start_x,
                            y: ed_obj.rendered_y.add(vert_align_offset),
                            w: obj_end_x - obj_start_x,
                            h: ed_obj.rendered_h,
                            start_color: theme.reference_pulse_start,
                            end_color: theme.reference_pulse_end,
                            animation_time: Duration::from_millis(1000),
                            repeat: true,
                        });
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
    result_change_flag: &BitFlag256,
    theme: &Theme,
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
                render_buckets.pulses.push(PulsingRectangle {
                    x: gr.result_gutter_x + RIGHT_GUTTER_WIDTH,
                    y: render_y,
                    w: longest_rendered_result_len,
                    h: gr.get_rendered_height(content_y(i)),
                    start_color: theme.change_result_pulse_start,
                    end_color: theme.change_result_pulse_end,
                    animation_time: Duration::from_millis(1000),
                    repeat: false,
                });
            }
        }
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
fn highlight_line_ref_background<'text_ptr>(
    editor_objs: &Vec<EditorObject>,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    r: &PerLineRenderData,
    gr: &GlobalRenderData,
    theme: &Theme,
) {
    for editor_obj in editor_objs.iter() {
        if matches!(editor_obj.typ, EditorObjectType::LineReference{..}) {
            let start_render_x = gr.left_gutter_width + editor_obj.rendered_x;
            let allowed_end_x =
                (start_render_x + editor_obj.rendered_w).min(gr.result_gutter_x - 1);
            let width = allowed_end_x as isize - start_render_x as isize;
            if width > 0 {
                let vert_align_offset = (r.rendered_row_height - editor_obj.rendered_h) / 2;
                render_buckets.set_color(Layer::BehindTextAboveCursor, theme.line_ref_bg);
                render_buckets.draw_rect(
                    Layer::BehindTextAboveCursor,
                    start_render_x,
                    editor_obj.rendered_y.add(vert_align_offset),
                    width as usize,
                    editor_obj.rendered_h,
                );
            }
        }
    }
}

#[inline]
fn underline_active_line_refs<'text_ptr>(
    editor_objs: &[EditorObject],
    render_buckets: &mut RenderBuckets<'text_ptr>,
    gr: &GlobalRenderData,
) {
    let mut color_index = 0;
    let mut colors: [Option<u32>; MAX_CLIENT_HEIGHT] = [None; MAX_CLIENT_HEIGHT];
    for editor_obj in editor_objs.iter() {
        match editor_obj.typ {
            EditorObjectType::LineReference { var_index }
            | EditorObjectType::Variable { var_index }
                if var_index < SUM_VARIABLE_INDEX =>
            {
                let color = if let Some(color) = colors[var_index] {
                    color
                } else {
                    let color = ACTIVE_LINE_REF_HIGHLIGHT_COLORS[color_index];
                    colors[var_index] = Some(color);
                    color_index = if color_index < 8 { color_index + 1 } else { 0 };
                    color
                };

                let start_render_x = gr.left_gutter_width + editor_obj.rendered_x;
                let allowed_end_x =
                    (start_render_x + editor_obj.rendered_w).min(gr.result_gutter_x - 1);
                let width = allowed_end_x as isize - start_render_x as isize;
                if width > 0 {
                    render_buckets.set_color(Layer::Text, color);
                    render_buckets.draw_underline(
                        Layer::Text,
                        start_render_x,
                        editor_obj.rendered_y.add(editor_obj.rendered_h - 1),
                        width as usize,
                    );
                }
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
    editor: &Editor<LineData>,
    matrix_editing: &Option<MatrixEditing>,
    vars: &Variables,
    units: &Units,
    need_matrix_renderer: bool,
    decimal_count: Option<usize>,
    theme: &Theme,
) {
    editor_objects.clear();
    let cursor_pos = editor.get_selection().get_cursor_pos();

    let parenthesis_around_cursor = find_parentesis_around_cursor(tokens, r, cursor_pos);

    let mut token_index = 0;
    while token_index < tokens.len() {
        let token = &tokens[token_index];

        if !need_matrix_renderer {
            simple_draw_normal(r, gr, render_buckets, editor_objects, token, theme);
            token_index += 1;
        } else {
            match &token.typ {
                TokenType::Operator(OperatorTokenType::Matrix {
                    row_count,
                    col_count,
                }) => {
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
                        theme,
                    );
                }
                TokenType::Variable { var_index } => {
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
                        false, // is bold
                        theme,
                    );

                    token_index += 1;
                    r.token_render_done(token.ptr.len(), token.ptr.len(), 0);
                }
                TokenType::LineReference { var_index } => {
                    let var = vars[*var_index].as_ref().unwrap();

                    let (rendered_width, rendered_height) = render_result_inside_editor(
                        units,
                        render_buckets,
                        &var.value,
                        r,
                        gr,
                        decimal_count,
                        theme,
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
                }
                TokenType::Operator(OperatorTokenType::ParenOpen) => {
                    let is_bold = parenthesis_around_cursor
                        .map(|(opening_paren_index, _closing_paren_index)| {
                            opening_paren_index == token_index
                        })
                        .unwrap_or(false);
                    simple_draw(r, gr, render_buckets, editor_objects, token, is_bold, theme);
                    token_index += 1;
                }
                TokenType::Operator(OperatorTokenType::ParenClose) => {
                    let is_bold = parenthesis_around_cursor
                        .map(|(_opening_paren_index, closing_paren_index)| {
                            closing_paren_index == token_index
                        })
                        .unwrap_or(false);
                    simple_draw(r, gr, render_buckets, editor_objects, token, is_bold, theme);
                    token_index += 1;
                }
                TokenType::StringLiteral
                | TokenType::Header
                | TokenType::NumberLiteral(_)
                | TokenType::Operator(_)
                | TokenType::Unit(_, _)
                | TokenType::NumberErr => {
                    simple_draw_normal(r, gr, render_buckets, editor_objects, token, theme);
                    token_index += 1;
                }
            }
        }
    }
}

fn find_parentesis_around_cursor(
    tokens: &[Token],
    r: &PerLineRenderData,
    cursor_pos: Pos,
) -> Option<(usize, usize)> {
    let mut active_parenthesis: Option<(usize, usize)> = None;
    if cursor_pos.row == r.editor_y.as_usize() {
        let mut parenth_token_indices_stack = [0; 32];
        let mut next_paren_stack_ptr = 0;
        let mut stack_index_of_closes_open_parenth = 0;
        let mut tokens_x_pos = 0;
        for (i, token) in tokens.iter().enumerate() {
            let before_cursor = tokens_x_pos < cursor_pos.column;
            if !before_cursor && next_paren_stack_ptr == 0 {
                // no opening bracket
                break;
            }
            match (&token.typ, before_cursor) {
                (TokenType::Operator(OperatorTokenType::ParenOpen), true) => {
                    parenth_token_indices_stack[next_paren_stack_ptr] = i;
                    stack_index_of_closes_open_parenth = next_paren_stack_ptr;
                    next_paren_stack_ptr += 1;
                }
                (TokenType::Operator(OperatorTokenType::ParenClose), true) => {
                    // TODO: kell ez az if, lehet ParenClose operator ParenOpen nélkül a tokenek között?
                    if next_paren_stack_ptr > 0 {
                        next_paren_stack_ptr -= 1;
                        if next_paren_stack_ptr > 0 {
                            stack_index_of_closes_open_parenth = next_paren_stack_ptr - 1;
                        } else {
                            stack_index_of_closes_open_parenth = 0;
                        }
                    }
                }
                //
                (TokenType::Operator(OperatorTokenType::ParenOpen), false) => {
                    parenth_token_indices_stack[next_paren_stack_ptr] = i;
                    next_paren_stack_ptr += 1;
                }
                (TokenType::Operator(OperatorTokenType::ParenClose), false) => {
                    if next_paren_stack_ptr - 1 == stack_index_of_closes_open_parenth {
                        // this is the closes closing parenthesis
                        active_parenthesis = Some((
                            parenth_token_indices_stack[stack_index_of_closes_open_parenth],
                            i,
                        ));
                        break;
                    }
                    next_paren_stack_ptr -= 1;
                }
                _ => {}
            }
            tokens_x_pos += token.ptr.len();
        }
    }
    active_parenthesis
}

fn simple_draw_normal<'text_ptr>(
    r: &mut PerLineRenderData,
    gr: &mut GlobalRenderData,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    editor_objects: &mut Vec<EditorObject>,
    token: &Token<'text_ptr>,
    theme: &Theme,
) {
    simple_draw(r, gr, render_buckets, editor_objects, token, false, theme);
}

fn simple_draw<'text_ptr>(
    r: &mut PerLineRenderData,
    gr: &mut GlobalRenderData,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    editor_objects: &mut Vec<EditorObject>,
    token: &Token<'text_ptr>,
    is_bold: bool,
    theme: &Theme,
) {
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
        is_bold,
        theme,
    );

    r.token_render_done(token.ptr.len(), token.ptr.len(), 0);
}

fn render_wrap_dots(
    render_buckets: &mut RenderBuckets,
    r: &PerLineRenderData,
    gr: &GlobalRenderData,
    theme: &Theme,
) {
    if r.render_x > gr.current_editor_width {
        // gutter is above text so it has to be abovetext as well
        render_buckets.set_color(Layer::AboveText, theme.text);
        for y in 0..r.rendered_row_height {
            render_buckets.draw_char(
                Layer::AboveText,
                gr.result_gutter_x - 1,
                r.render_y.add(y),
                '…', // '...'
            );
        }
    }
}

fn draw_line_ref_chooser(
    render_buckets: &mut RenderBuckets,
    r: &PerLineRenderData,
    gr: &GlobalRenderData,
    line_reference_chooser: &Option<ContentIndex>,
    result_gutter_x: usize,
    theme: &Theme,
) {
    if let Some(selection_row) = line_reference_chooser {
        if *selection_row == r.editor_y {
            render_buckets.set_color(Layer::Text, theme.line_ref_selector);
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
    theme: &Theme,
) {
    match editor_content.get_data(r.editor_y.as_usize()).result_format {
        ResultFormat::Hex => {
            render_buckets.set_color(Layer::AboveText, theme.cursor);
            render_buckets.draw_text(Layer::AboveText, result_gutter_x, r.render_y, &['0', 'x']);
        }
        ResultFormat::Bin => {
            render_buckets.set_color(Layer::AboveText, theme.cursor);
            render_buckets.draw_text(Layer::AboveText, result_gutter_x, r.render_y, &['0', 'b']);
        }
        ResultFormat::Dec => {}
    }
}

fn blend_color(src_color: u32, dst_color: u32, src_alpha: f32) -> u32 {
    let dst_alpha = 1f32 - src_alpha;
    let src_r = (src_color >> 24 & 0xFF) as f32;
    let src_g = (src_color >> 16 & 0xFF) as f32;
    let src_b = (src_color >> 8 & 0xFF) as f32;
    let dst_r = (dst_color >> 24 & 0xFF) as f32;
    let dst_g = (dst_color >> 16 & 0xFF) as f32;
    let dst_b = (dst_color >> 8 & 0xFF) as f32;
    let out_r = (src_r * src_alpha + dst_r * dst_alpha) as u32;
    let out_g = (src_g * src_alpha + dst_g * dst_alpha) as u32;
    let out_b = (src_b * src_alpha + dst_b * dst_alpha) as u32;
    return (out_r << 24 | out_g << 16 | out_b << 8) | 0xFF;
}

fn highlight_current_line(
    render_buckets: &mut RenderBuckets,
    editor: &Editor<LineData>,
    gr: &GlobalRenderData,
    theme: &Theme,
    cursor_inside_function_body: bool,
) {
    let cursor_row = editor.get_selection().get_cursor_pos().row;
    if let Some(render_y) = gr.get_render_y(content_y(cursor_row)) {
        let render_h = gr.get_rendered_height(content_y(cursor_row));
        render_buckets.set_color(Layer::BehindTextCursor, theme.current_line_bg);
        render_buckets.draw_rect(
            Layer::BehindTextCursor,
            0,
            render_y,
            gr.result_gutter_x + RIGHT_GUTTER_WIDTH + gr.current_result_panel_width,
            render_h,
        );
        // render a blended rectangle to the right gutter as if the highlighting rectangle
        // would blend into it (without it it hides the gutter and it is ugly).
        let blended_color = blend_color(theme.result_gutter_bg, theme.current_line_bg, 0.5);
        render_buckets.set_color(Layer::Text, blended_color);
        render_buckets.draw_rect(
            Layer::Text,
            gr.result_gutter_x,
            render_y,
            RIGHT_GUTTER_WIDTH,
            render_h,
        );

        if cursor_inside_function_body {
            let blended_color = blend_color(theme.func_bg, theme.current_line_bg, 0.5);
            render_buckets.set_color(Layer::BehindTextCursor, blended_color);
            render_buckets.draw_rect(
                Layer::BehindTextCursor,
                gr.left_gutter_width,
                render_y,
                gr.current_editor_width,
                render_h,
            );
        }
    };
}

fn sum_result(sum_var: &mut Variable, result: &CalcResult, sum_is_null: &mut bool) {
    if *sum_is_null {
        sum_var.value = Ok(result.clone());
        *sum_is_null = false;
    } else {
        sum_var.value = match &sum_var.value {
            Ok(current_sum) => {
                if let Some(ok) = add_op(current_sum, &result) {
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
    editor: &Editor<LineData>,
    matrix_editing: &Option<MatrixEditing>,
    // TODO: why unused?
    _decimal_count: Option<usize>,
    theme: &Theme,
) -> usize {
    let mut text_width = 0;
    let mut end_token_index = token_index;
    while tokens[end_token_index].typ != TokenType::Operator(OperatorTokenType::BracketClose) {
        text_width += tokens[end_token_index].ptr.len();
        end_token_index += 1;
    }
    let matrix_has_errors = tokens[end_token_index].has_error;
    // ignore the bracket as well
    text_width += 1;
    end_token_index += 1;

    let cursor_pos = editor.get_selection().get_cursor_pos();
    let cursor_inside_matrix: bool = if !editor.get_selection().is_range()
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
            &THEMES[gr.theme_index],
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
            theme,
            matrix_has_errors,
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
    editor: &Editor<LineData>,
    editor_content: &EditorContent<LineData>,
    vars: &Variables,
    func_defs: &FunctionDefinitions,
    results: &[LineResult],
    allocator: &Bump,
    apptokens: &AppTokens,
) -> Option<String> {
    let sel = editor.get_selection();
    // TODO optimize vec allocations
    let mut parsing_tokens = Vec::with_capacity(128);
    // TODO we should be able to mark the arena allcoator and free it at the end of the function
    if sel.start.row == sel.end.unwrap().row {
        if let Some(selected_text) = Editor::get_selected_text_single_line(sel, &editor_content) {
            if let Ok(Some(result)) = evaluate_text(
                units,
                selected_text,
                vars,
                func_defs,
                &mut parsing_tokens,
                sel.start.row,
                allocator,
                editor_content,
                apptokens,
            ) {
                if result.there_was_operation {
                    let result_str = render_result(
                        &units,
                        &result.result,
                        &editor_content.get_data(sel.start.row).result_format,
                        result.there_was_unit_conversion,
                        Some(RENDERED_RESULT_PRECISION),
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
                Some(RENDERED_RESULT_PRECISION),
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
    func_defs: &FunctionDefinitions<'text_ptr>,
    parsing_tokens: &mut Vec<Token<'text_ptr>>,
    editor_y: usize,
    allocator: &'text_ptr Bump,
    editor_content: &EditorContent<LineData>,
    apptokens: &AppTokens<'text_ptr>,
) -> Result<Option<EvaluationResult>, EvalErr> {
    let func_def_tmp: [Option<FunctionDef>; MAX_LINE_COUNT] = [None; MAX_LINE_COUNT];
    TokenParser::parse_line(
        text,
        vars,
        parsing_tokens,
        &units,
        editor_y,
        allocator,
        0,
        &func_def_tmp,
    );
    let mut shunting_output_stack = Vec::with_capacity(4);
    ShuntingYard::shunting_yard(parsing_tokens, &mut shunting_output_stack, units, func_defs);
    let (_, result) = evaluate_tokens(
        editor_y,
        apptokens,
        &vars,
        func_defs,
        units,
        editor_content,
        0,
        None,
    );
    return result;
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
    theme: &Theme,
    matrix_has_errors: bool,
) -> usize {
    let vert_align_offset = (rendered_row_height - MatrixData::calc_render_height(row_count)) / 2;

    if render_x < current_editor_width {
        render_matrix_left_brackets(
            render_x + left_gutter_width,
            render_y,
            row_count,
            render_buckets,
            vert_align_offset,
            matrix_has_errors,
        );
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
                if render_x <= current_editor_width {
                    draw_token(
                        token,
                        render_x + offset_x + local_x,
                        render_y.add(dst_y),
                        current_editor_width,
                        left_gutter_width,
                        render_buckets,
                        false,
                        theme,
                    );
                }
                local_x += token.ptr.len();
            }
        }
        render_x += if col_i + 1 < col_count {
            max_width + 2
        } else {
            max_width
        };
    }

    if render_x < current_editor_width {
        render_matrix_right_brackets(
            render_x + left_gutter_width,
            render_y,
            row_count,
            render_buckets,
            vert_align_offset,
            matrix_has_errors,
        );
    }
    render_x += 1;

    render_x
}

fn render_matrix_left_brackets(
    x: usize,
    render_y: CanvasY,
    row_count: usize,
    render_buckets: &mut RenderBuckets,
    vert_align_offset: usize,
    matrix_has_errors: bool,
) {
    let dst_bucket = if matrix_has_errors {
        &mut render_buckets.number_errors
    } else {
        &mut render_buckets.operators
    };
    if row_count == 1 {
        dst_bucket.push(RenderUtf8TextMsg {
            text: &['['],
            row: render_y.add(vert_align_offset),
            column: x,
        });
    } else {
        dst_bucket.push(RenderUtf8TextMsg {
            text: &['┌'],
            row: render_y.add(vert_align_offset),
            column: x,
        });
        for i in 0..row_count {
            dst_bucket.push(RenderUtf8TextMsg {
                text: &['│'],
                row: render_y.add(i + vert_align_offset + 1),
                column: x,
            });
        }
        dst_bucket.push(RenderUtf8TextMsg {
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
    matrix_has_errors: bool,
) {
    let dst_bucket = if matrix_has_errors {
        &mut render_buckets.number_errors
    } else {
        &mut render_buckets.operators
    };
    if row_count == 1 {
        dst_bucket.push(RenderUtf8TextMsg {
            text: &[']'],
            row: render_y.add(vert_align_offset),
            column: x,
        });
    } else {
        dst_bucket.push(RenderUtf8TextMsg {
            text: &['┐'],
            row: render_y.add(vert_align_offset),
            column: x,
        });
        for i in 0..row_count {
            dst_bucket.push(RenderUtf8TextMsg {
                text: &['│'],
                row: render_y.add(i + 1 + vert_align_offset),
                column: x,
            });
        }
        dst_bucket.push(RenderUtf8TextMsg {
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
    clip_right: usize,
    mat: &MatrixData,
    render_buckets: &mut RenderBuckets<'text_ptr>,
    prev_mat_result_lengths: Option<&ResultLengths>,
    rendered_row_height: usize,
    decimal_count: Option<usize>,
    text_color: u32,
) -> usize {
    let start_x = render_x;

    let vert_align_offset = (rendered_row_height - mat.render_height()) / 2;
    if render_x < clip_right {
        render_matrix_left_brackets(
            start_x,
            render_y,
            mat.row_count,
            render_buckets,
            vert_align_offset,
            false,
        );
    }
    render_x += 1;

    let cells_strs = {
        let mut tokens_per_cell: ArrayVec<[String; 32]> = ArrayVec::new();

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
    render_buckets.set_color(Layer::Text, text_color);

    for col_i in 0..mat.col_count {
        if render_x >= clip_right {
            break;
        }
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

    if render_x < clip_right {
        render_matrix_right_brackets(
            render_x,
            render_y,
            mat.row_count,
            render_buckets,
            vert_align_offset,
            false,
        );
    }
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
    theme: &Theme,
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
                gr.result_gutter_x - SCROLLBAR_WIDTH,
                mat,
                render_buckets,
                None,
                r.rendered_row_height,
                decimal_count,
                theme.referenced_matrix_text,
            );
            (rendered_width, mat.render_height())
        }
        Ok(result) => {
            // TODO: optimize string alloc
            let result_str = render_result(
                &units,
                result,
                &ResultFormat::Dec,
                false,
                decimal_count,
                true,
            );
            let text_len = result_str.chars().count();
            let bounded_text_len = text_len
                .min((gr.current_editor_width as isize - r.render_x as isize).max(0) as usize);

            render_buckets.line_ref_results.push(RenderStringMsg {
                text: result_str[0..bounded_text_len].to_owned(),
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

#[derive(Default)]
struct ResultTmp {
    buffer_ptr: Option<Range<usize>>,
    editor_y: ContentIndex,
    lengths: ResultLengths,
}

pub const MAX_VISIBLE_HEADER_COUNT: usize = 16;

struct ResultRender {
    result_ranges: ArrayVec<[ResultTmp; MAX_CLIENT_HEIGHT]>,
    max_len: usize,
    max_lengths: [ResultLengths; MAX_VISIBLE_HEADER_COUNT],
    result_counts_in_regions: [usize; MAX_VISIBLE_HEADER_COUNT],
}

impl ResultRender {
    pub fn new(vec: ArrayVec<[ResultTmp; MAX_CLIENT_HEIGHT]>) -> ResultRender {
        return ResultRender {
            result_ranges: vec,
            max_len: 0,
            max_lengths: [ResultLengths {
                int_part_len: 0,
                frac_part_len: 0,
                unit_part_len: 0,
            }; 16],
            result_counts_in_regions: [0; MAX_VISIBLE_HEADER_COUNT],
        };
    }
}

fn render_results_into_buf_and_calc_len<'text_ptr>(
    units: &Units,
    results: &[LineResult],
    tmp: &mut ResultRender,
    editor_content: &EditorContent<LineData>,
    gr: &GlobalRenderData,
    decimal_count: Option<usize>,
) {
    let mut result_buffer_index = 0;
    let result_buffer = unsafe { &mut RESULT_BUFFER };
    // calc max length and render results into buffer
    let mut region_index = 0;
    let mut region_count_offset = 0;
    for (editor_y, result) in results.iter().enumerate() {
        let editor_y = content_y(editor_y);
        let render_y = if let Some(render_y) = gr.get_render_y(editor_y) {
            render_y
        } else {
            continue;
        };
        if !gr.is_visible(editor_y) {
            continue;
        } else if editor_y.as_usize() >= editor_content.line_count() {
            break;
        }
        if render_y.as_usize() != 0 && editor_content.get_char(editor_y.as_usize(), 0) == '#' {
            let max_lens = &tmp.max_lengths[region_index];
            tmp.max_len = (max_lens.int_part_len
                + max_lens.frac_part_len
                + if max_lens.unit_part_len > 0 {
                    max_lens.unit_part_len + 1
                } else {
                    0
                })
            .max(tmp.max_len);
            tmp.result_counts_in_regions[region_index] =
                tmp.result_ranges.len() - region_count_offset;
            region_count_offset = tmp.result_ranges.len();
            // TODO remove this limitation
            if region_index < MAX_VISIBLE_HEADER_COUNT - 1 {
                region_index += 1;
            }
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
                    tmp.max_lengths[region_index].set_max(&lens);
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
    result_buffer[result_buffer_index] = 0; // tests depend on it

    tmp.result_counts_in_regions[region_index] = tmp.result_ranges.len() - region_count_offset;
    tmp.max_len = (tmp.max_lengths[region_index].int_part_len
        + tmp.max_lengths[region_index].frac_part_len
        + if tmp.max_lengths[region_index].unit_part_len > 0 {
            tmp.max_lengths[region_index].unit_part_len + 1
        } else {
            0
        })
    .max(tmp.max_len);
}

fn create_render_commands_for_results_and_render_matrices<'text_ptr>(
    tmp: &ResultRender,
    units: &Units,
    results: &[LineResult],
    render_buckets: &mut RenderBuckets<'text_ptr>,
    gr: &GlobalRenderData,
    decimal_count: Option<usize>,
    theme: &Theme,
) -> usize {
    let mut prev_result_matrix_length = None;
    let mut matrix_len = 0;
    let result_buffer = unsafe { &RESULT_BUFFER };
    let mut region_index = 0;
    let mut result_count_in_this_region = tmp.result_counts_in_regions[0];
    let mut max_lens = &tmp.max_lengths[0];
    for result_tmp in tmp.result_ranges.iter() {
        while result_count_in_this_region == 0 {
            region_index += 1;
            result_count_in_this_region = tmp.result_counts_in_regions[region_index];
            max_lens = &tmp.max_lengths[region_index];
        }
        let rendered_row_height = gr.get_rendered_height(result_tmp.editor_y);
        let render_y = gr.get_render_y(result_tmp.editor_y).expect("");
        if let Some(result_range) = &result_tmp.buffer_ptr {
            let lengths = &result_tmp.lengths;
            let from = result_range.start;
            let vert_align_offset = (rendered_row_height - 1) / 2;
            let row = render_y.add(vert_align_offset);
            enum ResultOffsetX {
                Err,
                Ok(usize),
                TooLong,
            }
            let offset_x = if max_lens.int_part_len < lengths.int_part_len {
                // it is an "Err"
                ResultOffsetX::Err
            } else {
                let offset_x = max_lens.int_part_len - lengths.int_part_len;
                let sum_len =
                    lengths.int_part_len + max_lens.frac_part_len + max_lens.unit_part_len;
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
                let offset_x = max_lens.unit_part_len - lengths.unit_part_len;
                render_buckets.ascii_texts.push(RenderAsciiTextMsg {
                    text: &result_buffer[from..result_range.end],
                    row,
                    column: gr.result_gutter_x
                        + RIGHT_GUTTER_WIDTH
                        + max_lens.int_part_len
                        + max_lens.frac_part_len
                        + 1
                        + offset_x,
                });
            }
            match offset_x {
                ResultOffsetX::TooLong => {
                    render_buckets.set_color(Layer::AboveText, theme.result_bg_color);
                    render_buckets.draw_char(
                        Layer::AboveText,
                        gr.result_gutter_x + RIGHT_GUTTER_WIDTH + gr.current_result_panel_width - 1,
                        row,
                        '█',
                    );
                    render_buckets.set_color(Layer::AboveText, theme.cursor);
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
                        gr.client_width,
                        mat,
                        render_buckets,
                        prev_result_matrix_length.as_ref(),
                        gr.get_rendered_height(result_tmp.editor_y),
                        decimal_count,
                        theme.result_text,
                    );
                    if width > matrix_len {
                        matrix_len = width;
                    }
                }
                _ => {
                    // no result but need rerender
                    prev_result_matrix_length = None;
                }
            }
        }
        result_count_in_this_region -= 1;
        if result_count_in_this_region == 0 {
            region_index += 1;
            result_count_in_this_region = tmp.result_counts_in_regions[region_index];
            max_lens = &tmp.max_lengths[region_index];
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
        let mut tokens_per_cell: ArrayVec<[String; 32]> = ArrayVec::new();

        for cell in mat.cells.iter() {
            let result_str = render_result(
                units,
                cell,
                &ResultFormat::Dec,
                false,
                Some(RENDERED_RESULT_PRECISION),
                true,
            );
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
) {
    let mut color_index = 0;
    // TODO: can be smaller, we need only SCREEN_HEIGHT amount of bit
    let mut highlighted = BitFlag256::empty();
    for editor_obj in editor_objs {
        match editor_obj.typ {
            EditorObjectType::LineReference { var_index }
            | EditorObjectType::Variable { var_index } => {
                if var_index >= SUM_VARIABLE_INDEX {
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
                    // render a rectangle on the *left gutter*
                    render_buckets.custom_commands[Layer::BehindTextAboveCursor as usize]
                        .push(OutputMessage::SetColor(color));
                    render_buckets.custom_commands[Layer::BehindTextAboveCursor as usize].push(
                        OutputMessage::RenderRectangle {
                            x: 0,
                            y: render_y,
                            w: gr.left_gutter_width,
                            h: gr.get_rendered_height(defined_at),
                        },
                    );
                    // render a rectangle on the *right gutter*
                    render_buckets.custom_commands[Layer::BehindTextAboveCursor as usize].push(
                        OutputMessage::RenderRectangle {
                            x: gr.result_gutter_x,
                            y: render_y,
                            w: RIGHT_GUTTER_WIDTH,
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
    is_bold: bool,
    theme: &Theme,
) {
    let dst = if token.has_error() {
        &mut render_buckets.number_errors
    } else {
        match &token.typ {
            TokenType::StringLiteral => &mut render_buckets.utf8_texts,
            TokenType::Header => &mut render_buckets.headers,
            TokenType::Variable { .. } => &mut render_buckets.variable,
            TokenType::LineReference { .. } => &mut render_buckets.variable,
            TokenType::NumberLiteral(_) => &mut render_buckets.numbers,
            TokenType::NumberErr => &mut render_buckets.number_errors,
            TokenType::Unit(_, _) => &mut render_buckets.units,
            TokenType::Operator(OperatorTokenType::ParenClose) => {
                if current_editor_width <= render_x {
                    return;
                }
                if is_bold {
                    render_buckets.set_color(Layer::Text, theme.line_ref_bg);
                    render_buckets.draw_rect(
                        Layer::Text,
                        render_x + left_gutter_width,
                        render_y,
                        1,
                        1,
                    );

                    render_buckets.set_color(Layer::Text, theme.parenthesis);

                    let b = &mut render_buckets.custom_commands[Layer::Text as usize];
                    b.push(OutputMessage::FollowingTextCommandsAreHeaders(true));
                    b.push(OutputMessage::RenderChar(RenderChar {
                        col: render_x + left_gutter_width,
                        row: render_y,
                        char: ')',
                    }));
                    b.push(OutputMessage::FollowingTextCommandsAreHeaders(false));
                } else {
                    render_buckets.parenthesis.push(RenderChar {
                        col: render_x + left_gutter_width,
                        row: render_y,
                        char: ')',
                    });
                }
                return;
            }
            TokenType::Operator(OperatorTokenType::ParenOpen) => {
                if current_editor_width <= render_x {
                    return;
                }
                if is_bold {
                    render_buckets.set_color(Layer::Text, theme.line_ref_bg);
                    render_buckets.draw_rect(
                        Layer::Text,
                        render_x + left_gutter_width,
                        render_y,
                        1,
                        1,
                    );
                    render_buckets.set_color(Layer::Text, theme.parenthesis);

                    render_buckets.custom_commands[Layer::Text as usize]
                        .push(OutputMessage::FollowingTextCommandsAreHeaders(true));
                    &mut render_buckets.custom_commands[Layer::Text as usize].push(
                        OutputMessage::RenderChar(RenderChar {
                            col: render_x + left_gutter_width,
                            row: render_y,
                            char: '(',
                        }),
                    );
                    render_buckets.custom_commands[Layer::Text as usize]
                        .push(OutputMessage::FollowingTextCommandsAreHeaders(false));
                } else {
                    &mut render_buckets.parenthesis.push(RenderChar {
                        col: render_x + left_gutter_width,
                        row: render_y,
                        char: '(',
                    });
                }
                return;
            }
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

    fn write_char(canvas: &mut [[char; 256]], row: CanvasY, col: usize, char: char) {
        let str = &mut canvas[row.as_usize()];
        str[col] = char;
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
            OutputMessage::RenderChar(r) => {
                write_char(canvas, r.row, r.col, r.char);
            }
            OutputMessage::RenderString(text) => {
                write_str(canvas, text.row, text.column, &text.text);
            }
            OutputMessage::RenderAsciiText(text) => {
                write_ascii(canvas, text.row, text.column, &text.text);
            }
            OutputMessage::FollowingTextCommandsAreHeaders { .. } => {}
            OutputMessage::RenderUnderline { .. } => {}
            OutputMessage::UpdatePulses => {}
        }
    }

    for command in &buckets.custom_commands[Layer::BehindTextBehindCursor as usize] {
        write_command(canvas, command);
    }

    for command in &buckets.custom_commands[Layer::BehindTextCursor as usize] {
        write_command(canvas, command);
    }
    for command in &buckets.custom_commands[Layer::BehindTextAboveCursor as usize] {
        write_command(canvas, command);
    }

    write_command(canvas, &OutputMessage::UpdatePulses);

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
    for command in &buckets.parenthesis {
        write_char(canvas, command.row, command.col, command.char);
    }

    for command in &buckets.line_ref_results {
        write_str(canvas, command.row, command.column, &command.text);
    }

    for command in &buckets.headers {
        write_char_slice(canvas, command.row, command.column, command.text);
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
    editor: &Editor<LineData>,
    editor_content: &EditorContent<LineData>,
    gr: &GlobalRenderData,
    vars: &Variables,
    func_defs: &FunctionDefinitions<'text_ptr>,
    allocator: &'text_ptr Bump,
    theme: &Theme,
    apptokens: &AppTokens,
) {
    render_buckets.set_color(Layer::BehindTextAboveCursor, theme.selection_color);
    if let Some((start, end)) = editor.get_selection().is_range_ordered() {
        if end.row > start.row {
            // first line
            if let Some(start_render_y) = gr.get_render_y(content_y(start.row)) {
                let height = gr.get_rendered_height(content_y(start.row));
                let w = editor_content.line_len(start.row);
                if w > start.column {
                    render_buckets.draw_rect(
                        Layer::BehindTextAboveCursor,
                        start.column + gr.left_gutter_width,
                        start_render_y,
                        (w - start.column).min(gr.current_editor_width),
                        height,
                    );
                }
            }
            // full lines
            for i in start.row + 1..end.row {
                if let Some(render_y) = gr.get_render_y(content_y(i)) {
                    let height = gr.get_rendered_height(content_y(i));
                    render_buckets.draw_rect(
                        Layer::BehindTextAboveCursor,
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
                    Layer::BehindTextAboveCursor,
                    gr.left_gutter_width,
                    end_render_y,
                    end.column.min(gr.current_editor_width),
                    height,
                );
            }
        } else if let Some(start_render_y) = gr.get_render_y(content_y(start.row)) {
            let height = gr.get_rendered_height(content_y(start.row));
            render_buckets.draw_rect(
                Layer::BehindTextAboveCursor,
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
            func_defs,
            results.as_slice(),
            allocator,
            apptokens,
        ) {
            if start.row == end.row {
                if let Some(start_render_y) = gr.get_render_y(content_y(start.row)) {
                    let selection_center = start.column + ((end.column - start.column) / 2);
                    partial_result.insert_str(0, "= ");
                    let result_w = partial_result.chars().count();
                    let centered_x =
                        (selection_center as isize - (result_w / 2) as isize).max(0) as usize;
                    render_buckets.set_color(Layer::AboveText, theme.sum_bg_color);
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
                    render_buckets.set_color(Layer::AboveText, theme.sum_text_color);
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
                render_buckets.set_color(Layer::AboveText, theme.sum_bg_color);
                render_buckets.draw_rect(
                    Layer::AboveText,
                    gr.left_gutter_width + x,
                    gr.get_render_y(frist_visible_row_index).expect(""),
                    result_w + 1,
                    inner_height + 1,
                );
                // draw the parenthesis
                render_buckets.set_color(Layer::AboveText, theme.sum_text_color);

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
    editor: &mut Editor<LineData>,
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
    // TODO: máshogy oldd meg, mert ez modositja az undo stacket is
    // és az miért baj, legalább tudom ctrl z-zni a mátrix edition-t

    // TODO: remove width limitation and allow it

    if editor_content.line_len(mat_editor.row_index.as_usize()) + concat.len()
        - (mat_editor.end_text_index - mat_editor.start_text_index)
        < MAX_EDITOR_WIDTH
    {
        let selection = Selection::range(
            Pos::from_row_column(mat_editor.row_index.as_usize(), mat_editor.start_text_index),
            Pos::from_row_column(mat_editor.row_index.as_usize(), mat_editor.end_text_index),
        );
        editor.set_selection_save_col(selection);
        editor.handle_input_undoable(
            EditorInputEvent::Del,
            InputModifiers::none(),
            editor_content,
        );
        // TODO: it can overflow, reasuling in AllLinesFrom..
        editor.insert_text_undoable(&concat, editor_content);
    }
    *matrix_editing = None;

    if let Some(new_cursor_pos) = new_cursor_pos {
        editor.set_selection_save_col(Selection::single(
            new_cursor_pos.with_column(
                new_cursor_pos
                    .column
                    .min(editor_content.line_len(new_cursor_pos.row) - 1),
            ),
        ));
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
        } else {
            // scroll down
            // if the new pos is 5. line and its height is 1, this var is 6
            let new_pos_bottom_y =
                if let Some(new_row_y) = render_data.get_render_y(content_y(current_row)) {
                    let new_h = render_data.get_rendered_height(content_y(current_row));
                    new_row_y.add(new_h)
                } else {
                    // find the last rendered line at the bottom
                    let mut assumed_heights = 1;
                    let mut prev_row_y = None;
                    let mut prev_row_i = current_row as isize - 1;
                    while prev_row_y.is_none() && prev_row_i >= 0 {
                        prev_row_y = render_data.get_render_y(content_y(prev_row_i as usize));
                        assumed_heights += 1;
                        prev_row_i -= 1;
                    }
                    // we assume that the non-yet-rendered lines' height will be 1
                    prev_row_y.unwrap_or(canvas_y(0)).add(assumed_heights)
                };
            let new_scroll_y = new_pos_bottom_y.as_isize() + render_data.scroll_y as isize
                - (render_data.client_height as isize);
            if new_scroll_y > render_data.scroll_y as isize {
                Some(new_scroll_y as usize)
            } else {
                None
            }
        }
    } else {
        None
    }
}

fn set_editor_and_result_panel_widths(
    client_width: usize,
    result_panel_width_percent: usize,
    gr: &mut GlobalRenderData,
) {
    let mut result_gutter_x: isize =
        (client_width * (100 - result_panel_width_percent) / 100) as isize;
    {
        // the editor pushes the gutter to right
        let editor_width =
            (result_gutter_x - SCROLLBAR_WIDTH as isize - LEFT_GUTTER_MIN_WIDTH as isize) - 1;
        let diff = gr.longest_visible_editor_line_len as isize - editor_width;
        if diff > 0 {
            result_gutter_x += diff;
        }
    }
    {
        // the result panel pushes the gutter to left, with higher priority
        let result_panel_w: isize =
            client_width as isize - result_gutter_x - RIGHT_GUTTER_WIDTH as isize;
        let diff = gr.longest_visible_result_len as isize - result_panel_w;
        if diff > 0 {
            result_gutter_x -= diff;
        }
    }
    // result panel width has a minimum required width
    let result_panel_w = client_width as isize - result_gutter_x - RIGHT_GUTTER_WIDTH as isize;
    let result_gutter_x = (client_width as isize - result_panel_w - RIGHT_GUTTER_WIDTH as isize)
        .max(MIN_RESULT_PANEL_WIDTH as isize) as usize;
    gr.set_result_gutter_x(result_gutter_x);
}

pub fn default_result_gutter_x(client_width: usize) -> usize {
    (client_width * (100 - DEFAULT_RESULT_PANEL_WIDTH_PERCENT) / 100)
        .max(LEFT_GUTTER_MIN_WIDTH + SCROLLBAR_WIDTH)
}
