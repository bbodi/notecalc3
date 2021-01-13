use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::helper::canvas_y;
use notecalc_lib::test_common::test_common::{
    assert_contains, assert_contains_pulse, create_test_app, create_test_app2, pulsing_ref_rect,
};
use notecalc_lib::{
    Layer, OutputMessage, ACTIVE_LINE_REF_HIGHLIGHT_COLORS, LEFT_GUTTER_MIN_WIDTH,
    RIGHT_GUTTER_WIDTH, THEMES,
};

#[test]
fn test_referenced_lineref_of_active_line_are_highlighted() {
    let test = create_test_app(35);
    test.paste("223456\nasd &[1] * 2");
    test.set_cursor_row_col(0, 0);

    test.render();
    let render_command_count_before = test.get_all_custom_commands_render_commands().len();

    test.input(EditorInputEvent::Down, InputModifiers::none());

    let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
    let render_commands = &test.get_all_custom_commands_render_commands();
    // (setcolor + underline) + (setcolor + 2*rect)
    assert_eq!(render_commands.len(), render_command_count_before + 5);
    assert_contains(
        render_commands,
        2,
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0]),
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(0),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(0),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd ".len(),
            y: canvas_y(1),
            w: "223 456".len(),
        },
    );
}

#[test]
fn test_multiple_referenced_linerefs_in_different_rows_of_active_line_are_highlighted() {
    let test = create_test_app(35);
    test.paste("234\n356789\nasd &[1] * &[2] * 2");
    test.set_cursor_row_col(1, 0);
    test.render();

    test.assert_no_highlighting_rectangle();
    let render_command_count_before = &test.get_all_custom_commands_render_commands().len();
    test.input(EditorInputEvent::Down, InputModifiers::none());

    let render_commands = &test.get_all_custom_commands_render_commands();
    // 2 underlines + 2 setcolors
    // 2*2 rectangles on the gutters + 1 setcolor for each (2)
    //
    assert_eq!(render_commands.len(), render_command_count_before + 10);

    assert_contains(
        render_commands,
        2,
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0]),
    );
    let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(0),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(0),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        2,
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1]),
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(1),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(1),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );

    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd ".len(),
            y: canvas_y(2),
            w: "234".len(),
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd 234 * ".len(),
            y: canvas_y(2),
            w: "356 789".len(),
        },
    );
}

#[test]
fn test_that_out_of_editor_line_ref_backgrounds_are_not_rendered() {
    let test = create_test_app2(51, 35);
    test.paste("234\n356789\nasd &[1] * &[2] * 2");
    test.input(EditorInputEvent::PageUp, InputModifiers::none());

    // line_ref rect would start on the result gutter
    for _i in 0..10 {
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    }

    test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 1, |cmd| match cmd {
        OutputMessage::RenderRectangle { y, .. } => *y == canvas_y(2),
        _ => false,
    });
    test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 1, |cmd| match cmd {
        OutputMessage::SetColor(c) => *c == THEMES[0].line_ref_bg,
        _ => false,
    });
}

// otherwise the content of a referenced matrix is not visible
#[test]
fn test_that_lineref_bg_is_behind_text() {
    let test = create_test_app2(51, 35);
    test.paste("234\n356789\nasd &[1] * &[2] * 2");

    test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 2, |cmd| match cmd {
        OutputMessage::RenderRectangle { y, .. } => *y == canvas_y(2),
        _ => false,
    });
    test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 2, |cmd| match cmd {
        OutputMessage::SetColor(c) => *c == THEMES[0].line_ref_bg,
        _ => false,
    });
}

#[test]
fn test_that_partial_out_of_editor_line_ref_backgrounds_are_rendered_partially() {
    let test = create_test_app2(51, 35);
    test.paste("234\n356789\nasd &[1] * &[2] * 2");
    test.input(EditorInputEvent::PageUp, InputModifiers::none());

    // line_ref rect would start on the result gutter
    for _i in 0..7 {
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    }

    // everything visible yet
    test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 1, |cmd| match cmd {
        OutputMessage::RenderRectangle { x, y, w, h } => {
            *y == canvas_y(2) && *x == 22 && *w == 7 && *h == 1
        }
        _ => false,
    });

    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 1, |cmd| match cmd {
        OutputMessage::RenderRectangle { x, y, w, h } => {
            *y == canvas_y(2) && *x == 23 && *w == 4 && *h == 1
        }
        _ => false,
    });

    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 1, |cmd| match cmd {
        OutputMessage::RenderRectangle { x, y, w, h } => {
            *y == canvas_y(2) && *x == 24 && *w == 2 && *h == 1
        }
        _ => false,
    });
}

#[test]
fn test_that_out_of_editor_line_ref_underlines_are_not_rendered() {
    let test = create_test_app2(51, 35);
    test.paste("234\n356789\nasd &[1] * &[2] * 2");
    test.input(EditorInputEvent::PageUp, InputModifiers::none());

    for _i in 0..10 {
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    }

    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());

    // there is only 1 line ref background since the 2nd one is out of editor
    // the one setcolor is for the rectangle, but there is no for the underline
    test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 1, |cmd| match cmd {
        OutputMessage::SetColor(color) => *color == ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1],
        _ => false,
    });
    test.assert_contains_custom_command(Layer::Text, 0, |cmd| match cmd {
        OutputMessage::SetColor(color) => *color == ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1],
        _ => false,
    });
    // just to be sure that there are 2 setcolor for normal cases
    test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 1, |cmd| match cmd {
        OutputMessage::SetColor(color) => *color == ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0],
        _ => false,
    });
    test.assert_contains_custom_command(Layer::Text, 1, |cmd| match cmd {
        OutputMessage::SetColor(color) => *color == ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0],
        _ => false,
    });
    test.assert_contains_custom_command(Layer::Text, 1, |cmd| match cmd {
        OutputMessage::RenderUnderline { y, .. } => *y == canvas_y(2),
        _ => false,
    });
    // no other colors
    for expected_color in ACTIVE_LINE_REF_HIGHLIGHT_COLORS.iter().skip(2) {
        test.assert_contains_custom_command(Layer::Text, 0, |cmd| match cmd {
            OutputMessage::SetColor(color) => *color == *expected_color,
            _ => false,
        });
        test.assert_contains_custom_command(Layer::BehindTextAboveCursor, 0, |cmd| match cmd {
            OutputMessage::SetColor(color) => *color == *expected_color,
            _ => false,
        });
    }
}

#[test]
fn test_that_partial_out_of_editor_line_ref_underlines_are_rendered_partially() {
    let test = create_test_app2(51, 35);
    test.paste("234\n356789\nasd &[1] * &[2] * 2");
    test.input(EditorInputEvent::PageUp, InputModifiers::none());

    // line_ref rect would start on the result gutter
    for _i in 0..7 {
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    }

    // everything visible yet
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.assert_contains_custom_command(Layer::Text, 1, |cmd| match cmd {
        OutputMessage::RenderUnderline { x, y, w } => *y == canvas_y(2) && *x == 22 && *w == 7,
        _ => false,
    });

    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.assert_contains_custom_command(Layer::Text, 1, |cmd| match cmd {
        OutputMessage::RenderUnderline { x, y, w } => *y == canvas_y(2) && *x == 23 && *w == 4,
        _ => false,
    });

    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.assert_contains_custom_command(Layer::Text, 1, |cmd| match cmd {
        OutputMessage::RenderUnderline { x, y, w } => *y == canvas_y(2) && *x == 24 && *w == 2,
        _ => false,
    });
}

#[test]
fn test_that_partial_out_of_editor_line_ref_pulses_are_rendered_partially() {
    let test = create_test_app2(51, 35);
    test.paste("234\n356789\nasd &[1] * &[2] * 2");
    test.input(EditorInputEvent::PageUp, InputModifiers::none());

    for _i in 0..7 {
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    }

    // everything visible yet
    test.input(EditorInputEvent::Down, InputModifiers::none());
    assert_contains_pulse(
        &test.render_bucket().pulses,
        1,
        pulsing_ref_rect(22, 2, 7, 1),
    );

    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    assert_contains_pulse(
        &test.render_bucket().pulses,
        1,
        pulsing_ref_rect(23, 2, 5, 1),
    );

    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    assert_contains_pulse(
        &test.render_bucket().pulses,
        1,
        pulsing_ref_rect(24, 2, 3, 1),
    );
}

#[test]
fn test_same_lineref_referenced_multiple_times_is_highlighted() {
    let test = create_test_app(35);
    test.paste("2345\nasd &[1] * &[1] * 2");
    test.set_cursor_row_col(0, 0);
    test.render();

    let render_command_count_before = test.get_all_custom_commands_render_commands().len();

    test.input(EditorInputEvent::Down, InputModifiers::none());

    let render_commands = &test.get_all_custom_commands_render_commands();
    // 2*(setcolor + underline) + setcolor + 2*rect
    assert_eq!(render_commands.len(), render_command_count_before + 7);
    assert_contains(
        render_commands,
        3,
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0]),
    );
    let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(0),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(0),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd ".len(),
            y: canvas_y(1),
            w: "2 345".len(),
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd 2 345 * ".len(),
            y: canvas_y(1),
            w: "2 345".len(),
        },
    );
}

#[test]
fn test_same_lineref_referenced_multiple_times_plus_another_in_diff_row_is_highlighted() {
    let test = create_test_app(35);
    test.paste("2345\n123\nasd &[1] * &[1] * &[2] * 2");
    test.set_cursor_row_col(1, 0);

    test.render();

    let render_command_count_before = &test.get_all_custom_commands_render_commands().len();

    test.input(EditorInputEvent::Down, InputModifiers::none());

    let render_commands = &test.get_all_custom_commands_render_commands();
    // 2*(setcolor + underline) + (setcolor + underline) +
    // 2*(setcolor + 2*rect)
    assert_eq!(render_commands.len(), render_command_count_before + 12);

    assert_contains(
        render_commands,
        3,
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0]),
    );
    let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
    // "2 345"
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(0),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(0),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );

    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd ".len(),
            y: canvas_y(2),
            w: "2 345".len(),
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd 2 345 * ".len(),
            y: canvas_y(2),
            w: "2 345".len(),
        },
    );

    // "123"
    assert_contains(
        render_commands,
        2,
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1]),
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(1),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(1),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );

    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd 2 345 * 2 345 * ".len(),
            y: canvas_y(2),
            w: "123".len(),
        },
    );
}

#[test]
fn test_out_of_screen_pulsing_var() {
    let test = create_test_app(20);
    test.paste("var = 4");
    test.repeated_paste("asd\n", 30);
    test.paste("var");
    test.set_cursor_row_col(0, 0);
    test.render();
    test.input(EditorInputEvent::PageDown, InputModifiers::none());
    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    // no pulsing should happen since the referencing line is out of view
    test.assert_no_pulsing();
}

#[test]
fn test_referenced_vars_and_linerefs_of_active_lines_are_pulsing() {
    let test = create_test_app(35);
    test.paste("2\n3\nvar = 4\nasd &[1] * &[2] * var");
    test.set_cursor_row_col(2, 0);

    test.render();
    let render_command_count_before = &test.get_all_custom_commands_render_commands().len();

    test.input(EditorInputEvent::Down, InputModifiers::none());

    let render_commands = &test.get_all_custom_commands_render_commands();
    // 3*(setcolor + underline) + 3(setcolor + 2*rect)
    assert_eq!(render_commands.len(), render_command_count_before + 15);
    let left_gutter_w = LEFT_GUTTER_MIN_WIDTH;
    // 1st
    assert_contains(
        render_commands,
        2,
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0]),
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(0),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(0),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd ".len(),
            y: canvas_y(3),
            w: "2".len(),
        },
    );

    // 2nd
    assert_contains(
        render_commands,
        2,
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[1]),
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(1),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(1),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd 2 * ".len(),
            y: canvas_y(3),
            w: "3".len(),
        },
    );

    // 3rd
    assert_contains(
        render_commands,
        2,
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[2]),
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(2),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(2),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "asd 2 * 3 * ".len(),
            y: canvas_y(3),
            w: "var".len(),
        },
    );
}

#[test]
fn test_bug_wrong_referenced_line_is_highlighted() {
    let test = create_test_app(35);
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
// n = term * 12 (1/year)
n = 360
r = interest rate / (12 (1/year))

monthly payment = r/(1 - (1 + r)^(-n)) *finance amount",
    );
    test.set_cursor_row_col(11, 0);
    test.render();

    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());

    let render_commands = &test.get_all_custom_commands_render_commands();
    assert_contains(
        render_commands,
        2, /*one for the underline and 1 for the gutter rectangles*/
        OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[0]),
    );
    let left_gutter_w = LEFT_GUTTER_MIN_WIDTH + 1;
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: 0,
            y: canvas_y(10),
            w: left_gutter_w,
            h: 1,
        },
    );
    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderRectangle {
            x: test.get_render_data().result_gutter_x,
            y: canvas_y(10),
            w: RIGHT_GUTTER_WIDTH,
            h: 1,
        },
    );

    assert_contains(
        render_commands,
        1,
        OutputMessage::RenderUnderline {
            x: left_gutter_w + "down payment = 20% * ".len(),
            y: canvas_y(11),
            w: "price".len(),
        },
    );
}
