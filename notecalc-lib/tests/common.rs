pub use bumpalo::Bump;
pub use notecalc_lib::editor::editor::*;
pub use notecalc_lib::helper::*;
pub use notecalc_lib::units::units::Units;
pub use notecalc_lib::*;
pub use std::ops::RangeInclusive;

static mut RESULT_BUFFER: [u8; 2048] = [0; 2048];

pub struct BorrowCheckerFighter {
    app_ptr: u64,
    units_ptr: u64,
    render_bucket_ptr: u64,
    tokens_ptr: u64,
    results_ptr: u64,
    vars_ptr: u64,
    editor_objects_ptr: u64,
    allocator: u64,
}

#[allow(dead_code)]
impl BorrowCheckerFighter {
    pub fn mut_app<'a>(&self) -> &'a mut NoteCalcApp {
        unsafe { &mut *(self.app_ptr as *mut NoteCalcApp) }
    }

    pub fn app<'a>(&self) -> &'a NoteCalcApp {
        unsafe { &*(self.app_ptr as *const NoteCalcApp) }
    }

    pub fn units<'a>(&self) -> &'a mut Units {
        unsafe { &mut *(self.units_ptr as *mut Units) }
    }

    pub fn mut_render_bucket<'a>(&self) -> &'a mut RenderBuckets<'a> {
        unsafe { &mut *(self.render_bucket_ptr as *mut RenderBuckets) }
    }

    pub fn tokens<'a>(&self) -> &'a AppTokens<'a> {
        unsafe { &*(self.tokens_ptr as *const AppTokens) }
    }

    pub fn mut_tokens<'a>(&self) -> &'a mut AppTokens<'a> {
        unsafe { &mut *(self.tokens_ptr as *mut AppTokens) }
    }

    pub fn mut_results<'a>(&self) -> &'a mut Results {
        unsafe { &mut *(self.results_ptr as *mut Results) }
    }

    pub fn mut_editor_objects<'a>(&self) -> &'a mut EditorObjects {
        unsafe { &mut *(self.editor_objects_ptr as *mut EditorObjects) }
    }

    pub fn editor_objects<'a>(&self) -> &'a EditorObjects {
        unsafe { &*(self.editor_objects_ptr as *const EditorObjects) }
    }

    pub fn mut_vars<'a>(&self) -> &'a mut [Option<Variable>] {
        unsafe { &mut (&mut *(self.vars_ptr as *mut [Option<Variable>; MAX_LINE_COUNT + 1]))[..] }
    }

    pub fn allocator<'a>(&self) -> &'a Bump {
        unsafe { &*(self.allocator as *const Bump) }
    }

    pub fn mut_allocator<'a>(&self) -> &'a mut Bump {
        unsafe { &mut *(self.allocator as *mut Bump) }
    }

    pub fn render(&self) {
        self.mut_app()
            .generate_render_commands_and_fill_editor_objs(
                self.units(),
                self.mut_render_bucket(),
                unsafe { &mut RESULT_BUFFER },
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_editor_objects(),
                EditorRowFlags::empty(),
            );
    }

    pub fn paste(&self, str: &str) {
        self.mut_app().handle_paste(
            str.to_owned(),
            self.units(),
            self.allocator(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_editor_objects(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn render_get_result_buf(&self, result_buffer: &mut [u8]) {
        self.mut_app()
            .generate_render_commands_and_fill_editor_objs(
                self.units(),
                self.mut_render_bucket(),
                result_buffer,
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_editor_objects(),
                EditorRowFlags::empty(),
            );
    }

    pub fn render_get_result_commands<'b>(
        &'b self,
        render_buckets: &mut RenderBuckets<'b>,
        result_buffer: &'b mut [u8],
    ) {
        self.mut_app()
            .generate_render_commands_and_fill_editor_objs(
                self.units(),
                render_buckets,
                result_buffer,
                self.allocator(),
                self.mut_tokens(),
                self.mut_results(),
                self.mut_vars(),
                self.mut_editor_objects(),
                EditorRowFlags::empty(),
            );
    }

    pub fn contains<'b>(
        &'b self,
        render_bucket: &[OutputMessage<'b>],
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

    pub fn set_normalized_content(&self, str: &str) {
        self.mut_app().set_normalized_content(
            str,
            self.units(),
            self.allocator(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_editor_objects(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn repeated_paste(&self, str: &str, times: usize) {
        self.mut_app().handle_paste(
            str.repeat(times),
            self.units(),
            self.allocator(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_editor_objects(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn click(&self, x: usize, y: isize) {
        self.mut_app().handle_click(
            x,
            canvas_y(y),
            self.mut_editor_objects(),
            self.units(),
            self.allocator(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn handle_resize(&self, new_client_width: usize) {
        self.mut_app().handle_resize(
            new_client_width,
            self.mut_editor_objects(),
            self.units(),
            self.allocator(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn handle_wheel(&self, dir: usize) {
        self.mut_app().handle_wheel(
            dir,
            self.mut_editor_objects(),
            self.units(),
            self.allocator(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn handle_drag(&self, x: usize, y: isize) {
        self.mut_app().handle_drag(
            x,
            canvas_y(y),
            self.mut_editor_objects(),
            self.units(),
            self.allocator(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn alt_key_released(&self) {
        self.mut_app().alt_key_released(
            self.units(),
            self.allocator(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_editor_objects(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn handle_time(&self, tick: u32) {
        self.mut_app().handle_time(
            tick,
            self.units(),
            self.allocator(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_editor_objects(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn input(&self, event: EditorInputEvent, modif: InputModifiers) {
        self.mut_app().handle_input(
            event,
            modif,
            self.allocator(),
            self.units(),
            self.mut_tokens(),
            self.mut_results(),
            self.mut_vars(),
            self.mut_editor_objects(),
            self.mut_render_bucket(),
            unsafe { &mut RESULT_BUFFER },
        );
    }

    pub fn autocomplete_matrix(&self) {
        self.input(EditorInputEvent::Char('.'), InputModifiers::none());
        self.input(EditorInputEvent::Char('m'), InputModifiers::none());
        self.input(EditorInputEvent::Char('a'), InputModifiers::none());
        self.input(EditorInputEvent::Char('t'), InputModifiers::none());
        self.input(EditorInputEvent::Tab, InputModifiers::none());
    }

    pub fn handle_mouse_up(&self) {
        self.mut_app().handle_mouse_up();
    }

    pub fn get_render_data(&self) -> GlobalRenderData {
        return self.mut_app().render_data.clone();
    }

    pub fn get_editor_content(&self) -> String {
        return self.mut_app().editor_content.get_content();
    }

    pub fn get_cursor_pos(&self) -> Pos {
        return self.mut_app().editor.get_selection().get_cursor_pos();
    }

    pub fn get_selection(&self) -> Selection {
        return self.mut_app().editor.get_selection();
    }

    pub fn set_selection(&self, selection: Selection) {
        let app = &mut self.mut_app();
        app.editor.set_selection_save_col(selection);
    }

    pub fn set_cursor_row_col(&self, row: usize, col: usize) {
        self.set_selection(Selection::single_r_c(row, col));
    }
}

pub fn create_app3<'a>(client_width: usize, client_height: usize) -> BorrowCheckerFighter {
    let app = NoteCalcApp::new(client_width, client_height);
    let editor_objects = EditorObjects::new();
    let tokens = AppTokens::new();
    let results = Results::new();
    let vars = create_vars();
    fn to_box_ptr<T>(t: T) -> u64 {
        let ptr = Box::into_raw(Box::new(t)) as u64;
        ptr
    }
    return BorrowCheckerFighter {
        app_ptr: to_box_ptr(app),
        units_ptr: to_box_ptr(Units::new()),
        render_bucket_ptr: to_box_ptr(RenderBuckets::new()),
        tokens_ptr: to_box_ptr(tokens),
        results_ptr: to_box_ptr(results),
        vars_ptr: to_box_ptr(vars),
        editor_objects_ptr: to_box_ptr(editor_objects),
        allocator: to_box_ptr(Bump::with_capacity(MAX_LINE_COUNT * 120)),
    };
}

pub fn create_app2<'a>(client_height: usize) -> BorrowCheckerFighter {
    create_app3(120, client_height)
}
