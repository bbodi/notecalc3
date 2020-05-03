use crate::editor::editor_content::{EditorCommand, EditorContent, JumpMode};

#[derive(Eq, PartialEq, Debug, Clone)]
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
    Char(char),
    Text(String),
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

    pub fn is_range(&self) -> bool {
        return self.end.is_some();
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
    pub fn new<T: Default + Clone>(content: &mut EditorContent<T>) -> Editor {
        let mut ed = Editor {
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

    pub fn is_cursor_at_eol<T: Default + Clone>(&self, content: &EditorContent<T>) -> bool {
        let cur_pos = self.selection.get_cursor_pos();
        cur_pos.column == content.line_len(cur_pos.row)
    }

    pub fn is_cursor_at_beginning(&self) -> bool {
        let cur_pos = self.selection.get_cursor_pos();
        cur_pos.column == 0
    }

    pub fn send_selection_to_clipboard<T: Default + Clone>(
        &mut self,
        selection: Selection,
        content: &EditorContent<T>,
    ) {
        self.clipboard.clear();
        // shitty borrow checker
        let mut dst = std::mem::replace(&mut self.clipboard, String::new());
        content.write_selection_into(selection, &mut dst);
        self.clipboard = dst;
    }

    pub fn get_selection(&self) -> Selection {
        self.selection
    }

    pub fn handle_click<T: Default + Clone>(
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

    pub fn handle_drag<T: Default + Clone>(
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

    pub fn get_selected_text_single_line<T: Default + Clone>(
        selection: Selection,
        content: &EditorContent<T>,
    ) -> Option<&[char]> {
        return if selection.end.is_none() || selection.start.row != selection.end.unwrap().row {
            None
        } else {
            let start = selection.get_first();
            let end = selection.get_second();
            Some(&content.get_line_chars(start.row)[start.column..end.column])
        };
    }

    pub fn clone_selected_text<T: Default + Clone>(
        selection: Selection,
        content: &EditorContent<T>,
    ) -> Option<String> {
        return if selection.end.is_none() {
            None
        } else {
            let start = selection.get_first();
            let end = selection.get_second();
            let mut result = String::with_capacity((end.row - start.row) * content.max_line_len());

            content.write_selection_into(selection, &mut result);
            Some(result)
        };
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
    }

    pub fn is_cursor_shown(&self) -> bool {
        self.show_cursor
    }

    pub fn blink_cursor(&mut self) {
        self.show_cursor = true;
        self.next_blink_at = self.time + 500;
    }

    pub fn handle_tick(&mut self, now: u32) -> bool {
        self.time = now;
        return if now >= self.next_blink_at {
            self.show_cursor = !self.show_cursor;
            self.next_blink_at = now + 500;
            true
        } else {
            false
        };
    }

    fn create_command<T: Default + Clone>(
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
                if let Some(end) = selection.end {
                    Some(EditorCommand::DelSelection {
                        removed_text: Editor::clone_selected_text(selection, content).unwrap(),
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
                    let removed_text = Editor::clone_selected_text(
                        Selection::range(cur_pos, cur_pos.with_column(col)),
                        content,
                    );
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
                } else if selection.is_range() {
                    Some(EditorCommand::EnterSelection {
                        selection,
                        selected_text: Editor::clone_selected_text(selection, content).unwrap(),
                    })
                } else {
                    Some(EditorCommand::Enter(cur_pos))
                }
            }
            EditorInputEvent::Backspace => {
                if selection.is_range() {
                    Some(EditorCommand::BackspaceSelection {
                        removed_text: Editor::clone_selected_text(selection, content).unwrap(),
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
                    let removed_text = Editor::clone_selected_text(
                        Selection::range(cur_pos.with_column(col), cur_pos),
                        content,
                    );
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
                    if selection.is_range() {
                        Some(EditorCommand::DelSelection {
                            selection,
                            removed_text: Editor::clone_selected_text(selection, content).unwrap(),
                        })
                    } else {
                        Some(EditorCommand::CutLine(cur_pos))
                    }
                } else if *ch == 'd' && modifiers.ctrl {
                    Some(EditorCommand::DuplicateLine(cur_pos))
                } else if *ch == 'a' && modifiers.ctrl {
                    None
                } else if *ch == 'z' && modifiers.ctrl && modifiers.shift {
                    None
                } else if *ch == 'z' && modifiers.ctrl {
                    None
                } else if selection.is_range() {
                    Some(EditorCommand::InsertCharSelection {
                        ch: *ch,
                        selection,
                        selected_text: Editor::clone_selected_text(selection, content).unwrap(),
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
            EditorInputEvent::Text(str) => {
                let cur_pos = selection.get_first();
                let inserted_text_end_pos =
                    Editor::get_str_range(str, cur_pos.row, cur_pos.column, content.max_line_len());
                let remaining_text_len_in_this_row = content.line_len(cur_pos.row) - cur_pos.column;
                let is_there_line_overflow = inserted_text_end_pos.column
                    + remaining_text_len_in_this_row
                    > content.max_line_len();
                if selection.is_range() {
                    Some(EditorCommand::InsertTextSelection {
                        selection,
                        removed_text: Editor::clone_selected_text(selection, content).unwrap(),
                        text: str.clone(),
                        is_there_line_overflow,
                    })
                } else {
                    Some(EditorCommand::InsertText {
                        pos: cur_pos,
                        text: str.clone(),
                        is_there_line_overflow,
                    })
                }
            }
        };
    }

    pub fn handle_input<T: Default + Clone>(
        &mut self,
        input: EditorInputEvent,
        modifiers: InputModifiers,
        content: &mut EditorContent<T>,
    ) -> bool {
        if (input == EditorInputEvent::Char('x') || input == EditorInputEvent::Char('c'))
            && modifiers.ctrl
        {
            self.send_selection_to_clipboard(self.selection, content);
        }

        if input == EditorInputEvent::Char('z') && modifiers.is_ctrl_shift() {
            self.redo(content);
            true
        } else if input == EditorInputEvent::Char('z') && modifiers.ctrl {
            self.undo(content);
            true
        } else if let Some(command) = self.create_command(&input, modifiers, content) {
            self.next_blink_at = self.time + 500;
            self.show_cursor = true;
            self.do_command(&command, content);
            if self.modif_time_treshold_expires_at < self.time || content.undo_stack.is_empty() {
                // new undo group
                content.undo_stack.push(Vec::with_capacity(4));
            }
            content.undo_stack.last_mut().unwrap().push(command);
            self.modif_time_treshold_expires_at = self.time + 500;
            true
        } else {
            self.next_blink_at = self.time + 500;
            self.show_cursor = true;
            self.handle_navigation_input(&input, modifiers, content);
            false
        }
    }

    fn do_command<T: Default + Clone>(
        &mut self,
        command: &EditorCommand<T>,
        content: &mut EditorContent<T>,
    ) {
        self.show_cursor = true;
        match command {
            EditorCommand::InsertText { pos, text, .. } => {
                let new_pos = content.insert_str_at(*pos, &text);
                self.set_selection_save_col(Selection::single(new_pos));
            }
            EditorCommand::InsertTextSelection {
                selection, text, ..
            } => {
                content.remove_selection(*selection);
                let new_pos = content.insert_str_at(selection.get_first(), &text);
                self.set_selection_save_col(Selection::single(new_pos));
            }
            EditorCommand::SwapLineUpwards(pos) => {
                content.swap_lines_upward(pos.row);
                self.selection = Selection::single(Pos::from_row_column(pos.row - 1, pos.column));
            }
            EditorCommand::SwapLineDownards(pos) => {
                content.swap_lines_upward(pos.row + 1);
                self.selection = Selection::single(Pos::from_row_column(pos.row + 1, pos.column));
            }
            EditorCommand::Del { removed_char, pos } => {
                if content.line_len(pos.row) == 0 && content.line_count() > 1 {
                    // if the current row is empty, the next line brings its data with itself
                    content.remove_line_at(pos.row);
                } else if pos.column == content.line_len(pos.row) {
                    if pos.row < content.line_count() - 1 {
                        content.merge_with_next_row(pos.row, content.line_len(pos.row), 0);
                    }
                } else {
                    content.remove_char(pos.row, pos.column);
                }
                self.selection = Selection::single(*pos);
            }
            EditorCommand::DelSelection {
                removed_text,
                selection,
            } => {
                content.remove_selection(*selection);
                let selection = Selection::single(selection.get_first());
                self.set_selection_save_col(selection);
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
            }
            EditorCommand::InsertEmptyRow(_) => {}
            EditorCommand::EnterSelection {
                selection,
                selected_text,
            } => {
                self.handle_enter(*selection, content);
            }
            EditorCommand::Enter(pos) => {
                self.handle_enter(Selection::single(*pos), content);
            }
            EditorCommand::MergeLineWithNextRow {
                upper_row_index,
                upper_line_data,
                lower_line_data,
                pos_before_merge,
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
            }
            EditorCommand::Backspace { removed_char, pos } => {
                if content.remove_char(pos.row, pos.column - 1) {
                    self.set_selection_save_col(Selection::single(pos.with_column(pos.column - 1)));
                }
            }
            EditorCommand::BackspaceSelection {
                removed_text,
                selection,
            } => {
                content.remove_selection(*selection);
                self.set_selection_save_col(Selection::single(selection.get_first()));
            }
            EditorCommand::BackspaceCtrl { removed_text, pos } => {
                let col = content.jump_word_backward(pos, JumpMode::IgnoreWhitespaces);
                let new_pos = pos.with_column(col);
                content.remove_selection(Selection::range(new_pos, *pos));
                self.set_selection_save_col(Selection::single(new_pos));
            }
            EditorCommand::InsertChar { pos, ch } => {
                if content.insert_char(pos.row, pos.column, *ch) {
                    self.set_selection_save_col(Selection::single(pos.with_next_col()));
                }
            }
            EditorCommand::InsertCharSelection {
                ch,
                selection,
                selected_text,
            } => {
                let mut first = selection.get_first();
                if content.remove_selection(Selection::range(
                    first.with_next_col(),
                    selection.get_second(),
                )) {
                    content.set_char(first.row, first.column, *ch);
                }
                self.set_selection_save_col(Selection::single(
                    selection.get_first().with_next_col(),
                ));
            }
            EditorCommand::CutLine(pos) => {
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
            }
            EditorCommand::DuplicateLine(pos) => {
                content.duplicate_line(pos.row);
                self.set_selection_save_col(Selection::single(pos.with_next_row()));
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

    pub fn handle_navigation_input<T: Default + Clone>(
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
                    if self.selection.is_range() {
                        Selection::single(self.selection.get_second())
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
                    if self.selection.is_range() {
                        Selection::single(self.selection.get_first())
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
            | EditorInputEvent::Text(_) => {}
        };
    }

    pub(super) fn undo<T: Default + Clone>(&mut self, content: &mut EditorContent<T>) {
        if let Some(command_group) = content.undo_stack.pop() {
            for command in command_group.iter().rev() {
                self.undo_command(command, content);
            }
            content.redo_stack.push(command_group);
        };
    }

    pub(super) fn redo<T: Default + Clone>(&mut self, content: &mut EditorContent<T>) {
        if let Some(command_group) = content.redo_stack.pop() {
            for command in command_group.iter() {
                self.do_command(command, content);
            }
            content.undo_stack.push(command_group);
        };
    }

    fn undo_command<T: Default + Clone>(
        &mut self,
        command: &EditorCommand<T>,
        content: &mut EditorContent<T>,
    ) {
        match command {
            EditorCommand::SwapLineUpwards(pos) => {
                content.swap_lines_upward(pos.row);
                self.selection = Selection::single(*pos);
            }
            EditorCommand::SwapLineDownards(pos) => {
                content.swap_lines_upward(pos.row + 1);
                self.selection = Selection::single(*pos);
            }
            EditorCommand::Del { removed_char, pos } => {
                content.insert_char(pos.row, pos.column, *removed_char);
                self.set_selection_save_col(Selection::single(*pos));
            }
            EditorCommand::DelSelection {
                removed_text,
                selection,
            } => {
                content.insert_str_at(selection.get_first(), &removed_text);
                self.set_selection_save_col(*selection);
            }
            EditorCommand::DelCtrl { removed_text, pos } => {
                if let Some(removed_text) = removed_text {
                    content.insert_str_at(*pos, removed_text);
                }
                self.set_selection_save_col(Selection::single(*pos));
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
            }
            EditorCommand::InsertEmptyRow(_) => {}
            EditorCommand::EnterSelection {
                selection,
                selected_text,
            } => {
                let first = selection.get_first();
                content.merge_with_next_row(first.row, first.column, 0);
                content.insert_str_at(first, selected_text);
                self.set_selection_save_col(*selection);
            }
            EditorCommand::Enter(pos) => {
                content.merge_with_next_row(pos.row, pos.column, 0);
                self.set_selection_save_col(Selection::single(*pos));
            }
            EditorCommand::Backspace { removed_char, pos } => {
                content.insert_char(pos.row, pos.column - 1, *removed_char);
                self.set_selection_save_col(Selection::single(*pos));
            }
            EditorCommand::BackspaceSelection {
                removed_text,
                selection,
            } => {
                content.insert_str_at(selection.get_first(), removed_text);
                self.set_selection_save_col(*selection);
            }
            EditorCommand::BackspaceCtrl { removed_text, pos } => {
                if let Some(removed_text) = removed_text {
                    let col = pos.column - removed_text.chars().count();
                    content.insert_str_at(pos.with_column(col), removed_text);
                }
                self.set_selection_save_col(Selection::single(*pos));
            }
            EditorCommand::InsertChar { pos, ch } => {
                content.remove_char(pos.row, pos.column);
                self.set_selection_save_col(Selection::single(*pos));
            }
            EditorCommand::InsertCharSelection {
                ch,
                selection,
                selected_text,
            } => {
                let first = selection.get_first();
                content.remove_char(first.row, first.column);
                content.insert_str_at(first, selected_text);
                self.set_selection_save_col(*selection);
            }
            EditorCommand::CutLine(_) => {}
            EditorCommand::DuplicateLine(_) => {}
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
                let end_pos = content.insert_str_at(first, removed_text);
                if *is_there_line_overflow {
                    //originally the next line was part of this line, so merge them
                    content.merge_with_next_row(end_pos.row, content.line_len(end_pos.row), 0);
                }
                self.set_selection_save_col(*selection);
            }
        }
    }

    fn handle_enter<T: Default + Clone>(
        &mut self,
        selection: Selection,
        content: &mut EditorContent<T>,
    ) {
        if let Some(end) = selection.end {
            let first_cursor = selection.get_first();
            content.remove_selection(selection);
            content.split_line(first_cursor.row, first_cursor.column);
            self.set_selection_save_col(Selection::single(Pos::from_row_column(
                first_cursor.row + 1,
                0,
            )));
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
