pub mod test_common {
    use crate::borrow_checker_fighter::BorrowCheckerFighter;
    pub use crate::editor::editor::*;
    pub use crate::helper::*;
    pub use crate::units::units::Units;
    pub use crate::*;
    pub use bumpalo::Bump;
    pub use std::ops::RangeInclusive;

    pub struct TestHelper {
        pub bcf: BorrowCheckerFighter,
    }

    #[allow(dead_code)]
    impl TestHelper {
        pub fn render(&self) {
            self.bcf
                .mut_app()
                .generate_render_commands_and_fill_editor_objs(
                    self.bcf.units(),
                    self.bcf.mut_render_bucket(),
                    self.bcf.allocator(),
                    self.bcf.mut_tokens(),
                    self.bcf.mut_results(),
                    self.bcf.mut_vars(),
                    self.bcf.mut_func_defs(),
                    self.bcf.mut_editor_objects(),
                    BitFlag256::empty(),
                );
        }

        pub fn paste(&self, str: &str) {
            self.bcf.mut_app().handle_paste(
                str.to_owned(),
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_editor_objects(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn assert_no_highlighting_rectangle(&self) {
            let render_buckets =
                &self.bcf.render_bucket().custom_commands[Layer::BehindTextAboveCursor as usize];
            for i in 0..9 {
                assert_contains(
                    render_buckets,
                    0,
                    OutputMessage::SetColor(ACTIVE_LINE_REF_HIGHLIGHT_COLORS[i]),
                );
            }
        }

        pub fn assert_results(&self, expected_results: &[&str]) {
            let mut i = 0;
            let mut ok_chars = Vec::with_capacity(32);
            let expected_len = expected_results.iter().map(|it| it.len()).sum();
            unsafe {
                for (result_index, expected_result) in expected_results.iter().enumerate() {
                    for ch in expected_result.bytes() {
                        assert_eq!(
                            RESULT_BUFFER[i] as char,
                            ch as char,
                            "{}. result, at char {}: {:?}, result_buffer: {:?}",
                            result_index,
                            i,
                            String::from_utf8(ok_chars).unwrap(),
                            &RESULT_BUFFER[0..expected_len]
                                .iter()
                                .map(|it| *it as char)
                                .collect::<Vec<char>>()
                        );
                        ok_chars.push(ch);
                        i += 1;
                    }
                    ok_chars.push(',' as u8);
                    ok_chars.push(' ' as u8);
                }
                assert_eq!(
                    RESULT_BUFFER[i], 0,
                    "more results than expected at char {}.",
                    i
                );
            }
        }

        pub fn assert_contains_operator<F>(&self, expected_count: usize, expected_command: F)
        where
            F: Fn(&RenderUtf8TextMsg) -> bool,
        {
            let mut count = 0;
            let operators = &self.bcf.render_bucket().operators;
            for op in operators {
                if expected_command(op) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, operators
            );
        }

        pub fn assert_contains_paren<F>(&self, expected_count: usize, expected_command: F)
        where
            F: Fn(&RenderChar) -> bool,
        {
            let mut count = 0;
            let operators = &self.bcf.render_bucket().parenthesis;
            for op in operators {
                if expected_command(op) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, operators
            );
        }

        pub fn assert_contains_error<F>(&self, expected_count: usize, expected_command: F)
        where
            F: Fn(&RenderUtf8TextMsg) -> bool,
        {
            let mut count = 0;
            let operators = &self.bcf.render_bucket().number_errors;
            for op in operators {
                if expected_command(op) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, operators
            );
        }

        pub fn assert_contains_custom_command<F>(
            &self,
            layer: Layer,
            expected_count: usize,
            expected_command: F,
        ) where
            F: Fn(&OutputMessage) -> bool,
        {
            let mut count = 0;
            let commands = &self.bcf.render_bucket().custom_commands[layer as usize];
            for op in commands {
                if expected_command(op) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, commands
            );
        }

        pub fn assert_contains_line_ref_result<F>(&self, expected_count: usize, expected_command: F)
        where
            F: Fn(&RenderStringMsg) -> bool,
        {
            let mut count = 0;
            let operators = &self.bcf.render_bucket().line_ref_results;
            for op in operators {
                if expected_command(op) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, operators
            );
        }

        pub fn assert_contains_result<F>(&self, expected_count: usize, expected_command: F)
        where
            F: Fn(&RenderAsciiTextMsg) -> bool,
        {
            let mut count = 0;
            let commands = &self.bcf.render_bucket().ascii_texts;
            for cmd in commands {
                if expected_command(cmd) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, commands
            );
        }

        pub fn assert_contains_text<F>(&self, expected_count: usize, expected_command: F)
        where
            F: Fn(&RenderUtf8TextMsg) -> bool,
        {
            let mut count = 0;
            let operators = &self.bcf.render_bucket().utf8_texts;
            for op in operators {
                if expected_command(op) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, operators
            );
        }

        pub fn assert_contains_num<F>(&self, expected_count: usize, expected_command: F)
        where
            F: Fn(&RenderUtf8TextMsg) -> bool,
        {
            let mut count = 0;
            let operators = &self.bcf.render_bucket().numbers;
            for op in operators {
                if expected_command(op) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, operators
            );
        }

        pub fn assert_contains_unit<F>(&self, expected_count: usize, expected_command: F)
        where
            F: Fn(&RenderUtf8TextMsg) -> bool,
        {
            let mut count = 0;
            let operators = &self.bcf.render_bucket().units;
            for op in operators {
                if expected_command(op) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, operators
            );
        }

        pub fn assert_contains_variable<F>(&self, expected_count: usize, expected_command: F)
        where
            F: Fn(&RenderUtf8TextMsg) -> bool,
        {
            let mut count = 0;
            let operators = &self.bcf.render_bucket().variable;
            for op in operators {
                if expected_command(op) {
                    count += 1;
                }
            }
            assert_eq!(
                count, expected_count,
                "Found {} times.\nExpected: {}\nin\n{:?}",
                count, expected_count, operators
            );
        }

        pub fn assert_no_pulsing(&self) {
            assert!(self.bcf.render_bucket().pulses.is_empty());
        }

        pub fn set_normalized_content(&self, str: &str) {
            self.bcf.mut_app().set_normalized_content(
                str,
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_editor_objects(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn repeated_paste(&self, str: &str, times: usize) {
            self.bcf.mut_app().handle_paste(
                str.repeat(times),
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_editor_objects(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn click(&self, x: usize, y: isize) {
            self.bcf.mut_app().handle_click(
                x,
                canvas_y(y),
                self.bcf.mut_editor_objects(),
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn handle_resize(&self, new_client_width: usize) {
            self.bcf.mut_app().handle_resize(
                new_client_width,
                self.bcf.mut_editor_objects(),
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn handle_wheel(&self, dir: usize) {
            self.bcf.mut_app().handle_wheel(
                dir,
                self.bcf.mut_editor_objects(),
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.tokens(),
                self.bcf.results(),
                self.bcf.vars(),
                self.bcf.func_defs(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn handle_drag(&self, x: usize, y: isize) {
            self.bcf.mut_app().handle_drag(
                x,
                canvas_y(y),
                self.bcf.mut_editor_objects(),
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn handle_mouse_move(&self, x: usize, y: isize) {
            self.bcf.mut_app().handle_mouse_move(
                x,
                canvas_y(y),
                self.bcf.mut_editor_objects(),
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn alt_key_released(&self) {
            self.bcf.mut_app().alt_key_released(
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_editor_objects(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn handle_time(&self, tick: u32) {
            self.bcf.mut_app().handle_time(
                tick,
                self.bcf.units(),
                self.bcf.allocator(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_editor_objects(),
                self.bcf.mut_render_bucket(),
            );
        }

        pub fn input(
            &self,
            event: EditorInputEvent,
            modif: InputModifiers,
        ) -> Option<RowModificationType> {
            self.bcf.mut_app().handle_input(
                event,
                modif,
                self.bcf.allocator(),
                self.bcf.units(),
                self.bcf.mut_tokens(),
                self.bcf.mut_results(),
                self.bcf.mut_vars(),
                self.bcf.mut_func_defs(),
                self.bcf.mut_editor_objects(),
                self.bcf.mut_render_bucket(),
            )
        }

        pub fn handle_mouse_up(&self) {
            self.bcf.mut_app().handle_mouse_up();
        }

        pub fn get_render_data(&self) -> &GlobalRenderData {
            return &self.bcf.app().render_data;
        }

        pub fn get_editor_content(&self) -> String {
            return self.bcf.app().editor_content.get_content();
        }

        pub fn get_cursor_pos(&self) -> Pos {
            return self.bcf.app().editor.get_selection().get_cursor_pos();
        }

        pub fn get_selection(&self) -> Selection {
            return self.bcf.app().editor.get_selection();
        }

        pub fn set_selection(&self, selection: Selection) {
            let app = &mut self.bcf.mut_app();
            app.editor.set_selection_save_col(selection);
        }

        pub fn set_cursor_row_col(&self, row: usize, col: usize) {
            self.set_selection(Selection::single_r_c(row, col));
        }

        pub fn mut_app<'a>(&self) -> &'a mut NoteCalcApp {
            self.bcf.mut_app()
        }

        pub fn app<'a>(&self) -> &'a NoteCalcApp {
            self.bcf.app()
        }

        pub fn units<'a>(&self) -> &'a mut Units {
            self.bcf.units()
        }

        pub fn mut_render_bucket<'a>(&self) -> &'a mut RenderBuckets<'a> {
            self.bcf.mut_render_bucket()
        }

        pub fn render_bucket<'a>(&self) -> &'a RenderBuckets<'a> {
            self.bcf.render_bucket()
        }

        pub fn get_all_custom_commands_render_commands<'a>(&self) -> Vec<OutputMessage> {
            let rb = self.bcf.render_bucket();
            let mut vec = Vec::with_capacity(
                rb.custom_commands[0].len()
                    + rb.custom_commands[1].len()
                    + rb.custom_commands[2].len(),
            );
            for layer in &rb.custom_commands {
                for cmd in layer.iter() {
                    vec.push(cmd.clone());
                }
            }
            return vec;
        }

        pub fn mut_tokens<'a>(&self) -> &'a mut AppTokens<'a> {
            self.bcf.mut_tokens()
        }

        pub fn tokens<'a>(&self) -> &'a AppTokens<'a> {
            self.bcf.tokens()
        }

        pub fn mut_results<'a>(&self) -> &'a mut Results {
            self.bcf.mut_results()
        }

        pub fn results<'a>(&self) -> &'a Results {
            self.bcf.results()
        }

        pub fn editor_objects<'a>(&self) -> &'a EditorObjects {
            self.bcf.editor_objects()
        }

        pub fn mut_editor_objects<'a>(&self) -> &'a mut EditorObjects {
            self.bcf.mut_editor_objects()
        }

        pub fn mut_vars<'a>(&self) -> &'a mut [Option<Variable>] {
            self.bcf.mut_vars()
        }

        pub fn vars<'a>(&self) -> &'a [Option<Variable>] {
            self.bcf.vars()
        }

        pub fn allocator<'a>(&self) -> &'a Bump {
            self.bcf.allocator()
        }

        pub fn mut_allocator<'a>(&self) -> &'a mut Bump {
            self.bcf.mut_allocator()
        }

        pub fn autocomplete_matrix(&self) {
            self.input(EditorInputEvent::Char('.'), InputModifiers::none());
            self.input(EditorInputEvent::Char('m'), InputModifiers::none());
            self.input(EditorInputEvent::Char('a'), InputModifiers::none());
            self.input(EditorInputEvent::Char('t'), InputModifiers::none());
            self.input(EditorInputEvent::Tab, InputModifiers::none());
        }
    }

    pub fn create_test_app<'a>(client_height: usize) -> TestHelper {
        create_test_app2(120, client_height)
    }

    pub fn to_char_slice(str: &str) -> Vec<char> {
        str.chars().collect::<Vec<char>>()
    }

    pub fn create_test_app2<'a>(w: usize, client_height: usize) -> TestHelper {
        for b in unsafe { &mut RESULT_BUFFER } {
            *b = 0;
        }
        TestHelper {
            bcf: BorrowCheckerFighter::new(w, client_height),
        }
    }

    pub fn pulsing_ref_rect(x: usize, y: usize, w: usize, h: usize) -> PulsingRectangle {
        PulsingRectangle {
            x,
            y: canvas_y(y as isize),
            w,
            h,
            start_color: THEMES[0].reference_pulse_start,
            end_color: THEMES[0].reference_pulse_end,
            animation_time: Duration::from_millis(1000),
            repeat: true,
        }
    }

    pub fn pulsing_result_rect(x: usize, y: usize, w: usize, h: usize) -> PulsingRectangle {
        PulsingRectangle {
            x,
            y: canvas_y(y as isize),
            w,
            h,
            start_color: THEMES[0].change_result_pulse_start,
            end_color: THEMES[0].change_result_pulse_end,
            animation_time: Duration::from_millis(1000),
            repeat: false,
        }
    }

    pub fn pulsing_changed_content_rect(
        x: usize,
        y: usize,
        w: usize,
        h: usize,
    ) -> PulsingRectangle {
        PulsingRectangle {
            x,
            y: canvas_y(y as isize),
            w,
            h,
            start_color: THEMES[0].change_result_pulse_start,
            end_color: THEMES[0].change_result_pulse_end,
            animation_time: Duration::from_millis(2000),
            repeat: false,
        }
    }

    pub fn assert_contains_pulse(
        render_bucket: &[PulsingRectangle],
        expected_count: usize,
        expected_command: PulsingRectangle,
    ) {
        let mut count = 0;
        for command in render_bucket {
            if *command == expected_command {
                count += 1;
            }
        }
        assert_eq!(
            count, expected_count,
            "Found {} times, expected {}.\n{:?}\nin\n{:?}",
            count, expected_count, expected_command, render_bucket
        );
    }

    pub fn assert_contains(
        render_bucket: &[OutputMessage],
        expected_count: usize,
        expected_command: OutputMessage,
    ) {
        let mut count = 0;
        for command in render_bucket {
            if *command == expected_command {
                count += 1;
            }
        }
        assert_eq!(
            count, expected_count,
            "Found {} times, expected {}.\n{:?}\nin\n{:?}",
            count, expected_count, expected_command, render_bucket
        );
    }
}
