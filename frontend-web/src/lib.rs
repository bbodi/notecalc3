use wasm_bindgen::prelude::*;

use notecalc_lib::editor::{Editor, InputKey, InputModifiers};
use notecalc_lib::OutputMessage;
use wasm_bindgen::__rt::std::io::BufWriter;
use wasm_bindgen::__rt::WasmRefCell;

mod utils;
extern crate web_sys;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const WASM_MEMORY_BUFFER_SIZE: usize = 256;
static mut WASM_MEMORY_BUFFER: [u8; WASM_MEMORY_BUFFER_SIZE] = [0; WASM_MEMORY_BUFFER_SIZE];

#[wasm_bindgen]
extern "C" {
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn create_editor() -> u32 {
    to_wasm_ptr(Editor::new())
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
pub fn handle_input(editor_ptr: u32, input: char, modifiers: u8) {
    let editor = unsafe { &*(editor_ptr as *mut WasmRefCell<Editor>) };
    let first_modified_row_index = editor
        .borrow_mut()
        .handle_input(InputKey::Char(input), InputModifiers::none());

    let mut commands = Vec::with_capacity(128);
    let editor = editor.borrow();
    notecalc_lib::renderer::render(
        editor.get_content(),
        first_modified_row_index,
        &mut commands,
    );

    log(&format!("commands: {:?}", commands));
    use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
    let mut js_command_buffer = unsafe { BufWriter::new(&mut WASM_MEMORY_BUFFER[..]) };
    // js_command_buffer.clear();
    js_command_buffer
        .write_u16::<LittleEndian>(commands.len() as u16)
        .expect("");
    for command in &commands {
        match command {
            OutputMessage::RenderText(text) => {
                js_command_buffer.write_u8(0).expect("");

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
            OutputMessage::SetStyle(_) => {}
            OutputMessage::SetColor(_) => {}
        }
    }
}
