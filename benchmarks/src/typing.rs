use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::test_common::test_common::create_test_app2;

pub fn bench_typing_the_tutorial(iteration_count: usize) {
    let text = include_str!("../../examples/tutorial.notecalc");
    let test = create_test_app2(73, 40);

    for _ in 0..iteration_count {
        for lines in text.lines() {
            for ch in lines.chars() {
                test.input(EditorInputEvent::Char(ch), InputModifiers::none());
            }
            test.input(EditorInputEvent::Enter, InputModifiers::none());
        }
        // clear the editor
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Del, InputModifiers::none());
    }
}
