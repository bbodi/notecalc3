const text_decoder = new TextDecoder();

let NoteCalcInstance = function(
        id,
        wasm, 
        wasm_bindgen, 
        dom,
        configs
) {
    const canvas = document.createElement("canvas");
    dom.appendChild(canvas);
    
    
    canvas.addEventListener('cut', (event) => {});
   
    let notecalc_data;
    if (id === 'home') {
        notecalc_data = JSON.parse(localStorage.getItem(id));
    } else {
        notecalc_data = {
            encoded_content: '',
            theme: configs.theme
        };
    }
    if (!notecalc_data) {
        notecalc_data = {
            encoded_content: '',
            theme: configs.theme
        };
        if (id) {
            localStorage.setItem(id, JSON.stringify(notecalc_data));
        }
    }
    if (notecalc_data.theme === undefined) {
        notecalc_data.theme = configs.theme;
    }
    configs.theme = notecalc_data.theme;
    if (configs.theme === THEME_LIGHT) {
        dom.style.backgroundColor = "#FFFFFF";
    } else {
        dom.style.backgroundColor = "#282a36";
    }
    if (notecalc_data.result_prec === undefined) {
        notecalc_data.result_prec = configs.decimal_places;
    }
    if (notecalc_data.show_line_numbers === undefined) {
        notecalc_data.show_line_numbers = configs.show_line_numbers;
    }
    if (notecalc_data.fancy_chars_enabled === undefined) {
        notecalc_data.fancy_chars_enabled = false;
    }
    if (id === 'home') {
        localStorage.setItem(id, JSON.stringify(notecalc_data));
    }

    const command_buffer_ptr = wasm.get_command_buffer_ptr();
    let instance = {
        id: id,
        app_ptr: 0,
        command_buffer_ptr: command_buffer_ptr,
        parent_dom: dom,
        font_width: 0,
        FONT_HEIGHT:  configs.font_size,
        FONT_VERT_PADDING:  configs.font_vertical_padding,
        line_height:  configs.font_size + 2 * configs.font_vertical_padding,
        visible_line_count: configs.visible_line_count,
        canvas_dirty: false,
        can_scroll: true,
        CLIENT_WIDTH_IN_CHARS: 0,
        CLIENT_HEIGHT_IN_CHARS: 0,
        app_initialized: false,
        content_was_modified: false,
        last_drag_event: {x: -1, y: -1},
        single_pulsing_rects: [],
        repeating_pulsing_rects: [],
        measure_start: 0,
        next_measure_tick: 0,
        theme: configs.theme,
        current_second_info: {
            render_time: 0,
            render_count: 0,
            command_count: 0,
        },
        per_second_info: {
            render_time: 0,
            render_count: 0,
            command_count: 0,
        },
        canvas: canvas,
        ctx: canvas.getContext("2d"),
        init_canvas() {
            this.parent_dom.setAttribute('style', 'height:' + (this.visible_line_count * this.line_height) + 'px');
            let size = this.calc_client_width();
            this.set_client_width(size);
                
            // some browser extension add a class to the body, so rmeove them
            let body = document.getElementsByTagName('body').item(0);
            body.style = "margin: 0";

            this.ctx.font = this.FONT_HEIGHT + NORMAL_FONT;
            this.ctx.textBaseline = "bottom";
        },
        add_event_listeners(dom) { 
            const self = this;        
            this.canvas.addEventListener('resize',       function(e) { self.resizeCanvas(e)   ;}, false);
            this.canvas.addEventListener('mousedown',    function(e) { self.on_mouse_down(e)  ;});
            this.canvas.addEventListener('mouseup',      function(e) { self.on_mouse_up(e)    ;});
            this.canvas.addEventListener('mousemove',    function(e) { self.on_drag(e)        ;});
            this.canvas.addEventListener('wheel',        function(e) { self.on_wheel(e)       ;});
            this.canvas.addEventListener('focus',        function(e) { self.activate()        ;});
            this.canvas.addEventListener('contextmenu',  function(e) { e.preventDefault()     ;});

            dom.addEventListener('keydown', function (event) {
                console.log(event, 'keydown');
                self.do_key_down(event);
                event.stopPropagation();
            });
            dom.addEventListener('keyup', (event) => {
                console.log(event, 'keyup');
                self.doKeyUp(event);
            }, true);
        
            {
                // TODO what are these two used for? The first one is not called
                //      Maybe for mobiles?
                //hidden_input.addEventListener('input', function (_e) {
                //    let key = hidden_input.value;
                //    if (key.length > 0) {
                //        hidden_input.value = '';
                //        send_input_event_to_notecalc(key.codePointAt(0), 0);
                //        render_for_reason('input');
                //    }
                //});
                //hidden_input.addEventListener('focusout', function() {
                //    window.setTimeout(function() {
                //        document.getElementById('notecalc_input').focus();
                //    }, 10);
                //});
        
                let start_touch_y = 0;
                this.canvas.addEventListener('touchstart', function (event) {
                    let touch = event.changedTouches[0];
                    start_touch_y = touch.pageY;
                });
                this.canvas.addEventListener('touchmove', function (event) {
                    event.preventDefault();
                    let touch = event.changedTouches[0];
                    let deltaY = (((start_touch_y - touch.pageY) | 0) / self.line_height) | 0;
                    if (deltaY !== 0) {
                        start_touch_y = touch.pageY;
                        const e = {
                            deltaY: deltaY,
                            preventDefault: function () {
                            }
                        };
                        self.on_wheel(e);
                    }
                });
            }
        },
        find_font_width() {
            const ctx = this.ctx;
            if (is_mobile) {
                // on mobile, find a font size which allows only 45 chars in a row
                this.font_width = 0;
                while ((font_width * 45) < window.innerWidth) {
                    ctx.font = FONT_HEIGHT + BOLD_FONT;
                    ctx.textBaseline = "bottom";
                    this.font_width = ctx.measureText('a').width;
                    this.FONT_HEIGHT += 1;
                }
            } else {
                ctx.font = this.FONT_HEIGHT + BOLD_FONT;
                ctx.textBaseline = "bottom";
                this.font_width = ctx.measureText('a').width;
            }
            this.font_width = Math.ceil(this.font_width);
    
            let max_h = 0;
            const find_max_char_h = function (ch) {
                let m = ctx.measureText(ch);
                let font_height = m.actualBoundingBoxAscent + m.actualBoundingBoxDescent;
                console.log(ch, ', code =', ch.charCodeAt(0), ', height =', font_height);
                if (font_height > max_h) {
                    max_h = font_height;
                }
            };
            find_max_char_h('▏');
            find_max_char_h('⎡');
            find_max_char_h('⎤');
            find_max_char_h('⎫');
            this.line_height = max_h + 2 * this.FONT_VERT_PADDING;
            console.log('line_height', this.line_height);
        },
        calc_client_width() {
            let width = ((this.parent_dom.clientWidth / this.font_width) | 0) * this.font_width;
            let height = ((this.parent_dom.clientHeight / this.line_height) | 0) * this.line_height;
            if (is_mobile) {
                height /= 2;
            }
            return {
                width_px: width,
                height_px: height,
                width_char: (width / this.font_width) | 0,
                height_char: (height / this.line_height) | 0,
            };
        },
        set_client_width(size) {
            let dpi = window.devicePixelRatio;
            if (dpi <= 2) {
                dpi = 2;
            }
            dpi = Math.ceil(dpi);
            console.log(
                'window.devicePixelRatio is', window.devicePixelRatio,
                'canvas scale is', dpi
            );
            // Make sure that the height and width are always even numbers.
            // otherwise, the page renders blurry on some platforms.
            // See https://github.com/emilk/egui/issues/103
            function round_to_even(v) {
                return Math.round(v / 2.0) * 2.0;
            }
            this.canvas.width = round_to_even(size.width_px * dpi);
            this.canvas.height = round_to_even(size.height_px * dpi);
            this.ctx.scale(dpi, dpi);
            this.canvas.style.width = round_to_even(size.width_px) + 'px';
            this.canvas.style.height = round_to_even(size.height_px) + 'px';
        
            // WebGL Canvas
            //webgl_canvas.width = size.width_px * dpi;
            //webgl_canvas.height = size.height_px * dpi;
            //webgl_canvas.style.width = size.width_px + 'px';
            //webgl_canvas.style.height = size.height_px + 'px';
        
            this.CLIENT_WIDTH_IN_CHARS = size.width_char;
            this.CLIENT_HEIGHT_IN_CHARS = size.height_char;
            if (this.CLIENT_WIDTH_IN_CHARS * this.font_width < size.width_px) {
                this.CLIENT_WIDTH_IN_CHARS += 1;
            }
        },
        resizeCanvas() {
            const new_size = this.calc_client_width();
            // if only the height has changed
            if (is_mobile &&
                this.app_initialized &&
                (this.CLIENT_HEIGHT_IN_CHARS > new_size.height_char) &&
                (this.CLIENT_WIDTH_IN_CHARS === new_size.width_char)
            ) {
                // virtual keyboard can resize the canvas, don't allow it
                return;
            }
            this.set_client_width(new_size);
        
            this.parent_dom.style.width  = (this.canvas.width  + 2 * this.font_width)  + 'px';
            this.parent_dom.style.height = (this.canvas.height + 2 * this.line_height) + 'px';
        
            this.init_canvas();
            this.measure_start = Date.now();
            wasm_bindgen.handle_resize(this.app_ptr, this.CLIENT_WIDTH_IN_CHARS);
            this.render_for_reason('resize canvas');
        },
        activate() {
            // active_notecalc_editor is a global variable
            wasm_bindgen.set_focus(active_notecalc_editor.app_ptr, false);
            active_notecalc_editor.parent_dom.tabIndex = 10;
            active_notecalc_editor.render_for_reason('lose focus');

            active_notecalc_editor = this;
            active_notecalc_editor.parent_dom.tabIndex = 1;
            wasm_bindgen.set_focus(active_notecalc_editor.app_ptr, true);
        },
        on_mouse_down(e) {
            if (e.buttons === 1) {
                this.activate();
                const char_x = (e.offsetX) / this.font_width;
                const char_y = (e.offsetY) / this.line_height;
                const now = Date.now();
                this.measure_start = now;
                wasm_bindgen.handle_click(this.app_ptr, now, char_x | 0, char_y | 0);
                this.render_for_reason('click');
            }
        },
        on_drag(e) {
            const char_x = ((e.offsetX) / this.font_width) | 0;
            const char_y = ((e.offsetY) / this.line_height) | 0;
            if (e.buttons === 1) { //dragged with left mouse button
                const need_update = this.last_drag_event.x !== char_x || this.last_drag_event.y !== char_y;
                this.measure_start = Date.now();
                if (need_update && wasm_bindgen.handle_drag(this.app_ptr, char_x, char_y)) {
                    this.render_for_reason('dragging');
                    this.last_drag_event = {x: char_x, y: char_y};
                }
            } else {
                this.measure_start = Date.now();

                this.canvas.style.cursor = cursor_styles[wasm_bindgen.handle_mouse_move(this.app_ptr, char_x, char_y)];
                //webgl_canvas.style.cursor = cursor_styles[wasm_bindgen.handle_mouse_move(app_ptr, char_x, char_y)];
                this.render_for_reason('mouse move');
            }
        
        },
        doKeyUp(e) {
            if (e.key === 'Alt') {
                this.measure_start = Date.now();
                wasm_bindgen.alt_key_released(this.app_ptr);
                this.set_content_was_modified();
                this.render_for_reason('key up');
                e.preventDefault();
                return false;
            }
        },
        set_content_was_modified() {
            this.content_was_modified = true;
            if (!is_mobile && active_tab_btn_dom) {
                active_tab_btn_dom.className = 'unsaved tablinks';
                active_tab_btn_dom.children.item(0).className = 'nav-link active';
            }
        },
        send_input_event_to_notecalc(key, modifiers) {
            console.log('key ' + key);
            let now = Date.now();
            this.measure_start = now;
            console.log('handled', modifiers, key);
            let content_was_modified = wasm_bindgen.handle_input(this.app_ptr, now, key, modifiers);
            if (content_was_modified) {
                this.set_content_was_modified();
                const maybe_title = wasm_bindgen.get_and_clear_title_changed(this.app_ptr);
                if (maybe_title !== null && maybe_title !== undefined) {
                    //tab
                    //this.set_active_tabname(maybe_title);
                }
            }
        },
        do_key_down(event) {
            if (event.isComposing || event.keyCode == 229) {
                // https://www.fxsitecompat.dev/en-CA/docs/2018/keydown-and-keyup-events-are-now-fired-during-ime-composition/
                console.log(event, 'composing');
                return;
            }
        
            const modifiers      = modifiers_from_event(event);
            const translated_key = translate_key(event);
            if (translated_key > 0) {
                this.send_input_event_to_notecalc(translated_key, modifiers);
                // The app copied something to its internal clipboard (e.g. the user selected "Copy as plain text" action in the autocompletion box)
                // Let's copy it to the system clipboard, and clear the app's clipboard
                let clipboard_len = wasm_bindgen.get_clipboard_len(this.app_ptr);
                if (clipboard_len !== 0) {
                    let clipboard_ptr = wasm_bindgen.get_clipboard_ptr(this.app_ptr);
                    let clipboard = new Uint8Array(wasm.memory.buffer, clipboard_ptr, clipboard_len);
                    navigator.clipboard.writeText(text_decoder.decode(clipboard)).then(function () {
                        console.log('Copying to clipboard was successful!');
                        wasm_bindgen.clear_clipboard(this.app_ptr);
                    }, function (err) {
                        console.error('Could not copy text: ', err);
                        wasm_bindgen.clear_clipboard(this.app_ptr);
                    });
                }
                event.preventDefault();
            } else if (!event.ctrlKey && event.key.length === 1) {
                this.send_input_event_to_notecalc(event.key.charCodeAt(0), modifiers);
                event.preventDefault();
            }
            this.render_for_reason('keydown');
        },
        render_for_reason(_reason) {
            this.current_second_info.render_count += 1;
            //console.log(this.id, "DIRTY", reason);
            this.canvas_dirty = true;
            //if (this.canvas_dirty) {
                const now = Date.now();
                wasm_bindgen.render(this.app_ptr);
                let command_count = this.redraw(now);
                this.current_second_info.command_count += command_count;
        
                let took = Date.now() - this.measure_start;
                this.current_second_info.render_time += took;
        
                this.canvas_dirty = false;
                this.can_scroll = true;
            //}
        },
        tick(_now) {
            let now = Date.now();
            this.measure_start = Date.now();
            if (wasm_bindgen.handle_time(this.app_ptr, now)) {
                this.render_for_reason('tick');
                if (this.content_was_modified && active_notecalc_editor.id === 'home') {
                    this.save_content();
                }
            } else if (this.single_pulsing_rects.length > 0 || this.repeating_pulsing_rects.length) {
                this.render_for_reason('pulse update');
            }
        
            if (this.next_measure_tick <= now) {
                this.per_second_info.render_time = this.current_second_info.render_time / this.current_second_info.render_count;
                this.per_second_info.command_count = this.current_second_info.command_count / this.current_second_info.render_count;
                this.per_second_info.render_count = this.current_second_info.render_count;
        
                function round(num) {
                    return Math.round((num + Number.EPSILON) * 100) / 100;
                }
        
                console.log('Avg render_time: ', round(this.per_second_info.render_time));
                console.log('Avg command_count: ', round(this.per_second_info.command_count));
                console.log('render_count per second: ', this.per_second_info.render_count);
                this.next_measure_tick = now + 1000;
        
                this.current_second_info.render_time = 0;
                this.current_second_info.render_count = 0;
                this.current_second_info.command_count = 0;
            }
            const self = this;
            requestAnimationFrame(function() {self.tick()});
        },
        save_content() {
            // TODO tab
            let content = wasm_bindgen.get_compressed_encoded_content(this.app_ptr);
            history.replaceState(undefined, undefined, '#' + content)
            let notecalc_data = localStorage.getItem(this.id);
            if (notecalc_data === null) {
                notecalc_data = {
                    encoded_content: '',
                    theme: this.theme
                };
            } else {
                notecalc_data = JSON.parse(notecalc_data);
            }
            notecalc_data.encoded_content = content;
            localStorage.setItem(this.id, JSON.stringify(notecalc_data));
            this.content_was_modified = false;
            if (!is_mobile && active_tab_btn_dom) {
                active_tab_btn_dom.className = 'tablinks nav-item'; // remove 'unsaved' class
                active_tab_btn_dom.children.item(0).className = 'nav-link active';
            }
        },
        on_mouse_up(_e) {
            wasm_bindgen.handle_mouse_up(this.app_ptr);
        },
        on_wheel(evt) {
            if (evt.ctrlKey || this.can_scroll === false) {
                return;
            }
            evt.preventDefault();
            let dir;
            if (evt.deltaY > 0) {
                // down
                dir = 1;
            } else if (evt.deltaY < 0) {
                dir = 0;
            } else {
                return;
            }
            this.measure_start = Date.now();
            if (wasm_bindgen.handle_wheel(this.app_ptr, dir) === true) {
                this.can_scroll = false;
                this.render_for_reason('wheel');
            }
        },
        redraw(now) {
            this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        
            let command_count = -1;
            const command_buffer = create_buffer(wasm.memory.buffer, this.command_buffer_ptr);
        
            const debug_commands = [];
            let is_header = false;
            while (true) {
                command_count += 1;
                const command_id = command_buffer.read_u8();
                if (command_id === 0 || command_id === undefined) {
                    break;
                } else if (command_id === 1) { //  SetStyle
                } else if (command_id === 2) { //  SetColor
                    let a = command_buffer.read_u8();
                    let b = command_buffer.read_u8();
                    let g = command_buffer.read_u8();
                    let r = command_buffer.read_u8();
        
                    let fillStyle = 'rgba(' + r + ', ' + g + ', ' + b + ', ' + a / 255.0 + ')';
                    if (is_debug) {
                        debug_commands.push(fillStyle);
                    }
                    this.ctx.fillStyle = fillStyle;
                } else if (command_id === 3) { // RenderChar
                    let column_i = command_buffer.read_u8();
                    let row_i    = command_buffer.read_u8();
                    let ch       = command_buffer.read_u32();
        
                    let rch = String.fromCharCode(ch);
                    if (is_debug) {
                        debug_commands.push([column_i, row_i, rch]);
                    }
                    this.ctx.fillText(
                        rch,
                        column_i * this.font_width,
                        this.line_height * row_i + this.line_height - this.FONT_VERT_PADDING
                    );
                } else if (command_id === 4) { // RenderUtf8Text | RenderString
                    let row_i    = command_buffer.read_u8();
                    let column_i = command_buffer.read_u8();
                    let len      = command_buffer.read_u8();
                    let j;
                    let str = '';
                    for (j = 0; j < len; j++) {
                        let ch = command_buffer.read_u32();
                        let rch = String.fromCharCode(ch);
                        str += rch;
        
                        this.ctx.fillText(
                            rch,
                            (column_i + j) * this.font_width,
                            this.line_height * row_i + this.line_height - this.FONT_VERT_PADDING
                        );
                    }
                    if (is_debug) {
                        debug_commands.push([column_i, row_i, str]);
                    }
                } else if (command_id === 5) { // RenderAsciiText
                    let row_i    = command_buffer.read_u8();
                    let column_i = command_buffer.read_u8();
                    let len      = command_buffer.read_u8();
                    let j;
                    let str = '';
                    for (j = 0; j < len; j++) {
                        let ch = command_buffer.read_u8();
                        let rch = String.fromCharCode(ch);
                        str += rch;
                        this.ctx.fillText(
                            rch,
                            (column_i + j) * this.font_width,
                            this.line_height * row_i + this.line_height - this.FONT_VERT_PADDING
                        );
                    }
        
                    if (is_debug) {
                        debug_commands.push(str);
                    }
                } else if (command_id === 7) { // RenderRectangle
                    let x = command_buffer.read_u8();
                    let y = command_buffer.read_s16();
                    let w = command_buffer.read_u8();
                    let h = command_buffer.read_u8();
                    if (is_debug) {
                        debug_commands.push([x, y, w, h]);
                    }
                    this.ctx.fillRect(
                        x * this.font_width,
                        y * this.line_height,
                        w * this.font_width,
                        h * this.line_height
                    );
                } else if (command_id === 8) { // FollowingTextCommandsAreHeaders
                    is_header = command_buffer.read_u8() === 1;
                    if (is_header) {
                        this.ctx.font = this.FONT_HEIGHT + BOLD_FONT;
                    } else {
                        this.ctx.font = this.FONT_HEIGHT + NORMAL_FONT;
                    }
                    this.ctx.textBaseline = "bottom";
                    if (is_debug) {
                        debug_commands.push('Set Header: ' + is_header);
                    }
                } else if (command_id === 9) { // RenderUnderline
                    let x = command_buffer.read_u8();
                    let y = command_buffer.read_u8();
                    let w = command_buffer.read_u8();
                    let j;
                    for (j = 0; j < w; j++) {
                        let rch = '▁';
                        this.ctx.fillText(
                            rch,
                            (x + j) * this.font_width,
                            this.line_height * y + this.line_height - this.FONT_VERT_PADDING
                        );
                    }
                } else if (command_id === 10) { // UpdatePulses
                    this.draw_pulsing_rects(this.single_pulsing_rects, this.repeating_pulsing_rects, now);
                } else if (command_id === 101) { // Clear Pulses
                    this.repeating_pulsing_rects.length = 0;
                    this.single_pulsing_rects.length = 0;
                } else if (command_id === 100) { // Pulses
                    let count = command_buffer.read_u8();
        
                    for (let i = 0; i < count; i++) {
                        let x = command_buffer.read_u8();
                        let y = command_buffer.read_u8();
                        let w = command_buffer.read_u8();
                        let h = command_buffer.read_u8();
        
                        const start_color       = command_buffer.read_u32();
                        const end_color         = command_buffer.read_u32();
                        const animation_time_ms = command_buffer.read_u16();
                        const repeat            = command_buffer.read_u8() !== 0;
        
                        let item = {
                            x, y, w, h,
                            start_color,
                            end_color,
                            start_time: now,
                            duration_ms: animation_time_ms,
                            repeat: repeat
                        };
                        if (is_debug) {
                            debug_commands.push(item);
                        }
                        if (repeat) {
                            this.repeating_pulsing_rects.push(item);
                        } else {
                            this.single_pulsing_rects.push(item);
                        }
                    }
                }
            }
            if (is_debug) {
                //console.log(debug_commands);
            }
            return command_count;
        },
        draw_pulsing_rects(single_pulsing_rects, repeating_pulsing_rects, now) {
            const self = this;
            function interp(start, end, x) {
                return start + (end - start) * x;
            }
        
            function draw_pulsing_rect(ctx, pulsing_rect, delta) {
                let r = interp((pulsing_rect.start_color & 0xFF000000) >>> 24, (pulsing_rect.end_color & 0xFF000000) >>> 24, delta);
                let g = interp((pulsing_rect.start_color & 0x00FF0000) >>> 16, (pulsing_rect.end_color & 0x00FF0000) >>> 16, delta);
                let b = interp((pulsing_rect.start_color & 0x0000FF00) >>> 8, (pulsing_rect.end_color & 0x0000FF00) >>> 8, delta);
                let a = interp((pulsing_rect.start_color & 0x000000FF) >>> 0, (pulsing_rect.end_color & 0x000000FF) >>> 0, delta);
                // TODO @perf
                ctx.fillStyle = 'rgba(' + r + ', ' + g + ', ' + b + ', ' + a / 255.0 + ')';
                ctx.fillRect(
                    pulsing_rect.x * self.font_width,
                    pulsing_rect.y * self.line_height,
                    pulsing_rect.w * self.font_width,
                    pulsing_rect.h * self.line_height
                );
            }
        
            for (const pulsing_rect of single_pulsing_rects) {
                let delta = (now - pulsing_rect.start_time) / pulsing_rect.duration_ms;
                if (delta > 1) {
                    single_pulsing_rects.shift();
                    continue;
                }
                draw_pulsing_rect(this.ctx, pulsing_rect, delta);
            }
            for (const pulsing_rect of repeating_pulsing_rects) {
                let delta = (now - pulsing_rect.start_time) / pulsing_rect.duration_ms;
                if (delta > 1) {
                    pulsing_rect.start_time = now;
                    delta = 0;
                }
                draw_pulsing_rect(this.ctx, pulsing_rect, delta);
            }
        },
        paste_from_clipboard(pasted_text) {
            measure_start = Date.now();
            wasm_bindgen.handle_paste(this.app_ptr, pasted_text);
            const maybe_title = wasm_bindgen.get_and_clear_title_changed(this.app_ptr);
            if (maybe_title !== null && maybe_title !== undefined) {
                set_active_tabname(maybe_title);
            }
            this.render_for_reason('paste');
            if (this.id === 'home') {
                this.save_content();
            }
            return false;
        }
    };
    instance.find_font_width();
    instance.init_canvas();
    instance.add_event_listeners(dom);
    instance.app_ptr = wasm.create_app(
        instance.CLIENT_WIDTH_IN_CHARS, 
        instance.CLIENT_HEIGHT_IN_CHARS, 
        notecalc_data.theme, 
        notecalc_data.result_prec,
        notecalc_data.fancy_chars_enabled,
        notecalc_data.show_line_numbers
    );
    instance.app_initialized = true;
    if (id === 'home' && notecalc_data.encoded_content !== null && notecalc_data.encoded_content !== '') {
        wasm_bindgen.set_compressed_encoded_content(
            instance.app_ptr,
            notecalc_data.encoded_content
        );
        history.replaceState(undefined, undefined, '#' + notecalc_data.encoded_content);
    } else if (configs.initial_content) {
        instance.paste_from_clipboard(configs.initial_content);
        wasm_bindgen.set_scroll_y(instance.app_ptr, 0);
    }
    return instance;
}

function translate_key(e) {
    if (e.key === "Backspace") {
        return 1;
    } else if (e.key === "Enter") {
        return 2;
    } else if (e.key === "Home") {
        return 3;
    } else if (e.key === "End") {
        return 4;
    } else if (e.key === "ArrowUp") {
        return 5;
    } else if (e.key === "ArrowDown") {
        return 6;
    } else if (e.key === "ArrowLeft") {
        return 7;
    } else if (e.key === "ArrowRight") {
        return 8;
    } else if (e.key === "Delete") {
        return 9;
    } else if (e.key === "Escape") {
        return 10;
    } else if (e.key === "PageUp") {
        return 11;
    } else if (e.key === "PageDown") {
        return 12;
    } else if (e.key === 'Tab') {
        return 13;
    } else if (e.ctrlKey && e.key === 'z' && !e.shiftKey) {
        // undo
        return 14;
    } else if (e.ctrlKey && e.key === 'z' && e.shiftKey) {
        // redo
        return 15;
    } else if (e.ctrlKey && e.key === 'a') {
        // select all
        return 16;
    } else if (e.ctrlKey && e.key === 'd') {
        // Duplicate line
        return 17;
    } else if (e.ctrlKey && e.key === 'w') {
        // Select word
        return 18;
    } else if (e.ctrlKey && e.key === 'b') {
        // Jump to definition
        return 19;
    } else if (e.ctrlKey && e.key === ' ') {
        // ShowAutocompletion
        return 20;
    } else {
        return 0;
    }
}
function modifiers_from_event(e) {
    let modifiers = 0;
    if (e.ctrlKey) {
        modifiers |= 2;
    }
    if (e.altKey) {
        modifiers |= 4;
    }
    if (e.shiftKey) {
        modifiers |= 1;
    }
    return modifiers;
}
function create_buffer(wasm_memory_buffer, command_buffer_ptr) {
    // wasm_memory_buffer = wasm.memory.buffer
    return {
        u8_command_buffer: new Uint8Array(wasm_memory_buffer, command_buffer_ptr),
        s8_command_buffer: new Int8Array(wasm_memory_buffer, command_buffer_ptr),
        buf_index: 0,
        read_u8: function () {
            const byte_pos = this.buf_index;
            this.buf_index += 1;
            return this.u8_command_buffer[byte_pos];
        },

        read_u16: function () {
            const byte_pos = this.buf_index;
            this.buf_index += 2;
            return (this.u8_command_buffer[byte_pos + 0] << 0) |
                this.u8_command_buffer[byte_pos + 1] << 8;
        },

        read_s16: function () {
            const byte_pos = this.buf_index;
            this.buf_index += 2;
            return (this.s8_command_buffer[byte_pos + 0] << 0) |
                this.s8_command_buffer[byte_pos + 1] << 8;
        },


        read_u32: function () {
            const byte_pos = this.buf_index;
            this.buf_index += 4;
            return (this.u8_command_buffer[byte_pos + 0] << 0) |
                (this.u8_command_buffer[byte_pos + 1] << 8) |
                (this.u8_command_buffer[byte_pos + 2] << 16) |
                (this.u8_command_buffer[byte_pos + 3] << 24);
        }
    };
}
