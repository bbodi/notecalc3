use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers, Pos, Selection};
use notecalc_lib::helper::{canvas_y, content_y};
use notecalc_lib::test_common::test_common::{
    assert_contains, assert_contains_pulse, create_test_app, create_test_app2, pulsing_ref_rect,
    to_char_slice, TestHelper,
};
use notecalc_lib::token_parser::TokenType;
use notecalc_lib::{
    EditorObjectType, Layer, NoteCalcApp, OutputMessage, RenderAsciiTextMsg, RenderChar,
    RenderStringMsg, RenderUtf8TextMsg, Tokens, Variables, DEFAULT_RESULT_PANEL_WIDTH_PERCENT,
    LEFT_GUTTER_MIN_WIDTH, MAX_EDITOR_WIDTH, MAX_LINE_COUNT, RIGHT_GUTTER_WIDTH, SCROLLBAR_WIDTH,
    THEMES, VARIABLE_ARR_SIZE,
};

const fn result_panel_w(client_width: usize) -> usize {
    client_width * (100 - DEFAULT_RESULT_PANEL_WIDTH_PERCENT) / 100
}

#[test]
fn test_that_paste_is_not_necessary_for_tests_to_work() {
    {
        let test = create_test_app(35);
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
    }
    {
        let test = create_test_app(35);

        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        assert_eq!("ab\nc", test.get_editor_content());
    }
}

#[test]
fn bug1() {
    let test = create_test_app(35);
    test.paste("[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]");

    test.set_cursor_row_col(0, 33);
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.render();
}

#[test]
fn bug2() {
    let test = create_test_app(35);
    test.paste("[123, 2, 3; 4567981, 5, 6] * [1; 2; 3;4]");
    test.set_cursor_row_col(0, 1);

    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.render();
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.render();
}

#[test]
fn bug3() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("123\na ");

    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    assert_eq!(
        3,
        test.bcf.tokens()[content_y(1)]
            .as_ref()
            .unwrap()
            .tokens
            .len()
    );
}

#[test]
fn it_is_not_allowed_to_ref_lines_below() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
fn bug8() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("2\n3 * ");
    test.set_cursor_row_col(1, 4);
    test.render();
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    assert_eq!("2\n3 * &[1]", test.get_editor_content());

    test.assert_results(&["2", "6"][..]);
}

#[test]
fn test_empty_right_gutter_min_len() {
    let test = create_test_app(35);
    test.set_normalized_content("");
    assert_eq!(test.get_render_data().result_gutter_x, result_panel_w(120));
}

mod scrollbar_tests {

    use super::*;
    use notecalc_lib::{MouseHoverType, Rect, MAX_LINE_COUNT, SCROLLBAR_WIDTH, THEMES};

    #[test]
    fn test_scrolling_by_single_click_in_scrollbar() {
        let test = create_test_app(30);
        test.repeated_paste("1\n", 60);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 0);

        for i in 0..4 {
            let mouse_x = test.get_render_data().result_gutter_x - SCROLLBAR_WIDTH;
            test.click(mouse_x, 20 + i);
            assert_eq!(test.get_render_data().scroll_y, i as usize);
            test.handle_mouse_up();
            assert_eq!(test.get_render_data().scroll_y, 1 + i as usize);
        }
        for i in 0..3 {
            let mouse_x = test.get_render_data().result_gutter_x - SCROLLBAR_WIDTH;
            test.click(mouse_x, 0);
            assert_eq!(test.get_render_data().scroll_y, 4 - i as usize);
            test.handle_mouse_up();
            assert_eq!(test.get_render_data().scroll_y, 3 - i as usize);
        }
    }

    #[test]
    fn test_scrollbar_is_highlighted_on_mouse_hover() {
        let test = create_test_app(30);
        test.repeated_paste("1\n", 60);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());

        let result_gutter_x = test.get_render_data().result_gutter_x;
        assert_eq!(test.app().mouse_hover_type, MouseHoverType::Normal);
        assert_eq!(
            test.render_bucket().scroll_bar,
            Some((
                THEMES[0].scrollbar_normal,
                Rect {
                    x: (result_gutter_x - SCROLLBAR_WIDTH) as u16,
                    y: 0,
                    w: SCROLLBAR_WIDTH as u16,
                    h: 1,
                }
            ))
        );

        test.handle_mouse_move(result_gutter_x - SCROLLBAR_WIDTH, 0);
        assert_eq!(
            test.render_bucket().scroll_bar,
            Some((
                THEMES[0].scrollbar_hovered,
                Rect {
                    x: (result_gutter_x - SCROLLBAR_WIDTH) as u16,
                    y: 0,
                    w: SCROLLBAR_WIDTH as u16,
                    h: 1,
                }
            ))
        );

        test.handle_mouse_move(result_gutter_x, 0);
        assert_eq!(test.app().mouse_hover_type, MouseHoverType::RightGutter);
        assert_eq!(
            test.render_bucket().scroll_bar,
            Some((
                THEMES[0].scrollbar_normal,
                Rect {
                    x: (result_gutter_x - SCROLLBAR_WIDTH) as u16,
                    y: 0,
                    w: SCROLLBAR_WIDTH as u16,
                    h: 1,
                }
            ))
        );
    }

    #[test]
    fn stepping_down_to_unrendered_line_scrolls_down_the_screen() {
        let test = create_test_app(35);
        test.repeated_paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n0", 6);
        assert_eq!(test.get_render_data().scroll_y, 20);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 0);
        test.input(EditorInputEvent::PageDown, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 20);
    }

    #[test]
    fn test_scrolling_by_keyboard() {
        let test = create_test_app(35);
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
#
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
        assert_eq!(test.get_render_data().scroll_y, 4);
        assert_eq!(
            test.get_render_data().get_render_y(content_y(33)),
            Some(canvas_y(29)),
        );
    }

    #[test]
    fn test_that_pressing_enter_eof_moves_scrollbar_down() {
        let test = create_test_app(35);
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
    fn test_that_scrollbar_stops_at_bottom() {
        let client_height = 25;
        let test = create_test_app(client_height);
        test.repeated_paste("1\n", client_height * 2);
        test.set_cursor_row_col(0, 0);

        test.render();

        test.input(EditorInputEvent::PageDown, InputModifiers::none());

        assert_eq!(test.get_render_data().scroll_y, 26);
    }

    #[test]
    fn test_that_scrollbar_stops_at_bottom2() {
        let client_height = 36;
        let test = create_test_app(client_height);
        test.paste("");
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Del, InputModifiers::none());

        for _ in 0..MAX_LINE_COUNT + 40 {
            test.input(EditorInputEvent::Enter, InputModifiers::none());
        }

        test.input(EditorInputEvent::PageUp, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 0);
        test.input(EditorInputEvent::PageDown, InputModifiers::none());
        assert_eq!(
            test.get_render_data().scroll_y,
            MAX_LINE_COUNT - client_height
        );
    }

    #[test]
    fn test_inserting_long_text_scrolls_down() {
        let test = create_test_app(32);
        test.paste("a");
        test.repeated_paste("asd\n", 40);
        assert_eq!(test.get_render_data().scroll_y, 9);
    }

    #[test]
    fn test_that_no_overscrolling() {
        let test = create_test_app(35);
        test.paste("1\n2\n3\n");
        test.render();

        test.handle_wheel(1);
        assert_eq!(0, test.get_render_data().scroll_y);
    }

    #[test]
    fn tall_rows_are_considered_in_scrollbar_height_calc() {
        const CANVAS_HEIGHT: usize = 25;
        let test = create_test_app(CANVAS_HEIGHT);
        test.repeated_paste("1\n2\n\n[1;2;3;4]", 5);
        test.render();
        assert_eq!(
            test.render_bucket().scroll_bar,
            Some((
                THEMES[0].scrollbar_normal,
                Rect {
                    x: (result_panel_w(120) - SCROLLBAR_WIDTH) as u16,
                    y: 0,
                    w: 1,
                    h: 19,
                }
            ))
        );
    }

    #[test]
    fn test_no_scrolling_in_empty_document() {
        let test = create_test_app(25);
        test.paste("1");

        test.render();

        test.handle_wheel(1);

        test.render();

        assert_eq!(0, test.get_render_data().scroll_y);
    }

    #[test]
    fn test_that_no_overscrolling2() {
        let test = create_test_app(35);
        test.repeated_paste("aaaaaaaaaaaa\n", 35);
        test.render();

        test.handle_wheel(1);
        assert_eq!(1, test.get_render_data().scroll_y);
        test.handle_wheel(1);
        assert_eq!(1, test.get_render_data().scroll_y);
    }

    #[test]
    fn test_scrolling_down_on_enter_even() {
        let test = create_test_app(32);
        test.paste("");
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Del, InputModifiers::none());

        for _i in 0..31 {
            test.input(EditorInputEvent::Enter, InputModifiers::none());
        }
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 1);
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!(test.get_render_data().scroll_y, 2);
    }

    #[test]
    fn test_scroll_bug_when_scrolling_upwrads_from_bottom() {
        let test = create_test_app(32);
        test.paste("");

        test.input(EditorInputEvent::PageDown, InputModifiers::none());
        for _i in 0..40 {
            test.input(EditorInputEvent::Enter, InputModifiers::none());
        }
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        let scroll_y_at_bottom = test.get_render_data().scroll_y;
        test.handle_wheel(0);
        assert_eq!(test.get_render_data().scroll_y, scroll_y_at_bottom - 1);
        test.handle_wheel(0);
        assert_eq!(test.get_render_data().scroll_y, scroll_y_at_bottom - 2);
    }
}

mod right_gutter_tests {
    use super::*;
    use notecalc_lib::default_result_gutter_x;
    use notecalc_lib::test_common::test_common::create_test_app2;

    #[test]
    fn right_gutter_is_moving_if_there_would_be_enough_space_for_result() {
        let test = create_test_app2(40, 35);
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
        let test = create_test_app2(40, 35);
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
        let test = create_test_app2(176, 35);
        test.paste("ok");
    }

    #[test]
    fn test_resize_keeps_result_width() {
        let test = create_test_app2(60, 35);
        test.set_normalized_content("80kg\n190cm\n0.0016\n0.128 kg");
        let check_longest_line_did_not_change = || {
            assert_eq!(test.get_render_data().longest_visible_result_len, 11);
        };
        let asset_result_x_pos = |expected: usize| {
            assert_eq!(test.get_render_data().result_gutter_x, expected);
        };

        let calc_result_gutter_x_wrt_client_width = |client_w: usize| {
            // the result panel width will be 61% (60 - 23) * 100 / 60
            let percent = 61f32;
            (client_w as f32
                - ((client_w as f32 * percent / 100f32)
                    .max((LEFT_GUTTER_MIN_WIDTH + SCROLLBAR_WIDTH) as f32))) as usize
        };

        check_longest_line_did_not_change();
        // min editor w + left g + scroll
        asset_result_x_pos(20 + 2 + 1);

        test.handle_resize(50);
        asset_result_x_pos(calc_result_gutter_x_wrt_client_width(50));
        check_longest_line_did_not_change();

        test.handle_resize(60);
        check_longest_line_did_not_change();
        asset_result_x_pos(calc_result_gutter_x_wrt_client_width(60));

        test.handle_resize(100);
        check_longest_line_did_not_change();
        asset_result_x_pos(calc_result_gutter_x_wrt_client_width(100));

        // there is no enough space for the panel,
        // so it becomes bigger than 30%
        test.handle_resize(40);
        check_longest_line_did_not_change();
        asset_result_x_pos(15);

        test.handle_resize(30);
        check_longest_line_did_not_change();
        asset_result_x_pos(12);

        test.handle_resize(20);
        asset_result_x_pos(7);

        // too small
        test.handle_resize(10);
        check_longest_line_did_not_change();
        asset_result_x_pos(7);
    }

    #[test]
    fn right_gutter_is_immediately_rendered_at_its_changed_position_after_scrolling() {
        let test = create_test_app2(76, 10);
        test.repeated_paste("1\n", 10);
        test.paste("111111111111111111111");
        test.input(EditorInputEvent::PageUp, InputModifiers::none());
        assert_eq!(
            test.get_render_data().result_gutter_x,
            default_result_gutter_x(76)
        );

        test.handle_wheel(1);

        let expected_result_pos = 76 - ("111 111 111 111 111 111 111".len());
        test.assert_contains_result(1, |cmd| {
            cmd.text == "111 111 111 111 111 111 111".as_bytes()
                && cmd.row == canvas_y(9)
                && cmd.column == expected_result_pos
        })
    }

    #[test]
    fn right_gutter_is_immediately_rendered_at_its_changed_position_after_input() {
        let test = create_test_app2(76, 10);
        test.repeated_paste("1\n", 10);
        test.paste("111111111111111111111");
        test.input(EditorInputEvent::PageUp, InputModifiers::none());
        assert_eq!(
            test.get_render_data().result_gutter_x,
            default_result_gutter_x(76)
        );

        test.input(EditorInputEvent::PageDown, InputModifiers::none());

        let expected_result_pos = 76 - ("111 111 111 111 111 111 111".len());
        test.assert_contains_result(1, |cmd| {
            cmd.text == "111 111 111 111 111 111 111".as_bytes()
                && cmd.row == canvas_y(9)
                && cmd.column == expected_result_pos
        })
    }
}

#[test]
fn test_that_alignment_is_considered_for_longest_result_len() {
    let test = create_test_app(35);
    test.set_normalized_content("80kg\n190cm\n0.0016\n0.128 kg");
    assert_eq!(test.get_render_data().longest_visible_result_len, 11);
}

#[test]
fn test_scroll_y_reset() {
    let test = create_test_app(35);
    test.mut_app().render_data.scroll_y = 1;
    test.set_normalized_content("1111\n2222\n14 * &[2]&[2]&[2]\n");
    assert_eq!(0, test.get_render_data().scroll_y);
}

#[test]
fn test_tab_change_clears_variables() {
    let test = create_test_app(35);
    test.set_normalized_content(
        "source: https://rippedbody.com/how-to-calculate-leangains-macros/

weight = 80 kg
height = 190 cm
age = 30

# Step 1: Calculate your  (Basal Metabolic Rate) (BMR)
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
    let test = create_test_app(35);
    test.set_normalized_content(
        "source: https://rippedbody.com/how-to-calculate-leangains-macros/

weight = 80 kg
height = 190 cm
age = 30

# Step 1: Calculate your  (Basal Metabolic Rate) (BMR)
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
    let test = create_test_app(35);
    test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n");
    test.set_cursor_row_col(12, 2);
    test.render();
    test.input(EditorInputEvent::Up, InputModifiers::shift());
    test.render();
}

#[test]
fn test_err_result_rendering() {
    let test = create_test_app(35);
    test.paste("'[X] nth, sum fv");
    test.render();
    test.set_cursor_row_col(0, 0);
    test.input(EditorInputEvent::Del, InputModifiers::none());

    match &test.render_bucket().ascii_texts[0] {
        RenderAsciiTextMsg { text, row, column } => {
            assert_eq!(text, &[b'E', b'r', b'r']);
            assert_eq!(*row, canvas_y(0));
            assert_eq!(*column, result_panel_w(120) + RIGHT_GUTTER_WIDTH);
        }
    }
}

#[test]
fn sum_is_nulled_in_new_header_region() {
    let test = create_test_app(35);
    test.paste(
        "3m * 2m
# new header
1
2
sum
# new header
4
5
sum",
    );
    test.assert_results(&["6 m^2", "", "1", "2", "3", "", "4", "5", "9"][..]);
}

#[test]
fn test_that_header_lengths_are_separate_and_not_add() {
    let test = create_test_app2(79, 32);
    test.set_normalized_content(
        "# Header 0\n\
                123\n\
                # Header 1\n\
                123\n\
                # Header 2\n\
                123",
    );
    assert_eq!(test.get_render_data().longest_visible_result_len, 3);
}

#[test]
fn no_sum_value_in_case_of_error() {
    let test = create_test_app(35);
    test.paste(
        "3m * 2m\n\
                    4\n\
                    sum",
    );
    test.assert_results(&["6 m^2", "4", "Err"][..]);
}

#[test]
fn test_ctrl_c() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("12*3");
    test.input(EditorInputEvent::Char('c'), InputModifiers::ctrl());
    assert_eq!(&Some("36".to_owned()), &test.app().clipboard);
    assert!(test.app().editor.clipboard.is_empty());
}

#[test]
fn test_ctrl_c_without_selection2() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste(
        "2\n\
                        4\n\
                        5",
    );
    test.render();
    test.input(EditorInputEvent::Up, InputModifiers::shift());
    test.input(EditorInputEvent::Up, InputModifiers::shift());
    test.input(EditorInputEvent::Left, InputModifiers::alt());

    test.assert_results(&["10", "100", "101"][..]);
}

#[test]
fn test_line_ref_selection() {
    // left
    {
        let test = create_test_app(35);
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
        let test = create_test_app(35);
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
fn test_space_is_inserted_before_lineref() {
    let requires_space = &['4', 'a', '_'];
    let does_not_requires_space = &['+', '*', '/', '(', ')', '[', ']'];
    for ch in requires_space {
        let test = create_test_app(35);
        test.paste("16892313\n");
        test.input(EditorInputEvent::Char(*ch), InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();

        let mut expected: String = "16892313\n".to_owned();
        expected.push(*ch);
        expected.push_str(" &[1]");
        assert_eq!(test.get_editor_content(), expected);
    }

    for ch in does_not_requires_space {
        let test = create_test_app(35);
        test.paste("16892313\n");
        test.input(EditorInputEvent::Char(*ch), InputModifiers::none());
        if *ch == '[' {
            test.input(EditorInputEvent::Del, InputModifiers::none());
        }
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();

        let mut expected: String = "16892313\n".to_owned();
        expected.push(*ch);
        expected.push_str("&[1]");
        let expected_len = "16892313\n(&[1]".len(); // it is needed since '(' inserts a ')' as well);
        assert_eq!(
            test.get_editor_content()[0..expected_len],
            expected[0..expected_len]
        );
    }
}

#[test]
fn test_line_refs_are_automatically_separated_by_space() {
    let test = create_test_app(35);
    test.paste("16892313\n");
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    assert_eq!("16892313\n&[1] &[1]", test.get_editor_content());
}

#[test]
fn test_line_ref_selection_with_mouse() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("apple = 12");
    test.render();
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.paste("apple + 2");
    test.assert_results(&["12", "14"][..]);
}

#[test]
fn test_variable_must_be_defined() {
    let test = create_test_app(35);
    test.paste("apple = 12");
    test.render();
    test.input(EditorInputEvent::Home, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.paste("apple + 2");

    test.assert_results(&["2", "12"][..]);
}

#[test]
fn test_variables_can_be_defined_afterwards_of_their_usage() {
    let test = create_test_app(35);
    test.paste("apple * 2");
    test.set_cursor_row_col(0, 0);

    test.render();
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.assert_results(&["", "2"][..]);
    // now define the variable 'apple'
    test.paste("apple = 3");

    test.assert_results(&["3", "6"][..]);
}

#[test]
fn test_variables_can_be_defined_afterwards_of_their_usage2() {
    let test = create_test_app(35);
    test.paste("apple asd * 2");
    test.set_cursor_row_col(0, 0);

    test.render();
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());

    test.assert_results(&["", "2"][..]);
    // now define the variable 'apple'
    test.paste("apple asd = 3");

    test.assert_results(&["3", "6"][..]);
}

#[test]
fn test_renaming_variable_declaration() {
    let test = create_test_app(35);
    test.paste("apple = 2\napple * 3");
    test.set_cursor_row_col(0, 0);

    test.assert_results(&["2", "6"][..]);

    // rename apple to aapple
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());

    test.assert_results(&["2", "3"][..]);
}

#[test]
fn test_moving_line_does_not_change_its_lineref() {
    let test = create_test_app(35);
    test.paste("1\n2\n3\n\n\n50year");
    // cursor is in 4th row
    test.set_cursor_row_col(3, 0);

    test.assert_results(&["1", "2", "3", "", "", "50 year"][..]);

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

    test.assert_results(&["1", "", "2", "3", "6", "", "50 year"][..]);

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

    use super::*;
    use notecalc_lib::test_common::test_common::{
        pulsing_changed_content_rect, pulsing_result_rect,
    };

    #[test]
    fn test_modifying_a_lineref_recalcs_its_dependants() {
        let test = create_test_app(35);
        test.paste("2\n * 3");
        test.set_cursor_row_col(1, 0);

        test.assert_results(&["2", "3"][..]);

        // insert linref of 1st line
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();

        test.assert_results(&["2", "6"][..]);

        // now modify the first row
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Home, InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;

        assert_contains_pulse(
            render_commands,
            1,
            pulsing_result_rect(
                test.get_render_data().result_gutter_x + RIGHT_GUTTER_WIDTH,
                0,
                2,
                1,
            ),
        );
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_result_rect(
                test.get_render_data().result_gutter_x + RIGHT_GUTTER_WIDTH,
                1,
                2,
                1,
            ),
        );

        test.assert_results(&["12", "36"][..]);
    }

    #[test]
    fn test_that_dependant_line_refs_are_pulsed_on_change() {
        let test = create_test_app(35);
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

        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_changed_content_rect(LEFT_GUTTER_MIN_WIDTH, 1, 2, 1),
        );
    }

    #[test]
    fn test_that_all_dependant_line_refs_in_same_row_are_pulsed_only_once_on_change() {
        let test = create_test_app(35);
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

        let render_commands = &test.render_bucket().pulses;

        let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_changed_content_rect(left_gutter_width, 1, 2, 1),
        );
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_changed_content_rect(left_gutter_width + 3, 1, 2, 1),
        );

        // the last 2 command is for pulsing references for the active row
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(left_gutter_width, 1, 2, 1),
        );
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(left_gutter_width + 3, 1, 2, 1),
        );
    }

    #[test]
    fn test_that_all_dependant_line_refs_in_different_rows_are_pulsed_on_change() {
        let test = create_test_app(35);
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

        let render_commands = &test.render_bucket().pulses;

        let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_changed_content_rect(left_gutter_width, 1, 2, 1),
        );

        assert_contains_pulse(
            render_commands,
            1,
            pulsing_changed_content_rect(left_gutter_width, 2, 2, 1),
        );
        // the last 2 command is for pulsing references for the active row
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(left_gutter_width, 1, 2, 1),
        );
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(left_gutter_width, 2, 2, 1),
        );
    }

    #[test]
    fn test_that_dependant_line_refs_are_pulsing_when_the_cursor_is_on_the_referenced_line() {
        let test = create_test_app(35);
        test.paste("2\n * 3");
        test.set_cursor_row_col(1, 0);
        test.render();

        // insert linref of 1st line
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        test.render(); // it is needed

        // there should not be pulsing here yet
        test.assert_no_pulsing();

        // step into the first row
        test.input(EditorInputEvent::Up, InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;

        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(LEFT_GUTTER_MIN_WIDTH, 1, 1, 1),
        );
    }

    #[test]
    fn test_that_variable_pulsing_appears_at_edge_of_editor_if_it_is_outside_of_it() {
        let test = create_test_app2(30, 30);
        test.paste(
            "b = 1
aaaaaaaaaaaaaaaaaaaaaa b",
        );
        test.input(EditorInputEvent::Up, InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(
                "aaaaaaaaaaaaaaaaaaaaaa".len() + LEFT_GUTTER_MIN_WIDTH + " ".len(),
                1,
                1,
                1,
            ),
        );

        test.input(EditorInputEvent::End, InputModifiers::none());
        // this step reduces the editor width
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        assert_eq!(test.get_render_data().current_editor_width, 23);
        // if it is out of screen, it is rendered on the '...'
        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(render_commands, 1, pulsing_ref_rect(25, 1, 1, 1));

        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        assert_eq!(test.get_render_data().current_editor_width, 22);

        // if it is out of screen, it is rendered on the '...'
        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(render_commands, 1, pulsing_ref_rect(24, 1, 1, 1));
    }

    #[test]
    fn test_that_variable_pulsing_appears_at_edge_of_editor_if_it_is_outside_of_it_2() {
        let test = create_test_app2(30, 30);
        test.paste(
            "bcdef = 1
aaaaaaaaaaaaaaaaaa bcdef",
        );
        test.input(EditorInputEvent::Up, InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(
                "aaaaaaaaaaaaaaaaaa".len() + LEFT_GUTTER_MIN_WIDTH + " ".len(),
                1,
                5,
                1,
            ),
        );

        test.input(EditorInputEvent::End, InputModifiers::none());
        // this step reduces the editor width
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        assert_eq!(test.get_render_data().current_editor_width, 23);
        // if it is out of screen, it is rendered on the '...'
        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(render_commands, 1, pulsing_ref_rect(21, 1, 5, 1));
        test.assert_contains_variable(1, |cmd| {
            cmd.text == &['b', 'c', 'd', 'e'] && cmd.row == canvas_y(1) && cmd.column == 21
        });

        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        assert_eq!(test.get_render_data().current_editor_width, 22);
        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(render_commands, 1, pulsing_ref_rect(21, 1, 4, 1));
        test.assert_contains_variable(1, |cmd| {
            cmd.text == &['b', 'c', 'd'] && cmd.row == canvas_y(1) && cmd.column == 21
        });

        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        assert_eq!(test.get_render_data().current_editor_width, 20);
        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(render_commands, 1, pulsing_ref_rect(21, 1, 2, 1));
        test.assert_contains_variable(1, |cmd| {
            cmd.text == &['b'] && cmd.row == canvas_y(1) && cmd.column == 21
        });

        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        assert_eq!(test.get_render_data().current_editor_width, 19);
        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(render_commands, 1, pulsing_ref_rect(21, 1, 1, 1));
        test.assert_contains_variable(0, |cmd| cmd.row == canvas_y(1) && !cmd.text.is_empty());
    }

    #[test]
    fn test_that_lineref_pulsing_appears_at_edge_of_editor_if_it_is_outside_of_it() {
        let test = create_test_app2(30, 30);
        test.paste(
            "1
aaaaaaaaaaaaaaaaaaaaaa &[1]",
        );
        test.input(EditorInputEvent::Up, InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(
                "aaaaaaaaaaaaaaaaaaaaaa".len() + LEFT_GUTTER_MIN_WIDTH + " ".len(),
                1,
                1,
                1,
            ),
        );

        test.input(EditorInputEvent::End, InputModifiers::none());
        // this step reduces the editor width
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        assert_eq!(test.get_render_data().current_editor_width, 23);
        // should appear
        // if it is out of screen, it is rendered on the '...'
        let render_commands = &test.render_bucket().custom_commands[Layer::AboveText as usize];
        assert_contains(
            render_commands,
            1,
            OutputMessage::RenderChar(RenderChar {
                col: test.get_render_data().result_gutter_x - 1,
                row: canvas_y(1),
                char: '…',
            }),
        );
        assert_contains_pulse(
            &test.render_bucket().pulses,
            1,
            pulsing_ref_rect(25, 1, 1, 1),
        );

        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        assert_eq!(test.get_render_data().current_editor_width, 22);

        // if it is out of screen, it is rendered on the '...'
        assert_contains_pulse(
            &test.render_bucket().pulses,
            1,
            pulsing_ref_rect(24, 1, 1, 1),
        );
    }

    #[test]
    fn test_that_lineref_pulsing_appears_at_edge_of_editor_if_it_is_outside_of_it_2() {
        let test = create_test_app2(30, 30);
        test.paste(
            "1
aaaaaaaaaaaaaaaaaaaa &[1]",
        );
        test.input(EditorInputEvent::Up, InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(
                "aaaaaaaaaaaaaaaaaaaa".len() + LEFT_GUTTER_MIN_WIDTH + " ".len(),
                1,
                1,
                1,
            ),
        );

        test.input(EditorInputEvent::End, InputModifiers::none());
        {
            // this step reduces the editor width
            test.input(EditorInputEvent::Char('2'), InputModifiers::none());
            test.input(EditorInputEvent::Char('3'), InputModifiers::none());
            assert_eq!(test.get_render_data().current_editor_width, 22);
            // if it is out of screen, it is rendered on the '...'
            let render_commands = &test.render_bucket().custom_commands[Layer::AboveText as usize];
            assert_contains(
                render_commands,
                1,
                OutputMessage::RenderChar(RenderChar {
                    col: test.get_render_data().result_gutter_x - 1,
                    row: canvas_y(1),
                    char: '…',
                }),
            );
            assert_contains_pulse(
                &test.render_bucket().pulses,
                1,
                pulsing_ref_rect(23, 1, 2, 1),
            );
            test.assert_contains_line_ref_result(1, |cmd| {
                cmd.text == "1".to_owned() && cmd.row == canvas_y(1) && cmd.column == 23
            });
        }

        {
            test.input(EditorInputEvent::Char('4'), InputModifiers::none());
            assert_eq!(test.get_render_data().current_editor_width, 20);
            let render_commands = &test.render_bucket().custom_commands[Layer::AboveText as usize];
            assert_contains_pulse(
                &test.render_bucket().pulses,
                1,
                pulsing_ref_rect(22, 1, 1, 1),
            );
            test.assert_contains_line_ref_result(0, |cmd| {
                !cmd.text.is_empty() && cmd.row == canvas_y(1)
            });
            assert_contains(
                render_commands,
                1,
                OutputMessage::RenderChar(RenderChar {
                    col: test.get_render_data().result_gutter_x - 1,
                    row: canvas_y(1),
                    char: '…',
                }),
            );
        }
    }

    #[test]
    fn test_that_multiple_dependant_line_refs_are_pulsed_when_the_cursor_is_on_the_referenced_line()
    {
        let test = create_test_app(35);
        test.paste("2\n * 3");
        test.set_cursor_row_col(1, 0);
        test.render();

        // insert linref of 1st line
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();
        test.input(EditorInputEvent::Char(' '), InputModifiers::alt());
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();

        // requires so the pulsings caused by the changes above are consumed
        test.render();

        // there should not be pulsing here yet
        test.assert_no_pulsing();

        // step into the first row
        test.input(EditorInputEvent::Up, InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;

        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(LEFT_GUTTER_MIN_WIDTH, 1, 1, 1),
        );
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(LEFT_GUTTER_MIN_WIDTH + 2, 1, 1, 1),
        );
    }

    #[test]
    fn test_that_multiple_dependant_vars_are_pulsed_when_the_cursor_is_on_the_definition_line() {
        let test = create_test_app(35);
        test.paste("var = 2\nvar * 3\n12 * var");
        test.set_cursor_row_col(1, 0);
        test.render();

        // there should not be pulsing here yet
        test.assert_no_pulsing();

        // step into the first row
        test.input(EditorInputEvent::Up, InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(LEFT_GUTTER_MIN_WIDTH, 1, 3, 1),
        );
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(LEFT_GUTTER_MIN_WIDTH + 5, 2, 3, 1),
        );
    }

    #[test]
    fn test_that_dependant_vars_are_pulsed_when_the_cursor_is_on_the_definition_line() {
        let test = create_test_app(35);
        test.paste("var = 2\nvar * 3");
        test.set_cursor_row_col(1, 0);
        test.render();

        // there should not be pulsing here yet
        test.assert_no_pulsing();

        // step into the first row
        test.input(EditorInputEvent::Up, InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;
        assert_contains_pulse(
            render_commands,
            1,
            pulsing_ref_rect(LEFT_GUTTER_MIN_WIDTH, 1, 3, 1),
        );
    }
}

#[test]
fn test_modifying_a_lineref_does_not_change_the_line_id() {
    let test = create_test_app(35);
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

    test.assert_results(&["2", "3", "6"][..]);

    // now modify the 2nd row
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Home, InputModifiers::none());
    test.input(EditorInputEvent::Del, InputModifiers::none());

    test.assert_results(&["2", "Err", ""][..]);

    test.input(EditorInputEvent::Char('4'), InputModifiers::none());

    test.assert_results(&["2", "4", "8"][..]);
}

mod dependent_lines_recalculation_tests {
    use super::*;

    use notecalc_lib::test_common::test_common::pulsing_changed_content_rect;

    #[test]
    fn test_modifying_a_lineref_recalcs_its_dependants_only_if_its_value_has_changed() {
        let test = create_test_app(35);
        test.paste("2\n * 3");
        test.set_cursor_row_col(1, 0);

        test.assert_results(&["2", "3"][..]);

        // insert linref of 1st line
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.alt_key_released();

        test.assert_results(&["2", "6"][..]);

        // now modify the first row
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::End, InputModifiers::none());
        test.render();
        // inserting a '.' does not modify the result of the line
        test.input(EditorInputEvent::Char('.'), InputModifiers::none());

        let render_commands = &test.render_bucket().pulses;
        // expect no pulsing since there were no value change
        assert_contains_pulse(
            render_commands,
            0,
            pulsing_changed_content_rect(90, 0, 2, 1),
        );
        assert_contains_pulse(
            render_commands,
            0,
            pulsing_changed_content_rect(90, 1, 2, 1),
        );

        test.assert_results(&["2", "6"][..]);
    }

    #[test]
    fn test_renaming_variable_declaration2() {
        let test = create_test_app(35);
        test.paste("apple = 2\naapple * 3");
        test.set_cursor_row_col(0, 0);

        test.assert_results(&["2", "3"][..]);

        // rename apple to aapple
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());

        test.assert_results(&["2", "6"][..]);
    }

    #[test]
    fn test_removing_variable_declaration() {
        let test = create_test_app(35);
        test.paste("apple = 2\napple * 3");
        test.set_cursor_row_col(0, 0);

        test.assert_results(&["2", "6"][..]);

        // remove the content of the first line
        test.input(EditorInputEvent::End, InputModifiers::shift());

        test.input(EditorInputEvent::Del, InputModifiers::none());

        test.assert_results(&["", "3"][..]);
    }

    #[test]
    fn test_that_variable_dependent_rows_are_recalculated() {
        let test = create_test_app(35);
        test.paste("apple = 2\napple * 3");
        test.set_cursor_row_col(0, 9);

        test.assert_results(&["2", "6"][..]);

        // change value of 'apple' from 2 to 24
        test.input(EditorInputEvent::Char('4'), InputModifiers::none());

        test.assert_results(&["24", "72"][..]);
    }

    #[test]
    fn test_that_sum_is_recalculated_if_anything_changes_above() {
        let test = create_test_app(35);
        test.paste("2\n3\nsum");
        test.set_cursor_row_col(0, 1);

        test.assert_results(&["2", "3", "5"][..]);

        // change value from 2 to 21
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());

        test.assert_results(&["21", "3", "24"][..]);
    }

    #[test]
    fn test_that_sum_is_recalculated_if_anything_changes_above2() {
        let test = create_test_app(35);
        test.paste("2\n3\n4 * sum");
        test.set_cursor_row_col(0, 1);

        test.assert_results(&["2", "3", "20"][..]);

        // change value from 2 to 21
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());

        test.assert_results(&["21", "3", "96"][..]);
    }

    #[test]
    fn test_that_sum_is_not_recalculated_if_there_is_separator() {
        let test = create_test_app(35);
        test.paste("2\n3\n#\n5\nsum");
        test.set_cursor_row_col(0, 1);

        test.assert_results(&["2", "3", "", "5", "5"][..]);

        // change value from 2 to 12
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());

        test.assert_results(&["21", "3", "", "5", "5"][..]);
    }

    #[test]
    fn test_that_sum_is_not_recalculated_if_there_is_separator_with_comment() {
        let test = create_test_app(35);
        test.paste("2\n3\n# some comment\n5\nsum");
        test.set_cursor_row_col(0, 1);

        test.assert_results(&["2", "3", "", "5", "5"][..]);

        // change value from 2 to 12
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());

        test.assert_results(&["21", "3", "", "5", "5"][..]);
    }

    #[test]
    fn test_adding_sum_updates_lower_sums() {
        let test = create_test_app(35);
        test.paste("2\n3\n\n4\n5\nsum\n# some comment\n24\n25\nsum");
        test.set_cursor_row_col(2, 0);

        test.assert_results(&["2", "3", "", "4", "5", "14", "", "24", "25", "49"][..]);

        test.paste("sum");

        test.assert_results(&["2", "3", "5", "4", "5", "19", "", "24", "25", "49"][..]);
    }

    #[test]
    fn test_updating_two_sums() {
        let test = create_test_app(35);
        test.paste("2\n3\nsum\n4\n5\nsum\n# some comment\n24\n25\nsum");
        test.set_cursor_row_col(0, 1);

        test.assert_results(&["2", "3", "5", "4", "5", "19", "", "24", "25", "49"][..]);

        // change value from 2 to 21
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());

        test.assert_results(&["21", "3", "24", "4", "5", "57", "", "24", "25", "49"][..]);
    }
}

#[test]
fn test_sum_inside_parens() {
    {
        let test = create_test_app(35);
        test.paste("12\n(2*sum)");
        test.assert_results(&["12", "24"]);
    }
}

#[test]
fn test_that_result_is_not_changing_if_tokens_change_before_it() {
    let test = create_test_app(35);
    test.paste("111");

    test.input(EditorInputEvent::Home, InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());

    // expect no pulsing since there were no value change
    test.assert_no_pulsing();
}

#[test]
fn test_variable_redefine() {
    let test = create_test_app(35);
    test.paste("apple = 12");
    test.render();
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.paste("apple + 2");
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.paste("apple = 0");
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.paste("apple + 3");

    test.assert_results(&["12", "14", "0", "3"][..]);
}

#[test]
fn test_backspace_bug_editor_obj_deletion_for_simple_tokens() {
    let test = create_test_app(35);
    test.paste("asd sad asd asd sX");
    test.render();
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    assert_eq!("asd sad asd asd s", test.get_editor_content());
}

#[test]
fn test_rendering_while_cursor_move() {
    let test = create_test_app(35);
    test.paste("apple = 12$\nasd q");
    test.render();
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.render();
}

#[test]
fn select_only_2_lines_render_bug() {
    let test = create_test_app(35);
    test.paste("1\n2\n3");
    test.render();
    test.input(EditorInputEvent::Up, InputModifiers::shift());

    let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
    let expected_x = left_gutter_width + 4;
    let commands = &test.render_bucket().custom_commands[Layer::AboveText as usize];
    assert_contains(
        commands,
        1,
        OutputMessage::RenderChar(RenderChar {
            col: expected_x,
            row: canvas_y(1),
            char: '⎫',
        }),
    );

    assert_contains(
        commands,
        1,
        OutputMessage::RenderChar(RenderChar {
            col: expected_x,
            row: canvas_y(2),
            char: '⎭',
        }),
    );
    assert_contains(
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
fn sum_popup_position_itself_if_there_is_not_enough_space() {
    let test = create_test_app(35);
    test.paste("1\n2\n3");
    test.render();
    test.input(EditorInputEvent::Up, InputModifiers::shift());

    let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
    let expected_x = left_gutter_width + 4;
    let commands = &test.render_bucket().custom_commands[Layer::AboveText as usize];
    assert_contains(
        commands,
        1,
        OutputMessage::RenderChar(RenderChar {
            col: expected_x,
            row: canvas_y(1),
            char: '⎫',
        }),
    );
    assert_contains(
        commands,
        1,
        OutputMessage::RenderChar(RenderChar {
            col: expected_x,
            row: canvas_y(2),
            char: '⎭',
        }),
    );
    assert_contains(
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
fn test_undoing_selection_removal_works() {
    let test = create_test_app(35);
    test.paste(
        "aaa
bbb
ccc

ddd",
    );
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::End, InputModifiers::none());
    test.handle_time(1000);
    test.input(EditorInputEvent::PageUp, InputModifiers::shift());
    test.handle_time(1000);
    test.input(EditorInputEvent::Del, InputModifiers::none());
    test.input(EditorInputEvent::Char('z'), InputModifiers::ctrl());

    let left_gutter_width = test.get_render_data().left_gutter_width;
    test.assert_contains_text(1, |cmd| {
        cmd.text == &['a', 'a', 'a'] && cmd.row == canvas_y(0) && cmd.column == left_gutter_width
    });
    test.assert_contains_text(1, |cmd| {
        cmd.text == &['b', 'b', 'b'] && cmd.row == canvas_y(1) && cmd.column == left_gutter_width
    });
    test.assert_contains_text(1, |cmd| {
        cmd.text == &['c', 'c', 'c'] && cmd.row == canvas_y(2) && cmd.column == left_gutter_width
    });
    test.assert_contains_text(1, |cmd| {
        cmd.text == &['d', 'd', 'd'] && cmd.row == canvas_y(4) && cmd.column == left_gutter_width
    });
}

#[test]
fn scroll_dragging_limit() {
    let test = create_test_app(35);
    test.repeated_paste("1\n", 39);
    test.render();

    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    assert_eq!(test.get_render_data().scroll_y, 0);

    test.click(test.get_render_data().result_gutter_x - SCROLLBAR_WIDTH, 0);
    for i in 0..5 {
        test.handle_drag(
            test.get_render_data().result_gutter_x - SCROLLBAR_WIDTH,
            1 + i,
        );
        assert_eq!(test.get_render_data().scroll_y, 1 + i as usize);
    }
    test.handle_drag(test.get_render_data().result_gutter_x - SCROLLBAR_WIDTH, 6);
    // the scrollbar reached its bottom position, it won't go further down
    assert_eq!(test.get_render_data().scroll_y, 5);
}

#[test]
fn scroll_dragging_upwards() {
    let test = create_test_app(35);
    test.repeated_paste("1\n", 39);

    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.click(test.get_render_data().result_gutter_x - SCROLLBAR_WIDTH, 0);

    assert_eq!(test.get_render_data().scroll_y, 0);

    for i in 0..5 {
        test.handle_drag(
            test.get_render_data().result_gutter_x - SCROLLBAR_WIDTH,
            1 + i,
        );
        assert_eq!(test.get_render_data().scroll_y, 1 + i as usize);
    }
    for i in 0..5 {
        test.handle_drag(
            test.get_render_data().result_gutter_x - SCROLLBAR_WIDTH,
            4 - i,
        );
        assert_eq!(test.get_render_data().scroll_y, 4 - i as usize);
    }
}

#[test]
fn limiting_cursor_does_not_kill_selection() {
    let test = create_test_app(35);

    test.repeated_paste("1\n", VARIABLE_ARR_SIZE);
    test.set_cursor_row_col(0, 0);
    test.render();
    test.input(EditorInputEvent::PageDown, InputModifiers::shift());
    test.render();
    assert_eq!(
        test.get_selection().is_range_ordered(),
        Some((
            Pos::from_row_column(0, 0),
            Pos::from_row_column(MAX_LINE_COUNT - 1, 0)
        ))
    );
}

#[test]
fn deleting_all_selected_lines_no_panic() {
    let test = create_test_app(35);
    test.repeated_paste("1\n", MAX_LINE_COUNT + 20);
    test.set_cursor_row_col(0, 0);
    test.render();
    test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
    test.render();
    test.input(EditorInputEvent::Del, InputModifiers::ctrl());
}

#[test]
fn test_setting_left_gutter_width() {
    // future proof test
    let test = create_test_app(35);
    test.paste("");
    for i in 0..MAX_LINE_COUNT {
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        let rendered_line_num = i + 2;
        let expected_w = format!("{}", rendered_line_num).len() + 1;
        assert_eq!(
            test.get_render_data().left_gutter_width,
            expected_w,
            "at line {}. the left gutter width should be {}",
            rendered_line_num,
            expected_w
        );
    }
}

#[test]
fn test_handling_too_much_rows_no_panic() {
    let test = create_test_app(35);
    test.paste(&("1\n".repeat(MAX_LINE_COUNT - 1).to_owned()));
    test.set_cursor_row_col(MAX_LINE_COUNT - 2, 1);

    test.render();
    test.input(EditorInputEvent::Enter, InputModifiers::none());
}

#[test]
fn inserting_too_many_rows_no_panic() {
    let test = create_test_app(35);
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
fn test_sum_rerender() {
    // rust's borrow checker forces me to do this
    {
        let test = create_test_app(35);
        test.paste("1\n2\n3\nsum");

        test.assert_results(&["1", "2", "3", "6"][..]);
    }
    {
        let test = create_test_app(35);
        test.paste("1\n2\n3\nsum");
        test.input(EditorInputEvent::Up, InputModifiers::none());

        test.assert_results(&["1", "2", "3", "6"][..]);
    }
    {
        let test = create_test_app(35);
        test.paste("1\n2\n3\nsum");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::none());

        test.assert_results(&["1", "2", "3", "6"][..]);
    }
    {
        let test = create_test_app(35);
        test.paste("1\n2\n3\nsum");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Down, InputModifiers::none());

        test.assert_results(&["1", "2", "3", "6"][..]);
    }
    {
        let test = create_test_app(35);
        test.paste("1\n2\n3\nsum");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Down, InputModifiers::none());
        test.input(EditorInputEvent::Down, InputModifiers::none());

        test.assert_results(&["1", "2", "3", "6"][..]);
    }
}

#[test]
fn test_sum_rerender_with_ignored_lines() {
    {
        let test = create_test_app(35);
        test.paste("1\n'2\n3\nsum");

        test.assert_results(&["1", "3", "4"][..]);
    }
    {
        let test = create_test_app(35);
        test.paste("1\n'2\n3\nsum");
        test.input(EditorInputEvent::Up, InputModifiers::none());

        test.assert_results(&["1", "3", "4"][..]);
    }
    {
        let test = create_test_app(35);
        test.paste("1\n'2\n3\nsum");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::none());

        test.assert_results(&["1", "3", "4"][..]);
    }
    {
        let test = create_test_app(35);
        test.paste("1\n'2\n3\nsum");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Down, InputModifiers::none());

        test.assert_results(&["1", "3", "4"][..]);
    }
    {
        let test = create_test_app(35);
        test.paste("1\n'2\n3\nsum");
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Down, InputModifiers::none());

        test.assert_results(&["1", "3", "4"][..]);
    }
}

#[test]
fn test_sum_rerender_with_sum_reset() {
    {
        let test = create_test_app(35);
        test.paste("1\n#2\n3\nsum");

        test.assert_results(&["1", "3", "3"][..]);
    }
    {
        let test = create_test_app(35);
        test.paste("1\n#2\n3\nsum");
        test.input(EditorInputEvent::Up, InputModifiers::none());

        test.assert_results(&["1", "3", "3"][..]);
    }
}

#[test]
fn test_paste_long_text() {
    let test = create_test_app(35);
    test.paste("a\nb\na\nb\na\nb\na\nb\na\nb\na\nb\n1");

    test.assert_results(&["", "", "", "", "", "", "", "", "", "", "", "1"][..]);
}

#[test]
fn test_thousand_separator_and_alignment_in_result() {
    let test = create_test_app(35);
    test.paste("1\n2.3\n2222\n4km\n50000");
    test.set_cursor_row_col(2, 0);
    // set result to binary repr
    test.input(EditorInputEvent::Left, InputModifiers::alt());

    let render_buckets = test.render_bucket();
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
fn test_results_have_same_alignment_only_within_single_region() {
    let test = create_test_app(35);
    test.paste("1\n2.3\n2222\n4km\n50000\n# header\n123456789");
    test.set_cursor_row_col(2, 0);

    let render_commands = &test.render_bucket().ascii_texts;
    let base_x = &test.get_render_data().result_gutter_x + RIGHT_GUTTER_WIDTH;
    // the last row is in a separate region, it does not affect the alignment for the first (unnamed) region
    assert_eq!(render_commands[0].text, "1".as_bytes());
    assert_eq!(render_commands[0].row, canvas_y(0));
    assert_eq!(render_commands[0].column, base_x + 5);

    assert_eq!(render_commands[1].text, "2".as_bytes());
    assert_eq!(render_commands[1].row, canvas_y(1));
    assert_eq!(render_commands[1].column, base_x + 5);

    assert_eq!(render_commands[2].text, ".3".as_bytes());
    assert_eq!(render_commands[2].row, canvas_y(1));
    assert_eq!(render_commands[2].column, base_x + 6);

    assert_eq!(render_commands[3].text, "2 222".as_bytes());
    assert_eq!(render_commands[3].row, canvas_y(2));
    assert_eq!(render_commands[3].column, base_x + 1);

    assert_eq!(render_commands[4].text, "4".as_bytes());
    assert_eq!(render_commands[4].row, canvas_y(3));
    assert_eq!(render_commands[4].column, base_x + 5);

    assert_eq!(render_commands[5].text, "km".as_bytes());
    assert_eq!(render_commands[5].row, canvas_y(3));
    assert_eq!(render_commands[5].column, base_x + 5 + 4);

    assert_eq!(render_commands[6].text, "50 000".as_bytes());
    assert_eq!(render_commands[6].row, canvas_y(4));
    assert_eq!(render_commands[6].column, base_x);

    assert_eq!(render_commands[7].text, "123 456 789".as_bytes());
    assert_eq!(render_commands[7].row, canvas_y(6));
    assert_eq!(render_commands[7].column, base_x);
}

#[test]
fn test_units_are_aligned_as_well() {
    let test = create_test_app(35);
    test.paste("1cm\n2.3m\n2222.33 km\n4km\n50000 mm");
    let render_buckets = test.render_bucket();

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
    let test = create_test_app(35);
    test.paste("1\n");
    test.set_cursor_row_col(1, 0);

    test.render();
    test.paste("4km");

    let render_buckets = test.render_bucket();

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
    let test = create_test_app(35);
    test.paste("0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12");
    test.render();

    test.input(EditorInputEvent::Up, InputModifiers::shift());
    test.input(EditorInputEvent::Up, InputModifiers::shift());
    test.input(EditorInputEvent::Up, InputModifiers::shift());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

    test.assert_results(&["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"][..]);
}

#[test]
fn test_ctrl_x_then_ctrl_z() {
    let test = create_test_app(35);
    test.paste("12");
    test.handle_time(1000);

    test.assert_results(&["12"][..]);

    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

    test.assert_results(&[""][..]);

    test.input(EditorInputEvent::Char('z'), InputModifiers::ctrl());

    test.assert_results(&["12"][..]);
}

#[test]
fn selection_in_the_first_row_should_not_panic() {
    let test = create_test_app(35);
    test.paste("1+1\nasd");
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Home, InputModifiers::shift());

    test.render();
}

#[test]
fn test_that_removed_tail_rows_are_cleared() {
    let test = create_test_app(35);
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
fn test_that_multiline_matrix_is_considered_when_scrolling() {
    let test = create_test_app(35);
    // editor height is 36 in tests, so create a 35 line text
    test.repeated_paste("a\n", 40);
    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());

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
    let test = create_test_app(35);
    test.repeated_paste("aaaaaaaaaaaa\n", 34);

    test.render();

    test.input(EditorInputEvent::PageDown, InputModifiers::none());
}

#[test]
fn ctrl_a_plus_typing() {
    let test = create_test_app(25);
    test.repeated_paste("1\n", 34);
    test.set_cursor_row_col(0, 0);

    test.render();

    test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());

    test.render();

    test.input(EditorInputEvent::Char('1'), InputModifiers::none());

    test.render();
}

#[test]
fn test_that_no_full_refresh_when_stepping_into_last_line() {
    let client_height = 25;
    let test = create_test_app(client_height);
    test.repeated_paste("1\n", client_height * 2);
    test.input(EditorInputEvent::PageUp, InputModifiers::none());

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
    let test = create_test_app(client_height);
    test.repeated_paste("1\n", client_height * 2);
    test.set_cursor_row_col(0, 0);

    test.render();

    test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());

    test.render();

    test.input(EditorInputEvent::Char('1'), InputModifiers::none());

    assert_eq!(
        None,
        test.app()
            .render_data
            .get_render_y(content_y(client_height * 2 - 1))
    );
}

#[test]
fn test_that_unvisible_rows_have_height_1() {
    let test = create_test_app(25);
    test.repeated_paste("1\n2\n\n[1;2;3;4]", 10);
    test.input(EditorInputEvent::PageUp, InputModifiers::none());

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
    let test = create_test_app(25);
    test.repeated_paste("1\n2\n\n[1;2;3;4]", 10);
    test.input(EditorInputEvent::PageUp, InputModifiers::none());

    for _ in 0..4 {
        test.handle_wheel(1);
    }
    test.render();
    assert_eq!(
        46,
        NoteCalcApp::calc_full_content_height(
            &test.get_render_data(),
            test.app().editor_content.line_count(),
        )
    );
}

#[test]
fn test_stepping_into_scrolled_matrix_panic() {
    let test = create_test_app(25);
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
fn test_that_scrolled_result_is_not_rendered() {
    {
        let test = create_test_app(35);
        test.paste("1\n2\n3\n");
        test.repeated_paste("aaaaaaaaaaaa\n", 34);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());

        test.assert_results(&["1", "2", "3"][..]);
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
        let test = create_test_app(35);
        test.paste("1\n2\n3\n");
        test.repeated_paste("aaaaaaaaaaaa\n", 34);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());

        test.handle_wheel(1);

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
        test.assert_results(&["2", "3"][..]);
    }

    {
        let test = create_test_app(35);
        test.paste("1\n2\n3\n");
        test.repeated_paste("aaaaaaaaaaaa\n", 34);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());

        test.handle_wheel(1);
        test.handle_wheel(1);

        test.assert_results(&["3"][..]);
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
        let test = create_test_app(35);
        test.paste("1\n2\n3\n");
        test.repeated_paste("aaaaaaaaaaaa\n", 34);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());

        test.handle_wheel(1);
        test.handle_wheel(1);
        test.handle_wheel(1);

        test.assert_results(&[""][..]);
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
        let test = create_test_app(35);
        test.paste("1\n2\n3\n");
        test.repeated_paste("aaaaaaaaaaaa\n", 34);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());

        test.handle_wheel(1);
        test.handle_wheel(1);
        test.handle_wheel(1);
        test.handle_wheel(0);

        test.assert_results(&["3"][..]);
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
        let test = create_test_app(35);
        test.paste("1\n2\n3\n");
        test.repeated_paste("aaaaaaaaaaaa\n", 34);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());

        test.handle_wheel(1);
        test.handle_wheel(1);
        test.handle_wheel(1);
        test.handle_wheel(0);
        test.handle_wheel(0);

        test.assert_results(&["2", "3"][..]);
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
        let test = create_test_app(35);
        test.paste("1\n2\n3\n");
        test.repeated_paste("aaaaaaaaaaaa\n", 34);
        test.input(EditorInputEvent::PageUp, InputModifiers::none());

        test.handle_wheel(1);
        test.handle_wheel(1);
        test.handle_wheel(1);
        test.handle_wheel(0);
        test.handle_wheel(0);
        test.handle_wheel(0);

        test.assert_results(&["1", "2", "3"][..]);
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
        let test = create_test_app(35);
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
    let test = create_test_app(32);
    test.paste("var = 2\n");
    test.repeated_paste("asd\n", 40);
    test.paste("var");
    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    assert_eq!(test.get_render_data().scroll_y, 0);
    test.input(EditorInputEvent::PageDown, InputModifiers::none());
    assert_eq!(test.get_render_data().scroll_y, 10 /*42 - 32*/);
    test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());
    assert_eq!(test.get_render_data().scroll_y, 0);
}

#[test]
fn test_ctrl_b_jumps_to_var_def_negative() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("var = 2\nvar * 3");
    test.set_cursor_row_col(1, 0);

    test.render();
    let left_gutter_width = LEFT_GUTTER_MIN_WIDTH;
    let render_commands = &test.render_bucket().pulses;
    //  dependant row is not pulsed yet
    assert_contains_pulse(
        render_commands,
        0,
        pulsing_ref_rect(left_gutter_width, 1, 3, 1),
    );

    // step into the first row
    test.input(EditorInputEvent::Char('b'), InputModifiers::ctrl());

    assert_contains_pulse(
        render_commands,
        1,
        pulsing_ref_rect(left_gutter_width, 1, 3, 1),
    );
}

// let say a 3rd row references a var from the 2nd row.
// Then I remove the first row, then, when the parser parses the new
// 2nd row (which was the 3rd one), in its vars[1] there is the variable,
// since in the previous parse it was defined at index 1.
// this test guarantee that when parsing row index 1, var index 1
// is not considered.
#[test]
fn test_that_var_from_prev_frame_in_the_current_line_is_not_considered_during_parsing() {
    let test = create_test_app(35);
    test.paste(
        "
a = 10
b = a * 20",
    );
    test.set_cursor_row_col(0, 0);
    test.input(EditorInputEvent::Del, InputModifiers::none());
    assert!(matches!(
        &test.editor_objects()[content_y(1)][1].typ,
        EditorObjectType::Variable { var_index: 0 }
    ))
}

#[test]
fn converting_unit_of_line_ref() {
    let test = create_test_app(35);
    test.paste("573 390 s\n&[1] in h");

    test.assert_results(&["573 390 s", "159.2750 h"][..]);
}

#[test]
fn test_unit_conversion_for_variable() {
    let test = create_test_app(35);
    test.paste("input = 573 390 s\ninput in h");

    test.assert_results(&["573 390 s", "159.2750 h"][..]);
}

#[test]
fn test_unit_conversion_for_variable2() {
    let test = create_test_app(35);
    test.paste("input = 1 s\ninput h");

    test.assert_results(&["1 s", "Err"][..]);
}

#[test]
fn test_ininin() {
    let test = create_test_app(35);
    test.paste("12 in in in");

    test.assert_results(&["12 in"][..]);
}

#[test]
fn calc_pow() {
    let test = create_test_app(35);
    test.paste(
        "price = 350k$
down payment = 20% * price
finance amount = price - down payment

interest rate = 0.037 (1/year)
term = 30 years
// n = term * 12 (1/year)
n = 360
r = interest rate / (12 (1/year))

monthly payment = r/(1 - (1 + r)^(-n)) *finance amount",
    );
}

#[test]
fn no_panic_on_huge_input() {
    let test = create_test_app(35);
    test.paste("3^300");
}

#[test]
fn no_panic_on_huge_input2() {
    let test = create_test_app(35);
    test.paste("300^300");
}

#[test]
fn test_error_related_to_variable_precedence() {
    let test = create_test_app(35);
    test.paste(
        "v0=2m/s
t=4s
0m+v0*t",
    );

    test.assert_results(&["2 m / s", "4 s", "8 m"][..]);
}

#[test]
fn test_error_related_to_variable_precedence2() {
    let test = create_test_app(35);
    test.paste(
        "a = -9.8m/s^2
v0 = 100m/s
x0 = 490m
t = 2s
1/2*a*t^2 + v0*t + x0",
    );

    test.assert_results(&["-9.8 m / s^2", "100 m / s", "490 m", "2 s", "670.4 m"][..]);
}

#[test]
fn test_no_panic_on_too_big_number() {
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
n = 36000
r = interest rate / (12 (1/year))

monthly payment = r/(1 - (1 + r)^(-n)) *finance amount",
    );
    test.set_cursor_row_col(17, 9);
    test.input(EditorInputEvent::Char('0'), InputModifiers::none());
}

#[test]
fn test_itself_unit_rendering() {
    let test = create_test_app(35);
    test.paste("a = /year");

    test.assert_results(&[""][..]);
}

#[test]
fn test_itself_unit_rendering2() {
    let test = create_test_app(35);
    test.paste("a = 2/year");

    test.assert_results(&["2 / year"][..]);
}

#[test]
fn test_editor_panic() {
    let test = create_test_app(35);
    test.paste(
        "
a",
    );
    test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
    test.input(EditorInputEvent::Char('p'), InputModifiers::none());
}

#[test]
fn test_wrong_selection_removal() {
    let test = create_test_app(35);
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
fn test_explicit_multipl_is_mandatory_before_units() {
    let test = create_test_app(35);
    test.paste("2m^4kg/s^3");
    test.assert_results(&["2 m^4"])
}

#[test]
fn integration_test() {
    let test = create_test_app(35);
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

    test.assert_results(
        &[
            "350 000 $",
            "70 000 $",
            "280 000 $",
            "",
            "0.037 / year",
            "30 year",
            "",
            "360",
            "0.003083",
            "",
            "1 288.792357188724336511790584 $",
        ][..],
    );
}

#[test]
fn test_line_ref_rendered_precision() {
    let test = create_test_app(35);
    test.paste("0.00005");
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();

    test.assert_contains_line_ref_result(1, |cmd| {
        cmd.row == canvas_y(1) && cmd.column == 2 && cmd.text == "0.00005".to_owned()
    })
}

#[test]
fn test_if_number_is_too_big_for_binary_repr_show_err() {
    let test = create_test_app(35);
    test.paste("10e24");
    test.input(EditorInputEvent::Left, InputModifiers::alt());

    test.assert_results(&["Err"][..]);
}

#[test]
fn test_u64_hex_form() {
    let test = create_test_app(35);
    test.paste("0xFFFFFFFFFFFFFFFF");
    test.input(EditorInputEvent::Right, InputModifiers::alt());

    test.assert_results(&["FF FF FF FF FF FF FF FF"][..]);
}

#[test]
fn test_u64_bin_form() {
    let test = create_test_app(35);
    test.paste("0xFFFFFFFFFFFFFFFF");
    test.input(EditorInputEvent::Left, InputModifiers::alt());

    test.assert_results(
        &["11111111 11111111 11111111 11111111 11111111 11111111 11111111 11111111"][..],
    );
}

#[test]
fn test_negative_num_bin_form() {
    let test = create_test_app(35);
    test.paste("-256");
    test.input(EditorInputEvent::Left, InputModifiers::alt());

    test.assert_results(
        &["11111111 11111111 11111111 11111111 11111111 11111111 11111111 00000000"][..],
    );
}

#[test]
fn test_negative_num_hex_form() {
    let test = create_test_app(35);
    test.paste("-256");
    test.input(EditorInputEvent::Right, InputModifiers::alt());

    test.assert_results(&["FF FF FF FF FF FF FF 00"][..]);
}

#[test]
fn test_if_number_is_too_big_for_hex_repr_show_err() {
    let test = create_test_app(35);
    test.paste("10e24");
    test.input(EditorInputEvent::Right, InputModifiers::alt());

    test.assert_results(&["Err"][..]);
}

#[test]
fn test_if_quantity_is_too_big_for_binary_repr_show_err() {
    let test = create_test_app(35);
    test.paste("12km");
    test.input(EditorInputEvent::Left, InputModifiers::alt());

    test.assert_results(&["Err"][..]);
}

#[test]
fn test_if_quantity_is_too_big_for_hex_repr_show_err() {
    let test = create_test_app(35);
    test.paste("12km");
    test.input(EditorInputEvent::Right, InputModifiers::alt());

    test.assert_results(&["Err"][..]);
}

#[test]
fn test_if_percentage_is_too_big_for_binary_repr_show_err() {
    let test = create_test_app(35);
    test.paste("12%");
    test.input(EditorInputEvent::Left, InputModifiers::alt());

    test.assert_results(&["Err"][..]);
}

#[test]
fn test_if_percentage_is_too_big_for_hex_repr_show_err() {
    let test = create_test_app(35);
    test.paste("12%");
    test.input(EditorInputEvent::Right, InputModifiers::alt());

    test.assert_results(&["Err"][..]);
}

#[test]
fn test_float_for_bin_repr_show_err() {
    let test = create_test_app(35);
    test.paste("1.2");
    test.input(EditorInputEvent::Left, InputModifiers::alt());

    test.assert_results(&["Err"][..]);
}

#[test]
fn test_float_for_hex_repr_show_err() {
    let test = create_test_app(35);
    test.paste("1.2");
    test.input(EditorInputEvent::Right, InputModifiers::alt());

    test.assert_results(&["Err"][..]);
}

#[test]
fn integration_test_for_rich_copy() {
    let test = create_test_app(35);
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
    let test = create_test_app(35);
    test.paste("20%");

    test.assert_results(&["20 %"][..]);
}

#[test]
fn test_parsing_panic_20201116() {
    let test = create_test_app(35);
    test.paste("2^63-1\n6*13\nennyi staging entity lehet &[1] / 50\n\nnaponta ennyit kell beszurni, hogy \'1 év alatt megteljen: &[1] / 365\n\nennyi évig üzemel, ha napi ezer sor szurodik be: &[1] / (365*1000)\n120 * 100 = \n1.23e20\n\n500$ / 20$/hour in hour\n1km + 1000m\n3 kg * 3 liter\n3 hours + 5minutes + 10 seconds in seconds\n20%\n\n1t in kg\nmass of earth = 5.972e18 Gg\n\n20%\n");
}

#[test]
fn test_matrix_renders_dots_on_gutter_on_every_line_it_takes() {
    let expected_char_at = |test: &TestHelper, at: usize| {
        OutputMessage::RenderChar(RenderChar {
            col: test.get_render_data().result_gutter_x - 1,
            row: canvas_y(at as isize),
            char: '…',
        })
    };

    let test = create_test_app2(25, 35);
    test.paste("[1,2,3,4,5,6,7,8]");
    test.render(); // must be rendered again, right gutter is updated within 2 renders :(
    let commands = &test.render_bucket().custom_commands[Layer::AboveText as usize];
    assert_contains(commands, 1, expected_char_at(&test, 0));

    let test = create_test_app2(25, 35);
    test.paste("[1,2,3,4,5,6,7,8;1,2,3,4,5,6,7,8]");
    test.render(); // must be rendered again, right gutter is updated within 2 renders :(
    let commands = &test.render_bucket().custom_commands[Layer::AboveText as usize];
    assert_contains(commands, 1, expected_char_at(&test, 0));
    assert_contains(commands, 1, expected_char_at(&test, 1));

    let test = create_test_app2(25, 35);
    test.paste("[1,2,3,4,5,6,7,8;1,2,3,4,5,6,7,8;1,2,3,4,5,6,7,8]");
    test.render(); // must be rendered again, right gutter is updated within 2 renders :(
    let commands = &test.render_bucket().custom_commands[Layer::AboveText as usize];
    assert_contains(commands, 1, expected_char_at(&test, 0));
    assert_contains(commands, 1, expected_char_at(&test, 1));
    assert_contains(commands, 1, expected_char_at(&test, 2));
}

#[test]
fn test_line_number_rendering_for_tall_rows() {
    let test = create_test_app2(25, 35);
    test.paste(
        "1
2
3
[0,0,0,0;0,0,0,0;0,0,0,0]

asd",
    );
    let commands = &test.render_bucket().custom_commands[Layer::Text as usize];
    let mut expected_text_buf: [char; 2] = ['0', '0'];
    for i in 0..35 {
        let rendered_num: u8 = i + 1;
        let expected_text = if rendered_num < 10 {
            expected_text_buf[0] = (b'0' + rendered_num) as char;
            &expected_text_buf[0..1]
        } else {
            expected_text_buf[0] = (b'0' + (rendered_num / 10)) as char;
            expected_text_buf[1] = (b'0' + (rendered_num % 10)) as char;
            &expected_text_buf[..]
        };
        let expected_y_coord = if rendered_num < 4 {
            rendered_num - 1
        } else if rendered_num == 4 {
            5 // this is sthe matrix row, it is vertically aligned
        } else {
            rendered_num + 3
        };
        assert_contains(
            commands,
            1,
            OutputMessage::RenderUtf8Text(RenderUtf8TextMsg {
                text: expected_text,
                row: canvas_y(expected_y_coord as isize),
                column: 0,
            }),
        );
    }
}

#[test]
fn test_right_gutter_is_updated_when_text_changes() {
    let test = create_test_app2(49, 32);
    test.paste("[0,0,0,0,0;0,0,0,0,0;0,0,0,0,0]");

    // drag the rught gutter to left
    test.click(test.get_render_data().result_gutter_x, 0);
    test.handle_drag(10, 0);

    // start typing at beginning of the line
    test.input(EditorInputEvent::Home, InputModifiers::none());
    for _ in 0..4 {
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    }
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    // 15 matrix
    //  2 left gutter
    //  5 '2's
    //  2 padding (scrollbar + 1)
    assert_eq!(test.get_render_data().result_gutter_x, 24);
}

#[test]
fn test_right_gutter_is_moved_when_there_is_enough_result_space_but_no_editor_space() {
    let test = create_test_app2(48, 32);
    test.paste("");

    // drag the rught gutter to left
    test.click(test.get_render_data().result_gutter_x, 0);
    test.handle_drag(0, 0);

    test.paste("[0,0,0,0,0;0,0,0,0,0;0,0,0,0,0]");
    // 15 matrix
    //  3 left gutter
    //  1 padding (scrollbar)
    assert_eq!(test.get_render_data().result_gutter_x, 19);
}

#[test]
fn test_if_left_gutter_width_changes_editor_size_changes_as_well() {
    let test = create_test_app2(48, 32);
    test.paste("");

    let orig_editor_w = test.get_render_data().current_editor_width;
    let orig_result_x = test.get_render_data().result_gutter_x;

    test.repeated_paste("a\n", 10);
    // now the left gutter contains 2 digits, so its length is 3, decrasing the
    // length of the editor
    assert_eq!(
        test.get_render_data().current_editor_width,
        orig_editor_w - 1
    );
    assert_eq!(test.get_render_data().result_gutter_x, orig_result_x);
}

#[test]
fn test_precision() {
    let test = create_test_app2(48, 32);
    test.paste("0.0000000001165124023817148381");

    test.assert_results(&["0.0000000001165124023817148381"][..]);
}

#[test]
fn test_that_cursor_is_rendered_at_the_end_of_the_editor() {
    let test = create_test_app2(44, 32);
    test.paste("1234567890123456");
    assert_contains(
        &test.render_bucket().custom_commands[Layer::AboveText as usize],
        1,
        OutputMessage::RenderChar(RenderChar {
            col: 18,
            row: canvas_y(0),
            char: '▏',
        }),
    );

    test.input(EditorInputEvent::Char('7'), InputModifiers::none());
    assert_contains(
        &test.render_bucket().custom_commands[Layer::AboveText as usize],
        1,
        OutputMessage::RenderChar(RenderChar {
            col: 19,
            row: canvas_y(0),
            char: '▏',
        }),
    );
}

#[test]
fn results_must_be_rendered() {
    let test = create_test_app2(84, 36);
    test.paste(
        "# Results must be rendered even if header is in the first line
69",
    );
    test.assert_contains_result(1, |cmd| cmd.text == "69".as_bytes());
}

#[test]
fn results_must_be_rendered2() {
    let test = create_test_app2(84, 36);
    test.paste(
        "empty row\n\
            # Results must be rendered even if there are 2 headers below each other and an empty row in front of them\n\
            # second header\n\
            69",
    );
    test.assert_contains_result(1, |cmd| cmd.text == "69".as_bytes());
}

#[test]
fn empty_variable_name() {
    let test = create_test_app2(84, 36);
    test.paste("    =5$2*x-2044923+/I2(397-293496(6[/7k9]/^*6490^)(5/j=");
    test.input(EditorInputEvent::Char('9'), InputModifiers::none());
    assert!(test.mut_vars()[0].is_none());
}

#[test]
fn test_modification_happens_on_selection() {
    let test = create_test_app2(84, 36);
    test.paste("asd");
    assert!(test
        .input(EditorInputEvent::Home, InputModifiers::shift())
        .is_some())
}

#[test]
fn test_insert_closing_parenthesis_when_opening_paren_inserted() {
    for (tested_opening_char, expected_closing_char) in
        &[('(', ')'), ('[', ']'), ('{', '}'), ('\"', '\"')]
    {
        let tested_opening_char = *tested_opening_char;
        let expected_closing_char = *expected_closing_char;
        {
            let test = create_test_app2(84, 36);
            test.paste("");
            test.input(
                EditorInputEvent::Char(tested_opening_char),
                InputModifiers::none(),
            );
            let mut expected_str = String::with_capacity(2);
            expected_str.push(tested_opening_char);
            expected_str.push(expected_closing_char);
            assert_eq!(expected_str, test.get_editor_content());
            assert_eq!(Pos::from_row_column(0, 1), test.get_cursor_pos());
        }
        {
            if tested_opening_char == '[' || tested_opening_char == '\"' {
                continue; // because of matrix, it does not work as for the other chars
            }
            let test = create_test_app2(84, 36);
            test.paste("");
            let mut expected_str = String::with_capacity(20);
            for i in 0..10 {
                test.input(
                    EditorInputEvent::Char(tested_opening_char),
                    InputModifiers::none(),
                );
                expected_str.clear();
                for _ in 0..i + 1 {
                    expected_str.push(tested_opening_char);
                }
                for _ in 0..i + 1 {
                    expected_str.push(expected_closing_char);
                }

                assert_eq!(expected_str, test.get_editor_content());
            }
        }
    }
}

#[test]
fn test_removing_opening_parenthesis_removes_closing_as_well_if_they_are_neighbours() {
    for (tested_opening_char, expected_closing_char) in
        &[('(', ')'), ('[', ']'), ('{', '}'), ('\"', '\"')]
    {
        let tested_opening_char = *tested_opening_char;
        let expected_closing_char = *expected_closing_char;
        {
            let test = create_test_app2(84, 36);

            let mut pasted_str = String::with_capacity(2);
            pasted_str.push(tested_opening_char);
            pasted_str.push(expected_closing_char);

            test.paste(&pasted_str);
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.input(EditorInputEvent::Backspace, InputModifiers::none());
            assert_eq!("", &test.get_editor_content());
            assert_eq!(Pos::from_row_column(0, 0), test.get_cursor_pos());
        }
        {
            let test = create_test_app2(84, 36);

            let mut pasted_str = String::with_capacity(2);
            pasted_str.push(tested_opening_char);
            pasted_str.push(expected_closing_char);

            test.paste(&pasted_str);
            test.input(EditorInputEvent::Backspace, InputModifiers::none());
            assert_eq!(tested_opening_char.to_string(), test.get_editor_content());
            assert_eq!(Pos::from_row_column(0, 1), test.get_cursor_pos());
        }
        {
            let test = create_test_app2(84, 36);

            let mut pasted_str = String::with_capacity(2);
            pasted_str.push(tested_opening_char);
            pasted_str.push(expected_closing_char);

            test.paste(&pasted_str);
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.input(EditorInputEvent::Del, InputModifiers::none());
            assert_eq!(tested_opening_char.to_string(), test.get_editor_content());
            assert_eq!(Pos::from_row_column(0, 1), test.get_cursor_pos());
        }
        {
            let test = create_test_app2(84, 36);

            let mut pasted_str = String::with_capacity(2);
            pasted_str.push(tested_opening_char);
            pasted_str.push(expected_closing_char);

            test.paste(&pasted_str);
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.input(EditorInputEvent::Left, InputModifiers::none());
            test.input(EditorInputEvent::Del, InputModifiers::none());
            assert_eq!(expected_closing_char.to_string(), test.get_editor_content());
            assert_eq!(Pos::from_row_column(0, 0), test.get_cursor_pos());
        }
    }
}

#[test]
fn test_removing_opening_parenthesis_multiple_times() {
    for (tested_opening_char, expected_closing_char) in &[('(', ')'), ('{', '}')] {
        let tested_opening_char = *tested_opening_char;
        let expected_closing_char = *expected_closing_char;
        let test = create_test_app2(84, 36);
        let mut expected_str = String::with_capacity(20);
        test.paste("");
        for i in 0..10 {
            for _ in 0..i + 1 {
                test.input(
                    EditorInputEvent::Char(tested_opening_char),
                    InputModifiers::none(),
                );
            }
            {
                expected_str.clear();
                for _ in 0..i + 1 {
                    expected_str.push(tested_opening_char);
                }
                for _ in 0..i + 1 {
                    expected_str.push(expected_closing_char);
                }
                assert_eq!(test.get_editor_content(), expected_str);
            }
            for _ in 0..i + 1 {
                test.input(EditorInputEvent::Backspace, InputModifiers::none());
            }
            assert_eq!(&test.get_editor_content(), "");
            assert_eq!(Pos::from_row_column(0, 0), test.get_cursor_pos());
        }
    }
}

#[test]
fn test_removing_opening_parenthesis_removes_only_inside_content() {
    let mut expected_str = String::with_capacity(2);
    for (tested_opening_char, expected_closing_char) in
        &[('(', ')'), ('[', ']'), ('{', '}'), ('\"', '\"')]
    {
        let tested_opening_char = *tested_opening_char;
        let expected_closing_char = *expected_closing_char;
        let test = create_test_app2(84, 36);
        test.paste("");
        test.input(
            EditorInputEvent::Char(tested_opening_char),
            InputModifiers::none(),
        );
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Char('s'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());

        test.input(EditorInputEvent::Left, InputModifiers::shift());
        test.input(EditorInputEvent::Left, InputModifiers::shift());
        test.input(EditorInputEvent::Left, InputModifiers::shift());

        test.input(EditorInputEvent::Backspace, InputModifiers::none());

        expected_str.clear();
        expected_str.push(tested_opening_char);
        expected_str.push(expected_closing_char);
        assert_eq!(expected_str, test.get_editor_content());
        assert_eq!(Pos::from_row_column(0, 1), test.get_cursor_pos());
    }
}

#[test]
fn test_parenthesis_completion_1() {
    let test = create_test_app2(84, 36);
    test.paste("");
    test.input(EditorInputEvent::Char('{'), InputModifiers::none());
    test.input(EditorInputEvent::Char('{'), InputModifiers::none());
    test.input(EditorInputEvent::Char('('), InputModifiers::none());
    test.input(EditorInputEvent::Char('('), InputModifiers::none());
    assert_eq!("{{(())}}", &test.get_editor_content());
    assert_eq!(Pos::from_row_column(0, 4), test.get_cursor_pos());
}

#[test]
fn test_parens_are_inserted_only_if_next_char_is_whitspace() {
    let mut expected_str = String::with_capacity(2);
    for (tested_opening_char, _expected_closing_char) in
        &[('(', ')'), ('[', ']'), ('{', '}'), ('\"', '\"')]
    {
        let test = create_test_app2(84, 36);
        test.paste("asd");
        test.input(EditorInputEvent::Home, InputModifiers::none());
        test.input(
            EditorInputEvent::Char(*tested_opening_char),
            InputModifiers::none(),
        );

        expected_str.clear();
        expected_str.push(*tested_opening_char);
        expected_str.push_str("asd");
        assert_eq!(expected_str, test.get_editor_content());
    }
}

#[test]
fn test_insert_closing_parenthesis_around_selected_text() {
    for (tested_opening_char, expected_closing_char) in
        &[('(', ')'), ('[', ']'), ('{', '}'), ('\"', '\"')]
    {
        let tested_opening_char = *tested_opening_char;
        let expected_closing_char = *expected_closing_char;
        {
            let test = create_test_app2(84, 36);
            test.paste("asd");
            test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
            test.input(
                EditorInputEvent::Char(tested_opening_char),
                InputModifiers::none(),
            );

            let mut expected_str = String::with_capacity(2);
            expected_str.push(tested_opening_char);
            expected_str.push_str("asd");
            expected_str.push(expected_closing_char);
            assert_eq!(expected_str, test.get_editor_content());
            assert_eq!(
                test.get_selection(),
                Selection::range(Pos::from_row_column(0, 1), Pos::from_row_column(0, 4)),
            );
        }
        {
            let test = create_test_app2(84, 36);
            test.paste("asd");
            test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
            let mut expected_str = String::with_capacity(20);
            for i in 0..10 {
                test.input(
                    EditorInputEvent::Char(tested_opening_char),
                    InputModifiers::none(),
                );
                expected_str.clear();
                for _ in 0..i + 1 {
                    expected_str.push(tested_opening_char);
                }
                expected_str.push_str("asd");
                for _ in 0..i + 1 {
                    expected_str.push(expected_closing_char);
                }

                assert_eq!(expected_str, test.get_editor_content());
                assert_eq!(
                    test.get_selection(),
                    Selection::range(
                        Pos::from_row_column(0, i + 1),
                        Pos::from_row_column(0, 3 + (i + 1)),
                    ),
                );
            }
        }
    }
}

#[test]
fn test_insert_closing_parenthesis_around_multiline_selected_text() {
    for (tested_opening_char, expected_closing_char) in
        &[('(', ')'), ('[', ']'), ('{', '}'), ('\"', '\"')]
    {
        let tested_opening_char = *tested_opening_char;
        let expected_closing_char = *expected_closing_char;
        {
            let test = create_test_app2(84, 36);
            test.paste("asd\nbsd");
            test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
            test.input(
                EditorInputEvent::Char(tested_opening_char),
                InputModifiers::none(),
            );

            let mut expected_str = String::with_capacity(2);
            expected_str.push(tested_opening_char);
            expected_str.push_str("asd\nbsd");
            expected_str.push(expected_closing_char);
            assert_eq!(expected_str, test.get_editor_content());
            assert_eq!(
                test.get_selection(),
                Selection::range(Pos::from_row_column(0, 1), Pos::from_row_column(1, 3)),
            );
        }
        {
            let test = create_test_app2(84, 36);
            test.paste("asd\nbsd");
            test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
            let mut expected_str = String::with_capacity(20);
            for i in 0..10 {
                test.input(
                    EditorInputEvent::Char(tested_opening_char),
                    InputModifiers::none(),
                );
                expected_str.clear();
                for _ in 0..i + 1 {
                    expected_str.push(tested_opening_char);
                }
                expected_str.push_str("asd\nbsd");
                for _ in 0..i + 1 {
                    expected_str.push(expected_closing_char);
                }

                assert_eq!(expected_str, test.get_editor_content());
                assert_eq!(
                    test.get_selection(),
                    Selection::range(Pos::from_row_column(0, i + 1), Pos::from_row_column(1, 3),),
                );
            }
        }
    }
}

#[test]
fn test_insert_closing_parenthesis_around_multiline_selected_text_backward_selection() {
    for (tested_opening_char, expected_closing_char) in
        &[('(', ')'), ('[', ']'), ('{', '}'), ('\"', '\"')]
    {
        let tested_opening_char = *tested_opening_char;
        let expected_closing_char = *expected_closing_char;
        {
            let test = create_test_app2(84, 36);
            test.paste("asd\nbsd");
            test.input(EditorInputEvent::Home, InputModifiers::shift());
            test.input(EditorInputEvent::Up, InputModifiers::shift());
            test.input(
                EditorInputEvent::Char(tested_opening_char),
                InputModifiers::none(),
            );

            let mut expected_str = String::with_capacity(2);
            expected_str.push(tested_opening_char);
            expected_str.push_str("asd\nbsd");
            expected_str.push(expected_closing_char);
            assert_eq!(expected_str, test.get_editor_content());
            assert_eq!(
                test.get_selection(),
                Selection::range(Pos::from_row_column(1, 3), Pos::from_row_column(0, 1)),
            );
        }
        {
            let test = create_test_app2(84, 36);
            test.paste("asd\nbsd");
            test.input(EditorInputEvent::Home, InputModifiers::shift());
            test.input(EditorInputEvent::Up, InputModifiers::shift());

            let mut expected_str = String::with_capacity(20);
            for i in 0..10 {
                test.input(
                    EditorInputEvent::Char(tested_opening_char),
                    InputModifiers::none(),
                );
                expected_str.clear();
                for _ in 0..i + 1 {
                    expected_str.push(tested_opening_char);
                }
                expected_str.push_str("asd\nbsd");
                for _ in 0..i + 1 {
                    expected_str.push(expected_closing_char);
                }

                assert_eq!(expected_str, test.get_editor_content());
                assert_eq!(
                    test.get_selection(),
                    Selection::range(Pos::from_row_column(1, 3), Pos::from_row_column(0, i + 1),),
                );
            }
        }
    }
}

#[test]
fn test_paren_competion_and_mat_editing_combo() {
    let test = create_test_app2(84, 36);
    test.paste("");
    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    test.input(EditorInputEvent::Char('('), InputModifiers::none());
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    // TODO it should support closing paren insertion "[(1)]"
    assert_eq!(&test.get_editor_content(), "[(1]");
}

#[test]
fn test_paren_removal_bug_when_cursor_eol() {
    let test = create_test_app2(84, 36);
    test.paste("\n\na");
    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    test.input(EditorInputEvent::Del, InputModifiers::none());
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    assert_eq!(&test.get_editor_content(), "\n\na");
}

#[test]
fn test_paren_competion_and_mat_editing_combo2() {
    let test = create_test_app2(84, 36);
    test.paste("");
    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    assert_eq!(&test.get_editor_content(), "");

    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    assert_eq!(&test.get_editor_content(), "1");
}

#[test]
fn test_paren_competion_and_mat_editing_combo3() {
    let test = create_test_app2(84, 36);
    test.paste("");
    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::shift());
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    assert_eq!(&test.get_editor_content(), "[]");
}

#[test]
fn test_paren_competion_and_mat_editing_combo4() {
    let test = create_test_app2(84, 36);
    test.paste("");
    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::shift());
    test.input(EditorInputEvent::Del, InputModifiers::none());
    assert_eq!(&test.get_editor_content(), "[]");
}

#[test]
fn test_single_unit_in_denom_output_format() {
    let test = create_test_app2(84, 36);
    test.paste("50 000 / year");
    test.assert_results(&["50 000 / year"][..]);
}

#[test]
fn test_units_can_be_applied_on_linerefs() {
    let test = create_test_app2(84, 36);
    test.paste("12\n&[1] m");
    test.assert_results(&["12", "12 m"][..]);
}

#[test]
fn test_units_right_after_each_other() {
    {
        let test = create_test_app2(84, 36);
        test.paste("var = 12 byte\nvar kilobyte");
        test.assert_results(&["12 bytes", "Err"][..]);
    }
    {
        let test = create_test_app2(84, 36);
        test.paste("var = 12 byte\nvar ok kilobyte");
        test.assert_results(&["12 bytes", "12 bytes"][..]);
    }
}

#[test]
fn test_paren_highlighting() {
    let test = create_test_app2(84, 36);
    test.paste("asdasd hehe(12)");
    test.input(EditorInputEvent::Left, InputModifiers::none());
    let render_bucket = &test.render_bucket().custom_commands[Layer::Text as usize];
    assert_contains(
        render_bucket,
        2,
        OutputMessage::SetColor(THEMES[0].parenthesis),
    );
    assert_contains(
        render_bucket,
        2,
        OutputMessage::FollowingTextCommandsAreHeaders(true),
    );
    assert_contains(
        render_bucket,
        2,
        OutputMessage::FollowingTextCommandsAreHeaders(false),
    );
    assert_contains(
        render_bucket,
        1,
        OutputMessage::RenderChar(RenderChar {
            col: 11 + test.get_render_data().left_gutter_width,
            row: canvas_y(0),
            char: '(',
        }),
    );

    assert_contains(
        render_bucket,
        1,
        OutputMessage::RenderChar(RenderChar {
            col: 14 + test.get_render_data().left_gutter_width,
            row: canvas_y(0),
            char: ')',
        }),
    );
}

#[test]
fn test_paren_highlighting2() {
    let test = create_test_app2(84, 36);
    test.paste("asdasd hehe((12))");
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    let render_bucket = &test.render_bucket().custom_commands[Layer::Text as usize];
    assert_contains(
        render_bucket,
        1,
        OutputMessage::RenderChar(RenderChar {
            col: 12 + test.get_render_data().left_gutter_width,
            row: canvas_y(0),
            char: '(',
        }),
    );

    assert_contains(
        render_bucket,
        1,
        OutputMessage::RenderChar(RenderChar {
            col: 15 + test.get_render_data().left_gutter_width,
            row: canvas_y(0),
            char: ')',
        }),
    );
}

#[test]
fn test_parenthesis_must_not_rendered_outside_of_editor() {
    let test = create_test_app2(51, 36);
    test.paste("1000 (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)");
    // there is enough space, everything is rendered
    let render_bucket = &test.render_bucket().parenthesis;
    assert_eq!(render_bucket.iter().filter(|it| it.char == '(').count(), 1);
    assert_eq!(render_bucket.iter().filter(|it| it.char == ')').count(), 1);

    // put the cursor right after '1000'
    test.input(EditorInputEvent::Home, InputModifiers::none());
    test.input(EditorInputEvent::Right, InputModifiers::ctrl());

    // the right parenth is on the scrollbar column, no render
    test.input(EditorInputEvent::Char('0'), InputModifiers::none());
    let render_bucket = &test.render_bucket().parenthesis;
    assert_eq!(render_bucket.iter().filter(|it| it.char == '(').count(), 1);
    assert_eq!(render_bucket.iter().filter(|it| it.char == ')').count(), 0);

    // the right parenth is on the right gutter, no render
    test.input(EditorInputEvent::Char('0'), InputModifiers::none());
    let render_bucket = &test.render_bucket().parenthesis;
    assert_eq!(render_bucket.iter().filter(|it| it.char == '(').count(), 1);
    assert_eq!(render_bucket.iter().filter(|it| it.char == ')').count(), 0);

    // the right parenth is on the result panel, no render
    test.input(EditorInputEvent::Char('0'), InputModifiers::none());
    let render_bucket = &test.render_bucket().parenthesis;
    assert_eq!(render_bucket.iter().filter(|it| it.char == '(').count(), 1);
    assert_eq!(render_bucket.iter().filter(|it| it.char == ')').count(), 0);

    // the left parenth is on the right gutter, no render
    for _ in 0..13 {
        test.input(EditorInputEvent::Char('0'), InputModifiers::none());
    }
    let render_bucket = &test.render_bucket().parenthesis;
    assert_eq!(render_bucket.iter().filter(|it| it.char == '(').count(), 0);
    assert_eq!(render_bucket.iter().filter(|it| it.char == ')').count(), 0);

    // the left parenth is on the result panel gutter, no render
    test.input(EditorInputEvent::Char('0'), InputModifiers::none());
    let render_bucket = &test.render_bucket().parenthesis;
    assert_eq!(render_bucket.iter().filter(|it| it.char == '(').count(), 0);
    assert_eq!(render_bucket.iter().filter(|it| it.char == ')').count(), 0);
}

#[test]
fn test_leave_matrix_text_insertion_overflow() {
    let test = create_test_app2(51, 36);
    test.paste(&("0".repeat(MAX_EDITOR_WIDTH - 10)));

    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    for _ in 0..20 {
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    }
    // it commits the matrix editing and tries to write its content back
    // which overflows
    assert!(test.app().matrix_editing.is_some());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert!(test.app().matrix_editing.is_none());
    // first, it should not panic
    assert_eq!(test.get_render_data().get_rendered_height(content_y(0)), 1);
    assert_eq!(test.get_render_data().get_rendered_height(content_y(1)), 0);
}

#[test]
fn test_leave_matrix_text_insertion_overflow2() {
    let test = create_test_app2(51, 36);
    test.paste(&("0".repeat(MAX_EDITOR_WIDTH - 10)));

    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    for _ in 0..20 {
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    }
    assert!(test.app().matrix_editing.is_some());
    // it commits the matrix editing and tries to write its content back
    // which overflows
    test.input(EditorInputEvent::Del, InputModifiers::none());
    assert!(test.app().matrix_editing.is_none());
    // first, it should not panic
    assert_eq!(test.get_render_data().get_rendered_height(content_y(0)), 1);
    assert_eq!(test.get_render_data().get_rendered_height(content_y(1)), 0);
}

#[test]
fn test_leave_matrix_text_insertion_overflow3() {
    let test = create_test_app2(51, 36);
    test.paste(&("0".repeat(MAX_EDITOR_WIDTH - 10)));

    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    for _ in 0..20 {
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Left, InputModifiers::none());
    }
    assert!(test.app().matrix_editing.is_some());
    // it commits the matrix editing and tries to write its content back
    // which overflows
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    assert!(test.app().matrix_editing.is_none());
    // first, it should not panic
    assert_eq!(test.get_render_data().get_rendered_height(content_y(0)), 1);
    assert_eq!(test.get_render_data().get_rendered_height(content_y(1)), 0);
}

#[test]
fn test_leave_matrix_text_insertion_overflow4() {
    let test = create_test_app2(51, 36);
    test.paste(&("0".repeat(MAX_EDITOR_WIDTH - 10)));
    test.input(EditorInputEvent::Home, InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    // inserting a matrix before a long text,
    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    for _ in 0..20 {
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Left, InputModifiers::none());
    }
    // it commits the matrix editing and tries to write its content back
    // which overflows
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    assert!(test.app().matrix_editing.is_none());
    // first, it should not panic
    assert_eq!(test.get_render_data().get_rendered_height(content_y(0)), 1);
    assert_eq!(test.get_render_data().get_rendered_height(content_y(1)), 0);
}

#[test]
fn test_leave_matrix_shorter_than_it_was_with_del() {
    let test = create_test_app2(51, 36);
    test.paste("[1+2+3]");
    test.input(EditorInputEvent::Left, InputModifiers::none());
    assert!(test.app().matrix_editing.is_some());

    test.input(EditorInputEvent::Del, InputModifiers::none());
    test.input(EditorInputEvent::Del, InputModifiers::none());
    assert!(test.app().matrix_editing.is_none());
}

#[test]
fn test_removing_matrix_closing_bracket() {
    let test = create_test_app2(51, 36);
    test.paste("");
    test.input(EditorInputEvent::Char('['), InputModifiers::none());
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    test.input(EditorInputEvent::Char('3'), InputModifiers::none());
    test.input(EditorInputEvent::Del, InputModifiers::none());
    assert_eq!(test.get_editor_content(), "[123");
}

#[test]
fn test_matrix_deletion_from_last_cell() {
    let test = create_test_app2(51, 36);
    test.paste("[1,2,3,4]");
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!(test.get_editor_content(), "[1,2,3,]");
}

#[test]
pub fn test_no_panic_when_matrix_full_height() {
    let test = create_test_app2(73, 40);

    for _ in 0..MAX_LINE_COUNT {
        test.input(EditorInputEvent::Char('['), InputModifiers::none());

        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char(';'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char(';'), InputModifiers::none());
        test.input(EditorInputEvent::Char('3'), InputModifiers::none());
        test.input(EditorInputEvent::Char(';'), InputModifiers::none());
        test.input(EditorInputEvent::Char('4'), InputModifiers::none());
        // commit matrix editing
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        // go to the next line
        test.input(EditorInputEvent::Enter, InputModifiers::none());
    }
}

#[test]
fn test_fuzz_bug_201220_2() {
    let test = create_test_app2(51, 36);
    test.paste("[]8F(*^5+[2)]/)=^]0/");

    // TODO '2' because matrix results are rendered into operators :(
    test.assert_contains_operator(2, |op| op.text == &['[']);
    test.assert_contains_operator(2, |op| op.text == &[']']);
    test.assert_contains_num(1, |op| op.text == &['2']);
    let start_x = 2;
    for (i, ch) in "[]8F(*^5+".chars().enumerate() {
        test.assert_contains_text(1, |op| op.text == &[ch] && op.column == start_x + i);
    }
    test.assert_contains_text(1, |op| op.text == &[')'] && op.column == 13);
    let start_x = 15;
    for (i, ch) in "/)=^]0/".chars().enumerate() {
        test.assert_contains_text(1, |op| op.text == &[ch] && op.column == start_x + i);
    }

    // just to be sure that the result in the prev row is "[2]" (matrices are not rendered into the result buff so cant assert on it)
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.paste("nth(");
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    test.paste(", 0)");
    test.assert_results(&["", "2"]);
}

#[test]
fn test_illegal_unary_minus_is_not_added_to_the_output() {
    let test = create_test_app2(51, 36);
    test.paste("[6*7]*9#8=-+");

    // TODO '2' because matrix results are rendered into operators :(
    test.assert_contains_operator(2, |op| op.text == &['[']);
    test.assert_contains_operator(2, |op| op.text == &[']']);
    test.assert_contains_operator(1, |op| op.text == &['*']);
    test.assert_contains_num(1, |op| op.text == &['6']);
    test.assert_contains_num(1, |op| op.text == &['7']);
    let start_x = 7;
    for (i, ch) in "*9".chars().enumerate() {
        test.assert_contains_text(1, |op| op.text == &[ch] && op.column == start_x + i);
    }

    test.assert_contains_text(1, |op| op.text == &['#', '8'] && op.column == 9);

    let start_x = 11;
    for (i, ch) in "=-+".chars().enumerate() {
        test.assert_contains_text(1, |op| op.text == &[ch] && op.column == start_x + i);
    }
}

#[test]
fn test_longer_texts3_tokens() {
    let test = create_test_app2(100, 36);
    test.paste("I traveled 13km at a rate / 40km/h in min");

    let mut expected_col = 2;
    for (i, str) in (&[
        to_char_slice("I"),
        to_char_slice(" "),
        to_char_slice("traveled"),
        to_char_slice(" "),
        to_char_slice("13"), // NUM
        to_char_slice("km"), // UNIT
        to_char_slice(" "),
        to_char_slice("at"),
        to_char_slice(" "),
        to_char_slice("a"),
        to_char_slice(" "),
        to_char_slice("rate"),
        to_char_slice(" "),
        to_char_slice("/"), // OP
        to_char_slice(" "),
        to_char_slice("40"),   // NUM
        to_char_slice("km/h"), // UNIT
        to_char_slice(" "),
        to_char_slice("in"), // OP
        to_char_slice(" "),
        to_char_slice("min"), // UNIT
    ])
        .iter()
        .enumerate()
    {
        if i == 4 || i == 15 {
            test.assert_contains_num(1, |op| op.text == str && op.column == expected_col);
        } else if i == 5 || i == 16 || i == 20 {
            test.assert_contains_unit(1, |op| op.text == str && op.column == expected_col);
        } else if i == 13 || i == 18 {
            test.assert_contains_operator(1, |op| op.text == str && op.column == expected_col);
        } else {
            dbg!(i);
            dbg!(expected_col);
            dbg!(str);
            test.assert_contains_text(1, |op| op.text == str && op.column == expected_col);
        }
        expected_col += str.len();
    }
}

#[test]
fn test_missing_arg_nth_panic() {
    let test = create_test_app2(100, 36);
    test.paste("nth(,[1])");

    let exp_col = 2;
    test.assert_contains_operator(1, |op| {
        op.text == to_char_slice("nth") && op.column == exp_col
    });
    let exp_col = exp_col + 3;
    test.assert_contains_paren(1, |op| op.char == '(' && op.col == exp_col);
    let exp_col = exp_col + 1;
    test.assert_contains_operator(1, |op| {
        op.text == to_char_slice(",") && op.column == exp_col
    });
    let exp_col = exp_col + 1;
    test.assert_contains_error(1, |op| {
        op.text == to_char_slice("[") && op.column == exp_col
    });
    let exp_col = exp_col + 1;
    test.assert_contains_num(1, |op| {
        op.text == to_char_slice("1") && op.column == exp_col
    });
    let exp_col = exp_col + 1;
    test.assert_contains_error(1, |op| {
        op.text == to_char_slice("]") && op.column == exp_col
    });
    let exp_col = exp_col + 1;
    test.assert_contains_paren(1, |op| op.char == ')' && op.col == exp_col);
}
