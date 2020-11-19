use crate::editor::editor_content::{EditorCommand, EditorContent, JumpMode};
use smallvec::alloc::fmt::Debug;
use std::ops::{Range, RangeInclusive};

pub const EDITOR_CURSOR_TICK_MS: u32 = 500;

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum EditorInputEvent {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Esc,
    PageUp,
    PageDown,
    Enter,
    Backspace,
    Del,
    Tab,
    Char(char),
}

#[repr(C)]
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct InputModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl InputModifiers {
    pub fn none() -> InputModifiers {
        InputModifiers {
            shift: false,
            ctrl: false,
            alt: false,
        }
    }

    pub fn ctrl() -> InputModifiers {
        InputModifiers {
            shift: false,
            ctrl: true,
            alt: false,
        }
    }

    pub fn alt() -> InputModifiers {
        InputModifiers {
            shift: false,
            ctrl: false,
            alt: true,
        }
    }

    pub fn shift() -> InputModifiers {
        InputModifiers {
            shift: true,
            ctrl: false,
            alt: false,
        }
    }

    pub fn ctrl_shift() -> InputModifiers {
        InputModifiers {
            shift: true,
            ctrl: true,
            alt: false,
        }
    }

    pub fn is_ctrl_shift(&self) -> bool {
        self.ctrl & self.shift
    }
}

#[derive(Default, Eq, PartialEq, Debug, Clone, Copy)]
pub struct Pos {
    pub row: usize,
    pub column: usize,
}

impl Pos {
    pub fn from_row_column(row_index: usize, column_index: usize) -> Pos {
        Pos {
            row: row_index,
            column: column_index,
        }
    }

    pub fn with_column(&self, col: usize) -> Pos {
        Pos {
            column: col,
            ..*self
        }
    }

    pub fn with_row(&self, row: usize) -> Pos {
        Pos { row, ..*self }
    }

    pub fn add_column(&self, col: usize) -> Pos {
        Pos {
            column: self.column + col,
            ..*self
        }
    }

    pub fn with_next_row(&self) -> Pos {
        Pos {
            row: self.row + 1,
            ..*self
        }
    }

    pub fn with_prev_row(&self) -> Pos {
        Pos {
            row: self.row - 1,
            ..*self
        }
    }

    pub fn with_next_col(&self) -> Pos {
        Pos {
            column: self.column + 1,
            ..*self
        }
    }

    pub fn with_prev_col(&self) -> Pos {
        Pos {
            column: self.column - 1,
            ..*self
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct Selection {
    pub start: Pos,
    pub end: Option<Pos>,
}

impl Selection {
    pub fn single(pos: Pos) -> Selection {
        Selection {
            start: pos,
            end: None,
        }
    }

    pub fn single_r_c(row_index: usize, column_index: usize) -> Selection {
        Selection {
            start: Pos {
                row: row_index,
                column: column_index,
            },
            end: None,
        }
    }

    pub fn range(start: Pos, end: Pos) -> Selection {
        Selection {
            start,
            end: if start == end { None } else { Some(end) },
        }
    }

    pub fn get_range(&self) -> (Pos, Pos) {
        if let Some(end) = self.end {
            let end_index = end.row * 1024 + end.column;
            let start_index = self.start.row * 1024 + self.start.column;
            if end_index < start_index {
                (end, self.start)
            } else {
                (self.start, end)
            }
        } else {
            (self.start, self.start)
        }
    }

    pub fn is_range(&self) -> Option<(Pos, Pos)> {
        if let Some(end) = self.end {
            let end_index = end.row * 1024 + end.column;
            let start_index = self.start.row * 1024 + self.start.column;
            if end_index < start_index {
                Some((end, self.start))
            } else {
                Some((self.start, end))
            }
        } else {
            None
        }
    }

    pub fn get_first(&self) -> Pos {
        if let Some(end) = self.end {
            let end_index = end.row * 1024 + end.column;
            let start_index = self.start.row * 1024 + self.start.column;
            if end_index < start_index {
                end
            } else {
                self.start
            }
        } else {
            self.start
        }
    }

    pub fn get_second(&self) -> Pos {
        if let Some(end) = self.end {
            let end_index = end.row * 1024 + end.column;
            let start_index = self.start.row * 1024 + self.start.column;
            if end_index > start_index {
                end
            } else {
                self.start
            }
        } else {
            self.start
        }
    }

    pub fn extend(&self, new_end: Pos) -> Selection {
        return if self.start == new_end {
            Selection::single_r_c(new_end.row, new_end.column)
        } else {
            Selection::range(self.start, new_end)
        };
    }

    pub fn get_cursor_pos(&self) -> Pos {
        self.end.unwrap_or(self.start)
    }

    pub fn get_row_iter_incl(&self) -> RangeInclusive<usize> {
        let start = self.get_first().row;
        let end = self.get_second().row;
        start..=end
    }

    pub fn get_row_iter_excl(&self) -> Range<usize> {
        let start = self.get_first().row;
        let end = self.get_second().row;
        start..end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowModificationType {
    SingleLine(usize),
    AllLinesFrom(usize),
}

impl RowModificationType {
    fn merge(&mut self, other: Option<&RowModificationType>) {
        let self_row = match self {
            RowModificationType::SingleLine(row) => *row,
            RowModificationType::AllLinesFrom(row) => *row,
        };
        if let Some(other) = other {
            let other_row = match other {
                RowModificationType::SingleLine(row) => row,
                RowModificationType::AllLinesFrom(row) => row,
            };
            *self = match (&self, other) {
                (
                    RowModificationType::SingleLine(self_row),
                    RowModificationType::SingleLine(other_row),
                ) if self_row == other_row => RowModificationType::SingleLine(*self_row),
                _ => RowModificationType::AllLinesFrom(self_row.min(*other_row)),
            };
        }
    }
}

pub struct Editor {
    selection: Selection,
    last_column_index: usize,
    time: u32,
    next_blink_at: u32,
    modif_time_treshold_expires_at: u32,
    show_cursor: bool,
    pub clipboard: String,
}

impl Editor {
    pub fn new<T: Default + Clone + Debug>(content: &mut EditorContent<T>) -> Editor {
        let ed = Editor {
            time: 0,
            selection: Selection::single_r_c(0, 0),
            last_column_index: 0,
            next_blink_at: 0,
            modif_time_treshold_expires_at: 0,
            show_cursor: false,
            clipboard: String::new(),
        };
        content.push_line();
        return ed;
    }

    pub fn is_cursor_at_eol<T: Default + Clone + Debug>(&self, content: &EditorContent<T>) -> bool {
        let cur_pos = self.selection.get_cursor_pos();
        cur_pos.column == content.line_len(cur_pos.row)
    }

    pub fn is_cursor_at_beginning(&self) -> bool {
        let cur_pos = self.selection.get_cursor_pos();
        cur_pos.column == 0
    }

    pub fn send_selection_to_clipboard<T: Default + Clone + Debug>(
        &mut self,
        selection: Selection,
        content: &EditorContent<T>,
    ) {
        self.clipboard.clear();
        let mut dst = std::mem::replace(&mut self.clipboard, String::new());
        content.write_selection_into(selection, &mut dst);
        self.clipboard = dst;
    }

    pub fn get_selection(&self) -> Selection {
        self.selection
    }

    pub fn handle_click<T: Default + Clone + Debug>(
        &mut self,
        x: usize,
        y: usize,
        content: &EditorContent<T>,
    ) {
        let line_count = content.line_count();
        let y = if y >= line_count { line_count - 1 } else { y };

        let col = x.min(content.line_len(y));
        self.set_cursor_pos_r_c(y, col);
    }

    pub fn handle_drag<T: Default + Clone + Debug>(
        &mut self,
        x: usize,
        y: usize,
        content: &EditorContent<T>,
    ) {
        let y = if y >= content.line_count() {
            content.line_count() - 1
        } else {
            y
        };
        let col = x.min(content.line_len(y));
        self.set_selection_save_col(self.selection.extend(Pos::from_row_column(y, col)));
    }

    pub fn get_selected_text_single_line<T: Default + Clone + Debug>(
        selection: Selection,
        content: &EditorContent<T>,
    ) -> Option<&[char]> {
        return if selection.end.is_none() || selection.start.row != selection.end.unwrap().row {
            None
        } else {
            let start = selection.get_first();
            let end = selection.get_second();
            Some(&content.get_line_valid_chars(start.row)[start.column..end.column])
        };
    }

    pub fn clone_range<T: Default + Clone + Debug>(
        start: Pos,
        end: Pos,
        content: &EditorContent<T>,
    ) -> String {
        let mut result = String::with_capacity((end.row - start.row) * content.max_line_len());

        content.write_selection_into(Selection::range(start, end), &mut result);
        result
    }

    #[inline]
    pub fn set_cursor_pos(&mut self, pos: Pos) {
        self.set_selection_save_col(Selection::single(pos));
    }

    #[inline]
    pub fn set_cursor_pos_r_c(&mut self, row_index: usize, column_index: usize) {
        self.set_selection_save_col(Selection::single_r_c(row_index, column_index));
    }

    #[inline]
    pub fn set_cursor_range(&mut self, start: Pos, end: Pos) {
        self.set_selection_save_col(Selection::range(start, end));
    }

    #[inline]
    pub fn set_selection_save_col(&mut self, selection: Selection) {
        self.selection = selection;
        self.last_column_index = selection.get_cursor_pos().column;
        debug_assert!(self.last_column_index <= 120, "{}", self.last_column_index);
    }

    pub fn is_cursor_shown(&self) -> bool {
        self.show_cursor
    }

    pub fn blink_cursor(&mut self) {
        self.show_cursor = true;
        self.next_blink_at = self.time + EDITOR_CURSOR_TICK_MS;
    }

    pub fn handle_tick(&mut self, now: u32) -> bool {
        self.time = now;
        return if now >= self.next_blink_at {
            self.show_cursor = !self.show_cursor;
            self.next_blink_at = now + EDITOR_CURSOR_TICK_MS;
            true
        } else {
            false
        };
    }

    fn create_command<T: Default + Clone + Debug>(
        &self,
        input: &EditorInputEvent,
        modifiers: InputModifiers,
        content: &EditorContent<T>,
    ) -> Option<EditorCommand<T>> {
        let selection = self.selection;
        let cur_pos = selection.get_cursor_pos();
        return match input {
            EditorInputEvent::Home => None,
            EditorInputEvent::End => None,
            EditorInputEvent::PageUp => None,
            EditorInputEvent::PageDown => None,
            EditorInputEvent::Right => None,
            EditorInputEvent::Tab => {
                let target_pos = ((cur_pos.column / 4) + 1) * 4;
                let space_count = target_pos - cur_pos.column;
                // TODO every tab is a string allocation :(
                let str = std::iter::repeat(' ').take(space_count).collect::<String>();
                Some(EditorCommand::InsertText {
                    pos: cur_pos,
                    text: str,
                    is_there_line_overflow: false,
                })
            }
            EditorInputEvent::Up => {
                if modifiers.ctrl && modifiers.shift {
                    return if cur_pos.row == 0 {
                        None
                    } else {
                        Some(EditorCommand::SwapLineUpwards(cur_pos))
                    };
                } else {
                    None
                }
            }
            EditorInputEvent::Left => None,
            EditorInputEvent::Down => {
                if modifiers.ctrl && modifiers.shift {
                    return if cur_pos.row == content.line_count() - 1 {
                        None
                    } else {
                        Some(EditorCommand::SwapLineDownards(cur_pos))
                    };
                } else {
                    None
                }
            }
            EditorInputEvent::Esc => None,
            EditorInputEvent::Del => {
                if let Some((start, end)) = selection.is_range() {
                    Some(EditorCommand::DelSelection {
                        removed_text: Editor::clone_range(start, end, content),
                        selection,
                    })
                } else if cur_pos.column == content.line_len(cur_pos.row) {
                    if cur_pos.row == content.line_count() - 1 {
                        None
                    } else if content.line_len(cur_pos.row) + content.line_len(cur_pos.row + 1)
                        > content.max_line_len()
                    {
                        return None;
                    } else {
                        Some(EditorCommand::MergeLineWithNextRow {
                            upper_row_index: cur_pos.row,
                            upper_line_data: Box::new(content.get_data(cur_pos.row).clone()),
                            lower_line_data: Box::new(content.get_data(cur_pos.row + 1).clone()),
                            pos_before_merge: cur_pos,
                            pos_after_merge: cur_pos,
                        })
                    }
                } else if modifiers.ctrl {
                    let col = content.jump_word_forward(&cur_pos, JumpMode::ConsiderWhitespaces);
                    let removed_text = if col == cur_pos.column {
                        None
                    } else {
                        Some(Editor::clone_range(
                            cur_pos,
                            cur_pos.with_column(col),
                            content,
                        ))
                    };
                    Some(EditorCommand::DelCtrl {
                        removed_text,
                        pos: cur_pos,
                    })
                } else {
                    Some(EditorCommand::Del {
                        removed_char: content.get_char(cur_pos.row, cur_pos.column),
                        pos: cur_pos,
                    })
                }
            }
            EditorInputEvent::Enter => {
                if modifiers.ctrl {
                    Some(EditorCommand::InsertEmptyRow(cur_pos.row))
                } else if let Some((start, end)) = selection.is_range() {
                    Some(EditorCommand::EnterSelection {
                        selection,
                        selected_text: Editor::clone_range(start, end, content),
                    })
                } else {
                    Some(EditorCommand::Enter(cur_pos))
                }
            }
            EditorInputEvent::Backspace => {
                if let Some((start, end)) = selection.is_range() {
                    Some(EditorCommand::BackspaceSelection {
                        removed_text: Editor::clone_range(start, end, content),
                        selection,
                    })
                } else if cur_pos.column == 0 {
                    if cur_pos.row == 0 {
                        None
                    } else if content.line_len(cur_pos.row) + content.line_len(cur_pos.row - 1)
                        > content.max_line_len()
                    {
                        return None;
                    } else {
                        Some(EditorCommand::MergeLineWithNextRow {
                            upper_row_index: cur_pos.row - 1,
                            upper_line_data: Box::new(content.get_data(cur_pos.row - 1).clone()),
                            lower_line_data: Box::new(content.get_data(cur_pos.row).clone()),
                            pos_before_merge: cur_pos,
                            pos_after_merge: Pos::from_row_column(
                                cur_pos.row - 1,
                                content.line_len(cur_pos.row - 1),
                            ),
                        })
                    }
                } else if modifiers.ctrl {
                    let col = content.jump_word_backward(&cur_pos, JumpMode::IgnoreWhitespaces);
                    let removed_text = if col == cur_pos.column {
                        None
                    } else {
                        Some(Editor::clone_range(
                            cur_pos.with_column(col),
                            cur_pos,
                            content,
                        ))
                    };
                    Some(EditorCommand::BackspaceCtrl {
                        removed_text,
                        pos: cur_pos,
                    })
                } else {
                    Some(EditorCommand::Backspace {
                        removed_char: content.get_char(cur_pos.row, cur_pos.column - 1),
                        pos: cur_pos,
                    })
                }
            }
            EditorInputEvent::Char(ch) => {
                if *ch == 'w' && modifiers.ctrl {
                    None
                } else if *ch == 'c' && modifiers.ctrl {
                    None
                } else if *ch == 'x' && modifiers.ctrl {
                    if let Some((start, end)) = selection.is_range() {
                        Some(EditorCommand::DelSelection {
                            selection,
                            removed_text: Editor::clone_range(start, end, content),
                        })
                    } else {
                        Some(EditorCommand::CutLine {
                            pos: cur_pos,
                            removed_text: Editor::clone_range(
                                cur_pos.with_column(0),
                                cur_pos.with_column(content.line_len(cur_pos.row)),
                                content,
                            ),
                        })
                    }
                } else if *ch == 'd' && modifiers.ctrl {
                    Some(EditorCommand::DuplicateLine {
                        pos: cur_pos,
                        inserted_text: Editor::clone_range(
                            cur_pos.with_column(0),
                            cur_pos.with_column(content.line_len(cur_pos.row)),
                            content,
                        ),
                    })
                } else if *ch == 'a' && modifiers.ctrl {
                    None
                } else if ch.to_ascii_lowercase() == 'z' && modifiers.ctrl && modifiers.shift {
                    None
                } else if ch.to_ascii_lowercase() == 'z' && modifiers.ctrl {
                    None
                } else if let Some((start, end)) = selection.is_range() {
                    Some(EditorCommand::InsertCharSelection {
                        ch: *ch,
                        selection,
                        selected_text: Editor::clone_range(start, end, content),
                    })
                } else {
                    if content.line_len(cur_pos.row) == content.max_line_len() {
                        None
                    } else {
                        Some(EditorCommand::InsertChar {
                            pos: cur_pos,
                            ch: *ch,
                        })
                    }
                }
            }
        };
    }

    pub fn insert_text<T: Default + Clone + Debug>(
        &mut self,
        str: &str,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        let selection = self.selection;
        let cur_pos = selection.get_first();
        let inserted_text_end_pos =
            Editor::get_str_range(str, cur_pos.row, cur_pos.column, content.max_line_len());
        let remaining_text_len_in_this_row = content.line_len(cur_pos.row) - cur_pos.column;
        let is_there_line_overflow =
            inserted_text_end_pos.column + remaining_text_len_in_this_row > content.max_line_len();
        let command = if let Some((start, end)) = selection.is_range() {
            EditorCommand::InsertTextSelection {
                selection,
                removed_text: Editor::clone_range(start, end, content),
                text: (*str).to_owned(),
                is_there_line_overflow,
            }
        } else {
            EditorCommand::InsertText {
                pos: cur_pos,
                // TODO: to owned...
                text: (*str).to_owned(),
                is_there_line_overflow,
            }
        };
        return self.execute_user_input(command, content);
    }

    pub fn handle_input<T: Default + Clone + Debug>(
        &mut self,
        input: EditorInputEvent,
        modifiers: InputModifiers,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        if (input == EditorInputEvent::Char('x') || input == EditorInputEvent::Char('c'))
            && modifiers.ctrl
        {
            self.send_selection_to_clipboard(self.selection, content);
        }

        match input {
            EditorInputEvent::Char(ch)
                if ch.to_ascii_lowercase() == 'z' && modifiers.is_ctrl_shift() =>
            {
                self.redo(content)
            }
            EditorInputEvent::Char(ch) if ch.to_ascii_lowercase() == 'z' && modifiers.ctrl => {
                self.undo(content)
            }
            _ => {
                if let Some(command) = self.create_command(&input, modifiers, content) {
                    self.execute_user_input(command, content)
                } else {
                    self.next_blink_at = self.time + EDITOR_CURSOR_TICK_MS;
                    self.show_cursor = true;
                    self.handle_navigation_input(&input, modifiers, content);
                    None
                }
            }
        }
    }

    fn execute_user_input<T: Default + Clone + Debug>(
        &mut self,
        command: EditorCommand<T>,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        self.next_blink_at = self.time + EDITOR_CURSOR_TICK_MS;
        self.show_cursor = true;
        let modif_type = self.do_command(&command, content);
        if modif_type.is_some() {
            if self.modif_time_treshold_expires_at < self.time || content.undo_stack.is_empty() {
                // new undo group
                content.undo_stack.push(Vec::with_capacity(4));
            }
            content.undo_stack.last_mut().unwrap().push(command);
            content.redo_stack.clear();
            self.modif_time_treshold_expires_at = self.time + EDITOR_CURSOR_TICK_MS;
        }
        modif_type
    }

    fn do_command<T: Default + Clone + Debug>(
        &mut self,
        command: &EditorCommand<T>,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        self.show_cursor = true;
        match command {
            EditorCommand::InsertText { pos, text, .. } => {
                let (new_pos, overflow) = content.insert_str_at(*pos, &text);
                self.set_selection_save_col(Selection::single(new_pos));
                if overflow || new_pos.row != pos.row {
                    Some(RowModificationType::AllLinesFrom(pos.row))
                } else {
                    Some(RowModificationType::SingleLine(pos.row))
                }
            }
            EditorCommand::InsertTextSelection {
                selection, text, ..
            } => {
                content.remove_selection(*selection);
                let first = selection.get_first();
                let (new_pos, overflow) = content.insert_str_at(first, &text);
                let second = selection.get_second();
                self.set_selection_save_col(Selection::single(new_pos));
                if !overflow && (new_pos.row == first.row && first.row == second.row) {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
            EditorCommand::SwapLineUpwards(pos) => {
                content.swap_lines_upward(pos.row);
                self.selection = Selection::single(Pos::from_row_column(pos.row - 1, pos.column));
                Some(RowModificationType::AllLinesFrom(pos.row - 1))
            }
            EditorCommand::SwapLineDownards(pos) => {
                content.swap_lines_upward(pos.row + 1);
                self.selection = Selection::single(Pos::from_row_column(pos.row + 1, pos.column));
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::Del {
                removed_char: _,
                pos,
            } => {
                let modif_type = if content.line_len(pos.row) == 0 && content.line_count() > 1 {
                    // if the current row is empty, the next line brings its data with itself
                    content.remove_line_at(pos.row);
                    Some(RowModificationType::AllLinesFrom(pos.row))
                } else if pos.column == content.line_len(pos.row) {
                    if pos.row < content.line_count() - 1 {
                        if content.merge_with_next_row(pos.row, content.line_len(pos.row), 0) {
                            Some(RowModificationType::AllLinesFrom(pos.row))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    content.remove_char(pos.row, pos.column);
                    Some(RowModificationType::SingleLine(pos.row))
                };
                self.selection = Selection::single(*pos);
                modif_type
            }
            EditorCommand::DelSelection {
                removed_text: _,
                selection,
            } => {
                let modif_type = content.remove_selection(*selection);
                if modif_type.is_some() {
                    let selection = Selection::single(selection.get_first());
                    self.set_selection_save_col(selection);
                }
                modif_type
            }
            EditorCommand::DelCtrl {
                removed_text: _removed_text,
                pos,
            } => {
                let col = content.jump_word_forward(&pos, JumpMode::ConsiderWhitespaces);
                let new_pos = pos.with_column(col);
                // TODO csinálj egy optimaliált metódust ami biztos h az adott sorból töröl csak
                content.remove_selection(Selection::range(*pos, new_pos));
                self.selection = Selection::single(*pos);
                Some(RowModificationType::SingleLine(new_pos.row))
            }
            EditorCommand::InsertEmptyRow(_) => {
                // TODO
                // Meg a Ctrl-D-t is
                None
            }
            EditorCommand::EnterSelection {
                selection,
                selected_text: _,
            } => {
                self.handle_enter(*selection, content);
                Some(RowModificationType::AllLinesFrom(selection.get_first().row))
            }
            EditorCommand::Enter(pos) => {
                self.handle_enter(Selection::single(*pos), content);
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::MergeLineWithNextRow {
                upper_row_index,
                upper_line_data: _,
                lower_line_data: _,
                pos_before_merge: _,
                pos_after_merge,
            } => {
                let upper_row_index = *upper_row_index;
                if content.line_len(upper_row_index) == 0 {
                    // if the prev row is empty, the line takes its data with itself
                    content.remove_line_at(upper_row_index);
                    self.set_selection_save_col(Selection::single(*pos_after_merge));
                } else {
                    let prev_len_before_merge = content.line_len(upper_row_index);
                    if content.merge_with_next_row(upper_row_index, prev_len_before_merge, 0) {
                        self.set_selection_save_col(Selection::single(*pos_after_merge));
                    }
                }
                Some(RowModificationType::AllLinesFrom(upper_row_index))
            }
            EditorCommand::Backspace {
                removed_char: _,
                pos,
            } => {
                content.remove_char(pos.row, pos.column - 1);
                self.set_selection_save_col(Selection::single(pos.with_column(pos.column - 1)));
                Some(RowModificationType::SingleLine(pos.row))
            }
            EditorCommand::BackspaceSelection {
                removed_text: _,
                selection,
            } => {
                let modif_type = content.remove_selection(*selection);
                if modif_type.is_some() {
                    self.set_selection_save_col(Selection::single(selection.get_first()));
                }
                modif_type
            }
            EditorCommand::BackspaceCtrl {
                removed_text: _,
                pos,
            } => {
                let col = content.jump_word_backward(pos, JumpMode::IgnoreWhitespaces);
                let new_pos = pos.with_column(col);
                content.remove_selection(Selection::range(new_pos, *pos));
                self.set_selection_save_col(Selection::single(new_pos));
                Some(RowModificationType::SingleLine(pos.row))
            }
            EditorCommand::InsertChar { pos, ch } => {
                if content.insert_char(pos.row, pos.column, *ch) {
                    self.set_selection_save_col(Selection::single(pos.with_next_col()));
                    Some(RowModificationType::SingleLine(pos.row))
                } else {
                    None
                }
            }
            EditorCommand::InsertCharSelection {
                ch,
                selection,
                selected_text: _,
            } => {
                let first = selection.get_first();
                let second = selection.get_second();
                if first.column == content.max_line_len {
                    None
                } else {
                    let merged_len_then_inserted_len =
                        first.column + (content.line_len(second.row) - second.column) + 1;
                    if merged_len_then_inserted_len > content.max_line_len {
                        return None;
                    }
                    let modif_type =
                        content.remove_selection(Selection::range(first, selection.get_second()));
                    if modif_type.is_some() {
                        content.insert_char(first.row, first.column, *ch);
                        self.set_selection_save_col(Selection::single(
                            selection.get_first().with_next_col(),
                        ));
                    }
                    modif_type
                }
            }
            EditorCommand::CutLine {
                pos,
                removed_text: _removed_text,
            } => {
                self.send_selection_to_clipboard(
                    Selection::range(
                        pos.with_column(0),
                        pos.with_column(content.line_len(pos.row)),
                    ),
                    content,
                );
                if content.line_count() > pos.row + 1 {
                    self.clipboard.push('\n');
                    content.remove_line_at(pos.row);
                } else {
                    content.remove_selection(Selection::range(
                        pos.with_column(0),
                        pos.with_column(content.line_len(pos.row)),
                    ));
                }
                self.set_selection_save_col(Selection::single(pos.with_column(0)));
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::DuplicateLine {
                pos,
                inserted_text: _inserted_text,
            } => {
                content.duplicate_line(pos.row);
                self.set_selection_save_col(Selection::single(pos.with_next_row()));
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
        }
    }

    pub fn get_str_range(str: &str, row_index: usize, insert_at: usize, maxlen: usize) -> Pos {
        let mut col = insert_at;
        let mut row = row_index;
        for ch in str.chars() {
            if ch == '\r' {
                // ignore
                continue;
            } else if ch == '\n' {
                row += 1;
                col = 0;
                continue;
            } else if col == maxlen {
                row += 1;
                col = 0;
            }
            col += 1;
        }
        return Pos::from_row_column(row, col);
    }

    pub fn handle_navigation_input<T: Default + Clone + Debug>(
        &mut self,
        input: &EditorInputEvent,
        modifiers: InputModifiers,
        content: &EditorContent<T>,
    ) {
        let cur_pos = self.selection.get_cursor_pos();

        match input {
            EditorInputEvent::PageUp => {
                let new_pos = Pos::from_row_column(0, 0);
                let new_selection = if modifiers.shift {
                    self.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
                self.set_selection_save_col(new_selection);
            }
            EditorInputEvent::PageDown => {
                let new_pos = Pos::from_row_column(
                    content.line_count() - 1,
                    content.line_len(content.line_count() - 1),
                );
                let new_selection = if modifiers.shift {
                    self.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
                self.set_selection_save_col(new_selection);
            }
            EditorInputEvent::Home => {
                let new_pos = cur_pos.with_column(0);
                let new_selection = if modifiers.shift {
                    self.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
                self.set_selection_save_col(new_selection);
            }
            EditorInputEvent::End => {
                let new_pos = cur_pos.with_column(content.line_len(cur_pos.row));
                let new_selection = if modifiers.shift {
                    self.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
                self.set_selection_save_col(new_selection);
            }
            EditorInputEvent::Right => {
                let new_pos = if cur_pos.column + 1 > content.line_len(cur_pos.row) {
                    if cur_pos.row + 1 < content.line_count() {
                        Pos::from_row_column(cur_pos.row + 1, 0)
                    } else {
                        cur_pos
                    }
                } else {
                    let col = if modifiers.ctrl {
                        content.jump_word_forward(&cur_pos, JumpMode::IgnoreWhitespaces)
                    } else {
                        cur_pos.column + 1
                    };
                    cur_pos.with_column(col)
                };
                let selection = if modifiers.shift {
                    self.selection.extend(new_pos)
                } else {
                    if let Some((_start, end)) = self.selection.is_range() {
                        Selection::single(end)
                    } else {
                        Selection::single(new_pos)
                    }
                };
                self.set_selection_save_col(selection);
            }
            EditorInputEvent::Left => {
                let new_pos = if cur_pos.column == 0 {
                    if cur_pos.row >= 1 {
                        Pos::from_row_column(cur_pos.row - 1, content.line_len(cur_pos.row - 1))
                    } else {
                        cur_pos
                    }
                } else {
                    let col = if modifiers.ctrl {
                        // check the type of the prev char
                        content.jump_word_backward(&cur_pos, JumpMode::IgnoreWhitespaces)
                    } else {
                        cur_pos.column - 1
                    };
                    cur_pos.with_column(col)
                };

                let selection = if modifiers.shift {
                    self.selection.extend(new_pos)
                } else {
                    if let Some((start, _end)) = self.selection.is_range() {
                        Selection::single(start)
                    } else {
                        Selection::single(new_pos)
                    }
                };
                self.set_selection_save_col(selection);
            }
            EditorInputEvent::Up => {
                if modifiers.ctrl && modifiers.shift {
                    return;
                }
                let new_pos = if cur_pos.row == 0 {
                    cur_pos.with_column(0)
                } else {
                    Pos::from_row_column(
                        cur_pos.row - 1,
                        self.last_column_index
                            .min(content.line_len(cur_pos.row - 1)),
                    )
                };
                self.selection = if modifiers.shift {
                    self.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
            }
            EditorInputEvent::Down => {
                if modifiers.ctrl && modifiers.shift {
                    return;
                }
                let new_pos = if cur_pos.row == content.line_count() - 1 {
                    cur_pos.with_column(content.line_len(cur_pos.row))
                } else {
                    Pos::from_row_column(
                        cur_pos.row + 1,
                        self.last_column_index
                            .min(content.line_len(cur_pos.row + 1)),
                    )
                };
                self.selection = if modifiers.shift {
                    self.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
            }
            EditorInputEvent::Char(ch) => {
                let selection = self.selection;
                if *ch == 'w' && modifiers.ctrl {
                    let prev_index = content.jump_word_backward(
                        &selection.get_first(),
                        if selection.is_range().is_some() {
                            JumpMode::IgnoreWhitespaces
                        } else {
                            JumpMode::BlockOnWhitespace
                        },
                    );
                    let next_index = content.jump_word_forward(
                        &selection.get_second(),
                        if selection.is_range().is_some() {
                            JumpMode::IgnoreWhitespaces
                        } else {
                            JumpMode::BlockOnWhitespace
                        },
                    );
                    self.set_selection_save_col(Selection::range(
                        cur_pos.with_column(prev_index),
                        cur_pos.with_column(next_index),
                    ));
                } else if *ch == 'a' && modifiers.ctrl {
                    self.set_selection_save_col(Selection::range(
                        Pos::from_row_column(0, 0),
                        Pos::from_row_column(
                            content.line_count() - 1,
                            content.line_len(content.line_count() - 1),
                        ),
                    ));
                }
            }
            EditorInputEvent::Del
            | EditorInputEvent::Esc
            | EditorInputEvent::Enter
            | EditorInputEvent::Backspace
            | EditorInputEvent::Tab => {}
        };
    }

    pub(super) fn undo<T: Default + Clone + Debug>(
        &mut self,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        let mut sum_modif_type: Option<RowModificationType> = None;
        if let Some(command_group) = content.undo_stack.pop() {
            for command in command_group.iter().rev() {
                let modif_type = self.undo_command(command, content);
                if let Some(sum_modif_type) = &mut sum_modif_type {
                    sum_modif_type.merge(modif_type.as_ref());
                } else {
                    sum_modif_type = modif_type;
                }
            }
            content.redo_stack.push(command_group);
        };
        sum_modif_type
    }

    pub(super) fn redo<T: Default + Clone + Debug>(
        &mut self,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        let mut sum_modif_type: Option<RowModificationType> = None;
        if let Some(command_group) = content.redo_stack.pop() {
            for command in command_group.iter() {
                let modif_type = self.do_command(command, content);
                if let Some(sum_modif_type) = &mut sum_modif_type {
                    sum_modif_type.merge(modif_type.as_ref());
                } else {
                    sum_modif_type = modif_type;
                }
            }
            content.undo_stack.push(command_group);
        };
        sum_modif_type
    }

    fn undo_command<T: Default + Clone + Debug>(
        &mut self,
        command: &EditorCommand<T>,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        match command {
            EditorCommand::SwapLineUpwards(pos) => {
                content.swap_lines_upward(pos.row);
                self.selection = Selection::single(*pos);
                Some(RowModificationType::AllLinesFrom(pos.row - 1))
            }
            EditorCommand::SwapLineDownards(pos) => {
                content.swap_lines_upward(pos.row + 1);
                self.selection = Selection::single(*pos);
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::Del { removed_char, pos } => {
                content.insert_char(pos.row, pos.column, *removed_char);
                self.set_selection_save_col(Selection::single(*pos));
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::DelSelection {
                removed_text,
                selection,
            } => {
                content.insert_str_at(selection.get_first(), &removed_text);
                self.set_selection_save_col(*selection);
                let first = selection.get_first();
                let second = selection.get_first();
                if first.row == second.row {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
            EditorCommand::DelCtrl { removed_text, pos } => {
                let modif_type = if let Some(removed_text) = removed_text {
                    let (new_pos, overflow) = content.insert_str_at(*pos, removed_text);
                    if !overflow && new_pos.row == pos.row {
                        Some(RowModificationType::SingleLine(pos.row))
                    } else {
                        Some(RowModificationType::AllLinesFrom(pos.row))
                    }
                } else {
                    None
                };
                self.set_selection_save_col(Selection::single(*pos));
                modif_type
            }
            EditorCommand::MergeLineWithNextRow {
                upper_row_index,
                upper_line_data,
                lower_line_data,
                pos_before_merge,
                pos_after_merge,
            } => {
                content.split_line(*upper_row_index, pos_after_merge.column);
                *content.mut_data(*upper_row_index) = upper_line_data.as_ref().clone();
                *content.mut_data(*upper_row_index + 1) = lower_line_data.as_ref().clone();
                self.set_selection_save_col(Selection::single(*pos_before_merge));
                Some(RowModificationType::AllLinesFrom(*upper_row_index))
            }
            EditorCommand::InsertEmptyRow(_) => {
                // TODO
                None
            }
            EditorCommand::EnterSelection {
                selection,
                selected_text,
            } => {
                let first = selection.get_first();
                content.merge_with_next_row(first.row, first.column, 0);
                content.insert_str_at(first, selected_text);
                self.set_selection_save_col(*selection);
                Some(RowModificationType::AllLinesFrom(first.row))
            }
            EditorCommand::Enter(pos) => {
                content.merge_with_next_row(pos.row, pos.column, 0);
                self.set_selection_save_col(Selection::single(*pos));
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::Backspace { removed_char, pos } => {
                content.insert_char(pos.row, pos.column - 1, *removed_char);
                self.set_selection_save_col(Selection::single(*pos));
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::BackspaceSelection {
                removed_text,
                selection,
            } => {
                let first = selection.get_first();
                content.insert_str_at(first, removed_text);
                self.set_selection_save_col(*selection);
                if first.row == selection.get_second().row {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
            EditorCommand::BackspaceCtrl { removed_text, pos } => {
                let modif_type = if let Some(removed_text) = removed_text {
                    let col = pos.column - removed_text.chars().count();
                    let (new_pos, overflow) =
                        content.insert_str_at(pos.with_column(col), removed_text);
                    if !overflow && new_pos.row == pos.row {
                        Some(RowModificationType::SingleLine(pos.row))
                    } else {
                        Some(RowModificationType::AllLinesFrom(pos.row))
                    }
                } else {
                    None
                };
                self.set_selection_save_col(Selection::single(*pos));
                modif_type
            }
            EditorCommand::InsertChar { pos, ch: _ } => {
                content.remove_char(pos.row, pos.column);
                self.set_selection_save_col(Selection::single(*pos));
                Some(RowModificationType::SingleLine(pos.row))
            }
            EditorCommand::InsertCharSelection {
                ch: _,
                selection,
                selected_text,
            } => {
                let first = selection.get_first();
                content.remove_char(first.row, first.column);
                content.insert_str_at(first, selected_text);
                self.set_selection_save_col(*selection);
                if selection.get_first().row == selection.get_second().row {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
            EditorCommand::CutLine { pos, removed_text } => {
                if pos.row != content.line_count() - 1 {
                    content.insert_line_at(pos.row);
                }
                content.insert_str_at(pos.with_column(0), removed_text);
                self.set_selection_save_col(Selection::single(*pos));
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::DuplicateLine { pos, .. } => {
                content.remove_line_at(pos.row + 1);
                self.set_selection_save_col(Selection::single(*pos));
                Some(RowModificationType::AllLinesFrom(pos.row + 1))
            }
            EditorCommand::InsertText {
                pos,
                text,
                is_there_line_overflow,
            } => {
                // calc the range of the pasted text
                let first = *pos;
                let inserted_text_range = Selection::range(
                    first,
                    Editor::get_str_range(text, first.row, first.column, content.max_line_len()),
                );
                content.remove_selection(inserted_text_range);
                if *is_there_line_overflow {
                    //originally the next line was part of this line, so merge them
                    content.merge_with_next_row(first.row, content.line_len(first.row), 0);
                }

                self.set_selection_save_col(Selection::single(*pos));
                if !*is_there_line_overflow
                    && inserted_text_range.get_first().row == inserted_text_range.get_second().row
                {
                    Some(RowModificationType::SingleLine(pos.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(pos.row))
                }
            }
            EditorCommand::InsertTextSelection {
                selection,
                text,
                removed_text,
                is_there_line_overflow,
            } => {
                // calc the range of the pasted text
                let first = selection.get_first();
                let inserted_text_range = Selection::range(
                    first,
                    Editor::get_str_range(text, first.row, first.column, content.max_line_len()),
                );
                content.remove_selection(inserted_text_range);
                let (end_pos, _overflow) = content.insert_str_at(first, removed_text);
                if *is_there_line_overflow {
                    //originally the next line was part of this line, so merge them
                    content.merge_with_next_row(end_pos.row, content.line_len(end_pos.row), 0);
                }
                self.set_selection_save_col(*selection);
                if !*is_there_line_overflow
                    && inserted_text_range.get_first().row == inserted_text_range.get_second().row
                {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
        }
    }

    fn handle_enter<T: Default + Clone + Debug>(
        &mut self,
        selection: Selection,
        content: &mut EditorContent<T>,
    ) {
        if let Some((first, second)) = selection.is_range() {
            content.remove_selection(Selection::range(first, second));
            content.split_line(first.row, first.column);
            self.set_selection_save_col(Selection::single(Pos::from_row_column(first.row + 1, 0)));
        } else {
            let cur_pos = selection.get_cursor_pos();
            if cur_pos.column == 0 {
                // the whole row is moved down, so take its line data as well
                content.insert_line_at(cur_pos.row);
            } else {
                content.split_line(cur_pos.row, cur_pos.column);
            }
            self.set_selection_save_col(Selection::single(Pos::from_row_column(
                cur_pos.row + 1,
                0,
            )));
        }
    }
}
