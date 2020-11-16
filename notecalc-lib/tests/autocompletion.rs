mod common;

use crate::common::create_app2;
use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};

#[test]
fn test_matrix_autocompletion() {
    let test = create_app2(35);
    test.paste(".mat");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("[0]", test.get_editor_content());
}

#[test]
fn test_matrix_autocompletion_enables_mat_editing() {
    let test = create_app2(35);
    test.paste(".mat");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!("[2]", test.get_editor_content());
}

#[test]
fn test_matrix_autocompletion2() {
    let test = create_app2(35);
    test.paste("m .mat");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!("m [0]", test.get_editor_content());
}

#[test]
fn test_matrix_autocompletion3() {
    let test = create_app2(35);
    test.paste("_.mat");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!("_[0]", test.get_editor_content());
}

#[test]
fn test_matrix_autocompletion4() {
    let test = create_app2(35);
    test.paste("a.mat");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!("a[0]", test.get_editor_content());
}

#[test]
fn test_matrix_autocompletion5() {
    let test = create_app2(35);
    test.paste("longer string asd .mat");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!("longer string asd [0]", test.get_editor_content());
}

#[test]
fn test_matrix_autocompletion6() {
    let test = create_app2(35);
    test.paste("longer string asd");
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Char('.'), InputModifiers::none());
    test.input(EditorInputEvent::Char('m'), InputModifiers::none());
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    test.input(EditorInputEvent::Char('t'), InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!("longer string [2]asd", test.get_editor_content());
}

#[test]
fn test_mat3_autocompletion() {
    let test = create_app2(35);
    test.paste(".mat3");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!("[0,0,0;0,0,0;0,0,0]", test.get_editor_content());
}

#[test]
fn test_mat4_autocompletion() {
    let test = create_app2(35);
    test.paste(".mat4");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!(
        "[0,0,0,0;0,0,0,0;0,0,0,0;0,0,0,0]",
        test.get_editor_content()
    );
}

#[test]
fn test_mat4_autocompletion2() {
    let test = create_app2(35);
    test.paste(".mat.mat3.mat4");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!(
        ".mat.mat3[0,0,0,0;0,0,0,0;0,0,0,0;0,0,0,0]",
        test.get_editor_content()
    );
}

#[test]
fn test_pow_autocompletion() {
    let test = create_app2(35);
    test.paste(".pow");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("^", test.get_editor_content());
}

#[test]
fn test_pow_autocompletion2() {
    let test = create_app2(35);
    test.paste("2.pow");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("2^", test.get_editor_content());
}

#[test]
fn test_pi_autocompletion() {
    let test = create_app2(35);
    test.paste("2.pi");
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("2π", test.get_editor_content());
}

#[test]
fn test_autocompletion_single() {
    let test = create_app2(35);
    test.paste("apple = 12$");
    test.render();
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("apple = 12$\napple", test.get_editor_content());
}

#[test]
fn test_autocompletion_var_name_with_space() {
    let test = create_app2(35);
    test.paste("some apples = 12$");
    test.render();
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Char('s'), InputModifiers::none());
    test.input(EditorInputEvent::Char('o'), InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("some apples = 12$\nsome apples", test.get_editor_content());
}

#[test]
fn test_autocompletion_var_name_with_space2() {
    let test = create_app2(35);
    test.paste("some apples = 12$");
    test.render();
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Char('s'), InputModifiers::none());
    test.input(EditorInputEvent::Char('o'), InputModifiers::none());
    test.input(EditorInputEvent::Char('m'), InputModifiers::none());
    test.input(EditorInputEvent::Char('e'), InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("some apples = 12$\nsome apples", test.get_editor_content());
}

#[test]
fn test_autocompletion_var_name_with_space3() {
    let test = create_app2(35);
    test.paste("men BMR = 12");
    test.render();
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Char('m'), InputModifiers::none());
    test.input(EditorInputEvent::Char('e'), InputModifiers::none());
    test.input(EditorInputEvent::Char('n'), InputModifiers::none());
    test.input(EditorInputEvent::Char(' '), InputModifiers::none());
    test.input(EditorInputEvent::Left, InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("men BMR = 12\nmen BMR ", test.get_editor_content());
}

#[test]
fn test_autocompletion_only_above_vars() {
    let test = create_app2(35);
    test.paste("apple = 12$");
    test.render();
    test.input(EditorInputEvent::Home, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("a   \napple = 12$", test.get_editor_content());
}

#[test]
fn test_autocompletion_two_vars() {
    let test = create_app2(35);
    test.paste("apple = 12$\nbanana = 7$\n");
    test.render();
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("apple = 12$\nbanana = 7$\napple", test.get_editor_content());

    test.input(EditorInputEvent::Char(' '), InputModifiers::none());
    test.input(EditorInputEvent::Char('b'), InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!(
        "apple = 12$\nbanana = 7$\napple banana",
        test.get_editor_content()
    );
}

#[test]
fn test_that_no_autocompletion_for_multiple_results() {
    let test = create_app2(35);
    test.paste("apple = 12$\nananas = 7$\n");
    test.render();
    test.input(EditorInputEvent::Char('a'), InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!("apple = 12$\nananas = 7$\na   ", test.get_editor_content());
}
