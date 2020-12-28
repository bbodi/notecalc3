use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::test_common::test_common::create_test_app2;
use notecalc_lib::MAX_LINE_COUNT;

pub fn bench_copypaste_math_expression(iteration_count: usize) {
    let test = create_test_app2(73, 40);

    let text = "(0.03^12 / 0.5)^-1 * (1M*10e-10)^6 * [1,2,3,4;5,6,7,8;1,2,3,4;5,6,7,8]"
        .repeat(MAX_LINE_COUNT - 1);
    for _ in 0..iteration_count {
        // paste
        test.paste(&text);
        // select all
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        // cut
        test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());
    }
}

pub fn bench_copypaste_tutorial(iteration_count: usize) {
    let text = include_str!("../../examples/tutorial.notecalc");
    let test = create_test_app2(73, 40);

    for _ in 0..iteration_count {
        // paste
        test.paste(&text);
        // select all
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        // cut
        test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());
    }
    println!("{}", test.get_editor_content());
}
