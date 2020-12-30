use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::test_common::test_common::{create_test_app2, TestHelper};
use notecalc_lib::MAX_LINE_COUNT;

pub fn bench_each_line_references_prev_line(iteration_count: usize) {
    for _ in 0..iteration_count {
        let test = create_test_app2(73, 40);
        for _ in 0..MAX_LINE_COUNT - 1 {
            test.input(EditorInputEvent::Char('+'), InputModifiers::none());
            test.input(EditorInputEvent::Char('1'), InputModifiers::none());
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            test.input(EditorInputEvent::Up, InputModifiers::alt());
            test.alt_key_released();
        }
    }
}

pub fn bench_each_line_references_first_line(iteration_count: usize) {
    let test = create_test_app2(73, 40);
    for _ in 0..iteration_count {
        test.mut_app().reset();
        fill_each_line_references_first_line(&test);
    }
}

pub fn bench_each_line_references_first_line_then_modify_first_line(iteration_count: usize) {
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
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    test.input(EditorInputEvent::Char('3'), InputModifiers::none());
    test.input(EditorInputEvent::Char('4'), InputModifiers::none());
    test.input(EditorInputEvent::Char('5'), InputModifiers::none());
    test.input(EditorInputEvent::Char('6'), InputModifiers::none());
    test.input(EditorInputEvent::Char('7'), InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
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
    for _ in 0..MAX_LINE_COUNT - 2 {
        test.input(EditorInputEvent::Char('d'), InputModifiers::ctrl());
    }
}
