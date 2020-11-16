// I hate Rust
mod common;

use crate::common::create_app2;
use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};

#[test]
fn end_matrix_edit_by_end_key() {
    let test = create_app2(35);
    test.paste("");
    test.autocomplete_matrix();
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    // inside the matrix
    test.input(EditorInputEvent::End, InputModifiers::none());
    test.input(EditorInputEvent::End, InputModifiers::none());
    assert_eq!(test.app().editor.get_selection().get_cursor_pos().column, 9);
}

#[test]
fn end_matrix_edit_by_right_key() {
    let test = create_app2(35);
    test.paste("");
    test.autocomplete_matrix();
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    // inside the matrix
    test.input(EditorInputEvent::End, InputModifiers::none());
    test.input(EditorInputEvent::Right, InputModifiers::none());
    test.input(EditorInputEvent::Right, InputModifiers::none());
    assert_eq!(test.app().editor.get_selection().get_cursor_pos().column, 9);
}

#[test]
fn end_matrix_edit_by_tab_key() {
    let test = create_app2(35);
    test.paste("");
    test.autocomplete_matrix();
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    test.input(EditorInputEvent::Right, InputModifiers::alt());
    // inside the matrix
    test.input(EditorInputEvent::End, InputModifiers::none());
    test.input(EditorInputEvent::Tab, InputModifiers::none());
    assert_eq!(test.app().editor.get_selection().get_cursor_pos().column, 9);
}

#[test]
fn test_that_cursor_is_inside_matrix_on_creation() {
    let test = create_app2(35);
    test.autocomplete_matrix();
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    assert_eq!("[1]", test.get_editor_content());
}

#[test]
fn test_matrix_alt_plus_left() {
    {
        let test = create_app2(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1, 2, 3]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,2]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1, 2, 3]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1, 2, 3; 4,5,6]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,2;4,5]", test.get_editor_content());
    }
}

#[test]
fn test_matrix_alt_plus_down() {
    {
        let test = create_app2(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1;0]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1;0;0;0]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1,2]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Down, InputModifiers::alt());
        // this render is important, it tests a bug!
        test.render();
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,2;0,0]", test.get_editor_content());
    }
}

#[test]
fn test_matrix_alt_plus_up() {
    {
        let test = create_app2(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1; 2; 3]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1;2]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1; 2; 3]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1, 2, 3; 4,5,6]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Up, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,2,3]", test.get_editor_content());
    }
}

#[test]
fn test_matrix_alt_plus_right() {
    {
        let test = create_app2(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,0]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,0,0,0]", test.get_editor_content());
    }
    {
        let test = create_app2(35);
        test.paste("[1;2]");
        test.render();
        test.input(EditorInputEvent::Left, InputModifiers::none());
        test.render();
        test.input(EditorInputEvent::Right, InputModifiers::alt());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        assert_eq!("[1,0;2,0]", test.get_editor_content());
    }
}
