mod common;

use crate::common::create_app2;
use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers, Pos, Selection};

#[test]
fn test_line_ref_normalization() {
    let test = create_app2(35);
    test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n");
    test.set_cursor_row_col(12, 2);
    test.render();
    // remove a line
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

    // Move to end
    test.input(EditorInputEvent::PageDown, InputModifiers::none());
    test.input(EditorInputEvent::End, InputModifiers::none());
    // ALT
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    assert_eq!(
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n12\n13\n&[13] &[13] &[13]",
        &test.get_editor_content()
    );
    assert_eq!(
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n12\n13\n&[12] &[12] &[12]\n",
        &test.app().get_line_ref_normalized_content()
    );
}

#[test]
fn test_line_ref_normalization2() {
    let test = create_app2(35);
    test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n");
    test.set_cursor_row_col(4, 0);

    // mess up the line_id-s
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Char('3'), InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

    // Move to end
    test.input(EditorInputEvent::PageDown, InputModifiers::none());
    // ALT
    for _ in 0..8 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..11 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..4 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();

    assert_eq!(
        "1\n2\n3\n4\n\n\n3\n7\n8\n9\n10\n11\n12\n13\n&[17] &[4] &[10]",
        &test.get_editor_content()
    );
    assert_eq!(
        "1\n2\n3\n4\n\n\n3\n7\n8\n9\n10\n11\n12\n13\n&[7] &[4] &[11]\n",
        &test.app().get_line_ref_normalized_content()
    );
}

// asd
#[test]
fn test_inplace_line_ref_normalization() {
    let test = create_app2(35);
    test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n");
    test.set_cursor_row_col(12, 2);
    test.render();
    // remove a line
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

    // Move to end
    test.input(EditorInputEvent::PageDown, InputModifiers::none());
    test.input(EditorInputEvent::End, InputModifiers::none());
    // ALT
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    test.input(EditorInputEvent::Up, InputModifiers::alt());
    test.alt_key_released();
    assert_eq!(
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n12\n13\n&[13] &[13] &[13]",
        &test.get_editor_content()
    );
    test.mut_app().normalize_line_refs_in_place();
    assert_eq!(
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n12\n13\n&[12] &[12] &[12]",
        &test.get_editor_content()
    );
    for i in 0..test.app().editor_content.line_count() {
        assert_eq!(test.app().editor_content.get_data(i).line_id, i + 1);
    }
    assert_eq!(
        test.app().line_id_generator,
        test.app().editor_content.line_count() + 1
    );
}

#[test]
fn test_inplace_line_ref_normalization2() {
    let test = create_app2(35);
    test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n");
    test.set_cursor_row_col(4, 0);

    // mess up the line_id-s
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Char('3'), InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

    // Move to end
    test.input(EditorInputEvent::PageDown, InputModifiers::none());
    // ALT
    for _ in 0..8 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..11 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..4 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();

    assert_eq!(
        "1\n2\n3\n4\n\n\n3\n7\n8\n9\n10\n11\n12\n13\n&[17] &[4] &[10]",
        &test.get_editor_content()
    );
    test.mut_app().normalize_line_refs_in_place();
    assert_eq!(
        &test.get_editor_content(),
        "1\n2\n3\n4\n\n\n3\n7\n8\n9\n10\n11\n12\n13\n&[7] &[4] &[11]"
    );
    for i in 0..test.app().editor_content.line_count() {
        assert_eq!(test.app().editor_content.get_data(i).line_id, i + 1);
    }
    assert_eq!(
        test.app().line_id_generator,
        test.app().editor_content.line_count() + 1
    );
}

#[test]
fn test_that_inplace_normalization_happens_on_select_all() {
    let test = create_app2(35);
    test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n");
    test.set_cursor_row_col(4, 0);

    // mess up the line_id-s
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Char('3'), InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

    // Move to end
    test.input(EditorInputEvent::PageDown, InputModifiers::none());
    // ALT
    for _ in 0..8 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..11 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..4 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();

    test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());

    let editor_content_str = test.get_editor_content();
    assert_eq!(
        &editor_content_str,
        "1\n2\n3\n4\n\n\n3\n7\n8\n9\n10\n11\n12\n13\n&[7] &[4] &[11]"
    );
    for i in 0..test.app().editor_content.line_count() {
        assert_eq!(test.app().editor_content.get_data(i).line_id, i + 1);
    }
    assert_eq!(
        test.app().line_id_generator,
        test.app().editor_content.line_count() + 1
    );
    // selection is kept
    assert_eq!(
        test.app().editor.get_selection(),
        Selection::range(
            Pos::from_row_column(0, 0),
            Pos::from_row_column(
                test.app().editor_content.line_count() - 1,
                test.app()
                    .editor_content
                    .line_len(test.app().editor_content.line_count() - 1)
            )
        )
    );
}

#[test]
fn test_that_inplace_normalization_happens_on_any_kind_of_select() {
    let test = create_app2(35);
    test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n");
    test.set_cursor_row_col(4, 0);

    // mess up the line_id-s
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Char('3'), InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

    // Move to end
    test.input(EditorInputEvent::PageDown, InputModifiers::none());
    // ALT
    for _ in 0..8 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..11 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..4 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();

    test.input(EditorInputEvent::Left, InputModifiers::shift());
    assert_eq!(
        &test.get_editor_content(),
        "1\n2\n3\n4\n\n\n3\n7\n8\n9\n10\n11\n12\n13\n&[7] &[4] &[11]"
    );
    for i in 0..test.app().editor_content.line_count() {
        assert_eq!(test.app().editor_content.get_data(i).line_id, i + 1);
    }
    assert_eq!(
        test.app().line_id_generator,
        test.app().editor_content.line_count() + 1
    );
    // selection is kept
    assert_eq!(
        test.app().editor.get_selection(),
        Selection::range(Pos::from_row_column(14, 15), Pos::from_row_column(14, 14),)
    );
}

#[test]
fn test_that_editor_objects_are_reacreated() {
    let test = create_app2(35);
    test.paste("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n");
    test.set_cursor_row_col(4, 0);

    // mess up the line_id-s
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Enter, InputModifiers::none());
    test.input(EditorInputEvent::Up, InputModifiers::none());
    test.input(EditorInputEvent::Char('3'), InputModifiers::none());
    test.input(EditorInputEvent::Down, InputModifiers::none());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());
    test.input(EditorInputEvent::Char('x'), InputModifiers::ctrl());

    // Move to end
    test.input(EditorInputEvent::PageDown, InputModifiers::none());
    // ALT
    for _ in 0..8 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..11 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();
    for _ in 0..4 {
        test.input(EditorInputEvent::Up, InputModifiers::alt());
    }
    test.alt_key_released();

    // trigger normalizing
    test.input(EditorInputEvent::Left, InputModifiers::shift());

    // end selection
    test.input(EditorInputEvent::Left, InputModifiers::none());

    assert_eq!(
        &test.mut_vars()[4].as_ref().unwrap().name[..],
        &['&', '[', '5', ']'][..]
    );
    assert_eq!(
        &test.mut_vars()[7].as_ref().unwrap().name[..],
        &['&', '[', '8', ']'][..]
    );
    assert_eq!(
        &test.mut_vars()[11].as_ref().unwrap().name[..],
        &['&', '[', '1', '2', ']'][..]
    );
}

#[test]
fn test_line_ref_denormalization() {
    let test = create_app2(35);
    test.set_normalized_content("1111\n2222\n14 * &[2] &[2] &[2]\n");
    let content = &test.app().editor_content;
    assert_eq!(1, content.get_data(0).line_id);
    assert_eq!(2, content.get_data(1).line_id);
    assert_eq!(3, content.get_data(2).line_id);
}
