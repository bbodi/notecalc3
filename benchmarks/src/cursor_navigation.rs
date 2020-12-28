use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::test_common::test_common::create_test_app2;
use notecalc_lib::MAX_LINE_COUNT;

pub fn bench_cursor_navigation(iteration_count: usize) {
    let test = create_test_app2(73, 40);

    test.repeated_paste(
        "(0.03^12 / 0.5)^-1 * (1M*10e-10)^6 * [1,2,3,4;5,6,7,8;1,2,3,4;5,6,7,8]",
        MAX_LINE_COUNT - 1,
    );
    for _ in 0..iteration_count {
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Down, InputModifiers::none());
    }
}
