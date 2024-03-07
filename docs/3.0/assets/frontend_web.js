let wasm_bindgen;
(function() {
    const __exports = {};
    let wasm;

    const heap = new Array(32).fill(undefined);

    heap.push(undefined, null, true, false);

function getObject(idx) { return heap[idx]; }

let heap_next = heap.length;

function dropObject(idx) {
    if (idx < 36) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });

cachedTextDecoder.decode();

let cachegetUint8Memory0 = null;
function getUint8Memory0() {
    if (cachegetUint8Memory0 === null || cachegetUint8Memory0.buffer !== wasm.memory.buffer) {
        cachegetUint8Memory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachegetUint8Memory0;
}

function getStringFromWasm0(ptr, len) {
    return cachedTextDecoder.decode(getUint8Memory0().subarray(ptr, ptr + len));
}
/**
* @param {number} client_width
* @param {number} client_height
* @returns {number}
*/
__exports.create_app = function(client_width, client_height) {
    var ret = wasm.create_app(client_width, client_height);
    return ret >>> 0;
};

/**
* @returns {number}
*/
__exports.get_command_buffer_ptr = function() {
    var ret = wasm.get_command_buffer_ptr();
    return ret;
};

/**
* @param {number} app_ptr
*/
__exports.alt_key_released = function(app_ptr) {
    wasm.alt_key_released(app_ptr);
};

/**
* @param {number} app_ptr
* @param {number} new_client_width
*/
__exports.handle_resize = function(app_ptr, new_client_width) {
    wasm.handle_resize(app_ptr, new_client_width);
};

/**
* @param {number} app_ptr
* @param {number} theme_index
*/
__exports.set_theme = function(app_ptr, theme_index) {
    wasm.set_theme(app_ptr, theme_index);
};

let cachegetInt32Memory0 = null;
function getInt32Memory0() {
    if (cachegetInt32Memory0 === null || cachegetInt32Memory0.buffer !== wasm.memory.buffer) {
        cachegetInt32Memory0 = new Int32Array(wasm.memory.buffer);
    }
    return cachegetInt32Memory0;
}
/**
* @param {number} app_ptr
* @returns {string}
*/
__exports.get_compressed_encoded_content = function(app_ptr) {
    try {
        wasm.get_compressed_encoded_content(8, app_ptr);
        var r0 = getInt32Memory0()[8 / 4 + 0];
        var r1 = getInt32Memory0()[8 / 4 + 1];
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_free(r0, r1);
    }
};

let WASM_VECTOR_LEN = 0;

let cachedTextEncoder = new TextEncoder('utf-8');

const encodeString = (typeof cachedTextEncoder.encodeInto === 'function'
    ? function (arg, view) {
    return cachedTextEncoder.encodeInto(arg, view);
}
    : function (arg, view) {
    const buf = cachedTextEncoder.encode(arg);
    view.set(buf);
    return {
        read: arg.length,
        written: buf.length
    };
});

function passStringToWasm0(arg, malloc, realloc) {

    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length);
        getUint8Memory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len);

    const mem = getUint8Memory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }

    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3);
        const view = getUint8Memory0().subarray(ptr + offset, ptr + len);
        const ret = encodeString(arg, view);

        offset += ret.written;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}
/**
* @param {number} app_ptr
* @param {string} compressed_encoded
*/
__exports.set_compressed_encoded_content = function(app_ptr, compressed_encoded) {
    var ptr0 = passStringToWasm0(compressed_encoded, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    var len0 = WASM_VECTOR_LEN;
    wasm.set_compressed_encoded_content(app_ptr, ptr0, len0);
};

/**
* @param {number} app_ptr
* @param {number} now
* @returns {boolean}
*/
__exports.handle_time = function(app_ptr, now) {
    var ret = wasm.handle_time(app_ptr, now);
    return ret !== 0;
};

/**
* @param {number} app_ptr
* @param {number} x
* @param {number} y
* @returns {number}
*/
__exports.handle_mouse_move = function(app_ptr, x, y) {
    var ret = wasm.handle_mouse_move(app_ptr, x, y);
    return ret >>> 0;
};

/**
* @param {number} app_ptr
* @param {number} x
* @param {number} y
* @returns {boolean}
*/
__exports.handle_drag = function(app_ptr, x, y) {
    var ret = wasm.handle_drag(app_ptr, x, y);
    return ret !== 0;
};

/**
* @param {number} app_ptr
* @returns {number}
*/
__exports.get_allocated_bytes_count = function(app_ptr) {
    var ret = wasm.get_allocated_bytes_count(app_ptr);
    return ret >>> 0;
};

/**
* @param {number} app_ptr
* @param {number} x
* @param {number} y
*/
__exports.handle_click = function(app_ptr, x, y) {
    wasm.handle_click(app_ptr, x, y);
};

/**
* @param {number} app_ptr
* @param {number} dir
* @returns {boolean}
*/
__exports.handle_wheel = function(app_ptr, dir) {
    var ret = wasm.handle_wheel(app_ptr, dir);
    return ret !== 0;
};

/**
* @param {number} app_ptr
*/
__exports.handle_mouse_up = function(app_ptr) {
    wasm.handle_mouse_up(app_ptr);
};

/**
* @param {number} app_ptr
* @returns {string}
*/
__exports.get_clipboard_text = function(app_ptr) {
    try {
        wasm.get_clipboard_text(8, app_ptr);
        var r0 = getInt32Memory0()[8 / 4 + 0];
        var r1 = getInt32Memory0()[8 / 4 + 1];
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_free(r0, r1);
    }
};

/**
* @param {number} app_ptr
* @returns {string | undefined}
*/
__exports.get_selected_text_and_clear_app_clipboard = function(app_ptr) {
    wasm.get_selected_text_and_clear_app_clipboard(8, app_ptr);
    var r0 = getInt32Memory0()[8 / 4 + 0];
    var r1 = getInt32Memory0()[8 / 4 + 1];
    let v0;
    if (r0 !== 0) {
        v0 = getStringFromWasm0(r0, r1).slice();
        wasm.__wbindgen_free(r0, r1 * 1);
    }
    return v0;
};

/**
* @param {number} app_ptr
* @param {string} input
*/
__exports.handle_paste = function(app_ptr, input) {
    var ptr0 = passStringToWasm0(input, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    var len0 = WASM_VECTOR_LEN;
    wasm.handle_paste(app_ptr, ptr0, len0);
};

/**
* @param {number} app_ptr
*/
__exports.reparse_everything = function(app_ptr) {
    wasm.reparse_everything(app_ptr);
};

/**
* @param {number} app_ptr
*/
__exports.render = function(app_ptr) {
    wasm.render(app_ptr);
};

/**
* @param {number} app_ptr
* @returns {string}
*/
__exports.get_selected_rows_with_results = function(app_ptr) {
    try {
        wasm.get_selected_rows_with_results(8, app_ptr);
        var r0 = getInt32Memory0()[8 / 4 + 0];
        var r1 = getInt32Memory0()[8 / 4 + 1];
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_free(r0, r1);
    }
};

/**
* @param {number} app_ptr
* @returns {string}
*/
__exports.get_plain_content = function(app_ptr) {
    try {
        wasm.get_plain_content(8, app_ptr);
        var r0 = getInt32Memory0()[8 / 4 + 0];
        var r1 = getInt32Memory0()[8 / 4 + 1];
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_free(r0, r1);
    }
};

/**
* @param {number} app_ptr
* @returns {string}
*/
__exports.get_cursor = function(app_ptr) {
    try {
        wasm.get_cursor(8, app_ptr);
        var r0 = getInt32Memory0()[8 / 4 + 0];
        var r1 = getInt32Memory0()[8 / 4 + 1];
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_free(r0, r1);
    }
};

/**
* @param {number} app_ptr
* @returns {string}
*/
__exports.get_top_of_undo_stack = function(app_ptr) {
    try {
        wasm.get_top_of_undo_stack(8, app_ptr);
        var r0 = getInt32Memory0()[8 / 4 + 0];
        var r1 = getInt32Memory0()[8 / 4 + 1];
        return getStringFromWasm0(r0, r1);
    } finally {
        wasm.__wbindgen_free(r0, r1);
    }
};

/**
* @param {number} app_ptr
* @param {number} input
* @param {number} modifiers
* @returns {boolean}
*/
__exports.handle_input = function(app_ptr, input, modifiers) {
    var ret = wasm.handle_input(app_ptr, input, modifiers);
    return ret !== 0;
};

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

async function load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);

            } catch (e) {
                if (module.headers.get('Content-Type') != 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else {
                    throw e;
                }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);

    } else {

        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };

        } else {
            return instance;
        }
    }
}

async function init(input) {
    if (typeof input === 'undefined') {
        let src;
        if (typeof document === 'undefined') {
            src = location.href;
        } else {
            src = document.currentScript.src;
        }
        input = src.replace(/\.js$/, '_bg.wasm');
    }
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbg_jslog_5575d9e0a489304a = function(arg0, arg1) {
        js_log(getStringFromWasm0(arg0, arg1));
    };
    imports.wbg.__wbg_new_59cb74e423758ede = function() {
        var ret = new Error();
        return addHeapObject(ret);
    };
    imports.wbg.__wbg_stack_558ba5917b466edd = function(arg0, arg1) {
        var ret = getObject(arg1).stack;
        var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        getInt32Memory0()[arg0 / 4 + 1] = len0;
        getInt32Memory0()[arg0 / 4 + 0] = ptr0;
    };
    imports.wbg.__wbg_error_4bb6c2a97407129a = function(arg0, arg1) {
        try {
            console.error(getStringFromWasm0(arg0, arg1));
        } finally {
            wasm.__wbindgen_free(arg0, arg1);
        }
    };
    imports.wbg.__wbindgen_object_drop_ref = function(arg0) {
        takeObject(arg0);
    };

    if (typeof input === 'string' || (typeof Request === 'function' && input instanceof Request) || (typeof URL === 'function' && input instanceof URL)) {
        input = fetch(input);
    }

    const { instance, module } = await load(await input, imports);

    wasm = instance.exports;
    init.__wbindgen_wasm_module = module;

    return wasm;
}

wasm_bindgen = Object.assign(init, __exports);

})();
