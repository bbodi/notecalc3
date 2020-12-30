use crate::editor::editor::{Pos, RowModificationType, Selection};
use smallvec::alloc::fmt::Debug;

pub type Canvas = Vec<char>;
type EditorCommandGroup<T> = Vec<EditorCommand<T>>;

#[derive(Debug)]
pub enum EditorCommand<T: Default + Clone + Debug> {
    SwapLineUpwards(Pos),
    SwapLineDownards(Pos),
    Del {
        removed_char: char,
        pos: Pos,
    },
    MergeLineWithNextRow {
        upper_row_index: usize,
        upper_line_data: Box<T>,
        lower_line_data: Box<T>,
        pos_before_merge: Pos,
        pos_after_merge: Pos,
    },
    DelSelection {
        removed_text: String,
        selection: Selection,
    },
    DelCtrl {
        removed_text: Option<String>,
        pos: Pos,
    },
    InsertEmptyRow(usize),
    EnterSelection {
        selection: Selection,
        selected_text: String,
    },
    Enter(Pos),
    Backspace {
        removed_char: char,
        pos: Pos,
    },
    BackspaceSelection {
        removed_text: String,
        selection: Selection,
    },
    BackspaceCtrl {
        removed_text: Option<String>,
        pos: Pos,
    },
    InsertChar {
        pos: Pos,
        ch: char,
    },
    InsertCharSelection {
        ch: char,
        selection: Selection,
        selected_text: String,
    },
    CutLine {
        pos: Pos,
        removed_text: String,
    },
    DuplicateLine {
        pos: Pos,
        inserted_text: String,
    },
    InsertText {
        pos: Pos,
        text: String,
        is_there_line_overflow: bool,
    },
    InsertTextSelection {
        selection: Selection,
        text: String,
        removed_text: String,
        is_there_line_overflow: bool,
    },
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum JumpMode {
    IgnoreWhitespaces,
    ConsiderWhitespaces,
    BlockOnWhitespace,
}

pub struct EditorContent<T: Default + Clone + Debug> {
    // TODO: need for fuzz testing, set it back to priv later
    pub undo_stack: Vec<EditorCommandGroup<T>>,
    pub(super) redo_stack: Vec<EditorCommandGroup<T>>,
    pub(super) max_line_len: usize,
    pub(super) line_lens: Vec<usize>,
    pub(super) canvas: Canvas,
    pub(super) line_data: Vec<T>,
}

impl<T: Default + Clone + Debug> EditorContent<T> {
    pub fn new(max_len: usize, max_row_count: usize) -> EditorContent<T> {
        EditorContent {
            undo_stack: Vec::with_capacity(32),
            redo_stack: Vec::with_capacity(32),
            canvas: Vec::with_capacity(max_len * max_row_count),
            line_lens: Vec::with_capacity(max_row_count),
            line_data: Vec::with_capacity(max_row_count),
            max_line_len: max_len,
        }
    }

    pub fn max_line_len(&self) -> usize {
        self.max_line_len
    }

    pub fn line_count(&self) -> usize {
        self.line_lens.len()
    }

    pub fn line_len(&self, row_i: usize) -> usize {
        self.line_lens[row_i]
    }

    pub fn lines(&self) -> impl Iterator<Item = &[char]> {
        return self
            .canvas
            .chunks(self.max_line_len)
            .zip(self.line_lens.iter())
            .map(|(line, len)| &line[0..*len]);
    }

    pub fn push_line(&mut self) {
        let line = std::iter::repeat(0 as char).take(self.max_line_len);
        self.canvas.extend(line);
        self.line_lens.push(0);
        if self.line_count() > self.line_data.len() {
            self.line_data.push(Default::default());
        }
    }

    pub fn insert_line_at(&mut self, at: usize) {
        let start_pos = self.max_line_len * at;
        let line = std::iter::repeat(0 as char).take(self.max_line_len);
        self.canvas.splice(start_pos..start_pos, line);
        self.line_lens.insert(at, 0);
        self.line_data.insert(at, Default::default());
    }

    pub fn remove_line_at(&mut self, at: usize) {
        let from = self.max_line_len * at;
        let to = from + self.max_line_len;
        self.canvas.splice(from..to, std::iter::empty());
        self.line_lens.remove(at);
        self.line_data.remove(at);
    }

    pub fn write_selection_into(&self, selection: Selection, result: &mut String) {
        if !selection.is_range() {
            return;
        }
        let start = selection.get_first();
        let end = selection.get_second();
        if end.row > start.row {
            // first line
            let from = self.get_char_pos(start.row, start.column);
            let to = self.get_char_pos(start.row, self.line_lens[start.row]);
            result.extend(&self.canvas[from..to]);
            result.push('\n');
            // full lines
            for i in start.row + 1..end.row {
                let from = self.get_char_pos(i, 0);
                let to = self.get_char_pos(i, self.line_lens[i]);
                result.extend(&self.canvas[from..to]);
                result.push('\n');
            }

            let from = self.get_char_pos(end.row, 0);
            let to = self.get_char_pos(end.row, end.column);
            result.extend(&self.canvas[from..to]);
        } else {
            let from = self.get_char_pos(start.row, start.column);
            let to = self.get_char_pos(start.row, end.column);
            for ch in &self.canvas[from..to] {
                result.push(*ch);
            }
        }
    }

    pub fn data_mut(&mut self) -> &mut [T] {
        &mut self.line_data
    }

    pub fn data(&self) -> &[T] {
        &self.line_data
    }

    pub fn get_data(&self, i: usize) -> &T {
        &self.line_data[i]
    }

    pub fn mut_data(&mut self, i: usize) -> &mut T {
        &mut self.line_data[i]
    }

    pub fn duplicate_line(&mut self, at: usize) {
        self.insert_line_at(at + 1);
        self.line_lens[at + 1] = self.line_lens[at];
        let from = at * self.max_line_len;
        let to = from + self.line_lens[at];
        let dst = (at + 1) * self.max_line_len;
        self.canvas.copy_within(from..to, dst);
    }

    pub fn get_char_pos(&self, row_index: usize, column_index: usize) -> usize {
        row_index * self.max_line_len + column_index
    }

    pub fn get_line_valid_chars(&self, row_index: usize) -> &[char] {
        let from = row_index * self.max_line_len;
        let to = from + self.line_len(row_index);
        &self.canvas[from..to]
    }

    pub(super) fn get_line_chars(&self, row_index: usize) -> &[char] {
        let from = row_index * self.max_line_len;
        let to = from + self.max_line_len;
        &self.canvas[from..to]
    }

    pub fn get_mut_line_chars(&mut self, row_index: usize) -> &mut [char] {
        let from = row_index * self.max_line_len;
        let to = from + self.max_line_len;
        &mut self.canvas[from..to]
    }

    pub fn get_char(&self, row_index: usize, column_index: usize) -> char {
        return self.canvas[self.get_char_pos(row_index, column_index)];
    }

    pub fn set_char(&mut self, row_index: usize, column_index: usize, ch: char) {
        let current_line_count = self.line_count();
        for _ in current_line_count..=row_index {
            self.push_line();
        }
        let char_pos = self.get_char_pos(row_index, column_index);
        self.canvas[char_pos] = ch;
    }

    pub fn insert_char(&mut self, row_index: usize, column_index: usize, ch: char) -> bool {
        if self.line_lens[row_index] == self.max_line_len {
            return false;
        }
        let from = self.get_char_pos(row_index, column_index);
        let len = self.line_lens[row_index];
        debug_assert!(len <= self.max_line_len);
        let to = self.get_char_pos(row_index, len);
        self.canvas.copy_within(from..to, from + 1);
        self.canvas[from] = ch;
        self.line_lens[row_index] += 1;
        return true;
    }

    pub fn remove_char(&mut self, row_index: usize, column_index: usize) {
        let from = self.get_char_pos(row_index, column_index);
        let len = self.line_lens[row_index];
        let to = self.get_char_pos(row_index, len);
        self.canvas.copy_within(from + 1..to, from);
        self.line_lens[row_index] -= 1;
    }

    pub fn clear(&mut self) {
        self.line_lens.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn init_with(&mut self, text: &str) {
        self.clear();
        self.push_line();
        self.set_str_at(text, 0, 0);
    }

    pub fn get_content(&self) -> String {
        let mut result = String::with_capacity(self.canvas.len() * self.max_line_len);
        self.write_content_into(&mut result);
        return result;
    }

    pub fn write_content_into(&self, result: &mut String) {
        for (i, line) in self.lines().enumerate() {
            if i > 0 {
                result.push('\n');
            }
            result.extend(line);
        }
    }

    pub fn set_str_at(&mut self, str: &str, row_index: usize, insert_at: usize) -> Pos {
        let mut col = insert_at;
        let mut row = row_index;
        for ch in str.chars() {
            if ch == '\r' {
                // ignore
                continue;
            } else if ch == '\n' {
                self.line_lens[row] = col;
                debug_assert!(self.line_lens[row] <= self.max_line_len);
                row += 1;
                self.insert_line_at(row);
                col = 0;
                continue;
            } else if col == self.max_line_len {
                self.line_lens[row] = col;
                debug_assert!(self.line_lens[row] <= self.max_line_len);
                row += 1;
                self.insert_line_at(row);
                col = 0;
            }
            self.set_char(row, col, ch);
            col += 1;
        }
        self.line_lens[row] = col;
        debug_assert!(self.line_lens[row] <= self.max_line_len);
        return Pos::from_row_column(row, col);
    }

    pub fn split_line(&mut self, row_index: usize, split_at: usize) {
        self.insert_line_at(row_index + 1);
        let new_line_pos = self.get_char_pos(row_index + 1, 0);

        {
            let from = self.get_char_pos(row_index, split_at);
            let to = self.get_char_pos(row_index, self.line_lens[row_index]);
            self.canvas.copy_within(from..to, new_line_pos);
            self.line_lens[row_index + 1] = to - from;
            debug_assert!(self.line_lens[row_index + 1] <= self.max_line_len);
        }
        self.line_lens[row_index] = split_at;
        debug_assert!(self.line_lens[row_index] <= self.max_line_len);
    }

    pub fn merge_with_next_row(
        &mut self,
        row_index: usize,
        first_row_col: usize,
        second_row_col: usize,
    ) -> bool {
        let merged_len = first_row_col + (self.line_len(row_index + 1) - second_row_col);
        if merged_len > self.max_line_len {
            return false;
        }
        if self.line_lens[row_index] == 0 && second_row_col == 0 {
            // keep the line_data of the 2nd row
            self.remove_line_at(row_index);
        } else if self.line_lens[row_index + 1] == 0 && first_row_col == self.line_len(row_index) {
            // keep the line_data of the 1st row
            self.remove_line_at(row_index + 1);
        } else {
            let dst = self.get_char_pos(row_index, first_row_col);
            let src_from = self.get_char_pos(row_index + 1, second_row_col);
            let src_to = self.get_char_pos(row_index + 1, self.line_lens[row_index + 1]);
            let new_line_len = first_row_col + (src_to - src_from);
            if new_line_len > self.max_line_len {
                return false;
            }
            self.canvas.copy_within(src_from..src_to, dst);
            self.line_lens[row_index] = new_line_len;
            debug_assert!(self.line_lens[row_index] <= self.max_line_len);
            self.remove_line_at(row_index + 1);
        }

        return true;
    }

    pub fn remove_selection(&mut self, selection: Selection) -> Option<RowModificationType> {
        // TODO: why do we have get_first and get_second here as well? redundant... The caller already does it.
        let first = selection.get_first();
        let second = selection.get_second();
        return if second.row > first.row {
            // check if there is enough space for the merged row in the line (< maxlen)
            let merged_len = first.column + (self.line_len(second.row) - second.column);
            if merged_len > self.max_line_len {
                return None;
            }

            for _ in first.row + 1..second.row {
                self.remove_line_at(first.row + 1);
            }
            if first.column == 0 {
                self.remove_selection(Selection::range(
                    Pos::from_row_column(first.row + 1, 0),
                    Pos::from_row_column(first.row + 1, second.column),
                ));
                self.remove_line_at(first.row);
                Some(RowModificationType::AllLinesFrom(first.row))
            } else if self.merge_with_next_row(first.row, first.column, second.column) {
                Some(RowModificationType::AllLinesFrom(first.row))
            } else {
                None
            }
        } else {
            self.get_mut_line_chars(first.row)
                .copy_within(second.column.., first.column);
            let selected_char_count = second.column - first.column;
            self.line_lens[first.row] -= selected_char_count;
            Some(RowModificationType::SingleLine(first.row))
        };
    }

    /// returns the new cursor pos after inserting the text,
    /// and whether there was a text overflow or not.
    pub fn insert_str_at(&mut self, pos: Pos, str: &str) -> (Pos, bool) {
        // save the content of first row which will be moved
        let mut text_to_move_buf: [u8; 4 * 128] = [0; 4 * 128];
        let mut text_to_move_buf_index = 0;

        for ch in &self.get_line_chars(pos.row)[pos.column..self.line_lens[pos.row]] {
            ch.encode_utf8(&mut text_to_move_buf[text_to_move_buf_index..]);
            text_to_move_buf_index += ch.len_utf8();
        }

        let new_pos = self.set_str_at(&str, pos.row, pos.column);
        if text_to_move_buf_index > 0 {
            let p = self.set_str_at(
                unsafe {
                    std::str::from_utf8_unchecked(&text_to_move_buf[0..text_to_move_buf_index])
                },
                new_pos.row,
                new_pos.column,
            );
            self.line_lens[p.row] = p.column;
            debug_assert!(self.line_lens[p.row] <= self.max_line_len);
        }
        return (new_pos, text_to_move_buf_index > 0);
    }

    pub fn swap_lines_upward(&mut self, lower_row: usize) {
        let maxlen = self.max_line_len();
        // swap lines
        {
            let upper_i = self.get_char_pos(lower_row - 1, 0);
            let cur_i = self.get_char_pos(lower_row, 0);
            let (left, right) = self.canvas.split_at_mut(cur_i);
            left[upper_i..].swap_with_slice(&mut right[0..maxlen]);
        }
        let tmp = self.line_lens[lower_row - 1];
        self.line_lens[lower_row - 1] = self.line_lens[lower_row];
        self.line_lens[lower_row] = tmp;

        let tmp = std::mem::replace(&mut self.line_data[lower_row - 1], Default::default());
        self.line_data[lower_row - 1] = std::mem::replace(&mut self.line_data[lower_row], tmp);
    }

    pub fn jump_word_backward(&self, cur_pos: &Pos, mode: JumpMode) -> usize {
        let mut col = cur_pos.column;
        let line = self.get_line_chars(cur_pos.row);
        while col > 0 {
            if line[col - 1].is_alphanumeric() || line[col - 1] == '_' {
                col -= 1;
                while col > 0 && (line[col - 1].is_alphanumeric() || line[col - 1] == '_') {
                    col -= 1;
                }
                break;
            } else if line[col - 1] == '\"' {
                col -= 1;
                break;
            } else if !line[col - 1].is_ascii_whitespace() {
                col -= 1;
                while col > 0
                    && !(line[col - 1].is_alphanumeric()
                        || line[col - 1] == '_'
                        || line[col - 1] == '\"'
                        || line[col - 1].is_ascii_whitespace())
                {
                    col -= 1;
                }
                break;
            } else {
                match mode {
                    JumpMode::IgnoreWhitespaces => {
                        col -= 1;
                    }
                    JumpMode::ConsiderWhitespaces => {
                        col -= 1;
                        while col > 0 && line[col - 1].is_ascii_whitespace() {
                            col -= 1;
                        }
                        break;
                    }
                    JumpMode::BlockOnWhitespace => {
                        break;
                    }
                }
            }
        }
        col
    }

    pub fn jump_word_forward(&self, cur_pos: &Pos, mode: JumpMode) -> usize {
        // check the type of the prev char
        let mut col = cur_pos.column;
        let line = self.get_line_chars(cur_pos.row);
        let len = self.line_len(cur_pos.row);
        while col < len {
            if line[col].is_alphanumeric() || line[col] == '_' {
                col += 1;
                while col < len && (line[col].is_alphanumeric() || line[col] == '_') {
                    col += 1;
                }
                break;
            } else if line[col] == '\"' {
                col += 1;
                break;
            } else if !line[col].is_ascii_whitespace() {
                col += 1;
                while col < len
                    && !(line[col].is_alphanumeric()
                        || line[col] == '_'
                        || line[col] == '\"'
                        || line[col].is_ascii_whitespace())
                {
                    col += 1;
                }
                break;
            } else {
                match mode {
                    JumpMode::IgnoreWhitespaces => {
                        col += 1;
                    }
                    JumpMode::ConsiderWhitespaces => {
                        col += 1;
                        while col < len && line[col].is_ascii_whitespace() {
                            col += 1;
                        }
                        break;
                    }
                    JumpMode::BlockOnWhitespace => {
                        break;
                    }
                }
            }
        }
        col
    }
}
