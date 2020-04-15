use wasm_bindgen::prelude::*;

use notecalc_lib::editor::{Editor, InputKey, InputModifiers};
use notecalc_lib::{NoteCalcApp, OutputMessage, RenderBuckets, RenderTextMsg};
use wasm_bindgen::__rt::std::io::BufWriter;
use wasm_bindgen::__rt::WasmRefCell;

mod utils;
extern crate web_sys;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const WASM_MEMORY_BUFFER_SIZE: usize = 1024 * 100;
static mut WASM_MEMORY_BUFFER: [u8; WASM_MEMORY_BUFFER_SIZE] = [0; WASM_MEMORY_BUFFER_SIZE];

#[wasm_bindgen]
extern "C" {
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn create_app(client_width: usize) -> u32 {
    log(&format!("client_width: {}", client_width));
    let mut app = NoteCalcApp::new(client_width);
    app.handle_input(
        InputKey::Text(
            "

Ez egy szöveg itt:
   12km/h * 45s ^^",
        ),
        InputModifiers::none(),
    );
    to_wasm_ptr(app)
}

#[wasm_bindgen]
pub fn create_command_buffer() -> *const u8 {
    unsafe {
        return WASM_MEMORY_BUFFER.as_ptr();
    }
}

fn to_wasm_ptr<T>(t: T) -> u32 {
    return unsafe {
        let ptr = Box::into_raw(Box::new(WasmRefCell::new(t))) as u32;
        ptr
    };
}

#[wasm_bindgen]
pub fn handle_resize(app_ptr: u32, new_client_width: usize) {
    let app = unsafe { &*(app_ptr as *mut WasmRefCell<NoteCalcApp>) };
    let mut app = app.borrow_mut();
    app.handle_resize(new_client_width);
    // TODO put render into the handle functions?
    let render_buckets = app.render();
    send_render_commands_to_js(&render_buckets);
}

#[wasm_bindgen]
pub fn get_content(app_ptr: u32) -> String {
    let app = unsafe { &*(app_ptr as *mut WasmRefCell<NoteCalcApp>) };
    let mut app = app.borrow_mut();
    return app.get_content();
}

#[wasm_bindgen]
pub fn set_content(app_ptr: u32, text: &str) {
    let app = unsafe { &*(app_ptr as *mut WasmRefCell<NoteCalcApp>) };
    let mut app = app.borrow_mut();
    return app.set_content(text);

    let render_buckets = app.render();
    send_render_commands_to_js(&render_buckets);
}

#[wasm_bindgen]
pub fn handle_time(app_ptr: u32, now: u32) -> bool {
    let app = unsafe { &*(app_ptr as *mut WasmRefCell<NoteCalcApp>) };
    let mut app = app.borrow_mut();
    let rerender_needed = app.editor.handle_tick(now);

    let render_buckets = app.render();
    send_render_commands_to_js(&render_buckets);
    return rerender_needed;
}

#[wasm_bindgen]
pub fn handle_drag(app_ptr: u32, x: usize, y: usize) {
    let app = unsafe { &*(app_ptr as *mut WasmRefCell<NoteCalcApp>) };
    let mut app: &mut NoteCalcApp = &mut app.borrow_mut();
    app.handle_drag(x, y);

    let render_buckets = app.render();
    send_render_commands_to_js(&render_buckets);
}

#[wasm_bindgen]
pub fn handle_click(app_ptr: u32, x: usize, y: usize) {
    let app = unsafe { &*(app_ptr as *mut WasmRefCell<NoteCalcApp>) };
    let mut app: &mut NoteCalcApp = &mut app.borrow_mut();
    app.handle_click(x, y);

    let render_buckets = app.render();
    send_render_commands_to_js(&render_buckets);
}

#[wasm_bindgen]
pub fn get_selected_text(app_ptr: u32) -> Option<String> {
    let app = unsafe { &*(app_ptr as *mut WasmRefCell<NoteCalcApp>) };
    let app = app.borrow();
    return app.get_selected_text();
}

#[wasm_bindgen]
pub fn handle_paste(app_ptr: u32, input: &str) {
    let app = unsafe { &*(app_ptr as *mut WasmRefCell<NoteCalcApp>) };
    let mut app = app.borrow_mut();
    app.handle_input(InputKey::Text(input), InputModifiers::none());

    let render_buckets = app.render();
    send_render_commands_to_js(&render_buckets);
}

#[wasm_bindgen]
pub fn handle_input(app_ptr: u32, input: u32, modifiers: u8) {
    let app = unsafe { &*(app_ptr as *mut WasmRefCell<NoteCalcApp>) };
    let modifiers = InputModifiers {
        shift: modifiers & 1 != 0,
        ctrl: modifiers & 2 != 0,
        alt: modifiers & 4 != 0,
    };
    let mut app = app.borrow_mut();
    let input = match input as u32 {
        1 => InputKey::Backspace,
        2 => InputKey::Enter,
        3 => InputKey::Home,
        4 => InputKey::End,
        5 => InputKey::Up,
        6 => InputKey::Down,
        7 => InputKey::Left,
        8 => InputKey::Right,
        9 => InputKey::Del,
        _ => {
            let ch = std::char::from_u32(input);
            if let Some(ch) = ch {
                InputKey::Char(ch)
            } else {
                return;
            }
        }
    };
    app.handle_input(input, modifiers);
    let mut render_buckets = app.render();

    send_render_commands_to_js(&render_buckets);
}

fn send_render_commands_to_js(render_buckets: &RenderBuckets) {
    // log(&format!("commands: {:?}", commands));
    use byteorder::{LittleEndian, WriteBytesExt};
    use std::io::Write;
    // let mut js_command_buffer = unsafe { BufWriter::new(&mut WASM_MEMORY_BUFFER[..]) };
    use std::io::Cursor;
    let mut js_command_buffer = unsafe { Cursor::new(&mut WASM_MEMORY_BUFFER[..]) };

    fn write_text_command(js_command_buffer: &mut Cursor<&mut [u8]>, text: &RenderTextMsg) {
        js_command_buffer.write_u8(1).expect("");

        js_command_buffer
            .write_u16::<LittleEndian>(text.row as u16)
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

    fn write_command(js_command_buffer: &mut Cursor<&mut [u8]>, command: &OutputMessage) {
        match command {
            OutputMessage::RenderText(text) => {
                write_text_command(js_command_buffer, text);
            }
            OutputMessage::SetStyle(style) => {
                js_command_buffer.write_u8(2).expect("");
                js_command_buffer.write_u8(*style as u8).expect("");
            }
            OutputMessage::SetColor(color) => {
                js_command_buffer.write_u8(3).expect("");
                js_command_buffer.write(color).expect("");
            }
            OutputMessage::RenderRectangle { x, y, w, h } => {
                js_command_buffer.write_u8(4).expect("");
                js_command_buffer.write_u8(*x as u8).expect("");
                js_command_buffer.write_u8(*y as u8).expect("");
                js_command_buffer.write_u8(*w as u8).expect("");
                js_command_buffer.write_u8(*h as u8).expect("");
            }
            OutputMessage::RenderChar(x, y, ch) => {
                js_command_buffer.write_u8(5).expect("");
                js_command_buffer.write_u8(*x as u8).expect("");
                js_command_buffer.write_u8(*y as u8).expect("");
                js_command_buffer
                    .write_u32::<LittleEndian>(*ch as u32)
                    .expect("");
            }
        }
    }

    fn write_commands(js_command_buffer: &mut Cursor<&mut [u8]>, commands: &[RenderTextMsg]) {
        for text in commands {
            write_text_command(js_command_buffer, text);
        }
    }

    for command in &render_buckets.custom_commands[0] {
        write_command(&mut js_command_buffer, command);
    }

    write_command(
        &mut js_command_buffer,
        &OutputMessage::SetColor([0, 0, 0, 255]),
    );

    write_commands(&mut js_command_buffer, &render_buckets.texts);

    write_command(
        &mut js_command_buffer,
        &OutputMessage::SetColor([0xDD, 0x67, 0x18, 255]),
    );
    write_commands(&mut js_command_buffer, &render_buckets.numbers);

    write_command(
        &mut js_command_buffer,
        &OutputMessage::SetColor([0x6A, 0x87, 0x59, 255]),
    );
    write_commands(&mut js_command_buffer, &render_buckets.units);

    write_command(
        &mut js_command_buffer,
        &OutputMessage::SetColor([0x20, 0x99, 0x9D, 255]),
    );
    write_commands(&mut js_command_buffer, &render_buckets.operators);

    write_commands(&mut js_command_buffer, &render_buckets.variable);

    for command in &render_buckets.custom_commands[1] {
        write_command(&mut js_command_buffer, command);
    }

    js_command_buffer.write_u8(0).expect("");
}
