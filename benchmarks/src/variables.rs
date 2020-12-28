use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::test_common::test_common::{create_test_app2, TestHelper};
use notecalc_lib::MAX_LINE_COUNT;

// each line uses the variable from the previous line
pub fn bench_line_uses_var_from_prev_line(iteration_count: usize) {
    let test = create_test_app2(73, 40);

    for _ in 0..iteration_count {
        fill_each_line_references_first_line(&test);
        // clear the editor
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Del, InputModifiers::none());
    }
}

pub fn bench_line_uses_var_from_prev_line_then_modify_first_line(iteration_count: usize) {
    let test = create_test_app2(73, 40);

    // init
    fill_each_line_references_first_line(&test);

    // go to end of 1st line
    test.input(EditorInputEvent::PageUp, InputModifiers::none());
    test.input(EditorInputEvent::End, InputModifiers::none());
    for _ in 0..iteration_count {
        test.input(EditorInputEvent::Char('8'), InputModifiers::none());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
    }
}

fn fill_each_line_references_first_line(test: &TestHelper) {
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());
    test.input(EditorInputEvent::Char('='), InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());
    test.input(EditorInputEvent::Char('+'), InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    test.input(EditorInputEvent::Char('3'), InputModifiers::none());
    test.input(EditorInputEvent::Char('4'), InputModifiers::none());
    test.input(EditorInputEvent::Char('5'), InputModifiers::none());
    test.input(EditorInputEvent::Char('6'), InputModifiers::none());
    test.input(EditorInputEvent::Char('7'), InputModifiers::none());
    for _ in 0..MAX_LINE_COUNT - 1 {
        test.input(EditorInputEvent::Char('d'), InputModifiers::ctrl());
    }
}
