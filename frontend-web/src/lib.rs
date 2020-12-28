// #![deny(
// warnings,
// anonymous_parameters,
// unused_extern_crates,
// unused_import_braces,
// trivial_casts,
// variant_size_differences,
// //missing_debug_implementations,
// trivial_numeric_casts,
// unused_qualifications,
// clippy::all
// )]
#![feature(const_in_array_repeat_expressions)]

use wasm_bindgen::prelude::*;

use crate::utils::set_panic_hook;
use notecalc_lib::borrow_checker_fighter::{to_box_ptr, BorrowCheckerFighter};
use notecalc_lib::editor::editor::{EditorInputEvent, InputModifiers};
use notecalc_lib::helper::*;
use notecalc_lib::{
    Layer, OutputMessage, OutputMessageCommandId, RenderAsciiTextMsg, RenderBuckets,
    RenderStringMsg, RenderUtf8TextMsg,
};

mod utils;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const RENDER_COMMAND_BUFFER_SIZE: usize = 1024 * 100;
static mut RENDER_COMMAND_BUFFER: [u8; RENDER_COMMAND_BUFFER_SIZE] =
    [0; RENDER_COMMAND_BUFFER_SIZE];

#[wasm_bindgen]
extern "C" {
    pub fn js_log(s: &str);
}

#[wasm_bindgen]
pub fn create_app(client_width: usize, client_height: usize) -> usize {
    set_panic_hook();
    js_log(&format!("client_width: {}", client_width));
    js_log(&format!("client_height: {}", client_height));
    return to_box_ptr(BorrowCheckerFighter::new(client_width, client_height));
}

#[wasm_bindgen]
pub fn get_command_buffer_ptr() -> *const u8 {
    unsafe {
        return RENDER_COMMAND_BUFFER.as_ptr();
    }
}

#[wasm_bindgen]
pub fn alt_key_released(app_ptr: usize) {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let rb = bcf.mut_render_bucket();

    bcf.mut_app().alt_key_released(
        bcf.units(),
        bcf.allocator(),
        bcf.mut_tokens(),
        bcf.mut_results(),
        bcf.mut_vars(),
        bcf.mut_editor_objects(),
        rb,
    );
}

#[wasm_bindgen]
pub fn handle_resize(app_ptr: usize, new_client_width: usize) {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    bcf.mut_app().handle_resize(
        new_client_width,
        bcf.mut_editor_objects(),
        bcf.units(),
        bcf.allocator(),
        bcf.mut_tokens(),
        bcf.mut_results(),
        bcf.mut_vars(),
        bcf.mut_render_bucket(),
    );
}

#[wasm_bindgen]
pub fn set_theme(app_ptr: usize, theme_index: usize) {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let app = bcf.mut_app();
    app.set_theme(
        theme_index,
        bcf.mut_editor_objects(),
        bcf.units(),
        bcf.allocator(),
        bcf.mut_tokens(),
        bcf.mut_results(),
        bcf.mut_vars(),
        bcf.mut_render_bucket(),
    );
}

#[wasm_bindgen]
pub fn get_compressed_encoded_content(app_ptr: usize) -> String {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let app = bcf.mut_app();
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
pub fn set_compressed_encoded_content(app_ptr: usize, compressed_encoded: String) {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
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
        let app = bcf.mut_app();

        app.set_normalized_content(
            &content.trim_end(),
            bcf.units(),
            bcf.allocator(),
            bcf.mut_tokens(),
            bcf.mut_results(),
            bcf.mut_vars(),
            bcf.mut_editor_objects(),
            bcf.mut_render_bucket(),
        );
    }
}

#[wasm_bindgen]
pub fn handle_time(app_ptr: usize, now: u32) -> bool {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let rerender_needed = bcf.mut_app().handle_time(
        now,
        bcf.units(),
        bcf.allocator(),
        bcf.mut_tokens(),
        bcf.mut_results(),
        bcf.mut_vars(),
        bcf.mut_editor_objects(),
        bcf.mut_render_bucket(),
    );

    return rerender_needed;
}

#[wasm_bindgen]
pub fn handle_mouse_move(app_ptr: usize, x: usize, y: usize) -> usize {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    return bcf.mut_app().handle_mouse_move(
        x,
        CanvasY::new(y as isize),
        bcf.mut_editor_objects(),
        bcf.units(),
        bcf.allocator(),
        bcf.tokens(),
        bcf.results(),
        bcf.vars(),
        bcf.mut_render_bucket(),
    );
}

#[wasm_bindgen]
pub fn handle_drag(app_ptr: usize, x: usize, y: usize) -> bool {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    return bcf.mut_app().handle_drag(
        x,
        CanvasY::new(y as isize),
        bcf.mut_editor_objects(),
        bcf.units(),
        bcf.allocator(),
        bcf.tokens(),
        bcf.results(),
        bcf.vars(),
        bcf.mut_render_bucket(),
    );
}

#[wasm_bindgen]
pub fn get_allocated_bytes_count(app_ptr: usize) -> usize {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    return bcf.allocator().allocated_bytes();
}

#[wasm_bindgen]
pub fn handle_click(app_ptr: usize, x: usize, y: usize) {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    bcf.mut_app().handle_click(
        x,
        CanvasY::new(y as isize),
        bcf.mut_editor_objects(),
        bcf.units(),
        bcf.allocator(),
        bcf.mut_tokens(),
        bcf.mut_results(),
        bcf.mut_vars(),
        bcf.mut_render_bucket(),
    );
}

#[wasm_bindgen]
pub fn handle_wheel(app_ptr: usize, dir: usize) -> bool {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    return bcf.mut_app().handle_wheel(
        dir,
        bcf.mut_editor_objects(),
        bcf.units(),
        bcf.allocator(),
        bcf.mut_tokens(),
        bcf.mut_results(),
        bcf.mut_vars(),
        bcf.mut_render_bucket(),
    );
}

#[wasm_bindgen]
pub fn handle_mouse_up(app_ptr: usize) {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    bcf.mut_app().handle_mouse_up();
}

#[wasm_bindgen]
pub fn get_clipboard_text(app_ptr: usize) -> String {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let app = bcf.app();
    return app.editor.clipboard.clone();
}

#[wasm_bindgen]
pub fn get_selected_text_and_clear_app_clipboard(app_ptr: usize) -> Option<String> {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    return bcf.mut_app().get_selected_text_and_clear_app_clipboard();
}

#[wasm_bindgen]
pub fn handle_paste(app_ptr: usize, input: String) {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    bcf.mut_app().handle_paste(
        input,
        bcf.units(),
        bcf.allocator(),
        bcf.mut_tokens(),
        bcf.mut_results(),
        bcf.mut_vars(),
        bcf.mut_editor_objects(),
        bcf.mut_render_bucket(),
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
pub fn reparse_everything(app_ptr: usize) {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    bcf.mut_allocator().reset();
    let app = bcf.mut_app();

    app.reparse_everything(
        bcf.allocator(),
        bcf.units(),
        bcf.mut_tokens(),
        bcf.mut_results(),
        bcf.mut_vars(),
        bcf.mut_editor_objects(),
        bcf.mut_render_bucket(),
    );
}

#[wasm_bindgen]
pub fn render(app_ptr: usize) {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let app = bcf.app();
    let bucket = bcf.mut_render_bucket();
    send_render_commands_to_js(bucket, &THEMES[app.render_data.theme_index]);
}

#[wasm_bindgen]
pub fn get_selected_rows_with_results(app_ptr: usize) -> String {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let app = bcf.mut_app();
    let units = bcf.units();
    return app.copy_selected_rows_with_result_to_clipboard(
        units,
        bcf.mut_render_bucket(),
        bcf.mut_tokens(),
        bcf.mut_vars(),
        bcf.mut_results(),
    );
}

#[wasm_bindgen]
pub fn get_plain_content(app_ptr: usize) -> String {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let app = bcf.app();
    app.editor_content.get_content()
}

#[wasm_bindgen]
pub fn get_cursor(app_ptr: usize) -> String {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let app = bcf.app();
    let sel = app.editor.get_selection();
    format!("{:?}", sel)
}

#[wasm_bindgen]
pub fn get_top_of_undo_stack(app_ptr: usize) -> String {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
    let app = bcf.app();
    format!("{:?}", app.editor_content.undo_stack.last())
}

#[wasm_bindgen]
pub fn handle_input(app_ptr: usize, input: u32, modifiers: u8) -> bool {
    let bcf = BorrowCheckerFighter::from_ptr(app_ptr);
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
    let app = bcf.mut_app();
    let modif = app.handle_input(
        input,
        modifiers,
        bcf.allocator(),
        bcf.units(),
        bcf.mut_tokens(),
        bcf.mut_results(),
        bcf.mut_vars(),
        bcf.mut_editor_objects(),
        bcf.mut_render_bucket(),
    );

    return modif.is_some();
}

fn send_render_commands_to_js(render_buckets: &RenderBuckets, theme: &Theme) {
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

    const PULSING_RECTANGLE_ID: usize = 100;
    const CLEAR_PULSING_RECTANGLE_ID: usize = 101;
    fn write_pulse_commands(
        js_command_buffer: &mut Cursor<&mut [u8]>,
        pulses: &[PulsingRectangle],
    ) {
        js_command_buffer
            .write_u8(PULSING_RECTANGLE_ID as u8)
            .expect("");
        js_command_buffer.write_u8(pulses.len() as u8).expect("");
        for p in pulses {
            js_command_buffer.write_u8(p.x as u8).expect("");
            js_command_buffer.write_u8(p.y.as_usize() as u8).expect("");
            js_command_buffer.write_u8(p.w as u8).expect("");
            js_command_buffer.write_u8(p.h as u8).expect("");
            js_command_buffer
                .write_u32::<LittleEndian>(p.start_color)
                .expect("");
            js_command_buffer
                .write_u32::<LittleEndian>(p.end_color)
                .expect("");
            js_command_buffer
                .write_u16::<LittleEndian>(p.animation_time.as_millis() as u16)
                .expect("");
            js_command_buffer.write_u8(p.repeat as u8).expect("");
        }
    }

    fn write_char(js_command_buffer: &mut Cursor<&mut [u8]>, cmd: &RenderChar) {
        js_command_buffer
            .write_u8(OutputMessageCommandId::RenderChar as u8 + 1)
            .expect("");
        js_command_buffer.write_u8(cmd.col as u8).expect("");
        js_command_buffer
            .write_u8(cmd.row.as_usize() as u8)
            .expect("");
        js_command_buffer
            .write_u32::<LittleEndian>(cmd.char as u32)
            .expect("");
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
            OutputMessage::SetColor(color) => write_color(js_command_buffer, *color),
            OutputMessage::RenderRectangle { x, y, w, h } => {
                write_rectangle(js_command_buffer, *x, *y, *w, *h)
            }
            OutputMessage::RenderChar(cmd) => {
                write_char(js_command_buffer, cmd);
            }
            OutputMessage::RenderString(text) => {
                write_string_command(js_command_buffer, text);
            }
            OutputMessage::RenderAsciiText(text) => {
                write_ascii_text_command(js_command_buffer, text);
            }
            OutputMessage::FollowingTextCommandsAreHeaders(b) => {
                js_command_buffer
                    .write_u8(OutputMessageCommandId::FollowingTextCommandsAreHeaders as u8 + 1)
                    .expect("");
                js_command_buffer.write_u8(*b as u8).expect("");
            }
            OutputMessage::RenderUnderline { x, y, w } => {
                js_command_buffer
                    .write_u8(OutputMessageCommandId::RenderUnderline as u8 + 1)
                    .expect("");
                js_command_buffer.write_u8(*x as u8).expect("");
                js_command_buffer.write_u8(y.as_usize() as u8).expect("");
                js_command_buffer.write_u8(*w as u8).expect("");
            }
            OutputMessage::UpdatePulses => {
                js_command_buffer
                    .write_u8(OutputMessageCommandId::UpdatePulses as u8 + 1)
                    .expect("");
            }
        }
    }

    fn write_color(js_command_buffer: &mut Cursor<&mut [u8]>, color: u32) {
        js_command_buffer
            .write_u8(OutputMessageCommandId::SetColor as u8 + 1)
            .expect("");
        js_command_buffer
            .write_u32::<LittleEndian>(color)
            .expect("");
    }

    fn write_rectangle(
        js_command_buffer: &mut Cursor<&mut [u8]>,
        x: usize,
        y: CanvasY,
        w: usize,
        h: usize,
    ) {
        js_command_buffer
            .write_u8(OutputMessageCommandId::RenderRectangle as u8 + 1)
            .expect("");
        js_command_buffer.write_u8(x as u8).expect("");
        js_command_buffer.write_u8(y.as_usize() as u8).expect("");
        js_command_buffer.write_u8(w as u8).expect("");
        js_command_buffer.write_u8(h as u8).expect("");
    }

    fn write_color_rect(js_command_buffer: &mut Cursor<&mut [u8]>, rect: &Rect, color: u32) {
        write_color(js_command_buffer, color);
        write_rectangle(
            js_command_buffer,
            rect.x as usize,
            canvas_y(rect.y as isize),
            rect.w as usize,
            rect.h as usize,
        );
    }

    fn write_commands(js_command_buffer: &mut Cursor<&mut [u8]>, commands: &[RenderUtf8TextMsg]) {
        for text in commands {
            write_utf8_text_command(js_command_buffer, text);
        }
    }

    // Current line highlight bg, it is at the most bottom position, so I can draw rectangles
    // between it and thexts above it
    write_color_rect(
        &mut js_command_buffer,
        &render_buckets.left_gutter_bg,
        theme.left_gutter_bg,
    );
    write_color_rect(
        &mut js_command_buffer,
        &render_buckets.right_gutter_bg,
        theme.result_gutter_bg,
    );
    write_color_rect(
        &mut js_command_buffer,
        &render_buckets.result_panel_bg,
        theme.result_bg_color,
    );
    if let Some((color, rect)) = &render_buckets.scroll_bar {
        write_color_rect(&mut js_command_buffer, rect, *color);
    }
    if let Some(rect) = &render_buckets.current_line_highlight {
        write_color_rect(&mut js_command_buffer, rect, theme.current_line_bg);
    }

    for command in &render_buckets.custom_commands[Layer::BehindText as usize] {
        write_command(&mut js_command_buffer, command);
    }

    if render_buckets.clear_pulses {
        js_command_buffer
            .write_u8(CLEAR_PULSING_RECTANGLE_ID as u8)
            .expect("");
    }
    if NOT(render_buckets.pulses.is_empty()) {
        write_pulse_commands(&mut js_command_buffer, &render_buckets.pulses);
    }
    write_command(&mut js_command_buffer, &OutputMessage::UpdatePulses);

    for command in &render_buckets.custom_commands[Layer::Text as usize] {
        write_command(&mut js_command_buffer, command);
    }

    if !render_buckets.utf8_texts.is_empty() {
        write_command(&mut js_command_buffer, &OutputMessage::SetColor(theme.text));
        write_commands(&mut js_command_buffer, &render_buckets.utf8_texts);
    }

    if !render_buckets.headers.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::FollowingTextCommandsAreHeaders(true),
        );
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(theme.header),
        );
        write_commands(&mut js_command_buffer, &render_buckets.headers);
        write_command(
            &mut js_command_buffer,
            &OutputMessage::FollowingTextCommandsAreHeaders(false),
        );
    }

    if !render_buckets.ascii_texts.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(theme.result_text),
        );
        for text in &render_buckets.ascii_texts {
            write_ascii_text_command(&mut js_command_buffer, text);
        }
    }

    if !render_buckets.numbers.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(theme.number),
        );
        write_commands(&mut js_command_buffer, &render_buckets.numbers);
    }
    if !render_buckets.number_errors.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(theme.number_error),
        );
        write_commands(&mut js_command_buffer, &render_buckets.number_errors);
    }

    if !render_buckets.units.is_empty() {
        write_command(&mut js_command_buffer, &OutputMessage::SetColor(theme.unit));
        write_commands(&mut js_command_buffer, &render_buckets.units);
    }

    if !render_buckets.operators.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(theme.operator),
        );
        write_commands(&mut js_command_buffer, &render_buckets.operators);
    }
    if !render_buckets.parenthesis.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(theme.parenthesis),
        );
        for cmd in &render_buckets.parenthesis {
            write_char(&mut js_command_buffer, cmd);
        }
    }
    if !render_buckets.line_ref_results.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(theme.line_ref_text),
        );
        for command in &render_buckets.line_ref_results {
            write_string_command(&mut js_command_buffer, command);
        }
    }

    if !render_buckets.variable.is_empty() {
        write_command(
            &mut js_command_buffer,
            &OutputMessage::SetColor(theme.variable),
        );
        write_commands(&mut js_command_buffer, &render_buckets.variable);
    }

    for command in &render_buckets.custom_commands[Layer::AboveText as usize] {
        write_command(&mut js_command_buffer, command);
    }

    js_command_buffer.write_u8(0).expect("");
}
