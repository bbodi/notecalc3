mod common;

use crate::common::create_app2;
use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};

#[test]
fn test_fuzz_panic_more_rows_than_64_click() {
    let test = create_app2(35);
    test.paste("asd");
    for _ in 0..200 {
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
    }
    test.click(10, 10);
}

#[test]
fn test_insert_char_selection_when_the_first_row_is_empty() {
    let test = create_app2(35);
    test.paste("\n\n3\n");

    test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
    test.input(EditorInputEvent::Char('H'), InputModifiers::none());
    test.input(EditorInputEvent::Char('h'), InputModifiers::none());
}

#[test]
fn test_panic_fuzz_3() {
    let test = create_app2(35);
    test.paste("+[$%Si/H*$=u-p$/k%-T+i]%^[))I*r-[+xT=D%$=z^k)b%$*]$5]/*)++][+aI-adk[8+%)-/^A)g9*=^+^*-//[H-))]=^**^$Sl)d+tf]3*5=Di]B");

    test.input(EditorInputEvent::Char('('), InputModifiers::none());
}

#[test]
fn test_multiple_equals_signs() {
    let test = create_app2(35);
    test.input(EditorInputEvent::Char('z'), InputModifiers::none());
    test.input(EditorInputEvent::Char('='), InputModifiers::none());
    test.input(EditorInputEvent::Char('1'), InputModifiers::none());
    test.input(EditorInputEvent::Char('='), InputModifiers::none());
    test.input(EditorInputEvent::Char('2'), InputModifiers::none());
}

#[test]
fn fuzz_5() {
    let test = create_app2(35);

    test.paste("=(Blq9h/Oq=7y^$o[/kR]*$*oReyMo-M++]");
}

#[test]
fn fuzz_4_panic_index2_into_tokens_out_of_bound() {
    let test = create_app2(35);
    test.paste("7))C]7=[1]%(%8^7ou9b%");
}

#[test]
fn fuzz_panic_calc_208() {
    let test = create_app2(35);
    test.paste("8709(%%8)3M3[076+4][39383]4804+^438189%^2");
}

#[test]
fn test_multiplying_bug_numbers_via_unit_no_panic() {
    let test = create_app2(35);

    test.paste("909636Yl");
}

#[test]
fn fuuzzzzz() {
    let test = create_app2(35);

    test.paste("90-/9b^72^4");
}

// dbg!(&test.get_editor_content().lines().collect::<Vec<_>>()[16]);
