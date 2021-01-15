use crate::editor::editor_content::{EditDelta, EditorCommand, EditorContent, JumpMode};
use crate::LineData;
use std::fmt::Debug;
use std::ops::{Range, RangeInclusive};

pub const EDITOR_CURSOR_TICK_MS: u32 = 500;
pub const TAB_SIZE: usize = 4;

pub const SPACES: [&str; 4] = [" ", "  ", "   ", "    "];
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
        self.ctrl && self.shift
    }

    pub fn is_none(&self) -> bool {
        !self.ctrl && !self.shift && !self.alt
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

    pub fn sub_column(&self, col: usize) -> Pos {
        Pos {
            column: self.column - col.min(self.column),
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

    pub fn get_range_ordered(&self) -> (Pos, Pos) {
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

    pub fn get_range(&self) -> (Pos, Pos) {
        if let Some(end) = self.end {
            (self.start, end)
        } else {
            (self.start, self.start)
        }
    }

    pub fn is_range(&self) -> bool {
        self.end.is_some()
    }

    pub fn is_range_unordered(&self) -> Option<(Pos, Pos)> {
        if let Some(end) = self.end {
            Some((self.start, end))
        } else {
            None
        }
    }

    pub fn is_range_ordered(&self) -> Option<(Pos, Pos)> {
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

    pub fn clone_with_start(&self, start: Pos) -> Selection {
        Selection {
            start,
            end: self.end,
        }
    }

    pub fn clone_with_end(&self, end: Pos) -> Selection {
        Selection {
            start: self.start,
            end: Some(end),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowModificationType {
    SingleLine(usize),
    AllLinesFrom(usize),
}

impl RowModificationType {
    pub fn merge(&mut self, other: Option<&RowModificationType>) {
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

#[derive(Debug)]
struct CursorData {
    selection: Selection, // Vec when multicursor
    last_column_index: usize,
}

impl CursorData {
    #[inline]
    #[allow(dead_code)]
    pub fn set_cursor_pos(&mut self, pos: Pos) {
        self.set_selection_save_col(Selection::single(pos));
    }

    #[inline]
    #[allow(dead_code)]
    pub fn set_cursor_pos_r_c(&mut self, row_index: usize, column_index: usize) {
        self.set_selection_save_col(Selection::single_r_c(row_index, column_index));
    }

    #[inline]
    #[allow(dead_code)]
    pub fn set_cursor_range(&mut self, start: Pos, end: Pos) {
        self.set_selection_save_col(Selection::range(start, end));
    }

    #[inline]
    pub fn set_selection_save_col(&mut self, selection: Selection) {
        self.selection = selection;
        self.last_column_index = selection.get_cursor_pos().column;
        debug_assert!(self.last_column_index <= 120, "{}", self.last_column_index);
    }
}

type EditorCommandGroup = Vec<EditDelta>;

#[derive(Debug)]
pub struct Editor<T: Default + Clone + Debug> {
    // TODO: need for fuzz testing, set it back to priv later
    pub undo_stack: Vec<EditorCommandGroup>,
    pub command_history: Vec<EditorCommand<T>>,
    pub redo_stack: Vec<EditorCommandGroup>,
    time: u32,
    next_blink_at: u32,
    modif_time_treshold_expires_at: u32,
    show_cursor: bool,
    pub clipboard: String,
    cursor: CursorData,
}

impl<T: Default + Clone + Debug> Editor<T> {
    pub fn new(content: &mut EditorContent<T>) -> Editor<T> {
        let ed = Editor {
            command_history: Vec::with_capacity(512),
            undo_stack: Vec::with_capacity(32),
            redo_stack: Vec::with_capacity(32),
            time: 0,
            cursor: CursorData {
                selection: Selection::single_r_c(0, 0),
                last_column_index: 0,
            },
            next_blink_at: 0,
            modif_time_treshold_expires_at: 0,
            show_cursor: false,
            clipboard: String::new(),
        };
        content.push_line();
        return ed;
    }

    pub fn reset(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.time = 0;
        self.cursor.selection = Selection::single_r_c(0, 0);
        self.cursor.last_column_index = 0;
        self.next_blink_at = 0;
        self.modif_time_treshold_expires_at = 0;
        self.show_cursor = false;
    }

    #[inline]
    pub fn set_cursor_pos(&mut self, pos: Pos) {
        self.cursor.set_selection_save_col(Selection::single(pos));
    }

    #[inline]
    pub fn set_cursor_pos_r_c(&mut self, row_index: usize, column_index: usize) {
        self.cursor
            .set_selection_save_col(Selection::single_r_c(row_index, column_index));
    }

    #[inline]
    pub fn set_cursor_range(&mut self, start: Pos, end: Pos) {
        self.cursor
            .set_selection_save_col(Selection::range(start, end));
    }

    #[inline]
    pub fn set_selection_save_col(&mut self, selection: Selection) {
        self.cursor.set_selection_save_col(selection);
    }

    pub fn is_cursor_at_eol(&self, content: &EditorContent<T>) -> bool {
        let cur_pos = self.cursor.selection.get_cursor_pos();
        cur_pos.column == content.line_len(cur_pos.row)
    }

    pub fn is_cursor_at_beginning(&self) -> bool {
        let cur_pos = self.cursor.selection.get_cursor_pos();
        cur_pos.column == 0
    }

    pub fn send_selection_to_clipboard(
        clipboard: &mut String,
        selection: Selection,
        content: &EditorContent<T>,
    ) {
        clipboard.clear();
        let mut dst = std::mem::replace(clipboard, String::new());
        content.write_selection_into(selection, &mut dst);
        *clipboard = dst;
    }

    pub fn get_selection(&self) -> Selection {
        self.cursor.selection
    }

    pub fn handle_click(&mut self, x: usize, y: usize, content: &EditorContent<T>) {
        let line_count = content.line_count();
        let y = if y >= line_count { line_count - 1 } else { y };

        let col = x.min(content.line_len(y));
        self.set_cursor_pos_r_c(y, col);
    }

    pub fn handle_drag(&mut self, x: usize, y: usize, content: &EditorContent<T>) {
        let y = if y >= content.line_count() {
            content.line_count() - 1
        } else {
            y
        };
        let col = x.min(content.line_len(y));
        self.set_selection_save_col(self.cursor.selection.extend(Pos::from_row_column(y, col)));
    }

    pub fn get_selected_text_single_line(
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

    pub fn clone_range(start: Pos, end: Pos, content: &EditorContent<T>) -> String {
        let mut result = String::with_capacity((end.row - start.row) * content.max_line_len());

        content.write_selection_into(Selection::range(start, end), &mut result);
        result
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

    fn create_command(
        selection: Selection,
        input: &EditorInputEvent,
        modifiers: InputModifiers,
        content: &EditorContent<T>,
        commands: &mut Vec<EditorCommand<T>>,
    ) {
        let cur_pos = selection.get_cursor_pos();
        match input {
            EditorInputEvent::Home
            | EditorInputEvent::End
            | EditorInputEvent::PageUp
            | EditorInputEvent::PageDown
            | EditorInputEvent::Right => {
                return;
            }
            EditorInputEvent::Tab => {
                let rust_is_a_joke = selection
                    .is_range_ordered()
                    .map(|it| (it.0.row, it.1.row))
                    .or_else(|| {
                        if modifiers.shift {
                            Some((selection.start.row, selection.start.row))
                        } else {
                            None
                        }
                    });
                if let Some((start_row, end_row)) = rust_is_a_joke {
                    for row in start_row..=end_row {
                        // space count at beginning
                        let space_count_at_beginning_of_line = content
                            .get_line_valid_chars(row)
                            .iter()
                            .take_while(|it| **it == ' ')
                            .count();
                        if modifiers.shift {
                            let target_pos = ((space_count_at_beginning_of_line.max(1) - 1)
                                / TAB_SIZE)
                                * TAB_SIZE;
                            let space_count = space_count_at_beginning_of_line - target_pos;
                            if space_count > 0 {
                                commands.push(EditorCommand::Unident { row, space_count });
                            }
                        } else {
                            let space_count_at_beginning_of_line =
                                space_count_at_beginning_of_line.min(4);
                            let target_pos =
                                ((space_count_at_beginning_of_line / TAB_SIZE) + 1) * TAB_SIZE;
                            let space_count = target_pos - space_count_at_beginning_of_line;
                            commands.push(EditorCommand::Indent { row, space_count });
                        }
                    }
                } else {
                    let target_pos = ((cur_pos.column / TAB_SIZE) + 1) * TAB_SIZE;
                    let space_count = target_pos - cur_pos.column;
                    // TODO every tab is a string allocation :(
                    let str = std::iter::repeat(' ').take(space_count).collect::<String>();
                    commands.push(EditorCommand::InsertText {
                        text: str,
                        is_there_line_overflow: false,
                    });
                }
            }
            EditorInputEvent::Up => {
                if modifiers.ctrl && modifiers.shift {
                    if cur_pos.row == 0 {
                    } else {
                        commands.push(EditorCommand::SwapLineUpwards);
                    };
                } else {
                }
            }
            EditorInputEvent::Left => {}
            EditorInputEvent::Down => {
                if modifiers.ctrl && modifiers.shift {
                    if cur_pos.row == content.line_count() - 1 {
                    } else {
                        commands.push(EditorCommand::SwapLineDownards);
                    };
                } else {
                }
            }
            EditorInputEvent::Esc => {}
            EditorInputEvent::Del => {
                if let Some((start, end)) = selection.is_range_ordered() {
                    commands.push(EditorCommand::DelSelection {
                        removed_text: Editor::clone_range(start, end, content),
                    });
                } else if cur_pos.column == content.line_len(cur_pos.row) {
                    if cur_pos.row == content.line_count() - 1 {
                    } else if content.line_len(cur_pos.row) + content.line_len(cur_pos.row + 1)
                        > content.max_line_len()
                    {
                    } else {
                        commands.push(EditorCommand::MergeLineWithNextRow {
                            upper_row_index: cur_pos.row,
                            upper_line_data: Box::new(content.get_data(cur_pos.row).clone()),
                            lower_line_data: Box::new(content.get_data(cur_pos.row + 1).clone()),
                            cursor_pos_after_merge: cur_pos,
                        });
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
                    commands.push(EditorCommand::DelCtrl { removed_text });
                } else {
                    commands.push(EditorCommand::Del {
                        removed_char: content.get_char(cur_pos.row, cur_pos.column),
                    });
                }
            }
            EditorInputEvent::Enter => {
                if modifiers.ctrl {
                    commands.push(EditorCommand::InsertEmptyRow);
                } else if let Some((start, end)) = selection.is_range_ordered() {
                    commands.push(EditorCommand::EnterSelection {
                        selected_text: Editor::clone_range(start, end, content),
                    });
                } else {
                    commands.push(EditorCommand::Enter);
                }
            }
            EditorInputEvent::Backspace => {
                if let Some((start, end)) = selection.is_range_ordered() {
                    commands.push(EditorCommand::BackspaceSelection {
                        removed_text: Editor::clone_range(start, end, content),
                    });
                } else if cur_pos.column == 0 {
                    if cur_pos.row == 0 {
                    } else if content.line_len(cur_pos.row) + content.line_len(cur_pos.row - 1)
                        > content.max_line_len()
                    {
                    } else {
                        commands.push(EditorCommand::MergeLineWithNextRow {
                            upper_row_index: cur_pos.row - 1,
                            upper_line_data: Box::new(content.get_data(cur_pos.row - 1).clone()),
                            lower_line_data: Box::new(content.get_data(cur_pos.row).clone()),
                            cursor_pos_after_merge: Pos::from_row_column(
                                cur_pos.row - 1,
                                content.line_len(cur_pos.row - 1),
                            ),
                        });
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
                    commands.push(EditorCommand::BackspaceCtrl { removed_text });
                } else {
                    commands.push(EditorCommand::Backspace {
                        removed_char: content.get_char(cur_pos.row, cur_pos.column - 1),
                    });
                }
            }
            EditorInputEvent::Char(ch) => {
                if *ch == 'w' && modifiers.ctrl {
                } else if *ch == 'c' && modifiers.ctrl {
                } else if *ch == 'x' && modifiers.ctrl {
                    if let Some((start, end)) = selection.is_range_ordered() {
                        commands.push(EditorCommand::DelSelection {
                            removed_text: Editor::clone_range(start, end, content),
                        });
                    } else {
                        commands.push(EditorCommand::CutLine {
                            removed_text: Editor::clone_range(
                                cur_pos.with_column(0),
                                cur_pos.with_column(content.line_len(cur_pos.row)),
                                content,
                            ),
                        });
                    }
                } else if *ch == 'd' && modifiers.ctrl {
                    commands.push(EditorCommand::DuplicateLine {
                        inserted_text: Editor::clone_range(
                            cur_pos.with_column(0),
                            cur_pos.with_column(content.line_len(cur_pos.row)),
                            content,
                        ),
                    });
                } else if *ch == 'a' && modifiers.ctrl {
                } else if ch.to_ascii_lowercase() == 'z' && modifiers.ctrl && modifiers.shift {
                } else if ch.to_ascii_lowercase() == 'z' && modifiers.ctrl {
                } else if let Some((start, end)) = selection.is_range_ordered() {
                    commands.push(EditorCommand::InsertCharSelection {
                        ch: *ch,
                        selected_text: Editor::clone_range(start, end, content),
                    });
                } else if content.line_len(cur_pos.row) == content.max_line_len() {
                } else {
                    commands.push(EditorCommand::InsertChar { ch: *ch });
                }
            }
        };
    }

    pub fn insert_text_no_undo(
        &mut self,
        str: &str,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        self.insert_text(str, content, false)
    }

    pub fn insert_text_undoable(
        &mut self,
        str: &str,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        self.insert_text(str, content, true)
    }

    fn insert_text(
        &mut self,
        str: &str,
        content: &mut EditorContent<T>,
        undoable: bool,
    ) -> Option<RowModificationType> {
        let selection = self.cursor.selection;
        let cur_pos = selection.get_first();
        let inserted_text_end_pos = Editor::<LineData>::get_str_range(
            str,
            cur_pos.row,
            cur_pos.column,
            content.max_line_len(),
        );
        let remaining_text_len_in_this_row = content.line_len(cur_pos.row) - cur_pos.column;
        let is_there_line_overflow =
            inserted_text_end_pos.column + remaining_text_len_in_this_row > content.max_line_len();
        let command = if let Some((start, end)) = selection.is_range_ordered() {
            EditorCommand::InsertTextSelection {
                removed_text: Editor::clone_range(start, end, content),
                text: (*str).to_owned(),
                is_there_line_overflow,
            }
        } else {
            EditorCommand::InsertText {
                // TODO: to owned...
                text: (*str).to_owned(),
                is_there_line_overflow,
            }
        };
        let delta = EditDelta {
            start_i: self.command_history.len(),
            end_i: self.command_history.len() + 1,
            before_cursor: self.cursor.selection,
        };
        self.command_history.push(command);
        return self.execute_user_input(delta, content, undoable);
    }

    pub fn handle_input_no_undo(
        &mut self,
        input: EditorInputEvent,
        modifiers: InputModifiers,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        self.handle_input(input, modifiers, content, false)
    }

    pub fn handle_input_undoable(
        &mut self,
        input: EditorInputEvent,
        modifiers: InputModifiers,
        content: &mut EditorContent<T>,
    ) -> Option<RowModificationType> {
        self.handle_input(input, modifiers, content, true)
    }

    fn handle_input(
        &mut self,
        input: EditorInputEvent,
        modifiers: InputModifiers,
        content: &mut EditorContent<T>,
        undoable: bool,
    ) -> Option<RowModificationType> {
        if (input == EditorInputEvent::Char('x') || input == EditorInputEvent::Char('c'))
            && modifiers.ctrl
        {
            Editor::send_selection_to_clipboard(
                &mut self.clipboard,
                self.cursor.selection,
                content,
            );
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
                let before_len = self.command_history.len();
                Editor::create_command(
                    self.cursor.selection,
                    &input,
                    modifiers,
                    content,
                    &mut self.command_history,
                );
                let after_len = self.command_history.len();
                return if before_len != after_len {
                    self.execute_user_input(
                        EditDelta {
                            start_i: before_len,
                            end_i: after_len,
                            before_cursor: self.cursor.selection,
                        },
                        content,
                        undoable,
                    )
                } else {
                    self.next_blink_at = self.time + EDITOR_CURSOR_TICK_MS;
                    self.show_cursor = true;
                    self.handle_navigation_input(&input, modifiers, content);
                    None
                };
            }
        }
    }

    fn execute_user_input(
        &mut self,
        delta: EditDelta,
        content: &mut EditorContent<T>,
        undo_allowed: bool,
    ) -> Option<RowModificationType> {
        self.next_blink_at = self.time + EDITOR_CURSOR_TICK_MS;
        self.show_cursor = true;
        let mut modif_type: Option<RowModificationType> = None;
        for command in &self.command_history[delta.start_i..delta.end_i] {
            let local_modif_type =
                Editor::do_command(command, content, &mut self.cursor, &mut self.clipboard);
            if let Some(mut modif_type) = modif_type {
                modif_type.merge(local_modif_type.as_ref());
            } else {
                modif_type = local_modif_type;
            }
        }
        if modif_type.is_some() && undo_allowed {
            if self.modif_time_treshold_expires_at < self.time || self.undo_stack.is_empty() {
                // new undo group
                self.undo_stack.push(Vec::with_capacity(4));
            }
            self.undo_stack.last_mut().unwrap().push(delta);
            self.redo_stack.clear();
            self.modif_time_treshold_expires_at = self.time + EDITOR_CURSOR_TICK_MS;
        }
        modif_type
    }

    fn do_command(
        command: &EditorCommand<T>,
        content: &mut EditorContent<T>,
        cursor: &mut CursorData,
        clipboard: &mut String,
    ) -> Option<RowModificationType> {
        let selection = cursor.selection;
        let pos = selection.get_cursor_pos();
        match command {
            EditorCommand::Indent { row, space_count } => {
                content.insert_str_at(Pos::from_row_column(*row, 0), &SPACES[*space_count - 1]);
                if *row == selection.start.row {
                    cursor.set_selection_save_col(
                        cursor
                            .selection
                            .clone_with_start(selection.start.add_column(*space_count)),
                    );
                }
                if let Some(end) = selection.end {
                    if end.row == *row {
                        cursor.set_selection_save_col(
                            cursor
                                .selection
                                .clone_with_end(end.add_column(*space_count)),
                        );
                    }
                }
                Some(RowModificationType::AllLinesFrom(*row))
            }
            EditorCommand::Unident { row, space_count } => {
                content.remove_selection(Selection::range(
                    Pos::from_row_column(*row, 0),
                    Pos::from_row_column(*row, *space_count),
                ));
                if *row == selection.start.row {
                    cursor.set_selection_save_col(
                        cursor
                            .selection
                            .clone_with_start(selection.start.sub_column(*space_count)),
                    );
                }
                if let Some(end) = selection.end {
                    if end.row == *row {
                        cursor.set_selection_save_col(
                            cursor
                                .selection
                                .clone_with_end(end.sub_column(*space_count)),
                        );
                    }
                }
                Some(RowModificationType::AllLinesFrom(*row))
            }
            EditorCommand::InsertText { text, .. } => {
                let (new_pos, overflow) = content.insert_str_at(pos, &text);
                cursor.set_selection_save_col(Selection::single(new_pos));
                if overflow || new_pos.row != pos.row {
                    Some(RowModificationType::AllLinesFrom(pos.row))
                } else {
                    Some(RowModificationType::SingleLine(pos.row))
                }
            }
            EditorCommand::InsertTextSelection { text, .. } => {
                content.remove_selection(selection);
                let first = selection.get_first();
                let (new_pos, overflow) = content.insert_str_at(first, &text);
                let second = selection.get_second();
                cursor.set_selection_save_col(Selection::single(new_pos));
                if !overflow && (new_pos.row == first.row && first.row == second.row) {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
            EditorCommand::SwapLineUpwards => {
                content.swap_lines_upward(pos.row);
                cursor.selection = Selection::single(Pos::from_row_column(pos.row - 1, pos.column));
                Some(RowModificationType::AllLinesFrom(pos.row - 1))
            }
            EditorCommand::SwapLineDownards => {
                content.swap_lines_upward(pos.row + 1);
                cursor.selection = Selection::single(pos.with_next_row());
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::Del { removed_char: _ } => {
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
                cursor.selection = Selection::single(pos);
                modif_type
            }
            EditorCommand::DelSelection { removed_text: _ } => {
                let modif_type = content.remove_selection(selection);
                if modif_type.is_some() {
                    let selection = Selection::single(selection.get_first());
                    cursor.set_selection_save_col(selection);
                }
                modif_type
            }
            EditorCommand::DelCtrl {
                removed_text: _removed_text,
            } => {
                let col = content.jump_word_forward(&pos, JumpMode::ConsiderWhitespaces);
                let new_pos = pos.with_column(col);
                // TODO csinálj egy optimalizált metódust ami biztos h az adott sorból töröl csak
                content.remove_selection(Selection::range(pos, new_pos));
                cursor.selection = Selection::single(pos);
                Some(RowModificationType::SingleLine(new_pos.row))
            }
            EditorCommand::InsertEmptyRow => {
                // TODO
                // Meg a Ctrl-D-t is
                None
            }
            EditorCommand::EnterSelection { selected_text: _ } => {
                Editor::handle_enter(selection, content, cursor);
                Some(RowModificationType::AllLinesFrom(selection.get_first().row))
            }
            EditorCommand::Enter => {
                Editor::handle_enter(Selection::single(pos), content, cursor);
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::MergeLineWithNextRow {
                upper_row_index,
                upper_line_data: _,
                lower_line_data: _,
                cursor_pos_after_merge: pos_after_merge,
            } => {
                let upper_row_index = *upper_row_index;
                if content.line_len(upper_row_index) == 0 {
                    // if the prev row is empty, the line takes its data with itself
                    content.remove_line_at(upper_row_index);
                    cursor.set_selection_save_col(Selection::single(*pos_after_merge));
                } else {
                    let prev_len_before_merge = content.line_len(upper_row_index);
                    if content.merge_with_next_row(upper_row_index, prev_len_before_merge, 0) {
                        cursor.set_selection_save_col(Selection::single(*pos_after_merge));
                    }
                }
                Some(RowModificationType::AllLinesFrom(upper_row_index))
            }
            EditorCommand::Backspace { removed_char: _ } => {
                content.remove_char(pos.row, pos.column - 1);
                cursor.set_selection_save_col(Selection::single(pos.with_column(pos.column - 1)));
                Some(RowModificationType::SingleLine(pos.row))
            }
            EditorCommand::BackspaceSelection { removed_text: _ } => {
                let modif_type = content.remove_selection(selection);
                if modif_type.is_some() {
                    cursor.set_selection_save_col(Selection::single(selection.get_first()));
                }
                modif_type
            }
            EditorCommand::BackspaceCtrl { removed_text: _ } => {
                let col = content.jump_word_backward(&pos, JumpMode::IgnoreWhitespaces);
                let new_pos = pos.with_column(col);
                content.remove_selection(Selection::range(new_pos, pos));
                cursor.set_selection_save_col(Selection::single(new_pos));
                Some(RowModificationType::SingleLine(pos.row))
            }
            EditorCommand::InsertChar { ch } => {
                if content.insert_char(pos.row, pos.column, *ch) {
                    cursor.set_selection_save_col(Selection::single(pos.with_next_col()));
                    Some(RowModificationType::SingleLine(pos.row))
                } else {
                    None
                }
            }
            EditorCommand::InsertCharSelection {
                ch,
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
                        cursor.set_selection_save_col(Selection::single(
                            selection.get_first().with_next_col(),
                        ));
                    }
                    modif_type
                }
            }
            EditorCommand::CutLine {
                removed_text: _removed_text,
            } => {
                Editor::send_selection_to_clipboard(
                    clipboard,
                    Selection::range(
                        pos.with_column(0),
                        pos.with_column(content.line_len(pos.row)),
                    ),
                    content,
                );
                if content.line_count() > pos.row + 1 {
                    clipboard.push('\n');
                    content.remove_line_at(pos.row);
                } else {
                    content.remove_selection(Selection::range(
                        pos.with_column(0),
                        pos.with_column(content.line_len(pos.row)),
                    ));
                }
                cursor.set_selection_save_col(Selection::single(pos.with_column(0)));
                Some(RowModificationType::AllLinesFrom(pos.row))
            }
            EditorCommand::DuplicateLine {
                inserted_text: _inserted_text,
            } => {
                content.duplicate_line(pos.row);
                cursor.set_selection_save_col(Selection::single(pos.with_next_row()));
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

    pub fn handle_navigation_input(
        &mut self,
        input: &EditorInputEvent,
        modifiers: InputModifiers,
        content: &EditorContent<T>,
    ) {
        let cursor = &mut self.cursor;
        let cur_pos = cursor.selection.get_cursor_pos();

        match input {
            EditorInputEvent::PageUp => {
                let new_pos = Pos::from_row_column(0, 0);
                let new_selection = if modifiers.shift {
                    cursor.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
                cursor.set_selection_save_col(new_selection);
            }
            EditorInputEvent::PageDown => {
                let new_pos = Pos::from_row_column(
                    content.line_count() - 1,
                    content.line_len(content.line_count() - 1),
                );
                let new_selection = if modifiers.shift {
                    cursor.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
                cursor.set_selection_save_col(new_selection);
            }
            EditorInputEvent::Home => {
                let new_pos = cur_pos.with_column(0);
                let new_selection = if modifiers.shift {
                    cursor.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
                cursor.set_selection_save_col(new_selection);
            }
            EditorInputEvent::End => {
                let new_pos = cur_pos.with_column(content.line_len(cur_pos.row));
                let new_selection = if modifiers.shift {
                    cursor.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
                cursor.set_selection_save_col(new_selection);
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
                    cursor.selection.extend(new_pos)
                } else if let Some((_start, end)) = cursor.selection.is_range_ordered() {
                    Selection::single(end)
                } else {
                    Selection::single(new_pos)
                };
                cursor.set_selection_save_col(selection);
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
                    cursor.selection.extend(new_pos)
                } else if let Some((start, _end)) = cursor.selection.is_range_ordered() {
                    Selection::single(start)
                } else {
                    Selection::single(new_pos)
                };
                cursor.set_selection_save_col(selection);
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
                        cursor
                            .last_column_index
                            .min(content.line_len(cur_pos.row - 1)),
                    )
                };
                cursor.selection = if modifiers.shift {
                    cursor.selection.extend(new_pos)
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
                        cursor
                            .last_column_index
                            .min(content.line_len(cur_pos.row + 1)),
                    )
                };
                cursor.selection = if modifiers.shift {
                    cursor.selection.extend(new_pos)
                } else {
                    Selection::single(new_pos)
                };
            }
            EditorInputEvent::Char(ch) => {
                let selection = cursor.selection;
                if *ch == 'w' && modifiers.ctrl {
                    let prev_index = content.jump_word_backward(
                        &selection.get_first(),
                        if selection.is_range() {
                            JumpMode::IgnoreWhitespaces
                        } else {
                            JumpMode::BlockOnWhitespace
                        },
                    );
                    let next_index = content.jump_word_forward(
                        &selection.get_second(),
                        if selection.is_range() {
                            JumpMode::IgnoreWhitespaces
                        } else {
                            JumpMode::BlockOnWhitespace
                        },
                    );
                    cursor.set_selection_save_col(Selection::range(
                        cur_pos.with_column(prev_index),
                        cur_pos.with_column(next_index),
                    ));
                } else if *ch == 'a' && modifiers.ctrl {
                    cursor.set_selection_save_col(Selection::range(
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

    pub(super) fn undo(&mut self, content: &mut EditorContent<T>) -> Option<RowModificationType> {
        let mut sum_modif_type: Option<RowModificationType> = None;
        if let Some(delta_group) = self.undo_stack.pop() {
            for delta in delta_group.iter().rev() {
                for command in &self.command_history[delta.start_i..delta.end_i] {
                    let modif_type = self.undo_command(command, content, delta.before_cursor);
                    self.cursor.selection = delta.before_cursor;
                    if let Some(sum_modif_type) = &mut sum_modif_type {
                        sum_modif_type.merge(modif_type.as_ref());
                    } else {
                        sum_modif_type = modif_type;
                    }
                }
            }
            self.redo_stack.push(delta_group);
        };
        sum_modif_type
    }

    pub(super) fn redo(&mut self, content: &mut EditorContent<T>) -> Option<RowModificationType> {
        let mut sum_modif_type: Option<RowModificationType> = None;
        self.show_cursor = true;
        if let Some(delta_group) = self.redo_stack.pop() {
            for delta in delta_group.iter() {
                self.cursor.selection = delta.before_cursor;
                for command in &self.command_history[delta.start_i..delta.end_i] {
                    let modif_type =
                        Editor::do_command(command, content, &mut self.cursor, &mut self.clipboard);
                    if let Some(sum_modif_type) = &mut sum_modif_type {
                        sum_modif_type.merge(modif_type.as_ref());
                    } else {
                        sum_modif_type = modif_type;
                    }
                }
            }
            self.undo_stack.push(delta_group);
        };
        sum_modif_type
    }

    fn undo_command(
        &self,
        command: &EditorCommand<T>,
        content: &mut EditorContent<T>,
        cursor_before: Selection,
    ) -> Option<RowModificationType> {
        let pos_before = cursor_before.get_cursor_pos();
        match command {
            EditorCommand::SwapLineUpwards => {
                content.swap_lines_upward(pos_before.row);
                Some(RowModificationType::AllLinesFrom(pos_before.row - 1))
            }
            EditorCommand::SwapLineDownards => {
                content.swap_lines_upward(pos_before.row + 1);
                Some(RowModificationType::AllLinesFrom(pos_before.row))
            }
            EditorCommand::Del { removed_char } => {
                content.insert_char(pos_before.row, pos_before.column, *removed_char);
                Some(RowModificationType::AllLinesFrom(pos_before.row))
            }
            EditorCommand::DelSelection { removed_text } => {
                content.insert_str_at(cursor_before.get_first(), &removed_text);
                let first = cursor_before.get_first();
                let second = cursor_before.get_second();
                if first.row == second.row {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
            EditorCommand::DelCtrl { removed_text } => {
                let modif_type = if let Some(removed_text) = removed_text {
                    let (new_pos, overflow) = content.insert_str_at(pos_before, removed_text);
                    if !overflow && new_pos.row == pos_before.row {
                        Some(RowModificationType::SingleLine(pos_before.row))
                    } else {
                        Some(RowModificationType::AllLinesFrom(pos_before.row))
                    }
                } else {
                    None
                };
                modif_type
            }
            EditorCommand::MergeLineWithNextRow {
                upper_row_index,
                upper_line_data,
                lower_line_data,
                cursor_pos_after_merge: pos_after_merge,
            } => {
                content.split_line(*upper_row_index, pos_after_merge.column);
                *content.mut_data(*upper_row_index) = upper_line_data.as_ref().clone();
                *content.mut_data(*upper_row_index + 1) = lower_line_data.as_ref().clone();
                Some(RowModificationType::AllLinesFrom(*upper_row_index))
            }
            EditorCommand::InsertEmptyRow => {
                // TODO
                None
            }
            EditorCommand::EnterSelection { selected_text } => {
                let first = cursor_before.get_first();
                content.merge_with_next_row(first.row, first.column, 0);
                content.insert_str_at(first, selected_text);
                Some(RowModificationType::AllLinesFrom(first.row))
            }
            EditorCommand::Enter => {
                content.merge_with_next_row(pos_before.row, pos_before.column, 0);
                Some(RowModificationType::AllLinesFrom(pos_before.row))
            }
            EditorCommand::Backspace { removed_char } => {
                content.insert_char(pos_before.row, pos_before.column - 1, *removed_char);
                Some(RowModificationType::AllLinesFrom(pos_before.row))
            }
            EditorCommand::BackspaceSelection { removed_text } => {
                let first = cursor_before.get_first();
                content.insert_str_at(first, removed_text);
                if first.row == cursor_before.get_second().row {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
            EditorCommand::BackspaceCtrl { removed_text } => {
                let modif_type = if let Some(removed_text) = removed_text {
                    let col = pos_before.column - removed_text.chars().count();
                    let (new_pos, overflow) =
                        content.insert_str_at(pos_before.with_column(col), removed_text);
                    if !overflow && new_pos.row == pos_before.row {
                        Some(RowModificationType::SingleLine(pos_before.row))
                    } else {
                        Some(RowModificationType::AllLinesFrom(pos_before.row))
                    }
                } else {
                    None
                };
                modif_type
            }
            EditorCommand::InsertChar { ch: _ } => {
                content.remove_char(pos_before.row, pos_before.column);
                Some(RowModificationType::SingleLine(pos_before.row))
            }
            EditorCommand::InsertCharSelection {
                ch: _,
                selected_text,
            } => {
                let first = cursor_before.get_first();
                content.remove_char(first.row, first.column);
                content.insert_str_at(first, selected_text);
                if cursor_before.get_first().row == cursor_before.get_second().row {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
            EditorCommand::CutLine { removed_text } => {
                if pos_before.row != content.line_count() - 1 {
                    content.insert_line_at(pos_before.row);
                }
                content.insert_str_at(pos_before.with_column(0), removed_text);
                Some(RowModificationType::AllLinesFrom(pos_before.row))
            }
            EditorCommand::DuplicateLine { .. } => {
                content.remove_line_at(pos_before.row + 1);
                Some(RowModificationType::AllLinesFrom(pos_before.row + 1))
            }
            EditorCommand::InsertText {
                text,
                is_there_line_overflow,
            } => {
                let first = pos_before;
                let inserted_text_range = Selection::range(
                    first,
                    Editor::<LineData>::get_str_range(
                        text,
                        first.row,
                        first.column,
                        content.max_line_len(),
                    ),
                );
                content.remove_selection(inserted_text_range);
                if *is_there_line_overflow {
                    //originally the next line was part of this line, so merge them
                    content.merge_with_next_row(first.row, content.line_len(first.row), 0);
                }

                if !*is_there_line_overflow
                    && inserted_text_range.get_first().row == inserted_text_range.get_second().row
                {
                    Some(RowModificationType::SingleLine(pos_before.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(pos_before.row))
                }
            }
            EditorCommand::InsertTextSelection {
                text,
                removed_text,
                is_there_line_overflow,
            } => {
                // calc the range of the pasted text
                let first = cursor_before.get_first();
                let inserted_text_range = Selection::range(
                    first,
                    Editor::<T>::get_str_range(
                        text,
                        first.row,
                        first.column,
                        content.max_line_len(),
                    ),
                );
                content.remove_selection(inserted_text_range);
                let (end_pos, _overflow) = content.insert_str_at(first, removed_text);
                if *is_there_line_overflow {
                    //originally the next line was part of this line, so merge them
                    content.merge_with_next_row(end_pos.row, content.line_len(end_pos.row), 0);
                }
                if !*is_there_line_overflow
                    && inserted_text_range.get_first().row == inserted_text_range.get_second().row
                {
                    Some(RowModificationType::SingleLine(first.row))
                } else {
                    Some(RowModificationType::AllLinesFrom(first.row))
                }
            }
            EditorCommand::Indent { row, space_count } => {
                content.remove_selection(Selection::range(
                    Pos::from_row_column(*row, 0),
                    Pos::from_row_column(*row, *space_count),
                ));
                Some(RowModificationType::AllLinesFrom(*row))
            }
            EditorCommand::Unident { row, space_count } => {
                content.insert_str_at(Pos::from_row_column(*row, 0), &SPACES[*space_count - 1]);
                Some(RowModificationType::AllLinesFrom(*row))
            }
        }
    }

    fn handle_enter(selection: Selection, content: &mut EditorContent<T>, cursor: &mut CursorData) {
        if let Some((first, second)) = selection.is_range_ordered() {
            content.remove_selection(Selection::range(first, second));
            content.split_line(first.row, first.column);
            cursor
                .set_selection_save_col(Selection::single(Pos::from_row_column(first.row + 1, 0)));
        } else {
            let cur_pos = selection.get_cursor_pos();
            if cur_pos.column == 0 {
                // the whole row is moved down, so take its line data as well
                content.insert_line_at(cur_pos.row);
            } else {
                content.split_line(cur_pos.row, cur_pos.column);
            }
            cursor.set_selection_save_col(Selection::single(Pos::from_row_column(
                cur_pos.row + 1,
                0,
            )));
        }
    }
}
