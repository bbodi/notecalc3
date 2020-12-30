use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::test_common::test_common::create_test_app2;
use notecalc_lib::MAX_LINE_COUNT;

pub fn bench_select_all_mathy_text(iteration_count: usize) {
    let test = create_test_app2(73, 40);

    test.repeated_paste(
        "(0.03^12 / 0.5)^-1 * (1M*10e-10)^6 * [1,2,3,4;5,6,7,8;1,2,3,4;5,6,7,8]",
        MAX_LINE_COUNT - 1,
    );
    for _ in 0..iteration_count {
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Up, InputModifiers::none());
    }
}

pub fn bench_select_all_simple_text(iteration_count: usize) {
    let test = create_test_app2(73, 40);

    test.repeated_paste(
        "There is no one who loves pain itself, who seeks after it and wants to have it, simply because it is pain...",
        MAX_LINE_COUNT - 1,
    );
    for _ in 0..iteration_count {
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Up, InputModifiers::none());
    }
}
