#[cfg(test)]
mod shit {
    use notecalc_lib::editor::editor::{Pos, Selection};
    use notecalc_lib::test_common::test_common::create_test_app;
    use std::ops::RangeInclusive;

    fn t(content: &str, expected: &str, selected_range: RangeInclusive<usize>) {
        let test = create_test_app(35);
        test.paste(content);
        test.set_selection(Selection::range(
            Pos::from_row_column(*selected_range.start(), 0),
            Pos::from_row_column(
                *selected_range.end(),
                test.app().editor_content.line_len(*selected_range.end()),
            ),
        ));
        fn trimmed_compare(a: &str, b: &str) {
            assert_eq!(a.lines().count(), b.lines().count(), "{} != {}", a, b);

            let a_lines = a.lines();
            let b_lines = b.lines();
            for (a_line, b_line) in a_lines.zip(b_lines) {
                assert_eq!(a_line.trim_start(), b_line.trim_start());
            }
        }
        trimmed_compare(
            expected,
            &test.mut_app().copy_selected_rows_with_result_to_clipboard(
                &test.units(),
                test.mut_render_bucket(),
                &test.tokens(),
                &test.mut_vars(),
                &test.mut_results(),
            ),
        );
    }

    #[test]
    fn test_rich_copy() {
        t("1", "1  █ 1\n", 0..=0);
        t("1 + 2", "1 + 2  █ 3\n", 0..=0);
        t("23", "23  █ 23\n", 0..=0);
        t(
            "1\n\
               23",
            "1   █  1\n\
                 23  █ 23\n",
            0..=1,
        );
        t(
            "1\n\
               23\n\
               99999.66666",
            "1   █  1\n\
                     23  █ 23\n",
            0..=1,
        );
        t(
            "1\n\
               23\n\
               99999.66666",
            "1            █      1\n\
                 23           █     23\n\
                 99999.66666  █ 99 999.66666\n",
            0..=2,
        );
        t("[1]", "[1]  █ [1]\n", 0..=0);
        t(
            "[1]\n\
                 [23]",
            "[1]  █ [1]\n",
            0..=0,
        );
        t(
            "[1]\n\
                 [23]",
            "[1]   █ [ 1]\n\
                 [23]  █ [23]\n",
            0..=1,
        );
        t("[1,2,3]", "[1  2  3]  █ [1  2  3]\n", 0..=0);
        t(
            "[1,2,3]\n[33, 44, 55]",
            "[1  2  3]     █ [ 1   2   3]\n\
                 [33  44  55]  █ [33  44  55]\n",
            0..=1,
        );
        t(
            "[1;2;3]",
            "┌ ┐  █ ┌ ┐\n\
                 │1│  █ │1│\n\
                 │2│  █ │2│\n\
                 │3│  █ │3│\n\
                 └ ┘  █ └ ┘\n",
            0..=0,
        );
        t(
            "[1, 2, 3] * [1;2;3]",
            "            ┌ ┐  █\n\
                             │1│  █\n\
                 [1  2  3] * │2│  █ [14]\n\
                             │3│  █\n\
                             └ ┘  █\n",
            0..=0,
        );
        // test alignment
        t(
            "[1, 2, 3]\n'asd\n[1, 2, 3]\n[10, 20, 30]",
            "[1  2  3]     █ [1  2  3]\n\
                 'asd          █\n\
                 [1  2  3]     █ [ 1   2   3]\n\
                 [10  20  30]  █ [10  20  30]\n",
            0..=3,
        );

        // test alignment + thousand grouping
        t(
            "[1;2.3;2222;4km;50000]",
            // Result
            "┌     ┐  █ ┌           ┐\n\
                 │    1│  █ │     1.0   │\n\
                 │  2.3│  █ │     2.3   │\n\
                 │ 2222│  █ │ 2 222.0   │\n\
                 │  4km│  █ │     4.0 km│\n\
                 │50000│  █ │50 000.0   │\n\
                 └     ┘  █ └           ┘\n",
            0..=0,
        );
        // test selecting only a single line
        t(
            "[1, 2, 3]\n'asd\n[1, 2, 3]\n[10, 20, 30]",
            "[1  2  3]  █ [1  2  3]\n",
            2..=2,
        );
        t(
            "_999
    22222
    3
    4
    2
    &[4]
    722
    alma = 3
    alma * 2
    alma * &[7] + &[6]
    &[7]
    2222222222222222222722.22222
    ^
    human brain: 10^16 op/s
    so far 100 000 000 000 humans lived
    avg. human lifespan is 50 years
    total human brain activity is &[14] * &[15] * (&[16]/1s)",
            "2222222222222222222722.22222  █ 2 222 222 222 222 222 222 722.22222\n",
            11..=11,
        );
    }

    #[test]
    fn test_line_ref_inlining() {
        t(
            "23\n\
         &[1]",
            "23  █ 23\n\
         23  █ 23\n",
            0..=1,
        );
    }

    #[test]
    fn test_headers() {
        t(
            "# Header1\n\
         23\n\
         ## Header 2\n\
         &[2]",
            "# Header1    █\n\
         23           █ 23\n\
         ## Header 2  █\n\
         23           █ 23\n",
            0..=3,
        );
    }
}
