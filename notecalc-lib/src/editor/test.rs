#[cfg(test)]
mod tests {
    use crate::editor::editor::{
        Editor, EditorInputEvent, InputModifiers, Pos, RowModificationType, Selection,
    };
    use crate::editor::editor_content::EditorContent;

    const CURSOR_MARKER: char = '█';
    // U+2770	❰	e2 9d b0	HEAVY LEFT-POINTING ANGLE BRACKET OR­NA­MENT
    const SELECTION_START_MARK: char = '❱';
    const SELECTION_END_MARK: char = '❰';

    #[derive(Clone)]
    struct TestParams2<'a> {
        initial_content: &'a str,
        text_input: Option<&'static str>,
        inputs: &'a [EditorInputEvent],
        delay_after_inputs: &'a [u32],
        modifiers: InputModifiers,
        expected_content: &'a str,
    }

    #[derive(Clone)]
    struct TestParams<'a> {
        initial_content: &'a str,
        inputs: &'a [EditorInputEvent],
        delay_after_inputs: &'a [u32],
        text_input: Option<&'static str>,
        modifiers: InputModifiers,
        undo_count: usize,
        redo_count: usize,
        expected_content: &'a str,
    }

    fn test_normal_undo_redo(params: TestParams2) {
        // normal test
        let mut content = EditorContent::<usize>::new(80);
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: params.text_input,
                initial_content: params.initial_content,
                inputs: params.inputs,
                delay_after_inputs: params.delay_after_inputs,
                modifiers: params.modifiers,
                undo_count: 0,
                redo_count: 0,
                expected_content: params.expected_content,
            },
        );
        // undo test
        let mut content = EditorContent::<usize>::new(80);
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: params.text_input,
                initial_content: params.initial_content,
                inputs: params.inputs,
                delay_after_inputs: params.delay_after_inputs,
                modifiers: params.modifiers,
                undo_count: 1,
                redo_count: 0,
                expected_content: params.initial_content,
            },
        );
        // redo test
        let mut content = EditorContent::<usize>::new(80);
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: params.text_input,
                initial_content: params.initial_content,
                inputs: params.inputs,
                delay_after_inputs: params.delay_after_inputs,
                modifiers: params.modifiers,
                undo_count: 1,
                redo_count: 1,
                expected_content: params.expected_content,
            },
        );
    }

    fn test_undo(params: TestParams) {
        let mut content = EditorContent::<usize>::new(80);
        let mut editor = Editor::new(&mut content);
        test0(&mut editor, &mut content, params);
    }

    fn test(
        initial_content: &'static str,
        inputs: &[EditorInputEvent],
        modifiers: InputModifiers,
        expected_content: &'static str,
    ) {
        let mut content = EditorContent::<usize>::new(80);
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content,
                inputs,
                delay_after_inputs: &[],
                modifiers,
                undo_count: 0,
                redo_count: 0,
                expected_content,
            },
        );
    }

    /// the strings in the parameter list are kind of a markup language
    /// '|' marks the cursor's position. If there are two of them, then
    /// it means a selection's begin and end.
    fn test0(editor: &mut Editor, content: &mut EditorContent<usize>, params: TestParams) {
        // we can assume here that it does not contain illegal or complex input
        // so we can just set it as it is
        let mut selection_found = false;
        let mut selection_start = Pos { row: 0, column: 0 };
        let mut selection_end = Pos { row: 0, column: 0 };
        for (row_index, line) in params.initial_content.lines().enumerate() {
            let mut row_len = 0;
            for char in line.chars() {
                if char == CURSOR_MARKER {
                    editor.set_cursor_pos_r_c(row_index, row_len);
                } else if char == SELECTION_START_MARK {
                    selection_found = true;
                    selection_start = Pos {
                        row: row_index,
                        column: row_len,
                    };
                } else if char == SELECTION_END_MARK {
                    selection_end = Pos {
                        row: row_index,
                        column: row_len,
                    };
                } else {
                    content.set_char(row_index, row_len, char);
                    row_len += 1;
                }
            }
            if content.line_lens.len() <= row_index {
                content.push_line();
            }
            content.line_lens[row_index] = row_len;
        }
        if selection_found {
            editor.set_cursor_range(selection_start, selection_end);
        }

        if let Some(text) = params.text_input {
            editor.insert_text(text, content);
        }
        let mut now = 0;
        for (i, input) in params.inputs.iter().enumerate() {
            editor.handle_input(input.clone(), params.modifiers, content);
            if i < params.delay_after_inputs.len() {
                now += params.delay_after_inputs[i];
                editor.handle_tick(now);
            }
        }

        for _ in 0..params.undo_count {
            editor.undo(content);
        }

        for _ in 0..params.redo_count {
            editor.redo(content);
        }

        // assert
        let editor: &Editor = editor;
        let mut expected_cursor = Selection::single_r_c(0, 0);
        let mut expected_selection_start = Pos { row: 0, column: 0 };
        let mut expected_selection_end = Pos { row: 0, column: 0 };
        let mut selection_found = false;
        for (row_index, expected_line) in params.expected_content.lines().enumerate() {
            let mut expected_row_len = 0;
            for char in expected_line.chars() {
                if char == CURSOR_MARKER {
                    expected_cursor = Selection::single_r_c(row_index, expected_row_len);
                } else if char == SELECTION_START_MARK {
                    selection_found = true;
                    expected_selection_start = Pos {
                        row: row_index,
                        column: expected_row_len,
                    }
                } else if char == SELECTION_END_MARK {
                    expected_selection_end = Pos {
                        row: row_index,
                        column: expected_row_len,
                    }
                } else {
                    assert_eq!(
                        content.get_line_chars(row_index)[expected_row_len],
                        char,
                        "row: {}, column: {}, chars: {:?}",
                        row_index,
                        expected_row_len,
                        content
                            .get_line_chars(row_index)
                            .iter()
                            .take_while(|it| **it as u8 != 0)
                            .collect::<String>()
                    );
                    expected_row_len += 1;
                }
            }

            assert_eq!(
                params.expected_content.lines().count(),
                content.line_count(),
                "Expected line count differs. Current content: {}",
                content.get_content()
            );
            assert!(
                content.line_lens[row_index] <= expected_row_len,
                "Line {}, Actual data is longer: {:?}",
                row_index,
                &content.get_line_chars(row_index)[expected_row_len..content.line_lens[row_index]]
            );
            assert!(
                content.line_lens[row_index] >= expected_row_len,
                "Line {}, Actual data is shorter,  actual: {:?} \n, expected: {:?}",
                row_index,
                &content.get_line_chars(row_index)[0..content.line_lens[row_index]],
                &expected_line[content.line_lens[row_index]..expected_row_len]
            );
        }
        if selection_found {
            assert_eq!(
                editor.get_selection().start,
                expected_selection_start,
                "Selection start"
            );
            assert!(editor.get_selection().is_range().is_some());
            assert_eq!(
                editor.get_selection().end.unwrap(),
                expected_selection_end,
                "Selection end"
            );
        } else if expected_cursor.is_range().is_none() && params.undo_count > 0 {
            assert_eq!(
                editor.get_selection().start.row,
                expected_cursor.start.row,
                "The cursor is not reverted back during undo"
            );
        } else {
            assert_eq!(editor.get_selection(), expected_cursor, "Cursor");
        }
    }

    #[test]
    fn test_the_test() {
        let mut content = EditorContent::<usize>::new(80);
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "█abcdefghijklmnopqrstuvwxyz",
                inputs: &[],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "█abcdefghijklmnopqrstuvwxyz",
            },
        );
        assert_eq!(editor.get_selection().start.column, 0);
        assert_eq!(editor.get_selection().start.row, 0);
        assert_eq!(editor.get_selection().end, None);

        assert_eq!(content.line_count(), 1);
        assert_eq!(content.line_len(0), 26);
        assert_eq!(content.canvas[0], 'a');
        assert_eq!(content.canvas[3], 'd');
        assert_eq!(content.canvas[25], 'z');

        // single codepoint
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "█abcdeéfghijklmnopqrstuvwxyz",
                inputs: &[],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "██abcdee\u{301}fghijklmnopqrstuvwxyz",
            },
        );
        assert_eq!(editor.get_selection().start.column, 0);
        assert_eq!(editor.get_selection().start.row, 0);
        assert_eq!(editor.get_selection().end, None);

        assert_eq!(content.line_count(), 1);
        assert_eq!(content.line_lens[0], 28);
        assert_eq!(content.canvas[0], 'a');
        assert_eq!(content.canvas[3], 'd');
        assert_eq!(content.canvas[25], 'x');

        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCD█EFGHIJKLMNOPQRSTUVWXY",
                inputs: &[],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCD█EFGHIJKLMNOPQRSTUVWXY",
            },
        );
        assert!(
            matches!(
                editor.get_selection(),
                Selection {
                    start: Pos { row: 1, column: 4 },
                    end: None
                }
            ),
            "selection: {:?}",
            editor.get_selection()
        );
        assert_eq!(content.line_count(), 2);
        assert_eq!(content.line_lens[1], 25);
        assert_eq!(content.get_char(1, 0), 'A');
        assert_eq!(content.get_char(1, 3), 'D');
        assert_eq!(content.get_char(1, 24), 'Y');
    }

    #[test]
    #[should_panic(expected = "Cursor")]
    fn test_the_test2() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[],
            InputModifiers::none(),
            "a█bcdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    #[should_panic(expected = "row: 0, column: 1")]
    fn test_the_test3() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[],
            InputModifiers::none(),
            "█aacdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    #[should_panic(expected = "Actual data is longer: ['x', 'y', 'z']")]
    fn test_the_test4() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvw",
        );
    }

    #[test]
    #[should_panic(expected = "row: 0, column: 23")]
    fn test_the_test5() {
        test(
            "█abcdefghijklmnopqrstuvw",
            &[],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_the_test_selection() {
        let mut editor = Editor::new(&mut EditorContent::<usize>::new(80));
        test0(
            &mut editor,
            &mut EditorContent::<usize>::new(80),
            TestParams {
                text_input: None,
                initial_content: "a❱bcdefghij❰klmnopqrstuvwxyz",
                inputs: &[],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "a❱bcdefghij❰klmnopqrstuvwxyz",
            },
        );
        assert!(
            matches!(
                editor.get_selection(),
                Selection {
                    start: Pos { row: 0, column: 1 },
                    end: Some(Pos { row: 0, column: 10 })
                }
            ),
            "selection: {:?}",
            editor.get_selection()
        );

        test0(
            &mut editor,
            &mut EditorContent::<usize>::new(80),
            TestParams {
                text_input: None,
                initial_content: "a❱bcdefghijklmnopqrstuvwxyz\n\
            abcdefghij❰klmnopqrstuvwxyz",
                inputs: &[],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "a❱bcdefghijklmnopqrstuvwxyz\n\
            abcdefghij❰klmnopqrstuvwxyz",
            },
        );
        assert!(
            matches!(
                editor.get_selection(),
                Selection {
                    start: Pos { row: 0, column: 1 },
                    end: Some(Pos { row: 1, column: 10 })
                }
            ),
            "selection: {:?}",
            editor.get_selection()
        );

        test0(
            &mut editor,
            &mut EditorContent::<usize>::new(80),
            TestParams {
                text_input: None,
                initial_content: "a❰bcdefghijklmnopqrstuvwxyz\n\
            abcdefghij❱klmnopqrstuvwxyz",
                inputs: &[],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "a❰bcdefghijklmnopqrstuvwxyz\n\
            abcdefghij❱klmnopqrstuvwxyz",
            },
        );
        assert!(
            matches!(
                editor.get_selection(),
                Selection {
                    start: Pos { row: 1, column: 10 },
                    end: Some(Pos { row: 0, column: 1 })
                }
            ),
            "selection: {:?}",
            editor.get_selection()
        );
    }

    #[test]
    fn test_moving_line_data() {
        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);

        // if the whole line is moved down, the line takes its data with itself
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "█111111111\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Enter],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "\n\
            █111111111\n\
            2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[0, 1, 2, 3]);

        // otherwise...
        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "11█1111111\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Enter],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "11\n\
            █1111111\n\
            2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 0, 2, 3]);

        // if the prev row is empty, the line takes its data with itself
        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Backspace],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "█2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Backspace],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "111█2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 3]);

        // if the current row is empty, the next line brings its data with itself
        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "█\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Del],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "█2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111█\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Del],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "111█2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 3]);
    }

    #[test]
    fn test_moving_line_data_undo() {
        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);

        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "█111111111\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Enter],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 0,
                expected_content: "█111111111\n\
            2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "11█1111111\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Enter],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 0,
                expected_content: "11█1111111\n\
            2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Backspace],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 0,
                expected_content: "\n\
            █2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Backspace],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 0,
                expected_content: "111\n\
            █2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "█\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Del],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 0,
                expected_content: "█\n\
            2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111█\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Del],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 0,
                expected_content: "111█\n\
            2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Up],
                delay_after_inputs: &[],
                modifiers: InputModifiers::ctrl_shift(),
                undo_count: 1,
                redo_count: 0,
                expected_content: "111\n\
            █2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Down],
                delay_after_inputs: &[],
                modifiers: InputModifiers::ctrl_shift(),
                undo_count: 1,
                redo_count: 0,
                expected_content: "111\n\
            █2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 2, 3]);
    }

    #[test]
    fn test_moving_line_data_redo() {
        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);

        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "█111111111\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Enter],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 1,
                expected_content: "\n\
                █111111111\n\
            2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[0, 1, 2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "11█1111111\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Enter],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 1,
                expected_content: "11\n\
                █1111111\n\
            2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 0, 2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Backspace],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 1,
                expected_content: "█2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Backspace],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 1,
                expected_content: "111█2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "█\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Del],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 1,
                expected_content: "█2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[2, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "11111111█\n\
            \n\
            3333333333",
                inputs: &[EditorInputEvent::Del],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 1,
                expected_content: "11111111█\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111█\n\
            2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Del],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 1,
                redo_count: 1,
                expected_content: "111█2222222222\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[1, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Up],
                delay_after_inputs: &[],
                modifiers: InputModifiers::ctrl_shift(),
                undo_count: 1,
                redo_count: 1,
                expected_content: "█2222222222\n\
            111\n\
            3333333333",
            },
        );
        assert_eq!(&content.line_data, &[2, 1, 3]);

        let mut content = EditorContent::new(80);
        content.line_data = vec![1, 2, 3];
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "111\n\
            █2222222222\n\
            3333333333",
                inputs: &[EditorInputEvent::Down],
                delay_after_inputs: &[],
                modifiers: InputModifiers::ctrl_shift(),
                undo_count: 1,
                redo_count: 1,
                expected_content: "111\n\
            3333333333\n\
            █2222222222",
            },
        );
        assert_eq!(&content.line_data, &[1, 3, 2]);
    }

    #[test]
    #[should_panic(expected = "Selection start")]
    fn test_the_test_selection2() {
        let mut editor = Editor::new(&mut EditorContent::<usize>::new(80));
        test0(
            &mut editor,
            &mut EditorContent::<usize>::new(80),
            TestParams {
                text_input: None,
                initial_content: "a❱bcdefghij❰klmnopqrstuvwxyz",
                inputs: &[],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "ab❱cdefghij❰klmnopqrstuvwxyz",
            },
        );
    }

    #[test]
    #[should_panic(expected = "Selection end")]
    fn test_the_test_selection3() {
        let mut editor = Editor::new(&mut EditorContent::<usize>::new(80));
        test0(
            &mut editor,
            &mut EditorContent::<usize>::new(80),
            TestParams {
                text_input: None,
                initial_content: "a❱bcdefghij❰klmnopqrstuvwxyz",
                inputs: &[],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "a❱bcdefghijk❰lmnopqrstuvwxyz",
            },
        );
    }

    #[test]
    fn test_simple_right_cursor() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::none(),
            "a█bcdefghijklmnopqrstuvwxyz",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            InputModifiers::none(),
            "abc█defghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Right],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            █ABCDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            AB█CDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY█",
            &[EditorInputEvent::Right],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY█",
        );
    }

    #[test]
    fn test_simple_left_cursor() {
        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::none(),
            "abcdefghi█jklmnopqrstuvwxyz",
        );

        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Left,
            ],
            InputModifiers::none(),
            "abcdefg█hijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            █ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Left],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            █ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Left,
            ],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwx█yz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Left],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
        );
    }

    #[test]
    fn test_simple_up_cursor() {
        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[EditorInputEvent::Up],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Up],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            █ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Up],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Up],
            InputModifiers::none(),
            "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
        );
    }

    #[test]
    fn test_simple_down_cursor() {
        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[EditorInputEvent::Down],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Down],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Down],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY█",
        );

        test(
            "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Down],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXY",
        );
    }

    #[test]
    fn test_column_index_keeping_navigation_up() {
        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr█stuvwxyz",
            &[EditorInputEvent::Up],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl█\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr█stuvwxyz",
            &[EditorInputEvent::Up, EditorInputEvent::Up],
            InputModifiers::none(),
            "abcdefghijklmnopqr█stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr█stuvwxyz",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Up,
                EditorInputEvent::Up,
            ],
            InputModifiers::none(),
            "abcdefghijklmnopq█rstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr█stuvwxyz",
            &[
                EditorInputEvent::Right,
                EditorInputEvent::Up,
                EditorInputEvent::Up,
            ],
            InputModifiers::none(),
            "abcdefghijklmnopqrs█tuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Up, EditorInputEvent::Up],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxy\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Up, EditorInputEvent::Up],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxy█\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_column_index_keeping_navigation_down() {
        test(
            "abcdefghijklmnopqr█stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Down],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl█\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqr█stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Down, EditorInputEvent::Down],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr█stuvwxyz",
        );

        test(
            "abcdefghijklmnopqr█stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Down,
                EditorInputEvent::Down,
            ],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopq█rstuvwxyz",
        );

        test(
            "abcdefghijklmnopqr█stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::Right,
                EditorInputEvent::Down,
                EditorInputEvent::Down,
            ],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrs█tuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Down, EditorInputEvent::Down],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxy",
            &[EditorInputEvent::Down, EditorInputEvent::Down],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxy█",
        );
    }

    #[test]
    fn test_home_btn() {
        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Home],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnop█qrstuvwxyz",
            &[EditorInputEvent::Home],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Home],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_page_up_btn() {
        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::PageUp],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnop█qrstuvwxyz",
            &[EditorInputEvent::PageUp],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::PageUp],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz",
        );
        test(
            "abcdefghijklmnopqrstuvwxyz\naaaaaa█aaaaaaaa",
            &[EditorInputEvent::PageUp],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz\naaaaaaaaaaaaaa",
        );
    }

    #[test]
    fn test_end_btn() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::End],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::End],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::End],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█",
        );
    }

    #[test]
    fn test_page_down_btn() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::PageDown],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdefghijklmnop█qrstuvwxyz",
            &[EditorInputEvent::PageDown],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::PageDown],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█",
        );
        test(
            "abcdef█ghijklmnopqrstuvwxyz\naaaaaaaaaaaaaa",
            &[EditorInputEvent::PageDown],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\naaaaaaaaaaaaaa█",
        );
    }

    #[test]
    fn test_ctrl_plus_left() {
        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl mnopqrstuvwxyz█",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl █mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "█abcdefghijkl mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█ mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "█abcdefghijkl mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl    █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "█abcdefghijkl    mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  )  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl  █)  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  |()-+%'^%/=?{}#<>&@[]*  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl  █|()-+%'^%/=?{}#<>&@[]*  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  \"  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl  █\"  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  12  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl  █12  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  12a  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl  █12a  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  a12  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl  █a12  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  _  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl  █_  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  _1a  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl  █_1a  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  \"❤(  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl(),
            "abcdefghijkl  \"█❤(  mnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_ctrl_plus_right() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "█abcdefghijkl mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl█ mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█ mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl mnopqrstuvwxyz█",
        );

        test(
            "abcdefghijkl █mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl mnopqrstuvwxyz█",
        );

        test(
            "abcdefghijkl█    mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl    mnopqrstuvwxyz█",
        );

        test(
            "abcdefghijkl█  )  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl  )█  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  |()-+%'^%/=?{}#<>&@[]*  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl  |()-+%'^%/=?{}#<>&@[]*█  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  \"  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl  \"█  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  12  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl  12█  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  12a  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl  12a█  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  a12  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl  a12█  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  _  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl  _█  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  _1a  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl  _1a█  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  \"❤(  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl(),
            "abcdefghijkl  \"█❤(  mnopqrstuvwxyz",
        );
    }

    ///////////////////////////////////////////////////////
    ///////////////////////////////////////////////////////
    ///////////////////////////////////////////////////////
    /// SELECTION
    ///////////////////////////////////////////////////////
    ///////////////////////////////////////////////////////
    ///////////////////////////////////////////////////////
    ///////////////////////////////////////////////////////
    #[test]
    fn test_simple_right_cursor_selection() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::shift(),
            "❱a❰bcdefghijklmnopqrstuvwxyz",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            InputModifiers::shift(),
            "❱abc❰defghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Right],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz❱\n\
            ❰ABCDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz❱\n\
            AB❰CDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY█",
            &[EditorInputEvent::Right],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY█",
        );
    }

    #[test]
    fn test_simple_left_cursor_selection() {
        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::shift(),
            "abcdefghi❰j❱klmnopqrstuvwxyz",
        );

        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Left,
            ],
            InputModifiers::shift(),
            "abcdefg❰hij❱klmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            █ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Left],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz❰\n\
            ❱ABCDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            █ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Left,
            ],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwx❰yz\n\
            ❱ABCDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Left],
            InputModifiers::shift(),
            "█abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
        );
    }

    #[test]
    fn test_left_right_cursor_selection() {
        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            InputModifiers::shift(),
            "abcdefghij█klmnopqrstuvwxyz",
        );

        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            InputModifiers::shift(),
            "abcdefghij❱k❰lmnopqrstuvwxyz",
        );

        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Left,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            InputModifiers::shift(),
            "abcdefghij❱klm❰nopqrstuvwxyz",
        );
    }

    #[test]
    fn test_simple_up_cursor_selection() {
        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[EditorInputEvent::Up],
            InputModifiers::shift(),
            "❰abcdefghij❱klmnopqrstuvwxyz",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Up],
            InputModifiers::shift(),
            "█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            █ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Up],
            InputModifiers::shift(),
            "❰abcdefghijklmnopqrstuvwxyz\n\
            ❱ABCDEFGHIJKLMNOPQRSTUVWXY",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Up],
            InputModifiers::shift(),
            "abcdefghi❰jklmnopqrstuvwxyz\n\
            ABCDEFGHI❱JKLMNOPQRSTUVWXY",
        );
    }

    #[test]
    fn test_simple_down_cursor_selection() {
        test(
            "abcdefghij█klmnopqrstuvwxyz",
            &[EditorInputEvent::Down],
            InputModifiers::shift(),
            "abcdefghij❱klmnopqrstuvwxyz❰",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Down],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Down],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz❱\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY❰",
        );

        test(
            "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXY",
            &[EditorInputEvent::Down],
            InputModifiers::shift(),
            "abcdefghi❱jklmnopqrstuvwxyz\n\
            ABCDEFGHI❰JKLMNOPQRSTUVWXY",
        );
    }

    #[test]
    fn test_column_index_keeping_navigation_up_selection() {
        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr█stuvwxyz",
            &[EditorInputEvent::Up],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❰\n\
            abcdefghijklmnopqr❱stuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr█stuvwxyz",
            &[EditorInputEvent::Up, EditorInputEvent::Up],
            InputModifiers::shift(),
            "abcdefghijklmnopqr❰stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr❱stuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr█stuvwxyz",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Up,
                EditorInputEvent::Up,
            ],
            InputModifiers::shift(),
            "abcdefghijklmnopq❰rstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr❱stuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr█stuvwxyz",
            &[
                EditorInputEvent::Right,
                EditorInputEvent::Up,
                EditorInputEvent::Up,
            ],
            InputModifiers::shift(),
            "abcdefghijklmnopqrs❰tuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr❱stuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Up, EditorInputEvent::Up],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz❰\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz❱",
        );

        test(
            "abcdefghijklmnopqrstuvwxy\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Up, EditorInputEvent::Up],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxy❰\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz❱",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            █abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::End, EditorInputEvent::Up],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❰\n\
            ❱abcdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_column_index_keeping_navigation_down_selection() {
        test(
            "abcdefghijklmnopqr█stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Down],
            InputModifiers::shift(),
            "abcdefghijklmnopqr❱stuvwxyz\n\
            abcdefghijkl❰\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqr█stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Down, EditorInputEvent::Down],
            InputModifiers::shift(),
            "abcdefghijklmnopqr❱stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqr❰stuvwxyz",
        );

        test(
            "abcdefghijklmnopqr█stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::Left,
                EditorInputEvent::Down,
                EditorInputEvent::Down,
            ],
            InputModifiers::shift(),
            "abcdefghijklmnopqr❱stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopq❰rstuvwxyz",
        );

        test(
            "abcdefghijklmnopqr█stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::Right,
                EditorInputEvent::Down,
                EditorInputEvent::Down,
            ],
            InputModifiers::shift(),
            "abcdefghijklmnopqr❱stuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrs❰tuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Down, EditorInputEvent::Down],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz❱\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz❰",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxy",
            &[EditorInputEvent::Down, EditorInputEvent::Down],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz❱\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxy❰",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::End, EditorInputEvent::Down],
            InputModifiers::shift(),
            "❱abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❰\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Home, EditorInputEvent::Down],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz❱\n\
            ❰abcdefghijkl\n\
            abcdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_home_btn_selection() {
        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Home],
            InputModifiers::shift(),
            "❰abcdefghijklmnopqrstuvwxyz❱",
        );

        test(
            "abcdefghijklmnop█qrstuvwxyz",
            &[EditorInputEvent::Home],
            InputModifiers::shift(),
            "❰abcdefghijklmnop❱qrstuvwxyz",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Home],
            InputModifiers::shift(),
            "█abcdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_pageup_btn_selection() {
        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::PageUp],
            InputModifiers::shift(),
            "❰abcdefghijklmnopqrstuvwxyz❱",
        );

        test(
            "abcdefghijklmnop█qrstuvwxyz",
            &[EditorInputEvent::PageUp],
            InputModifiers::shift(),
            "❰abcdefghijklmnop❱qrstuvwxyz",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::PageUp],
            InputModifiers::shift(),
            "█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\naaaaaaaaaaaaaa\nbbbb█bbbbb",
            &[EditorInputEvent::PageUp],
            InputModifiers::shift(),
            "❰abcdefghijklmnopqrstuvwxyz\naaaaaaaaaaaaaa\nbbbb❱bbbbb",
        );
    }

    #[test]
    fn test_end_btn_selection() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::End],
            InputModifiers::shift(),
            "❱abcdefghijklmnopqrstuvwxyz❰",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::End],
            InputModifiers::shift(),
            "❱abcdefghijklmnopqrstuvwxyz❰",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::End],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz█",
        );
    }

    #[test]
    fn test_pagedown_btn_selection() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::PageDown],
            InputModifiers::shift(),
            "❱abcdefghijklmnopqrstuvwxyz❰",
        );

        test(
            "abcdefghijklmnop█qrstuvwxyz",
            &[EditorInputEvent::PageDown],
            InputModifiers::shift(),
            "abcdefghijklmnop❱qrstuvwxyz❰",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::PageDown],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdefgh█ijklmnopqrstuvwxyz\naaaaaaaaaaaaaa\nbbbbbbbbb",
            &[EditorInputEvent::PageDown],
            InputModifiers::shift(),
            "abcdefgh❱ijklmnopqrstuvwxyz\naaaaaaaaaaaaaa\nbbbbbbbbb❰",
        );
    }

    #[test]
    fn test_home_end_btn_selection() {
        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Home, EditorInputEvent::End],
            InputModifiers::shift(),
            "abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdefghijklmno█pqrstuvwxyz",
            &[EditorInputEvent::Home, EditorInputEvent::End],
            InputModifiers::shift(),
            "abcdefghijklmno❱pqrstuvwxyz❰",
        );
    }

    #[test]
    fn test_ctrl_shift_left() {
        test(
            "abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "❰abcdefghijklmnopqrstuvwxyz❱",
        );

        test(
            "abcdefghijkl mnopqrstuvwxyz█",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl ❰mnopqrstuvwxyz❱",
        );

        test(
            "abcdefghijkl █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "❰abcdefghijkl ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█ mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "❰abcdefghijkl❱ mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl    █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "❰abcdefghijkl    ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  )  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl  ❰)  ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  |()-+%'^%/=?{}#<>&@[]*  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl  ❰|()-+%'^%/=?{}#<>&@[]*  ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  \"  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl  ❰\"  ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  12  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl  ❰12  ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  12a  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl  ❰12a  ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  a12  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl  ❰a12  ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  _  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl  ❰_  ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  _1a  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl  ❰_1a  ❱mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl  \"❤(  █mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl  \"❰❤(  ❱mnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_ctrl_shift_right() {
        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "❱abcdefghijklmnopqrstuvwxyz❰",
        );

        test(
            "█abcdefghijkl mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "❱abcdefghijkl❰ mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█ mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱ mnopqrstuvwxyz❰",
        );

        test(
            "abcdefghijkl █mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl ❱mnopqrstuvwxyz❰",
        );

        test(
            "abcdefghijkl█    mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱    mnopqrstuvwxyz❰",
        );

        test(
            "abcdefghijkl█  )  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱  )❰  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  |()-+%'^%/=?{}#<>&@[]*  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱  |()-+%'^%/=?{}#<>&@[]*❰  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  \"  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱  \"❰  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  12  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱  12❰  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  12a  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱  12a❰  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  a12  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱  a12❰  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  _  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱  _❰  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  _1a  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱  _1a❰  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  \"❤(  mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::ctrl_shift(),
            "abcdefghijkl❱  \"❰❤(  mnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_ctrl_shift_up() {
        test(
            "abcdefgh█ijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            &[EditorInputEvent::Up],
            InputModifiers::ctrl_shift(),
            "abcdefgh█ijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
            &[EditorInputEvent::Up],
            InputModifiers::ctrl_shift(),
            "ABCDEFGHI█JKLMNOPQRSTUVWXYZ\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            123456789█12345678123456",
            &[EditorInputEvent::Up, EditorInputEvent::Up],
            InputModifiers::ctrl_shift(),
            "123456789█12345678123456\n\
            abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        );
    }

    #[test]
    fn test_ctrl_shift_up_undo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefgh█ijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            inputs: &[EditorInputEvent::Up],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefgh█ijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
            inputs: &[EditorInputEvent::Up],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            123456789█12345678123456",
            inputs: &[EditorInputEvent::Up, EditorInputEvent::Up],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            123456789█12345678123456",
        });
    }

    #[test]
    fn test_ctrl_shift_up_redo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefgh█ijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            inputs: &[EditorInputEvent::Up],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefgh█ijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        });
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
            inputs: &[EditorInputEvent::Up],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "ABCDEFGHI█JKLMNOPQRSTUVWXYZ\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            123456789█12345678123456",
            inputs: &[EditorInputEvent::Up, EditorInputEvent::Up],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "123456789█12345678123456\n\
            abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        });
    }

    #[test]
    fn test_ctrl_shift_down() {
        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
            &[EditorInputEvent::Down],
            InputModifiers::ctrl_shift(),
            "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
        );

        test(
            "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            &[EditorInputEvent::Down],
            InputModifiers::ctrl_shift(),
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            abcdefghi█jklmnopqrstuvwxyz",
        );

        test(
            "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            12345678912345678123456",
            &[EditorInputEvent::Down, EditorInputEvent::Down],
            InputModifiers::ctrl_shift(),
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            12345678912345678123456\n\
            abcdefghi█jklmnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_ctrl_shift_down_undo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
            inputs: &[EditorInputEvent::Down],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            inputs: &[EditorInputEvent::Down],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            12345678912345678123456",
            inputs: &[EditorInputEvent::Down, EditorInputEvent::Down],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            12345678912345678123456",
        });
    }

    #[test]
    fn test_ctrl_shift_down_redo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
            inputs: &[EditorInputEvent::Down],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            ABCDEFGHI█JKLMNOPQRSTUVWXYZ",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            inputs: &[EditorInputEvent::Down],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            abcdefghi█jklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghi█jklmnopqrstuvwxyz\n\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            12345678912345678123456",
            inputs: &[EditorInputEvent::Down, EditorInputEvent::Down],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::ctrl_shift(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
            12345678912345678123456\n\
            abcdefghi█jklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_movement_cancels_selection() {
        test(
            "abcdef❱ghijklmnopqrstuvwxyz\n\
            abcdefghijkl❰mnopqrstuvwxyz",
            &[EditorInputEvent::Left],
            InputModifiers::none(),
            "abcdef█ghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdef❱ghijklmnopqrstuvwxyz\n\
            abcdefghijkl❰mnopqrstuvwxyz",
            &[EditorInputEvent::Right],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl█mnopqrstuvwxyz",
        );

        test(
            "abcdef❱ghijklmnopqrstuvwxyz\n\
            abcdefghijkl❰mnopqrstuvwxyz",
            &[EditorInputEvent::Down],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcdef❱ghijklmnopqrstuvwxyz\n\
            abcdefghijkl❰mnopqrstuvwxyz",
            &[EditorInputEvent::Up],
            InputModifiers::none(),
            "abcdefghijkl█mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdef❱ghijklmnopqrstuvwxyz\n\
            abcdefghijkl❰mnopqrstuvwxyz",
            &[EditorInputEvent::Home],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdef❱ghijklmnopqrstuvwxyz\n\
            abcdefghijkl❰mnopqrstuvwxyz",
            &[EditorInputEvent::End],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz",
            &[EditorInputEvent::Home],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz",
            &[EditorInputEvent::End],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
        );
    }

    /// //////////////////////////////////////
    /// Edit
    /// //////////////////////////////////////
    #[test]
    fn test_insert_char() {
        test(
            "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Char('1')],
            InputModifiers::none(),
            "1█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Char('1')],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdef1█ghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Char('1')],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz1█\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Char('1')],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz1█",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            InputModifiers::none(),
            "1❤3█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        // line is full, no insertion is allowed
        let text_80_len =
            "█abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzab\n\
            abcdefghijklmnopqrstuvwxyz";
        test(
            text_80_len,
            &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            InputModifiers::none(),
            text_80_len,
        );
    }

    #[test]
    fn test_insert_char_undo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
                               abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('1')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "█abcdefghijklmnopqrstuvwxyz\n\
                               abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('1')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('1')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Char('1')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        // line is full, no insertion is allowed
        let text_80_len =
            "█abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzab\n\
            abcdefghijklmnopqrstuvwxyz";
        test_undo(TestParams {
            text_input: None,
            initial_content: text_80_len,
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: text_80_len,
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_insert_char_redo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
                               abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('1')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "1█abcdefghijklmnopqrstuvwxyz\n\
                               abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('1')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef1█ghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('1')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxyz1█\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Char('1')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz1█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "1❤3█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        // line is full, no insertion is allowed
        let text_80_len =
            "█abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzab\n\
            abcdefghijklmnopqrstuvwxyz";
        test_undo(TestParams {
            text_input: None,
            initial_content: text_80_len,
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: text_80_len,
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "1❤3█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_undo_command_grouping() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[501, 501, 501],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "1❤█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[501, 501, 501],
            modifiers: InputModifiers::none(),
            undo_count: 2,
            redo_count: 0,
            expected_content: "1█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[501, 501, 501],
            modifiers: InputModifiers::none(),
            undo_count: 3,
            redo_count: 0,
            expected_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[501, 0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "1█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[501, 0, 0],
            modifiers: InputModifiers::none(),
            undo_count: 2,
            redo_count: 0,
            expected_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[0, 501],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "1❤█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('1'),
                EditorInputEvent::Char('❤'),
                EditorInputEvent::Char('3'),
            ],
            delay_after_inputs: &[0, 501],
            modifiers: InputModifiers::none(),
            undo_count: 2,
            redo_count: 0,
            expected_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn insert_char_with_selection() {
        test(
            "abcd❰efghijk❱lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Char('X')],
            InputModifiers::none(),
            "abcdX█lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz",
            &[EditorInputEvent::Char('X')],
            InputModifiers::none(),
            "abcdX█mnopqrstuvwxyz",
        );

        test(
            "❰abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz❱",
            &[EditorInputEvent::Char('X')],
            InputModifiers::none(),
            "X█",
        );

        test(
            "ab❰c❱defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Char('X')],
            InputModifiers::none(),
            "abX█defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Char('X')],
            InputModifiers::none(),
            "abcdX█mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    fn insert_char_with_selection_undo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcd❰efghijk❱lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcd❰efghijk❱lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "❰abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz❱",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "❰abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz❱",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "ab❰c❱defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "ab❰c❱defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('X'),
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn insert_char_with_selection_redo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcd❰efghijk❱lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdX█lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdX█mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "❰abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz❱",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "X█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "ab❰c❱defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abX█defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Char('X')],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdX█mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Char('X'),
                EditorInputEvent::Right,
                EditorInputEvent::Right,
                EditorInputEvent::Right,
            ],
            delay_after_inputs: &[0],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdX█mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_backspace() {
        test(
            "a█",
            &[EditorInputEvent::Backspace],
            InputModifiers::none(),
            "█",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Backspace],
            InputModifiers::none(),
            "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Backspace],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcde█ghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Backspace],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxy█\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Backspace],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxy█",
        );

        test(
            "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
            ],
            InputModifiers::none(),
            "ab█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "█",
            &[
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
            ],
            InputModifiers::none(),
            "█",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Backspace],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrst█uvwxyz",
            &[
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
            ],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz",
        );

        // the last backspace is not allowed, there is no enough space for it
        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrst█uvwxyz",
            &[
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
            ],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_backspace_undo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "a█",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "a█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█",
            inputs: &[
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrst█uvwxyz",
            inputs: &[
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl\n\
            abcdefghijkl\n\
            abcdefghijkl\n\
            abcdef█ghijkl",
            inputs: &[
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl\n\
            abcdefghijkl\n\
            abcdefghijkl\n\
            █abcdefghijkl",
        });
        // the last backspace is not allowed, there is no enough space for it
        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrst█uvwxyz",
            inputs: &[
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_backspace_redo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "a█",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcde█ghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxy█\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxy█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "ab█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█",
            inputs: &[
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrst█uvwxyz",
            inputs: &[
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content:
                "abcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl\n\
            abcdefghijkl\n\
            abcdefghijkl\n\
            abcdef█ghijkl",
            inputs: &[
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█abcdefghijklabcdefghijklabcdefghijkl",
        });
    }

    #[test]
    fn test_ctrl_del() {
        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
        );

        test(
            "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::Del,
                EditorInputEvent::Del,
                EditorInputEvent::Del,
            ],
            InputModifiers::ctrl(),
            "abcde█",
        );

        test(
            "█",
            &[
                EditorInputEvent::Del,
                EditorInputEvent::Del,
                EditorInputEvent::Del,
            ],
            InputModifiers::ctrl(),
            "█",
        );

        test(
            "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "abcdefghijklmnop█qrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[
                EditorInputEvent::End,
                EditorInputEvent::Del,
                EditorInputEvent::End,
                EditorInputEvent::Del,
            ],
            InputModifiers::ctrl(),
            "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyz",
        );

        test(
            "█abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "█",
        );

        test(
            "█abcdefghijkl mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "█ mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█ mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl █mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl █",
        );

        test(
            "abcdefghijkl█    mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  )  mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█)  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  |()-+%'^%/=?{}#<>&@[]*  mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█|()-+%'^%/=?{}#<>&@[]*  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  \"  mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█\"  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  12  mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█12  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  12a  mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█12a  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  a12  mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█a12  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  _  mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█_  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  _1a  mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█_1a  mnopqrstuvwxyz",
        );

        test(
            "abcdefghijkl█  \"❤(  mnopqrstuvwxyz",
            &[EditorInputEvent::Del],
            InputModifiers::ctrl(),
            "abcdefghijkl█\"❤(  mnopqrstuvwxyz",
        );
    }

    #[test]
    fn test_ctrl_del_undo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Del,
                EditorInputEvent::Del,
                EditorInputEvent::Del,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█",
            inputs: &[
                EditorInputEvent::Del,
                EditorInputEvent::Del,
                EditorInputEvent::Del,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnop█qrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::End,
                EditorInputEvent::Del,
                EditorInputEvent::End,
                EditorInputEvent::Del,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "█abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijkl mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "█abcdefghijkl mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█ mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█ mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl █mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█    mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█    mnopqrstuvwxyz",
        });
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  )  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█  )  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  |()-+%'^%/=?{}#<>&@[]*  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█  |()-+%'^%/=?{}#<>&@[]*  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  \"  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█  \"  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  12  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█  12  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  12a  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█  12a  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  a12  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█  a12  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  _  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█  _  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  _1a  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█  _1a  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  \"❤(  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 0,
            expected_content: "abcdefghijkl█  \"❤(  mnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_ctrl_del_redo() {
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Del,
                EditorInputEvent::Del,
                EditorInputEvent::Del,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcde█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█",
            inputs: &[
                EditorInputEvent::Del,
                EditorInputEvent::Del,
                EditorInputEvent::Del,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijklmnop█qrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::End,
                EditorInputEvent::Del,
                EditorInputEvent::End,
                EditorInputEvent::Del,
            ],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content:
                "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "█",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "█abcdefghijkl mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "█ mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█ mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl █",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█    mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█mnopqrstuvwxyz",
        });
        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  )  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█)  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  |()-+%'^%/=?{}#<>&@[]*  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█|()-+%'^%/=?{}#<>&@[]*  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  \"  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█\"  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  12  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█12  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  12a  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█12a  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  a12  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█a12  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  _  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█_  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  _1a  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█_1a  mnopqrstuvwxyz",
        });

        test_undo(TestParams {
            text_input: None,
            initial_content: "abcdefghijkl█  \"❤(  mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            undo_count: 1,
            redo_count: 1,
            expected_content: "abcdefghijkl█\"❤(  mnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_ctrl_w() {
        test(
            "█",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "█",
        );
        test(
            "a█",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱a❰",
        );
        test(
            "█a",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱a❰",
        );

        test(
            "█asd",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱asd❰",
        );
        test(
            "asd█",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱asd❰",
        );
        test(
            "a█sd",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱asd❰",
        );
        test(
            "as█d",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱asd❰",
        );

        test(
            "as█d 12",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱asd❰ 12",
        );
        test(
            "asd █12",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "asd ❱12❰",
        );
        test(
            "asd 1█2",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "asd ❱12❰",
        );
        test(
            "asd 12█",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "asd ❱12❰",
        );

        test(
            "█asdasdasd\n\
            bbbbbbbbbbb",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱asdasdasd❰\n\
            bbbbbbbbbbb",
        );

        test(
            "asd 12█",
            &[EditorInputEvent::Char('w'), EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱asd 12❰",
        );

        test(
            "█asd 12",
            &[EditorInputEvent::Char('w'), EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱asd 12❰",
        );

        test(
            "asd █12 qwe",
            &[EditorInputEvent::Char('w'), EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱asd 12 qwe❰",
        );

        test(
            "vvv asd █12 qwe ttt",
            &[EditorInputEvent::Char('w'), EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "vvv ❱asd 12 qwe❰ ttt",
        );

        test(
            "vvv ❱asd 12 qwe❱ ttt",
            &[EditorInputEvent::Char('w')],
            InputModifiers::ctrl(),
            "❱vvv asd 12 qwe ttt❰",
        );

        test(
            "vvv asd █12 qwe ttt",
            &[
                EditorInputEvent::Char('w'),
                EditorInputEvent::Char('w'),
                EditorInputEvent::Char('w'),
            ],
            InputModifiers::ctrl(),
            "❱vvv asd 12 qwe ttt❰",
        );
    }

    #[test]
    fn test_ctrl_backspace() {
        test_normal_undo_redo(TestParams2 {
            initial_content: "a█",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            █ghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            █",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "█",
            inputs: &[
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
                EditorInputEvent::Backspace,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrst█uvwxyz",
            inputs: &[
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
                EditorInputEvent::Home,
                EditorInputEvent::Backspace,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content:
                "abcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl mnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl █",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl█ mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█ mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl    █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl  )  █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl  █mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl  |()-+%'^%/=?{}#<>&@[]*  █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl  █mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl  \"  █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl  █mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl  12  █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl  █mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl  12a  █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl  █mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl  a12  █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl  █mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl  _  █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl  █mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl  _1a  █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl  █mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijkl  \"❤(  █mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "abcdefghijkl  \"█mnopqrstuvwxyz",
        });
    }

    #[test]
    fn press_backspace_with_selection() {
        test_normal_undo_redo(TestParams2 {
            initial_content: "abcd❰efghijk❱lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcd█lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcd█mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "❰abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz❱",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "█",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "ab❰c❱defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "ab█defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Backspace],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcd█mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_del() {
        test_normal_undo_redo(TestParams2 {
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "█bcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█hijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Del,
                EditorInputEvent::Del,
                EditorInputEvent::Del,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcde█ijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "█",
            inputs: &[
                EditorInputEvent::Del,
                EditorInputEvent::Del,
                EditorInputEvent::Del,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "█",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnop█qrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::End,
                EditorInputEvent::Del,
                EditorInputEvent::End,
                EditorInputEvent::Del,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content:
                "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz█abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnop█qrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::End,
                EditorInputEvent::Del,
                EditorInputEvent::End,
                EditorInputEvent::Del,
                EditorInputEvent::End,
                EditorInputEvent::Del,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content:
                "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn press_del_with_selection() {
        test_normal_undo_redo(TestParams2 {
            initial_content: "abcd❰efghijk❱lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcd█lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcd█mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "❰abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz❱",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "█",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "ab❰c❱defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "ab█defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcd█mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            ❱abcdefghijkl❰mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            █mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        // the last cursor pos should set to zero after del
        test(
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            ❱abcdefghijkl❰mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            &[EditorInputEvent::Del, EditorInputEvent::Up],
            InputModifiers::none(),
            "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyz\n\
            mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        )
    }

    #[test]
    fn test_enter() {
        test_normal_undo_redo(TestParams2 {
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "\n\
            █abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef\n\
            █ghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            █\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            █",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcde█fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[
                EditorInputEvent::Enter,
                EditorInputEvent::Enter,
                EditorInputEvent::Enter,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcde\n\
            \n\
            \n\
            █fghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "█",
            inputs: &[
                EditorInputEvent::Enter,
                EditorInputEvent::Enter,
                EditorInputEvent::Enter,
            ],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "\n\
            \n\
            \n\
            █",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            █abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            \n\
            █abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content:
                "❰pPaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\
        ❱aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "█aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "❰asd\n\
            bsd\n\
            csd❱",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "█",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "❰asd\n\
            bsd\n\
            bsd\n\
            bsd\n\
            bsd\n\
            bsd\n\
            bsd\n\
            csd❱",
            inputs: &[EditorInputEvent::Del],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "█",
        });
    }

    #[test]
    fn press_enter_with_selection() {
        test_normal_undo_redo(TestParams2 {
            initial_content: "abcd❰efghijk❱lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcd\n\
            █lmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcd\n\
            █mnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "❰abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz❱",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "\n\
            █",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "ab❰c❱defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "ab\n\
            █defghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcd❰efghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijkl❱mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[EditorInputEvent::Enter],
            text_input: None,
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcd\n\
            █mnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_insert_text() {
        test_normal_undo_redo(TestParams2 {
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("long text"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "long text█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdef█ghijklmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("long text"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdeflong text█ghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz█\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("long text"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyzlong text█\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz█",
            inputs: &[],
            text_input: Some("long text"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyzlong text█",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("long text ❤"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "long text ❤█abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        // on insertion, characters are moved to the next line if exceeds line limit
        test_normal_undo_redo(TestParams2 {
            initial_content: "█abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzab\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("long text ❤"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "long text ❤█abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopq\n\
            rstuvwxyzab\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijk█lmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("long text ❤\nwith 3\nlines"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "abcdefghijklmnopqrstuvwxyz\n\
            abcdefghijklong text ❤\n\
            with 3\n\
            lines█lmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "aaaaaaaaaXaaaaaaaaaXaaaaaaaaaXaaaaa█aaaaXaaaaaaaaaXaaaaaaaaaX\n\
            abcdefghijkXlmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("xxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxX"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "aaaaaaaaaXaaaaaaaaaXaaaaaaaaaXaaaaaxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxx\n\
            xxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxX█aaaaX\n\
            aaaaaaaaaXaaaaaaaaaX\n\
            abcdefghijkXlmnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_insert_text_with_selection() {
        test_normal_undo_redo(TestParams2 {
            initial_content: "❰abcdefg❱ijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("long text"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "long text█ijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "❰abcdefgijklmnopqrstuvwxyz\n\
            abcdefghijklmnopqrstuvwxyz❱",
            inputs: &[],
            text_input: Some("long text"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "long text█",
        });
        // on insertion, characters are moved to the next line if exceeds line limit
        test_normal_undo_redo(TestParams2 {
            initial_content: "❰ab❱cdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzab\n\
            abcdefghijklmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("long text ❤"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "long text ❤█cdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrs\n\
            tuvwxyzab\n\
            abcdefghijklmnopqrstuvwxyz",
        });

        test_normal_undo_redo(TestParams2 {
            initial_content: "aaaaaaaaaXaaaaaaaaaXaaaaaaaaaXaaaaa❰ab❱aaXaaaaaaaaaXaaaaaaaaaX\n\
            abcdefghijkXlmnopqrstuvwxyz",
            inputs: &[],
            text_input: Some("xxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxX"),
            delay_after_inputs: &[],
            modifiers: InputModifiers::none(),
            expected_content: "aaaaaaaaaXaaaaaaaaaXaaaaaaaaaXaaaaaxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxx\n\
            xxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxXxxxxxxxxxX█aaXaa\n\
            aaaaaaaXaaaaaaaaaX\n\
            abcdefghijkXlmnopqrstuvwxyz",
        });
    }

    #[test]
    fn test_bug1() {
        test(
            "aaaaa❱12s aa\n\
            a\n\
            a\n\
            a\n\
            a❰",
            &[EditorInputEvent::Del],
            InputModifiers::none(),
            "aaaaa█",
        );

        test(
            "((0b00101 AND 0xFF) XOR 0xFF00) << 16 >> 16  ❱NOT(0xFF)❰",
            &[EditorInputEvent::Del],
            InputModifiers::none(),
            "((0b00101 AND 0xFF) XOR 0xFF00) << 16 >> 16  █",
        );
    }

    #[test]
    fn test_tab() {
        test(
            "█abcdef",
            &[EditorInputEvent::Tab],
            InputModifiers::none(),
            "    █abcdef",
        );
        test(
            "a█bcdef",
            &[EditorInputEvent::Tab],
            InputModifiers::none(),
            "a   █bcdef",
        );
        test(
            "ab█cdef",
            &[EditorInputEvent::Tab],
            InputModifiers::none(),
            "ab  █cdef",
        );
        test(
            "abc█def",
            &[EditorInputEvent::Tab],
            InputModifiers::none(),
            "abc █def",
        );
    }

    #[test]
    fn test_ctrl_a() {
        test(
            "aaa█aa12s aa\n\
            a\n\
            a\n\
            a\n\
            a",
            &[EditorInputEvent::Char('a')],
            InputModifiers::ctrl(),
            "❱aaaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            a❰",
        );
    }

    #[test]
    fn test_insert_char_selection_when_the_first_row_is_empty() {
        test(
            "❱\n\
            \n\
            3\n\
            ❰",
            &[EditorInputEvent::Char('h')],
            InputModifiers::none(),
            "h█",
        );
    }

    #[test]
    fn test_ctrl_d() {
        test(
            "aaa█aa12s aa\n\
            a\n\
            a\n\
            a\n\
            a",
            &[EditorInputEvent::Char('d')],
            InputModifiers::ctrl(),
            "aaaaa12s aa\n\
            aaa█aa12s aa\n\
            a\n\
            a\n\
            a\n\
            a",
        );
        test(
            "aaaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            a█",
            &[EditorInputEvent::Char('d')],
            InputModifiers::ctrl(),
            "aaaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            a\n\
            a█",
        );
    }

    #[test]
    fn test_ctrl_d_undo_redo() {
        test_normal_undo_redo(TestParams2 {
            text_input: None,
            initial_content: "aaa█aa12s aa\n\
            a\n\
            a\n\
            a\n\
            a",
            inputs: &[EditorInputEvent::Char('d')],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "aaaaa12s aa\n\
            aaa█aa12s aa\n\
            a\n\
            a\n\
            a\n\
            a",
        });

        test_normal_undo_redo(TestParams2 {
            text_input: None,
            initial_content: "aaaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            a█",
            inputs: &[EditorInputEvent::Char('d')],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "aaaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            a\n\
            a█",
        });
    }

    #[test]
    fn test_ctrl_x() {
        let mut content = EditorContent::<usize>::new(80);
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "aaa█aa12s aa\n\
            a\n\
            a\n\
            a\n\
            a",
                inputs: &[EditorInputEvent::Char('x')],
                delay_after_inputs: &[],
                modifiers: InputModifiers::ctrl(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "█a\n\
            a\n\
            a\n\
            a",
            },
        );
        assert_eq!("aaaaa12s aa\n", &editor.clipboard);
        test_normal_undo_redo(TestParams2 {
            text_input: None,
            initial_content: "aaa█aa12s aa\n\
            a\n\
            a\n\
            a\n\
            a",
            inputs: &[EditorInputEvent::Char('x')],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "█a\n\
            a\n\
            a\n\
            a",
        });

        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "aaaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            a█",
                inputs: &[EditorInputEvent::Char('x')],
                delay_after_inputs: &[],
                modifiers: InputModifiers::ctrl(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "aaaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            █",
            },
        );
        assert_eq!("a", &editor.clipboard);
        test_normal_undo_redo(TestParams2 {
            text_input: None,
            initial_content: "aaaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            a█",
            inputs: &[EditorInputEvent::Char('x')],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "aaaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            █",
        });

        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "aaa❱aa12s a❰a\n\
            a\n\
            a\n\
            a\n\
            a",
                inputs: &[EditorInputEvent::Char('x')],
                delay_after_inputs: &[],
                modifiers: InputModifiers::ctrl(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "aaa█a\n\
            a\n\
            a\n\
            a\n\
            a",
            },
        );
        assert_eq!("aa12s a", &editor.clipboard);
        test_normal_undo_redo(TestParams2 {
            text_input: None,
            initial_content: "aaa❱aa12s a❰a\n\
            a\n\
            a\n\
            a\n\
            a",
            inputs: &[EditorInputEvent::Char('x')],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "aaa█a\n\
            a\n\
            a\n\
            a\n\
            a",
        });
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "a❱aaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            ❰a",
                inputs: &[EditorInputEvent::Char('x')],
                delay_after_inputs: &[],
                modifiers: InputModifiers::ctrl(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "a█a",
            },
        );
        assert_eq!("aaaa12s aa\na\na\na\n", &editor.clipboard);
        test_normal_undo_redo(TestParams2 {
            text_input: None,
            initial_content: "a❱aaaa12s aa\n\
            a\n\
            a\n\
            a\n\
            ❰a",
            inputs: &[EditorInputEvent::Char('x')],
            delay_after_inputs: &[],
            modifiers: InputModifiers::ctrl(),
            expected_content: "a█a",
        });
    }

    #[test]
    fn test_copy() {
        let mut content = EditorContent::<usize>::new(80);
        let mut editor = Editor::new(&mut content);
        test0(
            &mut editor,
            &mut content,
            TestParams {
                text_input: None,
                initial_content: "aaaaa❱12s aa\n\
            a\n\
            a\n\
            a\n\
            a❰",
                inputs: &[],
                delay_after_inputs: &[],
                modifiers: InputModifiers::none(),
                undo_count: 0,
                redo_count: 0,
                expected_content: "aaaaa❱12s aa\n\
            a\n\
            a\n\
            a\n\
            a❰",
            },
        );
        assert_eq!(
            Editor::clone_range(
                editor.get_selection().start,
                editor.get_selection().end.unwrap(),
                &content
            ),
            "12s aa\na\na\na\na".to_owned()
        )
    }

    #[test]
    fn test_editor_panic() {
        test(
            "❱\n\
            a❰",
            &[EditorInputEvent::Char('p')],
            InputModifiers::none(),
            "p█",
        );
    }

    #[test]
    fn test_wrong_selection_removal() {
        //'❱';
        //const SELECTION_END_MARK: char = '❰';
        test(
            "❱
interest rate = 3.7%/year
term = 30 years
n = term * 12/year
interest rate / (12 (1/year))

2m^4kg/s^3
946728000 *1246728000 *12❰",
            &[EditorInputEvent::Char('p')],
            InputModifiers::none(),
            "p█",
        );
    }

    #[test]
    fn test_wrong_selection_removal2() {
        //'❱';
        //const SELECTION_END_MARK: char = '❰';
        test(
            "❱interest rate = 3.7%/year
❰",
            &[EditorInputEvent::Char('p')],
            InputModifiers::none(),
            "p█",
        );
    }

    #[test]
    fn test_insert_char_selection_should_not_set_cursor_pos_if_command_is_rejected() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        editor.insert_text(&"a".repeat(100), &mut content);
        editor.handle_input(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &mut content,
        );
        editor.insert_text(&"b".repeat(100), &mut content);
        editor.handle_input(EditorInputEvent::Up, InputModifiers::none(), &mut content);
        editor.handle_input(EditorInputEvent::End, InputModifiers::none(), &mut content);
        editor.handle_input(
            EditorInputEvent::Right,
            InputModifiers::shift(),
            &mut content,
        );
        editor.handle_input(
            EditorInputEvent::Right,
            InputModifiers::shift(),
            &mut content,
        );
        editor.handle_input(
            EditorInputEvent::Char('o'),
            InputModifiers::none(),
            &mut content,
        );
        // the previous command was rejected due to line len limitation, but this command should not
        // cause panic
        editor.handle_input(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &mut content,
        );
    }

    #[test]
    fn test_that_undo_stack_is_cleared() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        editor.handle_input(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &mut content,
        );

        content.init_with("");
        editor.set_cursor_pos_r_c(0, 0);

        assert_eq!(content.undo_stack.len(), 0);
        assert_eq!(content.redo_stack.len(), 0);

        // no panic
        editor.handle_input(
            EditorInputEvent::Char('z'),
            InputModifiers::ctrl(),
            &mut content,
        );
    }

    #[test]
    fn test_ctrl_z_case_insensitive() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        assert_eq!(&content.get_content(), "");
        editor.handle_input(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &mut content,
        );
        assert_eq!(&content.get_content(), "X");
        editor.handle_input(
            EditorInputEvent::Char('Z'),
            InputModifiers::ctrl(),
            &mut content,
        );
        assert_eq!(&content.get_content(), "");
    }

    #[test]
    fn test_ctrl_shift_z_case_insensitive() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        assert_eq!(&content.get_content(), "");
        editor.handle_input(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &mut content,
        );
        assert_eq!(&content.get_content(), "X");
        editor.handle_input(
            EditorInputEvent::Char('Z'),
            InputModifiers::ctrl(),
            &mut content,
        );
        assert_eq!(&content.get_content(), "");
        editor.handle_input(
            EditorInputEvent::Char('Z'),
            InputModifiers::ctrl_shift(),
            &mut content,
        );
        assert_eq!(&content.get_content(), "X");
    }

    #[test]
    fn test_that_redo_is_cleared_if_new_undo_inserted() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        // go to the second row to put an invalid row index (1) to the redo stack
        editor.handle_input(
            EditorInputEvent::Enter,
            InputModifiers::none(),
            &mut content,
        );
        editor.handle_tick(5000); // to put it into a separate undo group
        editor.handle_input(
            EditorInputEvent::Char('X'),
            InputModifiers::none(),
            &mut content,
        );
        assert_eq!(content.redo_stack.len(), 0);

        editor.handle_tick(10000); // to put it into a separate undo group
        editor.handle_input(
            EditorInputEvent::Char('z'),
            InputModifiers::ctrl(),
            &mut content,
        );
        assert_eq!(content.redo_stack.len(), 1);

        // remove the 2nd row so the '1' row index becomes invalid
        editor.handle_tick(15000); // to put it into a separate undo group
        editor.handle_input(
            EditorInputEvent::Backspace,
            InputModifiers::none(),
            &mut content,
        );
        assert_eq!(content.redo_stack.len(), 0);

        editor.handle_tick(20000); // to put it into a separate undo group
        editor.handle_input(
            EditorInputEvent::Char('Y'),
            InputModifiers::none(),
            &mut content,
        );
        assert_eq!(content.redo_stack.len(), 0);

        // no panic, and no content change
        editor.handle_tick(25000); // to put it into a separate undo group
        editor.handle_input(
            EditorInputEvent::Char('z'),
            InputModifiers::ctrl_shift(),
            &mut content,
        );
        assert_eq!(&content.get_content(), "Y");
    }

    #[test]
    fn test_merging_to_rows_to_maxlen_by_inserting_char_is_prohibited() {
        // test editor maxlen is 80
        test(
            "\
            01234567890❱0123456789001234567890\n\
            01234567890123456789012345678901234567890123456789012345678901234567890123456789\n\
            01234567890❰123456789012345678901234567890123456789012345678901234567890123456789",
            &[EditorInputEvent::Char('p')],
            InputModifiers::none(),
            "\
            01234567890❱0123456789001234567890\n\
            01234567890123456789012345678901234567890123456789012345678901234567890123456789\n\
            01234567890❰123456789012345678901234567890123456789012345678901234567890123456789",
        );
    }

    #[test]
    fn test_merging_to_rows_to_maxlen() {
        // test editor maxlen is 80
        test(
            "\
            01234567890❱0123456789001234567890\n\
            01234567890123456789012345678901234567890123456789012345678901234567890123456789\n\
            01234567890❰123456789012345678901234567890123456789012345678901234567890123456789",
            &[EditorInputEvent::Del],
            InputModifiers::none(),
            "01234567890█123456789012345678901234567890123456789012345678901234567890123456789",
        );
    }

    #[test]
    fn test_merging_to_rows_to_more_than_maxlen() {
        // test editor maxlen is 80
        // expect no change
        test(
            "\
            012345678900❱123456789001234567890\n\
            01234567890123456789012345678901234567890123456789012345678901234567890123456789\n\
            01234567890❰123456789012345678901234567890123456789012345678901234567890123456789",
            &[EditorInputEvent::Char('X')],
            InputModifiers::none(),
            "\
            012345678900❱123456789001234567890\n\
            01234567890123456789012345678901234567890123456789012345678901234567890123456789\n\
            01234567890❰123456789012345678901234567890123456789012345678901234567890123456789",
        );
    }

    #[test]
    fn insert_three_times_as_the_max_len() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        editor.insert_text(
            &("a".repeat(120) + &"b".repeat(120) + &"c".repeat(120)),
            &mut content,
        );
        assert_eq!(content.get_content().lines().next().unwrap(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(content.get_content().lines().skip(1).next().unwrap(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert_eq!(content.get_content().lines().skip(2).next().unwrap(), "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
        assert_eq!(content.line_count(), 3);
    }

    #[test]
    fn insert_three_times_as_the_max_len_with_text_overflow() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        editor.insert_text("this will be overflowed", &mut content);
        editor.set_cursor_pos_r_c(0, 0);

        editor.insert_text(
            &("a".repeat(120) + &"b".repeat(120) + &"c".repeat(120)),
            &mut content,
        );
        assert_eq!(content.get_content().lines().next().unwrap(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(content.get_content().lines().skip(1).next().unwrap(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert_eq!(content.get_content().lines().skip(2).next().unwrap(), "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
        assert_eq!(
            content.get_content().lines().skip(3).next().unwrap(),
            "this will be overflowed"
        );
        assert_eq!(content.line_count(), 4);
    }

    #[test]
    fn insert_three_times_as_the_max_len_with_text_overflow2() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        editor.insert_text("this will not be overflowed", &mut content);

        editor.insert_text(
            &("a".repeat(120) + &"b".repeat(120) + &"c".repeat(120)),
            &mut content,
        );
        assert_eq!(content.get_content().lines().next().unwrap(), "this will not be overflowedaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(content.get_content().lines().skip(1).next().unwrap(), "aaaaaaaaaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert_eq!(content.get_content().lines().skip(2).next().unwrap(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
        assert_eq!(
            content.get_content().lines().skip(3).next().unwrap(),
            "ccccccccccccccccccccccccccc"
        );
        assert_eq!(content.line_count(), 4);
    }

    #[test]
    fn test_that_if_row_overflows_than_modif_is_all_lines_from_0() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        editor.insert_text("this will be overflowed", &mut content);
        editor.set_cursor_pos_r_c(0, 0);

        assert_eq!(
            editor.insert_text(&("a".repeat(100)), &mut content),
            Some(RowModificationType::AllLinesFrom(0))
        );
        assert_eq!(content.get_content().lines().skip(0).next().unwrap(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaathis will be overflo");
        assert_eq!(content.get_content().lines().skip(1).next().unwrap(), "wed");
        assert_eq!(content.line_count(), 2);
    }

    #[test]
    fn test_that_if_row_overflows_than_modif_is_all_lines_from_1() {
        let mut content = EditorContent::<usize>::new(120);
        let mut editor = Editor::new(&mut content);

        editor.insert_text(
            "The first row is untouched\nthis will be overflowed",
            &mut content,
        );
        editor.set_cursor_pos_r_c(1, 0);

        assert_eq!(
            editor.insert_text(&("a".repeat(100)), &mut content),
            Some(RowModificationType::AllLinesFrom(1))
        );
        assert_eq!(
            content.get_content().lines().skip(0).next().unwrap(),
            "The first row is untouched"
        );
        assert_eq!(
            content.get_content().lines().skip(1).next().unwrap(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaathis will be overflo"
        );
        assert_eq!(content.get_content().lines().skip(2).next().unwrap(), "wed");
        assert_eq!(content.line_count(), 3);
    }
}
