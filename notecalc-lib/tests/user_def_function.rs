use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::helper::canvas_y;
use notecalc_lib::test_common::test_common::create_test_app;
use notecalc_lib::{Layer, OutputMessage, ACTIVE_LINE_REF_HIGHLIGHT_COLORS, MAX_LINE_COUNT};

#[test]
fn test_simple_user_func() {
    let test = create_test_app(35);
    test.paste(
        "my_func(a):\n
  12",
    );
    test.assert_results(&["", "12"][..]);
}

#[test]
fn test_simple_user_func_2_params() {
    let test = create_test_app(35);
    test.paste(
        "my_func(a, b):\n
  12",
    );
    test.assert_results(&["", "12"][..]);
}

#[test]
fn test_simple_user_func_3_params() {
    let test = create_test_app(35);
    test.paste(
        "my_func(a, b, c):\n
  12",
    );
    test.assert_results(&["", "12"][..]);
}

#[test]
fn test_function_invocation() {
    let test = create_test_app(35);
    test.paste(
        "my_func():
  12

my_func()",
    );
    test.assert_results(&["", "12", "", "12"][..]);
}

#[test]
fn test_empty_rows_are_not_part_of_fn() {
    let test = create_test_app(35);
    test.paste(
        "my_func(a):
  12 * a

my_func(2)",
    );
    test.assert_results(&["", "", "", "24"][..]);
}

#[test]
fn test_function_invocation_with_arg() {
    let test = create_test_app(35);
    test.paste(
        "my_func(a):
  12 * a
my_func(2)",
    );
    test.assert_results(&["", "", "", "24"][..]);
}

#[test]
fn test_user_fn_can_see_global_vars_defined_above() {
    let test = create_test_app(35);
    test.paste(
        "
outer_var = 12
my_func(a):
  2*a+outer_var

my_func(3)",
    );
    test.assert_results(&["", "12", "", "", "", "18"][..]);
}

#[test]
fn test_parameter_order() {
    let test = create_test_app(35);
    test.paste(
        "my_func(a, b):
  b
my_func(2, 3)",
    );
    test.assert_results(&["", "", "3"][..]);
}

#[test]
fn test_parameter_order2() {
    let test = create_test_app(35);
    test.paste(
        "my_func(a, b, c):
  c
my_func(2, 3, 4)",
    );
    test.assert_results(&["", "", "4"][..]);
}

#[test]
fn test_function_defs_are_cleared_on_modif() {
    let test = create_test_app(35);
    test.paste(
        "func1(a, b):
  2 * a + b

func2(a, b):
  2 * a + b

func1(2, 3)
func2(4, 5)",
    );
    // remove the second function
    for _ in 0..4 {
        test.input(EditorInputEvent::Up, InputModifiers::none());
    }
    test.input(EditorInputEvent::Home, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::shift());
    test.input(EditorInputEvent::End, InputModifiers::shift());
    test.input(EditorInputEvent::Del, InputModifiers::none());

    // remove the empty lines
    test.input(EditorInputEvent::Del, InputModifiers::none());
    // if one more line is deleted, the 'fun1' function call is at line 4,
    // exactly where the func2 definition was.
    // because of a bug, it was still interpreted as fn call, which called line 4
    // indefinitely.
    test.input(EditorInputEvent::Del, InputModifiers::none());

    test.assert_results(&["", "", "", "7", "5"][..]);
}

#[test]
fn test_changing_function_body_triggers_recalc_on_callsite() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
  a*2
func1(2)",
    );
    test.assert_results(&["", "", "4"][..]);

    // change "a*2" to "a*3"
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    test.input(EditorInputEvent::Char('3'), InputModifiers::none());

    test.assert_results(&["", "", "6"][..]);
}

#[test]
fn test_errors_during_func_evaluation_are_not_highlighted_inside_the_func() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
  a+2
func1(1m)",
    );
    test.assert_results(&["", "", "Err"][..]);

    // TODO
    // currently it does not bother me that much, it can be even useful
    //assert!(test.render_bucket().number_errors.is_empty());
}

#[test]
fn test_update_function_body_range_if_it_changes() {
    let test = create_test_app(35);
    test.paste(
        "func2(a):
  1
  3
func2(12)",
    );
    test.assert_results(&["", "1", "3", "3"][..]);

    // remove the line "  3"
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Home, InputModifiers::shift());
    test.input(EditorInputEvent::Del, InputModifiers::none());

    test.assert_results(&["", "1", "", "1"][..]);
}

#[test]
fn test_update_function_body_range_if_it_changes_2() {
    let test = create_test_app(35);
    test.paste(
        "func2(a):
  1
  2
  3
func2(12)",
    );
    test.assert_results(&["", "1", "2", "3", "3"][..]);

    // move the "2" to the beginning of the line
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Home, InputModifiers::shift());
    test.input(EditorInputEvent::Del, InputModifiers::none());

    test.assert_results(&["", "1", "2", "3", "1"][..]);
}

#[test]
fn test_update_function_body_range_if_it_changes_3() {
    // here the "3" should be part of the function again because
    // "2" connects it again to the definition

    let test = create_test_app(35);
    test.paste(
        "func2(a):
  1
2
  3
func2(12)",
    );
    test.assert_results(&["", "1", "2", "3", "1"][..]);

    // put a space in front of "2", making it part of the func body
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());

    test.assert_results(&["", "1", "2", "3", "3"][..]);
}

#[test]
fn test_update_function_body_range_if_it_changes_4() {
    let test = create_test_app(35);
    test.paste(
        "func2(a):
  1
  2*a
  3
func2(12)",
    );
    test.assert_results(&["", "1", "", "3", "3"][..]);

    // move the "2*a" to the beginning of the line
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Home, InputModifiers::shift());
    test.input(EditorInputEvent::Del, InputModifiers::none());

    test.assert_results(&["", "1", "2", "3", "1"][..]);
}

#[test]
fn test_update_function_body_range_if_it_changes_5() {
    let test = create_test_app(35);
    test.paste(
        "func2():
  1
func2()
2",
    );
    test.assert_results(&["", "1", "1", "2"][..]);

    // press space in front of "2"
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());
    test.assert_results(&["", "1", "1", "2"][..]);
    assert_eq!(
        test.bcf.func_defs()[0]
            .as_ref()
            .unwrap()
            .first_row_index
            .as_usize(),
        0
    );
    assert_eq!(
        test.bcf.func_defs()[0]
            .as_ref()
            .unwrap()
            .last_row_index
            .as_usize(),
        1
    );
}

#[test]
fn test_func_name_reuse() {
    let test = create_test_app(35);
    test.paste(
        "func2():
  2
func2():
  3
func2()",
    );
    test.assert_results(&["", "2", "", "3", "3"][..]);
}

#[test]
fn test_global_varuiables_same_name_as_param() {
    let test = create_test_app(35);
    test.paste(
        "a = 2
func1(a):
 3*a
func1(4)
a",
    );
    test.assert_results(&["2", "", "", "12", "2"][..]);
}

#[test]
fn test_create_local_var() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a = 5
 a*2
func1(4)
",
    );
    test.assert_results(&["", "5", "10", "10"][..]);
}

#[test]
fn test_create_local_var_2() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 b = 5
 b*2
func1(4)
",
    );
    test.assert_results(&["", "5", "10", "10"][..]);
}

#[test]
fn test_create_local_var_3() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 b = 5
 b*a
func1(4)
",
    );
    test.assert_results(&["", "5", "", "20"][..]);
}

#[test]
fn test_local_var_can_override_param() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a = 5
 a*2
func1(4)",
    );
    test.assert_results(&["", "5", "10", "10"][..]);
}

#[test]
fn test_local_var_can_override_param_2() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a = a + 5
 a*2
func1(4)",
    );
    test.assert_results(&["", "", "", "18"][..]);
}

#[test]
fn test_local_var_can_override_param_3() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a = a + 5
 ab*2
func1(4)",
    );
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    test.assert_results(&["", "", "", "18"][..]);
}

#[test]
fn test_local_var_can_override_param_4() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 b = a + 5
 c = b + 5
 d = c + 5
 e = d + 5
 f = e + 5
 fq*2
func1(4)",
    );
    test.assert_results(&["", "", "", "", "", "", "2", "2"][..]);
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Backspace, InputModifiers::none());
    test.assert_results(&["", "", "", "", "", "", "", "58"][..]);
}

#[test]
fn test_local_var_does_not_affect_global() {
    let test = create_test_app(35);
    test.paste(
        "b = 2
func1():
 b = 5
 1
func1()
b",
    );
    test.assert_results(&["2", "", "5", "1", "1", "2"][..]);
}

#[test]
fn test_local_var_does_not_other_local_var_from_other_func() {
    let test = create_test_app(35);
    test.paste(
        "func1():
 b = 5
 1
func2():
 b
func2()",
    );
    test.assert_results(&["", "5", "1", "", "", "Err"][..]);
}

#[test]
fn test_local_var_does_not_affect_global_2() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a = 5
 a*2
func1(4)
a",
    );
    test.assert_results(&["", "5", "10", "10", ""][..]);
}

#[test]
fn test_funcs_usual() {
    let test = create_test_app(35);
    test.paste(
        "kamatos_kamat(alap, kamat, year):
  alap*((1+kamat)^year)

kamatos_kamat(100, 0.1, 4)",
    );

    test.assert_results(&["", "", "", "146.41"][..]);
}

#[test]
fn test_funcs_usual_2() {
    let test = create_test_app(35);
    test.paste(
        "kamatos_kamat(alap, kamat, évek):
   alap*((1+kamat*1)^(évek/years))
kamatos_kamat(100, 10%, 4years)
kamatos_kamat(10M, 10%, 10years)",
    );

    test.assert_results(&["", "", "146.41", "25 937 424.601"][..]);
}

#[test]
fn test_function_with_empty_body() {
    let test = create_test_app(35);
    test.paste(
        "filler
func():
1",
    );
    assert!(test.bcf.func_defs()[1].is_some());
    assert_eq!(
        test.bcf.func_defs()[1]
            .as_ref()
            .unwrap()
            .first_row_index
            .as_usize(),
        1
    );
    assert_eq!(
        test.bcf.func_defs()[1]
            .as_ref()
            .unwrap()
            .last_row_index
            .as_usize(),
        1
    );
}

#[test]
fn test_calling_function_with_empty_body() {
    let test = create_test_app(35);
    test.paste(
        "filler
func():
1
func()",
    );
    test.assert_results(&["", "", "1", "Err"]);
}

#[test]
fn test_removing_func_removes_its_fd() {
    let test = create_test_app(35);
    test.paste(
        "filler
func():
1
func()",
    );
    assert!(test.bcf.func_defs()[1].is_some());
    test.input(EditorInputEvent::PageUp, InputModifiers::shift());
    test.input(EditorInputEvent::Del, InputModifiers::none());
    assert!(test.bcf.func_defs()[1].is_none());
}

#[test]
fn test_calling_function_defined_afterwards_due_to_deletion_beforehand() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a = 5
 a*2
func1(4)
a

func2(a):
 a = 5
 a*2
func2(4)
a",
    );
    for _ in 0..6 {
        test.input(EditorInputEvent::Up, InputModifiers::none());
    }
    test.input(EditorInputEvent::PageUp, InputModifiers::shift());

    test.input(EditorInputEvent::Del, InputModifiers::none());

    test.assert_results(&["", "", "", "5", "10", "10"][..]);
}

#[test]
fn test_parameter_highlighting() {
    let test = create_test_app(35);
    test.paste(
        "func(a):
a
func(1)",
    );
    test.input(EditorInputEvent::PageUp, InputModifiers::none());

    // no highlighting
    for expected_color in ACTIVE_LINE_REF_HIGHLIGHT_COLORS.iter().skip(2) {
        test.assert_contains_custom_command(Layer::Text, 0, |cmd| match cmd {
            OutputMessage::SetColor(color) => *color == *expected_color,
            _ => false,
        });
        test.assert_contains_custom_command(Layer::BehindTextCursor, 0, |cmd| match cmd {
            OutputMessage::SetColor(color) => *color == *expected_color,
            _ => false,
        });
    }
}

#[test]
fn test_prevent_recursion() {
    let test = create_test_app(35);
    test.paste(
        "b = 2
func1():
 b = 5
 1
func1()
b",
    );
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());

    test.assert_results(&["2", "", "5", "1", "Err", "2"][..]);
}

#[test]
fn test_that_successful_func_invocation() {
    let test = create_test_app(35);
    test.paste(
        "func(a, b, c):
  1
func(10M, 10%, 3years)",
    );
}

#[test]
fn test_fn_body_cannot_be_referenced_from_outside() {
    let test = create_test_app(35);
    test.paste(
        "func1():
 1
",
    );
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();

    test.assert_results(&["", "1", ""][..]);
    assert_eq!(
        &test.get_editor_content(),
        "func1():
 1
"
    )
}

#[test]
fn test_fn_body_cannot_be_referenced_from_other_func() {
    let test = create_test_app(35);
    test.paste(
        "func1():
 1
func2():
 1
",
    );
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();

    test.assert_results(&["", "1", "", "1", ""][..]);
    assert_eq!(
        &test.get_editor_content(),
        "func1():
 1
func2():
 1
"
    )
}

#[test]
fn test_fn_body_cannot_be_referenced_from_other_func_same_name() {
    let test = create_test_app(35);
    test.paste(
        "func1():
 1
func1():
 1
",
    );
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();

    test.assert_results(&["", "1", "", "1", ""][..]);
    assert_eq!(
        &test.get_editor_content(),
        "func1():
 1
func1():
 1
"
    )
}

#[test]
fn test_fn_body_can_reference_from_outside() {
    let test = create_test_app(35);
    test.paste(
        "2
func1():
 1+
func1()",
    );
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();

    test.assert_results(&["2", "", "3", "3"][..]);
    assert_eq!(
        &test.get_editor_content(),
        "2
func1():
 1+&[1]
func1()"
    )
}

#[test]
fn test_fn_body_has_its_own_sum() {
    let test = create_test_app(35);
    test.paste(
        "2
func1():
 3
 sum
func1()",
    );

    // TODO: sum here can be rendered as it does not depend on the parameters
    // but would make the code uglier. Let's wait with this feature
    test.assert_results(&["2", "", "3", "", "3"][..]);
}

#[test]
fn test_fn_body_does_not_affect_global_sum() {
    let test = create_test_app(35);
    test.paste(
        "2
func1():
 3
sum",
    );

    test.assert_results(&["2", "", "3", "2"][..]);
}

#[test]
fn test_calling_func_from_other_func() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a + 5
func2(a):
 func1(a) + 5
func3(a):
 func2(a) + 5
func4(a):
 func3(a) + 5
func5(a):
 func4(a) + 5
func5(5)",
    );
    test.assert_results(&["", "", "", "", "", "", "", "", "", "", "30"][..]);
}

#[test]
fn test_calling_func_from_other_func_change_tracking() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a + 5
func2(a):
 func1(a) + 5
func3(a):
 func2(a) + 5
func4(a):
 func3(a) + 5
func5(a):
 func4(a) + 5
func5(5)",
    );
    test.assert_results(&["", "", "", "", "", "", "", "", "", "", "30"][..]);

    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::End, InputModifiers::none());
    // change "a + 5" to "a + 50"
    test.input(EditorInputEvent::Char('0'), InputModifiers::none());

    test.assert_results(&["", "", "", "", "", "", "", "", "", "", "75"][..]);
}

#[test]
fn test_calling_func_from_other_func_change_tracking2() {
    // it is an important test, the diff between this and the prev is that
    // here we used 'b' as variable names for other rows.
    // So this to work, the parser has to set the variable names on the first line
    // (it was not set, but was set in the last line, so when the 2nd line was updated,
    // it worked since it found the variable, but with these different var names,
    // the second line is a simple string
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a + 5
func2(b):
 func1(b) + 5
func3(b):
 func2(b) + 5
func4(b):
 func3(b) + 5
func5(b):
 func4(b) + 5
func5(5)",
    );
    test.assert_results(&["", "", "", "", "", "", "", "", "", "", "30"][..]);

    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::End, InputModifiers::none());
    // change "a + 5" to "a + 50"
    test.input(EditorInputEvent::Char('0'), InputModifiers::none());

    test.assert_results(&["", "", "", "", "", "", "", "", "", "", "75"][..]);
}

#[test]
fn test_creating_fucntion_by_modifying_an_already_existing_one() {
    let test = create_test_app(35);
    test.paste(
        "func1(a):
 a + 50
 func2(b):
 12*b

func2(2)",
    );
    test.assert_results(&["", "", "", "12", "", "2"][..]);

    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Home, InputModifiers::none());
    // change "a + 5" to "a + 50"
    test.input(EditorInputEvent::Del, InputModifiers::none());

    test.assert_results(&["", "", "", "", "", "24"][..]);

    test.input(EditorInputEvent::Char(' '), InputModifiers::none());
    test.assert_results(&["", "", "", "12", "", "2"][..]);
}

#[test]
fn test_spaces_are_allowed_in_param_names() {
    let test = create_test_app(35);
    test.paste(
        "func1(param name):
 param name + 50
func1(2)",
    );
    test.assert_results(&["", "", "52"][..]);
}

#[test]
fn test_spaces_are_allowed_in_param_names2() {
    let test = create_test_app(35);
    test.paste(
        "func1(param name, param name2):
 param name * param name2
func1(2, 3)",
    );
    test.assert_results(&["", "", "6"][..]);
}

#[test]
fn test_func_defs_are_removed_when_ctrl_v() {
    let test = create_test_app(35);
    test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
    test.input(EditorInputEvent::Del, InputModifiers::none());

    test.paste(
        "func1(a):
 filler
 filler
 filler
 filler
 filler
 filler
 a + 50

func2(a):
 12",
    );

    assert!(test.bcf.func_defs()[0].is_some());
    assert!(test.bcf.func_defs()[9].is_some());

    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::End, InputModifiers::none());
    test.input(EditorInputEvent::PageUp, InputModifiers::shift());

    test.paste("2");

    assert!(test.bcf.func_defs()[0].is_none());
    assert!(test.bcf.func_defs()[1].is_none());
    assert!(test.bcf.func_defs()[2].is_some());
    for i in 3..MAX_LINE_COUNT {
        assert!(test.bcf.func_defs()[i].is_none());
    }
}

#[test]
fn test_fn_bg_is_drawed_at_the_correct_position() {
    let test = create_test_app(35);
    test.paste(
        "filler
filler
func2():
  1",
    );
    test.assert_contains_custom_command(Layer::BehindTextBehindCursor, 1, |cmd| match cmd {
        OutputMessage::RenderRectangle { x: 2, h: 2, y, .. } if *y == canvas_y(2) => true,
        _ => false,
    });
}

#[test]
fn test_fn_bg_is_drawed_at_the_correct_position_with_matrix_front_of_it() {
    let test = create_test_app(35);
    test.paste(
        "[1;2;3;4]
func2():
  1",
    );
    test.assert_contains_custom_command(Layer::BehindTextBehindCursor, 1, |cmd| match cmd {
        OutputMessage::RenderRectangle { x: 2, h: 2, y, .. } if *y == canvas_y(6) => true,
        _ => false,
    });
}

#[test]
fn test_fn_bg_is_drawed_at_the_correct_position_with_matrix_inside() {
    let test = create_test_app(35);
    test.paste(
        "func2():
  [1;2;3;4]
  1",
    );
    test.assert_contains_custom_command(Layer::BehindTextBehindCursor, 1, |cmd| match cmd {
        OutputMessage::RenderRectangle { x: 2, h: 8, y, .. } if *y == canvas_y(0) => true,
        _ => false,
    });
}

#[test]
fn test_fn_bg_is_drawed_at_the_correct_position_with_matrix_2() {
    let test = create_test_app(35);
    test.paste(
        "[1;2;3;4]
func2():
  [1;2;3;4]
  1",
    );
    test.assert_contains_custom_command(Layer::BehindTextBehindCursor, 1, |cmd| match cmd {
        OutputMessage::RenderRectangle { x: 2, h: 8, y, .. } if *y == canvas_y(6) => true,
        _ => false,
    });
}
