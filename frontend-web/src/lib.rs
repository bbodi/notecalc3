#![deny(
warnings,
anonymous_parameters,
unused_extern_crates,
unused_import_braces,
trivial_casts,
variant_size_differences,
//missing_debug_implementations,
trivial_numeric_casts,
unused_qualifications,
clippy::all
)]
#![feature(const_in_array_repeat_expressions)]

use wasm_bindgen::prelude::*;

use crate::utils::set_panic_hook;
use bumpalo::Bump;
use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::helper::*;
use notecalc_lib::units::units::Units;
use notecalc_lib::{
    Layer, NoteCalcApp, OutputMessage, OutputMessageCommandId, RenderAsciiTextMsg, RenderBuckets,
    RenderStringMsg, RenderUtf8TextMsg, Variable, MAX_LINE_COUNT,
};

mod utils;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

static mut RESULT_BUFFER: [u8; 2048] = [0; 2048];
const RENDER_COMMAND_BUFFER_SIZE: usize = 1024 * 100;
static mut RENDER_COMMAND_BUFFER: [u8; RENDER_COMMAND_BUFFER_SIZE] =
    [0; RENDER_COMMAND_BUFFER_SIZE];

#[wasm_bindgen]
extern "C" {
    pub fn js_log(s: &str);
}

struct AppPointers {
    app_ptr: u32,
    units_ptr: u32,
    render_bucket_ptr: u32,
    tokens_ptr: u32,
    results_ptr: u32,
    vars_ptr: u32,
    editor_objects_ptr: u32,
    allocator: u32,
}

impl AppPointers {
    fn mut_app<'a>(ptr: u32) -> &'a mut NoteCalcApp {
        let ptr_holder = unsafe { &*(ptr as *const AppPointers) };
        unsafe { &mut *(ptr_holder.app_ptr as *mut NoteCalcApp) }
    }

    fn app<'a>(ptr: u32) -> &'a NoteCalcApp {
        let ptr_holder = unsafe { &*(ptr as *const AppPointers) };
        unsafe { &*(ptr_holder.app_ptr as *const NoteCalcApp) }
    }

    fn units<'a>(ptr: u32) -> &'a mut Units {
        let ptr_holder = unsafe { &*(ptr as *const AppPointers) };
        unsafe { &mut *(ptr_holder.units_ptr as *mut Units) }
    }

    fn mut_render_bucket<'a>(ptr: u32) -> &'a mut RenderBuckets<'a> {
        let ptr_holder = unsafe { &*(ptr as *const AppPointers) };
        unsafe { &mut *(ptr_holder.render_bucket_ptr as *mut RenderBuckets) }
    }

    fn mut_tokens<'a>(ptr: u32) -> &'a mut AppTokens<'a> {
        let ptr_holder = unsafe { &*(ptr as *const AppPointers) };
        unsafe { &mut *(ptr_holder.tokens_ptr as *mut AppTokens) }
    }

    fn mut_results<'a>(ptr: u32) -> &'a mut Results {
        let ptr_holder = unsafe { &*(ptr as *const AppPointers) };
        unsafe { &mut *(ptr_holder.results_ptr as *mut Results) }
    }

    fn mut_editor_objects<'a>(ptr: u32) -> &'a mut EditorObjects {
        let ptr_holder = unsafe { &*(ptr as *const AppPointers) };
        unsafe { &mut *(ptr_holder.editor_objects_ptr as *mut EditorObjects) }
    }

    fn mut_vars<'a>(ptr: u32) -> &'a mut [Option<Variable>] {
        let ptr_holder = unsafe { &*(ptr as *const AppPointers) };
        unsafe {
            &mut (&mut *(ptr_holder.vars_ptr as *mut [Option<Variable>; MAX_LINE_COUNT + 1]))[..]
        }
    }

    fn allocator<'a>(ptr: u32) -> &'a Bump {
        let ptr_holder = unsafe { &*(ptr as *const AppPointers) };
        unsafe { &*(ptr_holder.allocator as *const Bump) }
    }

    fn mut_allocator<'a>(ptr: u32) -> &'a mut Bump {
        let ptr_holder = unsafe { &*(ptr as *mut AppPointers) };
        unsafe { &mut *(ptr_holder.allocator as *mut Bump) }
    }
}

#[wasm_bindgen]
pub fn create_app(client_width: usize, client_height: usize) -> u32 {
    set_panic_hook();
    js_log(&format!("client_width: {}", client_width));
    js_log(&format!("client_height: {}", client_height));
    let editor_objects = EditorObjects::new();
    let tokens = AppTokens::new();
    let results = Results::new();
    let vars = create_vars();

    let app = NoteCalcApp::new(client_width, client_height);
    to_box_ptr(AppPointers {
        app_ptr: to_box_ptr(app),
        units_ptr: to_box_ptr(Units::new()),
        render_bucket_ptr: to_box_ptr(RenderBuckets::new()),
        tokens_ptr: to_box_ptr(tokens),
        results_ptr: to_box_ptr(results),
        vars_ptr: to_box_ptr(vars),
        editor_objects_ptr: to_box_ptr(editor_objects),
        allocator: to_box_ptr(Bump::with_capacity(MAX_LINE_COUNT * 120)),
    })
}

#[wasm_bindgen]
pub fn get_command_buffer_ptr() -> *const u8 {
    unsafe {
        return RENDER_COMMAND_BUFFER.as_ptr();
    }
}

fn to_box_ptr<T>(t: T) -> u32 {
    let ptr = Box::into_raw(Box::new(t)) as u32;
    ptr
}

#[wasm_bindgen]
pub fn alt_key_released(app_ptr: u32) {
    let rb = AppPointers::mut_render_bucket(app_ptr);

    AppPointers::mut_app(app_ptr).alt_key_released(
        AppPointers::units(app_ptr),
        AppPointers::allocator(app_ptr),
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_results(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_editor_objects(app_ptr),
        rb,
        unsafe { &mut RESULT_BUFFER },
    );
}

#[wasm_bindgen]
pub fn handle_resize(app_ptr: u32, new_client_width: usize) {
    AppPointers::mut_app(app_ptr).handle_resize(
        new_client_width,
        AppPointers::mut_editor_objects(app_ptr),
        AppPointers::units(app_ptr),
        AppPointers::allocator(app_ptr),
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_results(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_render_bucket(app_ptr),
        unsafe { &mut RESULT_BUFFER },
    );
}

#[wasm_bindgen]
pub fn get_compressed_encoded_content(app_ptr: u32) -> String {
    let app = AppPointers::mut_app(app_ptr);
    let content = app.get_line_ref_normalized_content();
    {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::prelude::*;
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(content.as_bytes()).expect("");
        let compressed_encoded = e
            .finish()
            .map(|it| base64::encode_config(it, base64::URL_SAFE_NO_PAD));
        return compressed_encoded.unwrap_or("".to_owned());
    }
}

#[wasm_bindgen]
pub fn set_compressed_encoded_content(app_ptr: u32, compressed_encoded: String) {
    let content = {
        use flate2::write::ZlibDecoder;
        use std::io::prelude::*;

        let decoded = base64::decode_config(&compressed_encoded, base64::URL_SAFE_NO_PAD);
        decoded.ok().and_then(|it| {
            let mut writer = Vec::with_capacity(compressed_encoded.len() * 2);
            let mut z = ZlibDecoder::new(writer);
            z.write_all(&it[..]).expect("");
            writer = z.finish().unwrap_or(Vec::new());
            String::from_utf8(writer).ok()
        })
    };
    if let Some(content) = content {
        let app = AppPointers::mut_app(app_ptr);

        app.set_normalized_content(
            &content.trim_end(),
            AppPointers::units(app_ptr),
            AppPointers::allocator(app_ptr),
            AppPointers::mut_tokens(app_ptr),
            AppPointers::mut_results(app_ptr),
            AppPointers::mut_vars(app_ptr),
            AppPointers::mut_editor_objects(app_ptr),
            AppPointers::mut_render_bucket(app_ptr),
            unsafe { &mut RESULT_BUFFER },
        );
    }
}

#[wasm_bindgen]
pub fn handle_time(app_ptr: u32, now: u32) -> bool {
    let rerender_needed = AppPointers::mut_app(app_ptr).handle_time(
        now,
        AppPointers::units(app_ptr),
        AppPointers::allocator(app_ptr),
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_results(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_editor_objects(app_ptr),
        AppPointers::mut_render_bucket(app_ptr),
        unsafe { &mut RESULT_BUFFER },
    );

    return rerender_needed;
}

#[wasm_bindgen]
pub fn handle_drag(app_ptr: u32, x: usize, y: usize) -> bool {
    return AppPointers::mut_app(app_ptr).handle_drag(
        x,
        CanvasY::new(y as isize),
        AppPointers::mut_editor_objects(app_ptr),
        AppPointers::units(app_ptr),
        AppPointers::allocator(app_ptr),
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_results(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_render_bucket(app_ptr),
        unsafe { &mut RESULT_BUFFER },
    );
}

#[wasm_bindgen]
pub fn get_allocated_bytes_count(app_ptr: u32) -> usize {
    return AppPointers::allocator(app_ptr).allocated_bytes();
}

#[wasm_bindgen]
pub fn handle_click(app_ptr: u32, x: usize, y: usize) {
    AppPointers::mut_app(app_ptr).handle_click(
        x,
        CanvasY::new(y as isize),
        AppPointers::mut_editor_objects(app_ptr),
        AppPointers::units(app_ptr),
        AppPointers::allocator(app_ptr),
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_results(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_render_bucket(app_ptr),
        unsafe { &mut RESULT_BUFFER },
    );
}

#[wasm_bindgen]
pub fn handle_wheel(app_ptr: u32, dir: usize) -> bool {
    return AppPointers::mut_app(app_ptr).handle_wheel(
        dir,
        AppPointers::mut_editor_objects(app_ptr),
        AppPointers::units(app_ptr),
        AppPointers::allocator(app_ptr),
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_results(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_render_bucket(app_ptr),
        unsafe { &mut RESULT_BUFFER },
    );
}

#[wasm_bindgen]
pub fn handle_mouse_up(app_ptr: u32) {
    AppPointers::mut_app(app_ptr).handle_mouse_up();
}

#[wasm_bindgen]
pub fn get_clipboard_text(app_ptr: u32) -> String {
    let app = AppPointers::app(app_ptr);
    return app.editor.clipboard.clone();
}

#[wasm_bindgen]
pub fn get_selected_text_and_clear_app_clipboard(app_ptr: u32) -> Option<String> {
    return AppPointers::mut_app(app_ptr).get_selected_text_and_clear_app_clipboard();
}

#[wasm_bindgen]
pub fn handle_paste(app_ptr: u32, input: String) {
    AppPointers::mut_app(app_ptr).handle_paste(
        input,
        AppPointers::units(app_ptr),
        AppPointers::allocator(app_ptr),
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_results(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_editor_objects(app_ptr),
        AppPointers::mut_render_bucket(app_ptr),
        unsafe { &mut RESULT_BUFFER },
    );
}

// HACK: there is a memory leak in the app, so call this method every N second
// which clears the allocator, but it is only possible if after it everything is reparsed
// and rerendered.
// The reasons is that basically I could not solve that Tokens and RenderCommands both refer
// to the editor's canvas as a slice because of Rust's borrow checker, so I had to allocate them
// separately, and that separate allocation should be freed.
// But unfortunately the allocation from parsing and rendering are mixed, so
// we can't just free it up anywhere.
// It would be possible to free it up in the lib, but for that we would need a mut allocator,
// and again, Rust's borrow checker does not like it.
#[wasm_bindgen]
pub fn reparse_everything(app_ptr: u32) {
    AppPointers::mut_allocator(app_ptr).reset();
    let app = AppPointers::mut_app(app_ptr);

    app.reparse_everything(
        AppPointers::allocator(app_ptr),
        AppPointers::units(app_ptr),
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_results(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_editor_objects(app_ptr),
        AppPointers::mut_render_bucket(app_ptr),
        unsafe { &mut RESULT_BUFFER },
    );
}

#[wasm_bindgen]
pub fn render(app_ptr: u32) {
    send_render_commands_to_js(AppPointers::mut_render_bucket(app_ptr));
}

#[wasm_bindgen]
pub fn get_selected_rows_with_results(app_ptr: u32) -> String {
    let app = AppPointers::mut_app(app_ptr);
    let units = AppPointers::units(app_ptr);
    return app.copy_selected_rows_with_result_to_clipboard(
        units,
        AppPointers::mut_render_bucket(app_ptr),
        unsafe { &mut RESULT_BUFFER },
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_results(app_ptr),
    );
}

#[wasm_bindgen]
pub fn get_plain_content(app_ptr: u32) -> String {
    let app = AppPointers::mut_app(app_ptr);
    app.editor_content.get_content()
}

#[wasm_bindgen]
pub fn get_cursor(app_ptr: u32) -> String {
    let app = AppPointers::mut_app(app_ptr);
    let sel = app.editor.get_selection();
    format!("{:?}", sel)
}

#[wasm_bindgen]
pub fn handle_input(app_ptr: u32, input: u32, modifiers: u8) -> bool {
    let modifiers = InputModifiers {
        shift: modifiers & 1 != 0,
        ctrl: modifiers & 2 != 0,
        alt: modifiers & 4 != 0,
    };
    let input = match input {
        1 => EditorInputEvent::Backspace,
        2 => EditorInputEvent::Enter,
        3 => EditorInputEvent::Home,
        4 => EditorInputEvent::End,
        5 => EditorInputEvent::Up,
        6 => EditorInputEvent::Down,
        7 => EditorInputEvent::Left,
        8 => EditorInputEvent::Right,
        9 => EditorInputEvent::Del,
        10 => EditorInputEvent::Esc,
        11 => EditorInputEvent::PageUp,
        12 => EditorInputEvent::PageDown,
        13 => EditorInputEvent::Tab,
        _ => {
            let ch = std::char::from_u32(input);
            if let Some(ch) = ch {
                EditorInputEvent::Char(ch)
            } else {
                return false;
            }
        }
    };
    let app = AppPointers::mut_app(app_ptr);
    let modif = app.handle_input(
        input,
        modifiers,
        AppPointers::allocator(app_ptr),
        AppPointers::units(app_ptr),
        AppPointers::mut_tokens(app_ptr),
        AppPointers::mut_results(app_ptr),
        AppPointers::mut_vars(app_ptr),
        AppPointers::mut_editor_objects(app_ptr),
        AppPointers::mut_render_bucket(app_ptr),
        unsafe { &mut RESULT_BUFFER },
    );

    return modif.is_some();
}

pub const COLOR_TEXT: u32 = 0x595959_FF;
pub const COLOR_RESULTS: u32 = 0x000000_FF;
pub const COLOR_NUMBER: u32 = 0xF92672_FF;
pub const COLOR_NUMBER_ERROR: u32 = 0xFF0000_FF;
pub const COLOR_OPERATOR: u32 = 0x000000_FF;
pub const COLOR_UNIT: u32 = 0x000BED_FF;
pub const COLOR_VARIABLE: u32 = 0x269d94_FF;

fn send_render_commands_to_js(render_buckets: &RenderBuckets) {
    use byteorder::{LittleEndian, WriteBytesExt};
    use std::io::Cursor;
    let mut js_command_buffer = unsafe { Cursor::new(&mut RENDER_COMMAND_BUFFER[..]) };

    fn write_utf8_text_command(
        js_command_buffer: &mut Cursor<&mut [u8]>,
        text: &RenderUtf8TextMsg,
    ) {
        js_command_buffer
            .write_u8(OutputMessageCommandId::RenderUtf8Text as u8 + 1)
            .expect("");

        js_command_buffer
            .write_u16::<LittleEndian>(text.row.as_usize() as u16)
            .expect("");
        js_command_buffer
            .write_u16::<LittleEndian>(text.column as u16)
            .expect("");
        js_command_buffer
            .write_u16::<LittleEndian>(text.text.len() as u16)
            .expect("");
        for ch in text.text {
            js_command_buffer
                .write_u32::<LittleEndian>(*ch as u32)
                .expect("");
        }
    }

    fn write_ascii_text_command(
        js_command_buffer: &mut Cursor<&mut [u8]>,
        text: &RenderAsciiTextMsg,
    ) {
        js_command_buffer
            .write_u8(OutputMessageCommandId::RenderAsciiText as u8 + 1)
            .expect("");

        // TODO: these don't must to be u16 (row, column), maybe the column
        js_command_buffer
            .write_u16::<LittleEndian>(text.row.as_usize() as u16)
            .expect("");
        js_command_buffer
            .write_u16::<LittleEndian>(text.column as u16)
            .expect("");
        js_command_buffer
            .write_u16::<LittleEndian>(text.text.len() as u16)
            .expect("");
        for ch in text.text {
            js_command_buffer.write_u8(*ch).expect("");
        }
    }

    fn write_string_command(js_command_buffer: &mut Cursor<&mut [u8]>, text: &RenderStringMsg) {
        js_command_buffer
            .write_u8(OutputMessageCommandId::RenderUtf8Text as u8 + 1)
            .expect("");

        js_command_buffer
            .write_u16::<LittleEndian>(text.row.as_usize() as u16)
            .expect("");
        js_command_buffer
            .write_u16::<LittleEndian>(text.column as u16)
            .expect("");
        js_command_buffer
            .write_u16::<LittleEndian>(text.text.chars().count() as u16)
            .expect("");
        for ch in text.text.chars() {
            js_command_buffer
                .write_u32::<LittleEndian>(ch as u32)
                .expect("");
        }
    }

    fn write_command(js_command_buffer: &mut Cursor<&mut [u8]>, command: &OutputMessage) {
        match command {
            OutputMessage::RenderUtf8Text(text) => {
                write_utf8_text_command(js_command_buffer, text);
            }
            OutputMessage::SetStyle(style) => {
                js_command_buffer
                    .write_u8(OutputMessageCommandId::SetStyle as u8 + 1)
                    .expect("");
                js_command_buffer.write_u8(*style as u8).expect("");
            }
            OutputMessage::SetColor(color) => {
                js_command_buffer
                    .write_u8(OutputMessageCommandId::SetColor as u8 + 1)
                    .expect("");
                js_command_buffer
                    .write_u32::<LittleEndian>(*color)
                    .expect("");
            }
            OutputMessage::RenderRectangle { x, y, w, h } => {
                js_command_buffer
                    .write_u8(OutputMessageCommandId::RenderRectangle as u8 + 1)
                    .expect("");
                js_command_buffer.write_u8(*x as u8).expect("");
                js_command_buffer.write_u8(y.as_usize() as u8).expect("");
                js_command_buffer.write_u8(*w as u8).expect("");
                js_command_buffer.write_u8(*h as u8).expect("");
            }
            OutputMessage::RenderChar(x, y, ch) => {
                js_command_buffer
                    .write_u8(OutputMessageCommandId::RenderChar as u8 + 1)
                    .expect("");
                js_command_buffer.write_u8(*x as u8).expect("");
                js_command_buffer.write_u8(*y as u8).expect("");
                js_command_buffer
                    .write_u32::<LittleEndian>(*ch as u32)
                    .expect("");
            }
            OutputMessage::RenderString(text) => {
                write_string_command(js_command_buffer, text);
            }
            OutputMessage::RenderAsciiText(text) => {
                write_ascii_text_command(js_command_buffer, text);
            }
            OutputMessage::PulsingRectangle {
                x,
                y,
                w,
                h,
                start_color,
                end_color,
                animation_time,
            } => {
                js_command_buffer
                    .write_u8(OutputMessageCommandId::PulsingRectangle as u8 + 1)
                    .expect("");
                js_command_buffer.write_u8(*x as u8).expect("");
                js_command_buffer.write_u8(y.as_usize() as u8).expect("");
                js_command_buffer.write_u8(*w as u8).expect("");
                js_command_buffer.write_u8(*h as u8).expect("");
                js_command_buffer
                    .write_u32::<LittleEndian>(*start_color)
                    .expect("");
                js_command_buffer
                    .write_u32::<LittleEndian>(*end_color)
                    .expect("");
                js_command_buffer
                    .write_u16::<LittleEndian>(animation_time.as_millis() as u16)
                    .expect("");
            }
        }
    }

    fn write_commands(js_command_buffer: &mut Cursor<&mut [u8]>, commands: &[RenderUtf8TextMsg]) {
        for text in commands {
            write_utf8_text_command(js_command_buffer, text);
        }
    }

    for command in &render_buckets.clear_commands {
        write_command(&mut js_command_buffer, command);
    }

    for command in &render_buckets.custom_commands[Layer::BehindText as usize] {
        write_command(&mut js_command_buffer, command);
    }

    for command in &render_buckets.custom_commands[Layer::Text as usize] {
        write_command(&mut js_command_buffer, command);
    }

    if !render_buckets.utf8_texts.is_empty() {
        write_command(&mut js_command_buffer, &OutputMessage::SetColor(COLOR_TEXT));
        write_commands(&mut js_command_buffer, &render_buckets.utf8_texts);
    }

    if !render_buckets.ascii_texts.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(COLOR_RESULTS),
        );
        for text in &render_buckets.ascii_texts {
            write_ascii_text_command(&mut js_command_buffer, text);
        }
    }

    if !render_buckets.numbers.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(COLOR_NUMBER),
        );
        write_commands(&mut js_command_buffer, &render_buckets.numbers);
    }
    if !render_buckets.number_errors.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(COLOR_NUMBER_ERROR),
        );
        write_commands(&mut js_command_buffer, &render_buckets.number_errors);
    }

    if !render_buckets.units.is_empty() {
        write_command(&mut js_command_buffer, &OutputMessage::SetColor(COLOR_UNIT));
        write_commands(&mut js_command_buffer, &render_buckets.units);
    }

    if !render_buckets.operators.is_empty() || !render_buckets.line_ref_results.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(COLOR_OPERATOR),
        );
        write_commands(&mut js_command_buffer, &render_buckets.operators);
        for command in &render_buckets.line_ref_results {
            write_string_command(&mut js_command_buffer, command);
        }
    }

    if !render_buckets.variable.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(COLOR_VARIABLE),
        );
        write_commands(&mut js_command_buffer, &render_buckets.variable);
    }

    for command in &render_buckets.custom_commands[Layer::AboveText as usize] {
        write_command(&mut js_command_buffer, command);
    }

    js_command_buffer.write_u8(0).expect("");
}
