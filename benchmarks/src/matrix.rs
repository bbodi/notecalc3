use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::test_common::test_common::create_test_app2;
use notecalc_lib::MAX_LINE_COUNT;

pub fn bench_insert_matrix(iteration_count: usize) {
    let test = create_test_app2(73, 40);
    for _ in 0..iteration_count {
        test.mut_app().reset();
        for _ in 0..MAX_LINE_COUNT {
            test.input(EditorInputEvent::Char('['), InputModifiers::none());

            for ch in "1,2,3,4;5,6,7,8;1,2,3,4;5,6,7,8".chars() {
                test.input(EditorInputEvent::Char(ch), InputModifiers::none());
            }
            // commit matrix editing
            test.input(EditorInputEvent::Enter, InputModifiers::none());
            // go to the next line
            test.input(EditorInputEvent::Enter, InputModifiers::none());
        }
    }
}
