use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers, Pos};
use notecalc_lib::helper::{canvas_y, content_y};
use notecalc_lib::test_common::test_common::{
    assert_contains, assert_contains_pulse, create_test_app, create_test_app2, pulsing_ref_rect,
    to_char_slice, TestHelper,
};
use notecalc_lib::{
    EditorObjectType, Layer, OutputMessage, RenderChar, RenderStringMsg, LEFT_GUTTER_MIN_WIDTH,
};

#[test]
fn test_matrix_dots_are_not_rendered_sometimes() {
    let expected_char_at = |test: &TestHelper, at: usize| {
        OutputMessage::RenderChar(RenderChar {
            col: test.get_render_data().result_gutter_x - 1,
            row: canvas_y(at as isize),
            char: '…',
        })
    };

    let test = create_test_app2(30, 35);
    test.paste("[1,2,3,4,5,6,7,8]");
    for i in 0..20 {
        test.handle_resize(30 - i);
        test.render(); // must be rendered again, right gutter is updated within 2 renders :(
        let commands = &test.render_bucket().custom_commands[Layer::AboveText as usize];
        assert_contains(commands, 1, expected_char_at(&test, 0));
    }
}

#[test]
fn test_matrix_right_brackets_are_not_rendered_if_there_is_no_space() {
    let test = create_test_app2(48, 32);
    test.paste("[0,0,0,0,0;0,0,0,0,0;0,0,0,0,0]");

    // drag the rught gutter to left
    test.click(test.get_render_data().result_gutter_x, 0);
    test.handle_drag(0, 0);

    // start typing at beginning of the line
    test.input(EditorInputEvent::Home, InputModifiers::none());
    for _ in 0..12 {
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    }
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // just be sure that our testing function works
    let right_x = test.get_render_data().result_gutter_x;
    test.assert_contains_operator(1, |op| op.text == &['┐'] && op.column <= right_x);

    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // after the last char, the right brackets must not be rendered
    let right_x = test.get_render_data().result_gutter_x;
    test.assert_contains_operator(0, |op| op.text == &['┐'] && op.column <= right_x);

    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // neither here
    test.assert_contains_operator(0, |op| op.text == &['┐'] && op.column <= right_x);
}

#[test]
fn test_referenced_matrix_right_brackets_are_not_rendered_if_there_is_no_space() {
    let test = create_test_app2(62, 44);
    test.paste("[1,2,3,4,5;0,0,0,0,0;0,0,0,0,0]");
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();

    // drag the rught gutter to left
    test.click(test.get_render_data().result_gutter_x, 0);
    test.handle_drag(0, 0);

    // start typing at beginning of the line
    test.input(EditorInputEvent::Home, InputModifiers::none());
    for _ in 0..17 {
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    }
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // just be sure that our testing function works
    test.assert_contains_operator(1, |op| op.text == &['┐'] && op.row == canvas_y(5));
    //   all the matrix cells are renderet yet
    for i in 1..=5 {
        test.assert_contains_custom_command(Layer::Text, 1, |op| {
            matches!(
                op,
                OutputMessage::RenderString(RenderStringMsg { text, row, .. })
                if *text == format!("{}", i) && *row > canvas_y(4)
            )
        });
    }

    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // after the last char, the right brackets must not be rendered
    test.assert_contains_operator(0, |op| op.text == &['┐'] && op.row == canvas_y(5));
    //   last column should not appear
    for i in 1..=4 {
        test.assert_contains_custom_command(Layer::Text, 1, |op| {
            matches!(
                op,
                OutputMessage::RenderString(RenderStringMsg { text, row, .. })
                if *text == format!("{}", i) && *row > canvas_y(4)
            )
        });
    }
    test.assert_contains_custom_command(Layer::Text, 0, |op| {
        matches!(
            op,
            OutputMessage::RenderString(RenderStringMsg { text, row, .. })
            if text.as_str() == "5" && *row > canvas_y(4)
        )
    });

    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // neither here
    test.assert_contains_operator(0, |op| op.text == &['┐'] && op.row == canvas_y(5));
    //   last column should not appear
    for i in 1..=3 {
        test.assert_contains_custom_command(Layer::Text, 1, |op| {
            matches!(
                op,
                OutputMessage::RenderString(RenderStringMsg { text, row, .. })
                if *text == format!("{}", i) && *row > canvas_y(4)
            )
        });
    }
    test.assert_contains_custom_command(Layer::Text, 0, |op| {
        matches!(
            op,
            OutputMessage::RenderString(RenderStringMsg { text, row, .. })
            if text.as_str() == "5" && *row > canvas_y(4)
        )
    });
    test.assert_contains_custom_command(Layer::Text, 0, |op| {
        matches!(
            op,
            OutputMessage::RenderString(RenderStringMsg { text, row, .. })
            if text.as_str() == "4" && *row > canvas_y(4)
        )
    });
}

#[test]
fn test_matrix_left_brackets_are_not_rendered_if_there_is_no_space() {
    let test = create_test_app2(48, 32);
    test.paste("[0,0,0,0,0;0,0,0,0,0;0,0,0,0,0]");

    // drag the rught gutter to left
    test.click(test.get_render_data().result_gutter_x, 0);
    test.handle_drag(0, 0);

    // start typing at beginning of the line
    test.input(EditorInputEvent::Home, InputModifiers::none());
    for _ in 0..26 {
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    }
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // just be sure that our testing function works
    let right_x = test.get_render_data().result_gutter_x;
    test.assert_contains_operator(1, |op| op.text == &['┌'] && op.column <= right_x);

    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // after the last char, the right brackets must not be rendered
    let right_x = test.get_render_data().result_gutter_x;
    test.assert_contains_operator(0, |op| op.text == &['┌'] && op.column <= right_x);

    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // neither here
    test.assert_contains_operator(0, |op| op.text == &['┌'] && op.column <= right_x);
}

#[test]
fn test_insert_matrix_line_ref_panic() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("[6]\n&[1]");
    test.input(EditorInputEvent::Up, InputModifiers::none());
    assert_eq!(test.editor_objects()[content_y(1)][0].rendered_h, 1);
}

#[test]
fn test_referencing_matrix_size_correct2_vert_align() {
    let test = create_test_app(35);
    test.paste("[1;2;3]\n[4]\n&[1]  &[2]");
    test.input(EditorInputEvent::Up, InputModifiers::none());
    let first_line_h = 5;
    let second_line_half = (5 / 2) + 1;
    let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
    assert_contains_pulse(
        &test.render_bucket().pulses,
        1,
        pulsing_ref_rect(left_gutter_width + 5, first_line_h + second_line_half, 3, 1),
    )
}

#[test]
fn test_referencing_matrix_size_correct() {
    let test = create_test_app(35);
    test.paste("[1;2;3]\n&[1]");
    test.input(EditorInputEvent::Up, InputModifiers::none());
    assert_eq!(test.editor_objects()[content_y(1)][0].rendered_h, 5);
}

#[test]
fn stepping_into_a_matrix_renders_it_some_lines_below() {
    let test = create_test_app(35);
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
fn clicking_behind_matrix_should_move_the_cursor_there() {
    let test = create_test_app(35);

    test.paste("firs 1t\nasdsad\n[1;2;3;4]\nfirs 1t\nasdsad\n[1;2;3;4]");
    test.set_cursor_row_col(0, 0);
    test.render();
    assert_eq!(test.get_cursor_pos().row, 0);
    let left_gutter_width = 1;
    test.click(left_gutter_width + 50, 13);
    assert_eq!(test.get_cursor_pos().row, 5);
}

#[test]
fn click_into_a_row_with_matrix_put_the_cursor_after_the_rendered_matrix() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
    test.set_cursor_row_col(0, 0);
    test.render();
    let left_gutter_width = 1;
    test.click(left_gutter_width + 1, 5);
    assert!(test.app().matrix_editing.is_some());
}

#[test]
fn mouse_selecting_moving_mouse_out_of_editor() {
    let test = create_test_app(35);
    test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
    test.set_cursor_row_col(0, 0);
    test.render();
    let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
    test.click(left_gutter_width + 7, 0);
    test.handle_drag(0, 0);
    assert_eq!(
        test.get_selection().is_range_ordered(),
        Some((Pos::from_row_column(0, 0), Pos::from_row_column(0, 7)))
    );
}

#[test]
fn test_dragging_right_gutter_panic() {
    let test = create_test_app(35);
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
    let test = create_test_app2(20, 35);
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
    let test = create_test_app(35);
    test.paste("firs 1t\nasdsad\n[1,0;2,0;3,0;4,0;5,0;6,0]\nfirs 1t\nasdsad\n[1;2;3;4]");
    test.set_cursor_row_col(0, 0);
    test.render();
    let left_gutter_width = 2;
    test.click(left_gutter_width + 4, 0);
    test.render();

    test.handle_drag(left_gutter_width + 0, 1);

    let render_buckets = test.render_bucket();
    let pos = test
        .render_bucket()
        .custom_commands(Layer::BehindTextAboveCursor)
        .iter()
        .position(|it| matches!(it, OutputMessage::SetColor(0xA6D2FF_FF)))
        .expect("there is no selection box drawing");
    assert_eq!(
        render_buckets.custom_commands(Layer::BehindTextAboveCursor)[pos + 1],
        OutputMessage::RenderRectangle {
            x: left_gutter_width + 4,
            y: canvas_y(0),
            w: 3,
            h: 1,
        }
    );
    assert_eq!(
        render_buckets.custom_commands(Layer::BehindTextAboveCursor)[pos + 2],
        OutputMessage::RenderRectangle {
            x: left_gutter_width,
            y: canvas_y(1),
            w: 0,
            h: 1,
        }
    );
}

#[test]
fn end_matrix_edit_by_end_key() {
    let test = create_test_app(35);
    test.paste("");
    test.autocomplete_matrix();
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    // inside the matrix
    test.input(EditorInputEvent::End, InputModifiers::none());
    test.input(EditorInputEvent::End, InputModifiers::none());
    assert_eq!(test.app().editor.get_selection().get_cursor_pos().column, 9);
}

#[test]
fn end_matrix_edit_by_right_key() {
    let test = create_test_app(35);
    test.paste("");
    test.autocomplete_matrix();
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    // inside the matrix
    test.input(EditorInputEvent::End, InputModifiers::none());
    test.input(EditorInputEvent::Right, InputModifiers::none());
    test.input(EditorInputEvent::Right, InputModifiers::none());
    assert_eq!(test.app().editor.get_selection().get_cursor_pos().column, 9);
}

#[test]
fn end_matrix_edit_by_tab_key() {
    let test = create_test_app(35);
    test.paste("");
    test.autocomplete_matrix();
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    // inside the matrix
    test.input(EditorInputEvent::End, InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!(test.app().editor.get_selection().get_cursor_pos().column, 9);
}

#[test]
fn test_that_cursor_is_inside_matrix_on_creation() {
    let test = create_test_app(35);
    test.autocomplete_matrix();
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!("[1]", test.get_editor_content());
}

#[test]
fn test_matrix_alt_plus_left() {
    {
        let test = create_test_app(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1, 2, 3]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,2]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1, 2, 3]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1, 2, 3; 4,5,6]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,2;4,5]", test.get_editor_content());
    }
}

#[test]
fn test_matrix_alt_plus_down() {
    {
        let test = create_test_app(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1;0]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1;0;0;0]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1,2]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        // this render is important, it tests a bug!
        test.render();
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,2;0,0]", test.get_editor_content());
    }
}

#[test]
fn test_matrix_alt_plus_up() {
    {
        let test = create_test_app(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1; 2; 3]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1;2]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1; 2; 3]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1, 2, 3; 4,5,6]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,2,3]", test.get_editor_content());
    }
}

#[test]
fn test_matrix_alt_plus_right() {
    {
        let test = create_test_app(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,0]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,0,0,0]", test.get_editor_content());
    }
    {
        let test = create_test_app(35);
        test.paste("[1;2]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,0;2,0]", test.get_editor_content());
    }
}

#[test]
fn remove_matrix_backspace() {
    let test = create_test_app(35);
    test.paste("abcd [1,2,3;4,5,6]");
    test.render();
    test.input(EditorInputEvent::Backspace, InputModifiers::ctrl());
    assert_eq!("abcd ", test.get_editor_content());
}

#[test]
fn matrix_step_in_dir() {
    // from right
    {
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("abcd [1,2,3;4,5,6]");
    test.set_cursor_row_col(0, 5);
    test.render();
    test.input(EditorInputEvent::Del, InputModifiers::ctrl());
    assert_eq!("abcd ", test.get_editor_content());
}

#[test]
fn test_that_selected_matrix_content_is_copied_on_ctrl_c() {
    let test = create_test_app(35);
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
fn test_moving_inside_a_matrix() {
    // right to left, cursor at end
    {
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
fn matrix_deletion() {
    let test = create_test_app(35);
    test.paste(" [1,2,3]");
    test.set_cursor_row_col(0, 0);
    test.render();
    test.input(EditorInputEvent::Del, InputModifiers::none());
    assert_eq!("[1,2,3]", test.get_editor_content());
}

#[test]
fn matrix_insertion_bug() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("'[X] nth, sum fv");
    test.render();
    test.set_cursor_row_col(0, 0);
    test.input(EditorInputEvent::Del, InputModifiers::none());

    test.assert_results(&["Err"][..]);
}

#[test]
fn test_matrix_sum() {
    let test = create_test_app(35);
    test.paste("[1,2,3]\nsum");
    // both the first line and the 'sum' line renders a matrix, which leaves the result buffer empty
    test.assert_results(&["\u{0}"][..]);
}

#[test]
fn clicking_inside_matrix_while_selected_should_put_cursor_after_matrix() {
    let test = create_test_app(35);
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
fn clicking_inside_matrix_while_selected_should_put_cursor_after_matrix2() {
    let test = create_test_app(35);
    test.paste("firs 1t\nasdsad\n[1,2,3,4]\nfirs 1t\nasdsad\n[1;2;3;4]");
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
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!(
        "firs 1t\nasdsad\n[X,2,3,4]\nfirs 1t\nasdsad\n[1;2;3;4]",
        test.get_editor_content()
    );
}

#[test]
fn test_allow_empty_matrix_inside_parens() {
    let test = create_test_app(35);

    test.input(EditorInputEvent::Char('('), InputModifiers::none());
    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());

    assert!(test.bcf.app().matrix_editing.is_some());
    assert_eq!(test.bcf.editor_objects()[content_y(0)].len(), 3);
    assert_eq!(
        test.bcf.editor_objects()[content_y(0)][0].typ,
        EditorObjectType::SimpleTokens
    );
    assert_eq!(
        test.bcf.editor_objects()[content_y(0)][1].typ,
        EditorObjectType::Matrix {
            row_count: 1,
            col_count: 1
        }
    );
    assert_eq!(
        test.bcf.editor_objects()[content_y(0)][2].typ,
        EditorObjectType::SimpleTokens
    );
}

#[test]
fn matrixes_without_valid_content_should_be_still_considered_matrices() {
    let test = create_test_app(35);

    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    test.input(EditorInputEvent::Char('x'), InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert!(test.bcf.app().matrix_editing.is_none());
    assert_eq!(test.bcf.editor_objects()[content_y(0)].len(), 1);
    assert_eq!(
        test.bcf.editor_objects()[content_y(0)][0].typ,
        EditorObjectType::Matrix {
            row_count: 1,
            col_count: 1
        }
    );
}

#[test]
fn test_matrix_content_error() {
    for content in &["[2, asda]", "[2, ]", "[2,]"] {
        let test = create_test_app(35);
        test.paste(content);
        test.assert_contains_error(1, |cmd| cmd.text == to_char_slice("["));
        test.assert_contains_error(1, |cmd| cmd.text == to_char_slice("]"));
    }
}

#[test]
fn test_matrix_invalid_op_only_op_is_in_error() {
    let test = create_test_app(35);
    test.paste("1 + [2,3]");
    test.assert_contains_error(1, |_cmd| true);
    test.assert_contains_error(1, |cmd| cmd.text == to_char_slice("+"));
}

#[test]
fn test_sum_inside_matrix() {
    let test = create_test_app(35);
    test.paste("12\n[1, sum]");
    // there is no "Err" in the result
    test.assert_results(&["12"]);
}
