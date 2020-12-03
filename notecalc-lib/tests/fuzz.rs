mod common;

use crate::common::create_app2;
use crate::common::create_app3;
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
fn fuzz_test_panic_26() {
    let test = create_app2(35);

    test.paste("90-/9b^72^4");
}

#[test]
fn fuzzz_test_merging_rows_should_not_exceeds_the_max_line_len_no_panic() {
    let test = create_app3(92, 40);

    test.paste("I9MP84e1qZ4B\n\
    w889kI54m8a785uy6EvJAFA464n480\n\
    z04Y0swRqZ7k9\n\
    966E5995d13O5o19iBb8b0d75IUWx0o97SOf8x2C971D4v727fMVkts1R95x0h4N7con4mpKL687F07IO0058G78sWW8ew08QpC4M0u6FzjIdr7Vzg7S66j3");
    test.set_cursor_row_col(2, 13);
    test.input(EditorInputEvent::Right, InputModifiers::shift());
    test.input(EditorInputEvent::Right, InputModifiers::shift());
    test.input(EditorInputEvent::Right, InputModifiers::shift());
    test.input(EditorInputEvent::Char('t'), InputModifiers::none());
}

#[test]
fn fuzzz_test_merging_rows_should_not_exceeds_the_max_line_len_no_panic2() {
    let test = create_app3(92, 40);

    test.paste("966E5995d13O5o19iBb8b0d75IUWx0o97SOf8x2C971D4v727fMVkts1R95x0h4N7con4mpKL687F07IO0058G78sWW8ew08QpC4M0u6FzjIdr7Vzg7S66j3\n\n\n");
    test.set_cursor_row_col(0, 120);
    test.input(EditorInputEvent::Right, InputModifiers::shift());
    test.input(EditorInputEvent::Right, InputModifiers::shift());
    test.input(EditorInputEvent::Right, InputModifiers::shift());
    test.input(EditorInputEvent::Char('d'), InputModifiers::none());
}

// TODO I think it is caused because the undo stack has only 1 level in fuzz testing
#[test]
fn fuzz1() {
    let test = create_app3(73, 40);
    //test.paste("8mhnWl1KasLgO)uKGYYp7gHV(eDIAG5OVvlh7ra2y8iiJ(Qt7ob+M8  k   z^PMLjYxIC3JhVb=8He3JMIaN-boxcKlO%epK$VWtoR1A9#z-rh$Wr3Wr(xmpSa");
    //test.set_cursor_row_col(0, 36);

    //     test.set_selection(Selection::range(
    //         Pos::from_row_column(10, 11),
    //         Pos::from_row_column(8, 11),
    //     ));

    {
        test.paste("");
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        test.input(EditorInputEvent::Char('4'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Up, InputModifiers::none());
        test.input(EditorInputEvent::Char('g'), InputModifiers::none());
        test.input(EditorInputEvent::Char('0'), InputModifiers::none());
        test.input(EditorInputEvent::Char(']'), InputModifiers::none());
        test.input(EditorInputEvent::Char('m'), InputModifiers::none());
        test.input(EditorInputEvent::Char('6'), InputModifiers::none());
        test.input(EditorInputEvent::Char('N'), InputModifiers::none());
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('0'), InputModifiers::none());
        test.input(EditorInputEvent::Char('f'), InputModifiers::none());
        test.input(EditorInputEvent::Char('k'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('['), InputModifiers::none());
        test.input(EditorInputEvent::Char('R'), InputModifiers::none());
        test.input(EditorInputEvent::Char('/'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('T'), InputModifiers::none());
        test.input(EditorInputEvent::Char(']'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('S'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('w'), InputModifiers::none());
        test.input(EditorInputEvent::Char('9'), InputModifiers::none());
        test.input(EditorInputEvent::Char('T'), InputModifiers::none());
        test.input(EditorInputEvent::Char('s'), InputModifiers::none());
        test.input(EditorInputEvent::Char('G'), InputModifiers::none());
        test.input(EditorInputEvent::Char('G'), InputModifiers::none());
        test.input(EditorInputEvent::Char('T'), InputModifiers::none());
        test.input(EditorInputEvent::Char('^'), InputModifiers::none());
        test.input(EditorInputEvent::Char('y'), InputModifiers::none());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::PageUp, InputModifiers::shift());
        test.input(EditorInputEvent::Char('*'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Char('k'), InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('$'), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('P'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Char('L'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        test.input(EditorInputEvent::Char('T'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('x'), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('v'), InputModifiers::none());
        test.input(EditorInputEvent::Char('v'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('5'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('q'), InputModifiers::none());
        test.input(EditorInputEvent::Char(']'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('6'), InputModifiers::none());
        test.input(EditorInputEvent::Char('m'), InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('v'), InputModifiers::none());
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('f'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('u'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('4'), InputModifiers::none());
        test.input(EditorInputEvent::Char('*'), InputModifiers::none());
        test.input(EditorInputEvent::Char('z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('S'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('['), InputModifiers::none());
        test.input(EditorInputEvent::Char('i'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('r'), InputModifiers::none());
        test.input(EditorInputEvent::Char('O'), InputModifiers::none());
        test.input(EditorInputEvent::Char('g'), InputModifiers::none());
        test.input(EditorInputEvent::Char('u'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Char('R'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Char('G'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('['), InputModifiers::none());
        test.input(EditorInputEvent::Char('Z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('#'), InputModifiers::none());
        test.input(EditorInputEvent::Char('R'), InputModifiers::none());
        test.input(EditorInputEvent::Char('^'), InputModifiers::none());
        test.input(EditorInputEvent::Char('m'), InputModifiers::none());
        test.input(EditorInputEvent::Char('='), InputModifiers::none());
        test.input(EditorInputEvent::Char('Q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Char('w'), InputModifiers::none());
        test.input(EditorInputEvent::Char('i'), InputModifiers::none());
        test.input(EditorInputEvent::Char('L'), InputModifiers::none());
        test.input(EditorInputEvent::Char('g'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('O'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('w'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('L'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('E'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Char('6'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('v'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('g'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('i'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('$'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Char('/'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('u'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('['), InputModifiers::none());
        test.input(EditorInputEvent::Char('m'), InputModifiers::none());
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('O'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('8'), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('='), InputModifiers::none());
        test.input(EditorInputEvent::Char('5'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('e'), InputModifiers::none());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('S'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Char('='), InputModifiers::none());
        test.input(EditorInputEvent::Char('y'), InputModifiers::none());
        test.input(EditorInputEvent::Char(')'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('R'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('N'), InputModifiers::none());
        test.input(EditorInputEvent::Char('5'), InputModifiers::none());
        test.input(EditorInputEvent::Char('4'), InputModifiers::none());
        test.input(EditorInputEvent::Char('u'), InputModifiers::none());
        test.input(EditorInputEvent::Char('q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('F'), InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('N'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('f'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('F'), InputModifiers::none());
        test.input(EditorInputEvent::Char('u'), InputModifiers::none());
        test.input(EditorInputEvent::Char('P'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Q'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('i'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Char('6'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('o'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('L'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('E'), InputModifiers::none());
        test.input(EditorInputEvent::Char('k'), InputModifiers::none());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('L'), InputModifiers::none());
        test.input(EditorInputEvent::Char('y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('f'), InputModifiers::none());
        test.input(EditorInputEvent::Char('5'), InputModifiers::none());
        test.input(EditorInputEvent::Char('F'), InputModifiers::none());
        test.input(EditorInputEvent::Char('S'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('u'), InputModifiers::none());
        test.input(EditorInputEvent::Char('r'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('='), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('5'), InputModifiers::none());
        test.input(EditorInputEvent::Char('x'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('w'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('s'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('s'), InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        test.input(EditorInputEvent::Char('T'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('*'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('+'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('f'), InputModifiers::none());
        test.input(EditorInputEvent::Char('w'), InputModifiers::none());
        test.input(EditorInputEvent::Char('o'), InputModifiers::none());
        test.input(EditorInputEvent::Char('S'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('G'), InputModifiers::none());
        test.input(EditorInputEvent::Char('#'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Enter, InputModifiers::none());
        test.input(EditorInputEvent::Char('+'), InputModifiers::none());
        test.input(EditorInputEvent::Char('F'), InputModifiers::none());
        test.input(EditorInputEvent::Char('O'), InputModifiers::none());
        test.input(EditorInputEvent::Char('r'), InputModifiers::none());
        test.input(EditorInputEvent::Char('8'), InputModifiers::none());
        test.input(EditorInputEvent::Char('9'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('$'), InputModifiers::none());
        test.input(EditorInputEvent::Char('v'), InputModifiers::none());
        test.input(EditorInputEvent::Char('S'), InputModifiers::none());
        test.input(EditorInputEvent::Char('3'), InputModifiers::none());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        test.input(EditorInputEvent::Char('m'), InputModifiers::none());
        test.input(EditorInputEvent::Char('O'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('R'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('%'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        test.input(EditorInputEvent::Char('E'), InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Char('4'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('w'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('9'), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('e'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('0'), InputModifiers::none());
        test.input(EditorInputEvent::Char('6'), InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        test.input(EditorInputEvent::Char('o'), InputModifiers::none());
        test.input(EditorInputEvent::Char('='), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('/'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::ctrl());
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::PageUp, InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Char('r'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('8'), InputModifiers::none());
        test.input(EditorInputEvent::Char('i'), InputModifiers::none());
        test.input(EditorInputEvent::Char('i'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('Q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Char('o'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('+'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('8'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('k'), InputModifiers::none());
        test.input(EditorInputEvent::Char('g'), InputModifiers::none());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('^'), InputModifiers::none());
        test.input(EditorInputEvent::Char('P'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('L'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('x'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('C'), InputModifiers::none());
        test.input(EditorInputEvent::Char('3'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('='), InputModifiers::none());
        test.input(EditorInputEvent::Char('8'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('e'), InputModifiers::none());
        test.input(EditorInputEvent::Char('3'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Char('N'), InputModifiers::none());
        test.input(EditorInputEvent::Char('-'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('o'), InputModifiers::none());
        test.input(EditorInputEvent::Char('x'), InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('O'), InputModifiers::none());
        test.input(EditorInputEvent::Char('%'), InputModifiers::none());
        test.input(EditorInputEvent::Char('e'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('$'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('o'), InputModifiers::none());
        test.input(EditorInputEvent::Char('R'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Char('9'), InputModifiers::none());
        test.input(EditorInputEvent::Char('#'), InputModifiers::none());
        test.input(EditorInputEvent::Char('z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('-'), InputModifiers::none());
        test.input(EditorInputEvent::Char('r'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('$'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('r'), InputModifiers::none());
        test.input(EditorInputEvent::Char('3'), InputModifiers::none());
        test.input(EditorInputEvent::Char('y'), InputModifiers::none());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('r'), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('x'), InputModifiers::none());
        test.input(EditorInputEvent::Char('m'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('S'), InputModifiers::none());
        test.input(EditorInputEvent::PageUp, InputModifiers::none());
        test.input(EditorInputEvent::Char('x'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('R'), InputModifiers::none());
        test.input(EditorInputEvent::Char('6'), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('['), InputModifiers::none());
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('o'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('s'), InputModifiers::none());
        test.input(EditorInputEvent::Char('$'), InputModifiers::none());
        test.input(EditorInputEvent::Char('x'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        test.input(EditorInputEvent::Char('c'), InputModifiers::none());
        test.input(EditorInputEvent::Char('#'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('G'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('e'), InputModifiers::none());
        test.input(EditorInputEvent::Char('G'), InputModifiers::none());
        test.input(EditorInputEvent::Char('q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('*'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('v'), InputModifiers::none());
        test.input(EditorInputEvent::Char('+'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('J'), InputModifiers::none());
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('g'), InputModifiers::none());
        test.input(EditorInputEvent::Char('F'), InputModifiers::none());
        test.input(EditorInputEvent::Char('N'), InputModifiers::none());
        test.input(EditorInputEvent::Char('i'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Char('L'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('0'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('g'), InputModifiers::none());
        test.input(EditorInputEvent::Char('s'), InputModifiers::none());
        test.input(EditorInputEvent::Char('3'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('f'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('m'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('u'), InputModifiers::none());
        test.input(EditorInputEvent::Char('T'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('R'), InputModifiers::none());
        test.input(EditorInputEvent::Char('^'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Backspace, InputModifiers::none());
        test.input(EditorInputEvent::Char('y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('q'), InputModifiers::none());
        test.input(EditorInputEvent::Char('f'), InputModifiers::none());
        test.input(EditorInputEvent::Char('4'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('='), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Tab, InputModifiers::none());
        test.input(EditorInputEvent::Char('o'), InputModifiers::none());
        test.input(EditorInputEvent::Char('y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Char('/'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('*'), InputModifiers::none());
        test.input(EditorInputEvent::Char('e'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('U'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('z'), InputModifiers::ctrl_shift());
        test.input(EditorInputEvent::Char(']'), InputModifiers::none());
        test.input(EditorInputEvent::Char('t'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('b'), InputModifiers::none());
        test.input(EditorInputEvent::Char('f'), InputModifiers::none());
        test.input(EditorInputEvent::Char('B'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('i'), InputModifiers::none());
        test.input(EditorInputEvent::Home, InputModifiers::shift());
        test.input(EditorInputEvent::Char('8'), InputModifiers::none());
        test.input(EditorInputEvent::Char('m'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('n'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('1'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('a'), InputModifiers::none());
        test.input(EditorInputEvent::Char('s'), InputModifiers::none());
        test.input(EditorInputEvent::Char('L'), InputModifiers::none());
        test.input(EditorInputEvent::Char('g'), InputModifiers::none());
        test.input(EditorInputEvent::Char('O'), InputModifiers::none());
        test.input(EditorInputEvent::Char(')'), InputModifiers::none());
        test.input(EditorInputEvent::Char('u'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('G'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('Y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('7'), InputModifiers::none());
        test.input(EditorInputEvent::Char('g'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('('), InputModifiers::none());
        test.input(EditorInputEvent::Char('e'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Char('G'), InputModifiers::none());
        test.input(EditorInputEvent::Char('5'), InputModifiers::none());
        test.input(EditorInputEvent::Char('O'), InputModifiers::none());
        test.input(EditorInputEvent::Char('V'), InputModifiers::none());
        test.input(EditorInputEvent::Char('v'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('A'), InputModifiers::none());
        test.input(EditorInputEvent::Char('H'), InputModifiers::none());
        test.input(EditorInputEvent::Char('x'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Char('d'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('F'), InputModifiers::none());
        test.input(EditorInputEvent::Char('9'), InputModifiers::none());
        test.input(EditorInputEvent::Char('D'), InputModifiers::none());
        test.input(EditorInputEvent::Char('W'), InputModifiers::none());
        test.input(EditorInputEvent::Char('y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('M'), InputModifiers::none());
        test.input(EditorInputEvent::Char('j'), InputModifiers::none());
        test.input(EditorInputEvent::Char('N'), InputModifiers::none());
        test.input(EditorInputEvent::Char('6'), InputModifiers::none());
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('I'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('K'), InputModifiers::none());
        test.input(EditorInputEvent::Char('y'), InputModifiers::none());
        test.input(EditorInputEvent::Char('p'), InputModifiers::none());
        test.input(EditorInputEvent::Char('X'), InputModifiers::none());
        test.input(EditorInputEvent::Char('x'), InputModifiers::none());
        test.input(EditorInputEvent::Char('h'), InputModifiers::none());
        test.input(EditorInputEvent::Char('2'), InputModifiers::none());
        test.input(EditorInputEvent::Char('l'), InputModifiers::none());
        test.input(EditorInputEvent::Char('u'), InputModifiers::none());
        test.input(EditorInputEvent::Char('z'), InputModifiers::none());
        test.input(EditorInputEvent::Char('z'), InputModifiers::ctrl());
    }
}

#[test]
fn fuzz2() {
    let test = create_app3(73, 40);
    test.paste("309d4^204872e0)(16%$+2l)#-08=#=+297329+[+05*/69)68Q#30-0$/)]4/ks593[()$-3   -059]82980m11793%374^+98#40$1*73I67]6$#2^3#7
    =5$2*x-2044923+/I2(397-293496(6[/7k9]/^*6490^)(5/j=");
    test.set_cursor_row_col(1, 55);
    test.input(EditorInputEvent::Char('9'), InputModifiers::none());
}

// #[test]
// fn dummy_() {
//     let test = create_app3(73, 40);
//     test.paste("\n\n\n\n\n\n\n\n\n\n");
//     test.set_cursor_row_col(0, 0);
//     //     test.set_selection(Selection::range(
//     //         Pos::from_row_column(10, 11),
//     //         Pos::from_row_column(8, 11),
//     //     ));
//
//     {}
// }

#[allow(dead_code)]
mod fuzzer {
    use super::*;
    use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
    use rand::prelude::*;

    const CHARACTERS: &'static str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    const NUMBERS: &'static str = "0123456789";
    const OPERATORS: &'static str = "+-*/%[]()^$=";
    const NAVIGATION_EVENTS: [EditorInputEvent; 8] = [
        EditorInputEvent::Right,
        EditorInputEvent::Left,
        EditorInputEvent::Up,
        EditorInputEvent::Down,
        EditorInputEvent::Home,
        EditorInputEvent::End,
        EditorInputEvent::PageUp,
        EditorInputEvent::PageDown,
    ];

    fn generate_random_events(
        events: &mut Vec<(EditorInputEvent, InputModifiers)>,
        rng: &mut ThreadRng,
        chance_enter: f64,
        chance_char: f64,
        chance_backspace: f64,
        chance_del: f64,
        chance_navigation: f64,
        chance_ctrl_a: f64,
        chance_operator: f64,
        chance_number: f64,
        chance_select_multiple: f64,
        chance_undo: f64,
    ) {
        let rnd = rng.gen_range(0.0, 1.0);
        let mut acc = 0.0;
        let mut hit = |chance: f64| {
            acc += chance;
            return rnd < acc;
        };
        if hit(chance_enter) {
            events.push((EditorInputEvent::Enter, InputModifiers::none()));
        } else if hit(chance_char) {
            let i = rng.gen_range(0, CHARACTERS.len());
            events.push((
                EditorInputEvent::Char(CHARACTERS.chars().skip(i).next().unwrap()),
                InputModifiers::none(),
            ));
        } else if hit(chance_backspace) {
            events.push((EditorInputEvent::Backspace, InputModifiers::none()));
        } else if hit(chance_del) {
            events.push((EditorInputEvent::Del, InputModifiers::none()));
        } else if hit(chance_navigation) {
            let navi = rng.gen_range(0, NAVIGATION_EVENTS.len());
            events.push((NAVIGATION_EVENTS[navi], InputModifiers::none()));
        } else if hit(chance_ctrl_a) {
            events.push((EditorInputEvent::Char('a'), InputModifiers::ctrl()));
        } else if hit(chance_operator) {
            let i = rng.gen_range(0, OPERATORS.len());
            events.push((
                EditorInputEvent::Char(OPERATORS.chars().skip(i).next().unwrap()),
                InputModifiers::none(),
            ));
        } else if hit(chance_number) {
            let i = rng.gen_range(0, NUMBERS.len());
            events.push((
                EditorInputEvent::Char(NUMBERS.chars().skip(i).next().unwrap()),
                InputModifiers::none(),
            ));
        } else if hit(chance_select_multiple) {
            let dir = NAVIGATION_EVENTS[rng.gen_range(0, NAVIGATION_EVENTS.len())];
            let step = rng.gen_range(0, 20);
            for _ in 0..step {
                events.push((dir, InputModifiers::shift()));
            }
        } else if hit(chance_undo) {
            events.push((EditorInputEvent::Char('z'), InputModifiers::ctrl()));
        }
    }

    fn fuzzzzzz() {
        fn rnd_eql(rng: &mut ThreadRng, max: &mut f64) -> f64 {
            let rnd = rng.gen_range(0.0, *max);
            *max -= rnd;
            return rnd;
        };

        let test = create_app3(73, 40);

        let mut rng = rand::thread_rng();
        let mut events_buffer = Vec::with_capacity(16);
        loop {
            println!("START ===========");
            test.mut_allocator().reset();
            test.set_normalized_content("\n\n\n\n\n\n\n\n\n\n");
            test.set_cursor_row_col(0, 0);
            for _ in 0..2000 {
                let mut max = 1.0;
                let chance_enter = rnd_eql(&mut rng, &mut max);
                let chance_char = rnd_eql(&mut rng, &mut max);
                let chance_backspace = rnd_eql(&mut rng, &mut max);
                let chance_del = rnd_eql(&mut rng, &mut max);
                let chance_navigation = rnd_eql(&mut rng, &mut max);
                let chance_ctrl_a = rnd_eql(&mut rng, &mut max);
                let chance_operator = rnd_eql(&mut rng, &mut max);
                let chance_number = rnd_eql(&mut rng, &mut max);
                let chance_select_multiple = rnd_eql(&mut rng, &mut max);
                let chance_undo = rnd_eql(&mut rng, &mut max);

                events_buffer.clear();
                generate_random_events(
                    &mut events_buffer,
                    &mut rng,
                    chance_enter,
                    chance_char,
                    chance_backspace,
                    chance_del,
                    chance_navigation,
                    chance_ctrl_a,
                    chance_operator,
                    chance_number,
                    chance_select_multiple,
                    chance_undo,
                );
                for (input, modif) in &events_buffer {
                    let modif_str = if modif.ctrl {
                        if modif.shift {
                            "InputModifiers::ctrl_shift()"
                        } else {
                            "InputModifiers::ctrl()"
                        }
                    } else if modif.shift {
                        "InputModifiers::shift()"
                    } else if modif.alt {
                        "InputModifiers::alt()"
                    } else {
                        "InputModifiers::none()"
                    };
                    print!("test.input({:?}, {})\n", input, modif_str);

                    test.input(*input, *modif);
                }
            }
            println!("END ===========");
        }
    }
}

// dbg!(&test.get_editor_content().lines().collect::<Vec<_>>()[16]);
