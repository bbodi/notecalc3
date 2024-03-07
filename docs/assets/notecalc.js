const THEME_LIGHT = 0;
const THEME_DARK = 1;
const DARK_BG_COLOR = '#282a36';
const DARK_BG_COLOR_BRIGHTER = '#000000';
let notecalc_theme = THEME_LIGHT;

let NORMAL_FONT = 'px JetBrainsMono-Regular';
let BOLD_FONT = 'px JetBrainsMono-ExtraBold';

let active_tab_index = 0;
let active_tab_btn_dom;
let hidden_input;
let tab_index_counter = 0;
let is_mobile = false;
let is_debug = false;

//const webgl_canvas = document.getElementById("webgl_canvas");
//const webgl_ctx = webgl_canvas.getContext("webgl");
//                  Editor | Scrollbar | Right-gutter | Result panel
const cursor_styles = ['text', 'default', 'w-resize', 'default'];

function js_log(str) {
    console.log(str);
}

function create_hidden_input() {
    const el = document.createElement('input');
    el.id = 'notecalc_input'
    el.value = '';
    el.setAttribute('autocorrect', 'off');
    el.setAttribute('autocapitalize', 'off');
    el.style.opacity = '0';
    el.style.zIndex = '-1';
    el.style.position = 'absolute';
    el.style.left = '-9999px';
    el.style.top = '0px';
    // if it is less than 16 px, mobile browsers automatically zoom in to it
    el.style.fontSize = '32px';
    el.size = 1;
    el.autofocus = true;
    if (document.body === null) {
        window.setTimeout(function () {
            create_hidden_input();
        }, 100)
        return;
    }
    document.body.appendChild(el);
    return el;
}

function on_tab_click(index) {
    if (!is_mobile) {
        let tab_btn = document.getElementById('tablink_' + index);
        if (tab_btn === null) {
            return;
        }
        // Set other tabs inactive
        let tablinks = document.getElementsByClassName("tablinks");
        for (i = 0; i < tablinks.length; i++) {
            tablinks[i].className = 'tablinks nav-item';
            tablinks[i].children.item(0).className = 'nav-link';
            tablinks[i].children.item(0).style.backgroundColor = null;
            tablinks[i].children.item(0).style.borderColor = null;
        }
        // ---

        tab_btn.className = 'tablinks nav-item';
        tab_btn.children.item(0).className = 'nav-link active';
        if (notecalc_theme === THEME_DARK) {
            tab_btn.children.item(0).style.backgroundColor = '#A6B6E8';
            tab_btn.children.item(0).style.borderColor = DARK_BG_COLOR_BRIGHTER + ' ' + DARK_BG_COLOR_BRIGHTER + ' ' + DARK_BG_COLOR;
        }
    }
    reload_content(index);
}

function set_active_tabname(name) {
    if (!is_mobile) {
        let tab_btn = document.getElementById('tablink_' + active_tab_index);
        if (tab_btn === null) {
            return;
        }
        tab_btn.children.item(0).firstChild.textContent = name;
        let notecalc_data = JSON.parse(localStorage.getItem('notecalc'));
        notecalc_data.tabs[active_tab_index].name = name;
        localStorage.setItem('notecalc', JSON.stringify(notecalc_data));
    }
}

function on_tab_close(index, dangerous) {
    let notecalc_data = JSON.parse(localStorage.getItem('notecalc'));
    if (index === 0 && notecalc_data.tabs.length === 1) {
        return;
    }
    if (dangerous || window.confirm("Are you sure to delete this tab?")) {
        notecalc_data.tabs.splice(index, 1);
        localStorage.setItem('notecalc', JSON.stringify(notecalc_data));
        document.getElementById('tablink_' + index).remove();

        let tablinks = document.getElementsByClassName("tablinks");
        for (i = 0; i < tablinks.length; i++) {
            let li = tablinks[i];
            li.setAttribute('id', 'tablink_' + i);
            li.setAttribute('onclick', 'on_tab_click(' + i + ')');

            let dom_a = li.children.item(0);
            let dom_a_text = dom_a.childNodes[0];
            dom_a_text.nodeValue = 'Note ' + (i + 1) + ' ';

            let close_btn = dom_a.childNodes[1];
            close_btn.setAttribute('id', 'tablink_close' + i);
            close_btn.setAttribute('onclick', 'on_tab_close(' + i + ')');
        }
        tab_index_counter = tablinks.length;
        reload_content(Math.min(index, notecalc_data.tabs.length - 1));
        active_tab_btn_dom.className = 'tablinks nav-item';
        active_tab_btn_dom.children.item(0).className = 'nav-link active';
    }
    return false;
}

function set_active_tab_index(index) {
    active_tab_index = index;
    if (!is_mobile) {
        active_tab_btn_dom = document.getElementsByClassName('tablinks')[index];
    }
}

function insert_tab_dom(tab_data) {
    let index = tab_index_counter;
    tab_index_counter += 1;

    let li = document.createElement('li');
    li.setAttribute('id', 'tablink_' + index);
    li.className = 'tablinks nav-item';
    li.onclick = function () {
        on_tab_click(index)
    };
    let name = null;
    if (tab_data) {
        name = tab_data.name;
    }
    if (!name) {
        name = "Note " + (index + 1);
    }
    li.innerHTML = "<a class=\"nav-link\" href=\"javascript: void(0)\">"+ name + " " +
        "                <button id=\"tablink_close" + index + "\" type=\"button\" class=\"close tablinks_close\" " +
        "                       aria-label=\"Close\" " +
        "                       onclick=\"on_tab_close(" + index + ")\" " +
        "                       style=\"float: none;font-size: 1.2rem;\">\n" +
        "                    <span aria-hidden=\"true\">&times;</span>\n" +
        "                </button>\n" +
        "            </a>";

    document.getElementById('tabs').insertBefore(li, document.getElementById('tablink_add'));
    return index;
}

function init_tabs_dom() {
    let notecalc_data = JSON.parse(localStorage.getItem('notecalc'));
    for (i = 0; i < notecalc_data.tabs.length; ++i) {
        insert_tab_dom(notecalc_data.tabs[i]);
    }
}

function add_tab_and_switch_to_it(encoded_content) {
    let notecalc_data = JSON.parse(localStorage.getItem('notecalc'));
    let index = insert_tab_dom();
    notecalc_data.tabs.push({
        encoded_content: encoded_content
    });
    localStorage.setItem('notecalc', JSON.stringify(notecalc_data));
    on_tab_click(index);
}


function show_content_in_modal() {
    const str = wasm_bindgen.get_selected_rows_with_results(active_notecalc_editor.app_ptr);
    document.getElementById('export_modal_content').innerText = str;
}

function show_config_modal() {

}

function click_on_util_copy_to_clipboard(e) {
    navigator.clipboard.writeText(e.innerText).then(function () {
        console.log('Async: Copying to clipboard was successful!');
        $(e).tooltip({
            placement: 'top',
            title: 'Copied',
            trigger: 'manual'
        })
        $(e).tooltip('show');
        window.setTimeout(function () {
            $(e).tooltip('hide');
        }, 500)
    }, function (err) {
        console.error('Async: Could not copy text: ', err);
    });
}

function on_theme_btn_click(new_theme) {
    apply_theme(new_theme);
}

function apply_theme(new_theme) {
    notecalc_theme = new_theme;
    let notecalc_data = JSON.parse(localStorage.getItem('notecalc'));
    notecalc_data.theme = notecalc_theme;
    set_theme_dom(notecalc_theme);
    localStorage.setItem('notecalc', JSON.stringify(notecalc_data));
    on_tab_click(active_tab_index);
    wasm_bindgen.set_theme(active_notecalc_editor.app_ptr, notecalc_theme);
    active_notecalc_editor.render_for_reason('THEME changed');
}

function set_theme_dom(new_theme) {
    notecalc_theme = new_theme;
    if (notecalc_theme === THEME_LIGHT) {
        document.getElementById('settings_theme_radio_light').setAttribute('checked', 'checked');
        document.getElementById('settings_theme_radio_dark').removeAttribute('checked');

        document.getElementsByTagName('body')[0].style.backgroundColor = "#FFFFFF";
        document.getElementById('tabs').style.borderBottom = null;
        document.getElementById('body').className = '';
    } else {
        document.getElementById('settings_theme_radio_dark').setAttribute('checked', 'checked');
        document.getElementById('settings_theme_radio_light').removeAttribute('checked');

        document.getElementsByTagName('body')[0].style.backgroundColor = DARK_BG_COLOR;
        document.getElementById('tabs').style.borderBottom = "1px solid " + DARK_BG_COLOR_BRIGHTER;
        document.getElementById('body').className = 'bg-dark';
    }
}

function apply_result_precision(_new_prec) {
    let new_prec = parseInt(_new_prec, 10);
    let notecalc_data = JSON.parse(localStorage.getItem('notecalc'));
    notecalc_data.result_prec = new_prec;
    set_result_precision_dom(new_prec);
    localStorage.setItem('notecalc', JSON.stringify(notecalc_data));
    wasm_bindgen.set_result_precision(active_notecalc_editor.app_ptr, new_prec);
    active_notecalc_editor.render_for_reason('Result precision changed');
}

function set_result_precision_dom(new_prec) {
    document.getElementById('settings_input_prec').setAttribute('value', new_prec);
}

function on_fancy_btn_click(new_value) {
    apply_fancy_chars(new_value === 0);
}

function apply_fancy_chars(enabled) {
    let notecalc_data = JSON.parse(localStorage.getItem('notecalc'));
    notecalc_data.fancy_chars_enabled = enabled;
    set_fancy_char_dom(enabled);
    localStorage.setItem('notecalc', JSON.stringify(notecalc_data));
    wasm_bindgen.set_fancy_chars(active_notecalc_editor.app_ptr, enabled);
    active_notecalc_editor.render_for_reason('Fancy chars changed');
}

function set_fancy_char_dom(enabled) {
    if (enabled) {
        document.getElementById('settings_input_fancy_enabled').setAttribute('checked', 'checked');
        document.getElementById('settings_input_fancy_disabled').removeAttribute('checked');
    } else {
        document.getElementById('settings_input_fancy_disabled').setAttribute('checked', 'checked');
        document.getElementById('settings_input_fancy_enabled').removeAttribute('checked');
    }
}


function copy_modal_content_to_clipboard() {
    let content = document.getElementById('export_modal_content');

    const btn = $('#copy_to_clipboard_btn');
    navigator.clipboard.writeText(content.innerText).then(function () {
        console.log('Async: Copying to clipboard was successful!');
        $(btn).tooltip({
            placement: 'top',
            title: 'Copied',
            trigger: 'focus'
        })
        $(btn).tooltip('show');
        window.setTimeout(function () {
            $(btn).tooltip('hide');
        }, 500)
    }, function (err) {
        console.error('Async: Could not copy text: ', err);
    });
}

let active_notecalc_editor = null;

function reload_content(tab_index) {
    single_pulsing_rects.length = 0;
    repeating_pulsing_rects.length = 0;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    let notecalc_data = JSON.parse(localStorage.getItem('notecalc'));
    let tabs = notecalc_data.tabs;
    set_active_tab_index(tab_index);
    measure_start = Date.now();
    wasm_bindgen.set_compressed_encoded_content(
        active_notecalc_editor.app_ptr,
        tabs[active_tab_index].encoded_content
    );
    history.replaceState(undefined, undefined, '#' + tabs[active_tab_index].encoded_content)
    const maybe_title = wasm_bindgen.get_and_clear_title_changed(active_notecalc_editor.app_ptr);
    if (maybe_title !== null && maybe_title !== undefined) {
        set_active_tabname(maybe_title);
    }
    active_notecalc_editor.render_for_reason('reload content');
}

async function run() {
    if (is_debug) {
        wasm = await wasm_bindgen('frontend-web/pkg/frontend_web_bg.wasm?v=0.3.0');
    } else {
        wasm = await wasm_bindgen('assets/frontend_web_bg.wasm?v=0.3.0');
    }
    const instances = document.getElementsByClassName('notecalc-instance');
    function from_attribute_or(dom, name, def) {
        const attr = dom.getAttribute('data-notecalc-' + name);
        if (attr === null || attr == undefined) {
            return def;
        } else if (typeof def == 'number') {
            return parseInt(attr);
        } else if (typeof def == 'boolean') {
            return attr === "true" ? true : false;
        } else {
            return attr;
        }
    }
    for (let j = 0; j < instances.length; j++) {
        const i = instances[j];
        const configs = {
            decimal_places: from_attribute_or(i, 'decimal-places', 2),
            visible_line_count: from_attribute_or(i, 'visible-line-count', 10),
            font_size: from_attribute_or(i,'font-size', 16),
            font_vertical_padding: from_attribute_or(i, 'font-vertical-padding', 1),
            resizable: from_attribute_or(i, 'resizable', true),
            theme: from_attribute_or(i, 'theme', THEME_LIGHT),
            show_line_numbers: from_attribute_or(i, 'show-line-numbers', true)
        };
        configs.initial_content = i.innerText.trim();
        i.innerHTML = '';
        let notecalc_editor = NoteCalcInstance(
            i.getAttribute('id'),
            wasm,
            wasm_bindgen,
            i,
            configs
        );
        if (j === 0) {
            active_notecalc_editor = notecalc_editor;
        }
        notecalc_editor.activate();
        requestAnimationFrame(function() {notecalc_editor.tick()});
    }

    // window.addEventListener('keydown', function (event) {
    //     console.log(event, 'keydown');
    //     active_notecalc_editor.do_key_down(event);
    //     event.stopPropagation();
    // });
    // window.addEventListener('keyup', (event) => {
    //     console.log(event, 'keyup');
    //     active_notecalc_editor.doKeyUp(event);
    // }, true);
    window.addEventListener('paste', (event) => {
        console.log(event, 'paste');
        active_notecalc_editor.paste_from_clipboard((event.clipboardData || window.clipboardData).getData('text'));
        event.preventDefault();
        return false;
    });
    window.addEventListener('copy', (event) => {
        console.log("copy", event);
        let selected_text = wasm_bindgen.get_selected_text_and_clear_app_clipboard(active_notecalc_editor.app_ptr, false);
        // it can be null if there is no selection and no result in the copied line
        if (selected_text != null) {
            navigator.clipboard.writeText(selected_text).then(function() {
                console.log('ok');
            }, function() {
                console.log(arguments);
            });
        }
        // Copying the result highlights the copied result, so need a rerender
        active_notecalc_editor.render_for_reason('copy');
        event.preventDefault();
        return false;
    });
    window.addEventListener('cut', (event) => {
        active_notecalc_editor.measure_start = Date.now();
        let selected_text = wasm_bindgen.get_selected_text_and_clear_app_clipboard(active_notecalc_editor.app_ptr, true);
        navigator.clipboard.writeText(selected_text).then(function() {
            console.log('ok');
        }, function() {
            console.log(arguments);
        });
        active_notecalc_editor.render_for_reason('cut');
        active_notecalc_editor.save_content();
        event.preventDefault();
        return false;
    });

    // const size = calc_client_width();
    // set_client_width(size);

    // // window
    // window.addEventListener('keydown', function (event) {
    //     console.log(event, 'keydown');
    //     do_key_down(event);
    // });
    // window.addEventListener('keyup', doKeyUp, true);
    // window.addEventListener('paste', (event) => {
    //     paste_from_clipboard((event.clipboardData || window.clipboardData).getData('text'));
    //     event.preventDefault();
    //     return false;
    // });

    // window.addEventListener('copy', (event) => {
    //     console.log("copy", event);
    //     let selected_text = wasm_bindgen.get_selected_text_and_clear_app_clipboard(app_ptr, false);
    //     // it can be null if there is no selection and no result in the copied line
    //     if (selected_text != null) {
    //         navigator.clipboard.writeText(selected_text).then(function() {
    //             console.log('ok');
    //         }, function() {
    //             console.log(arguments);
    //         });
    //     }
    //     // Copying the result highlights the copied result, so need a rerender
    //     render_for_reason('copy');
    //     event.preventDefault();
    //     return false;
    // });
    // window.addEventListener('cut', (event) => {
    //     measure_start = Date.now();
    //     let selected_text = wasm_bindgen.get_selected_text_and_clear_app_clipboard(app_ptr, true);
    //     navigator.clipboard.writeText(selected_text).then(function() {
    //         console.log('ok');
    //     }, function() {
    //         console.log(arguments);
    //     });
    //     render_for_reason('cut');
    //     save_content();
    //     event.preventDefault();
    //     return false;
    // });

    // if (!is_mobile) {
    //     init_tabs_dom();
    // } else {
    //     document.getElementById('tablink_add').remove();
    // }
    // if (window.location.hash !== null && window.location.hash.length > 0) {
    //     // check if it already has it as a tab
    //     let content_from_url = window.location.hash.substr(1);
    //     let found = false;
    //     let tabs = notecalc_data.tabs;
    //     for (i = 0; i < tabs.length; ++i) {
    //         let encoded_content = tabs[i].encoded_content;
    //         if (encoded_content === content_from_url) {
    //             set_active_tab_index(i);
    //             found = true;
    //             break;
    //         }
    //     }
    //     if (!found) {
    //         add_tab_and_switch_to_it(content_from_url);
    //     }
    // } else if (notecalc_data) {
    //     set_active_tab_index(0);
    // }
    // on_tab_click(active_tab_index);

    // add_event_listeners();

    // set_theme_dom(notecalc_data.theme);
    // set_result_precision_dom(notecalc_data.result_prec);
    // set_fancy_char_dom(notecalc_data.fancy_chars_enabled);
    
}

WebFont.load({
    custom: {
        families: ['JetBrainsMono-Regular, JetBrainsMono-ExtraBold']
    },
    active: function () {
        is_debug = window.location.href.indexOf('debug') !== -1;
        let unloaded_scripts = 0;

        function load_script(src) {
            const script = document.createElement('script');
            script.src = src;
            document.head.appendChild(script);
            script.onload = function () {
                unloaded_scripts -= 1;
            };
            unloaded_scripts += 1;
        }

        if (is_debug) {
            load_script('assets/fuzz.js');
            load_script('assets/showcase.js');
            load_script('frontend-web/pkg/frontend_web.js?v=0.3.0');

            const fuzz_btn = document.createElement('li');
            fuzz_btn.className = 'nav-item'
            fuzz_btn.innerHTML = '<a class="badge badge-danger offset-1"\n' +
                '               href="javascript: void(0)"\n' +
                '               style=""\n' +
                '               onclick="toggle_fuzzing()"\n' +
                '            >\n' +
                '                Fuzz\n' +
                '            </a>';
            const demo_btn = document.createElement('li');
            demo_btn.className = 'nav-item'
            demo_btn.innerHTML = '<a class="badge badge-danger offset-1"\n' +
                '               href="javascript: void(0)"\n' +
                '               style=""\n' +
                '               onclick="demo_earth_circumference()"\n' +
                '            >\n' +
                '                Demo\n' +
                '            </a>';

            const tabs = document.getElementById('tabs');
            tabs.appendChild(fuzz_btn);
            tabs.appendChild(demo_btn);
        } else {
            load_script('assets/frontend_web.js?v=0.3.0');
        }
        is_mobile = (function () {
            let check = false;
            (function (a) {
                if (/(android|bb\d+|meego).+mobile|avantgo|bada\/|blackberry|blazer|compal|elaine|fennec|hiptop|iemobile|ip(hone|od)|iris|kindle|lge |maemo|midp|mmp|mobile.+firefox|netfront|opera m(ob|in)i|palm( os)?|phone|p(ixi|re)\/|plucker|pocket|psp|series(4|6)0|symbian|treo|up\.(browser|link)|vodafone|wap|windows ce|xda|xiino|android|ipad|playbook|silk/i.test(a) || /1207|6310|6590|3gso|4thp|50[1-6]i|770s|802s|a wa|abac|ac(er|oo|s\-)|ai(ko|rn)|al(av|ca|co)|amoi|an(ex|ny|yw)|aptu|ar(ch|go)|as(te|us)|attw|au(di|\-m|r |s )|avan|be(ck|ll|nq)|bi(lb|rd)|bl(ac|az)|br(e|v)w|bumb|bw\-(n|u)|c55\/|capi|ccwa|cdm\-|cell|chtm|cldc|cmd\-|co(mp|nd)|craw|da(it|ll|ng)|dbte|dc\-s|devi|dica|dmob|do(c|p)o|ds(12|\-d)|el(49|ai)|em(l2|ul)|er(ic|k0)|esl8|ez([4-7]0|os|wa|ze)|fetc|fly(\-|_)|g1 u|g560|gene|gf\-5|g\-mo|go(\.w|od)|gr(ad|un)|haie|hcit|hd\-(m|p|t)|hei\-|hi(pt|ta)|hp( i|ip)|hs\-c|ht(c(\-| |_|a|g|p|s|t)|tp)|hu(aw|tc)|i\-(20|go|ma)|i230|iac( |\-|\/)|ibro|idea|ig01|ikom|im1k|inno|ipaq|iris|ja(t|v)a|jbro|jemu|jigs|kddi|keji|kgt( |\/)|klon|kpt |kwc\-|kyo(c|k)|le(no|xi)|lg( g|\/(k|l|u)|50|54|\-[a-w])|libw|lynx|m1\-w|m3ga|m50\/|ma(te|ui|xo)|mc(01|21|ca)|m\-cr|me(rc|ri)|mi(o8|oa|ts)|mmef|mo(01|02|bi|de|do|t(\-| |o|v)|zz)|mt(50|p1|v )|mwbp|mywa|n10[0-2]|n20[2-3]|n30(0|2)|n50(0|2|5)|n7(0(0|1)|10)|ne((c|m)\-|on|tf|wf|wg|wt)|nok(6|i)|nzph|o2im|op(ti|wv)|oran|owg1|p800|pan(a|d|t)|pdxg|pg(13|\-([1-8]|c))|phil|pire|pl(ay|uc)|pn\-2|po(ck|rt|se)|prox|psio|pt\-g|qa\-a|qc(07|12|21|32|60|\-[2-7]|i\-)|qtek|r380|r600|raks|rim9|ro(ve|zo)|s55\/|sa(ge|ma|mm|ms|ny|va)|sc(01|h\-|oo|p\-)|sdk\/|se(c(\-|0|1)|47|mc|nd|ri)|sgh\-|shar|sie(\-|m)|sk\-0|sl(45|id)|sm(al|ar|b3|it|t5)|so(ft|ny)|sp(01|h\-|v\-|v )|sy(01|mb)|t2(18|50)|t6(00|10|18)|ta(gt|lk)|tcl\-|tdg\-|tel(i|m)|tim\-|t\-mo|to(pl|sh)|ts(70|m\-|m3|m5)|tx\-9|up(\.b|g1|si)|utst|v400|v750|veri|vi(rg|te)|vk(40|5[0-3]|\-v)|vm40|voda|vulc|vx(52|53|60|61|70|80|81|83|85|98)|w3c(\-| )|webc|whit|wi(g |nc|nw)|wmlb|wonu|x700|yas\-|your|zeto|zte\-/i.test(a.substr(0, 4))) check = true;
            })(navigator.userAgent || navigator.vendor || window.opera);
            return check;
        })();
        hidden_input = create_hidden_input();

        function wait_for_scripts_then_run() {
            if (unloaded_scripts === 0) {
                run();
            } else {
                setTimeout(function () {
                    wait_for_scripts_then_run();
                }, 100);
            }
        }

        wait_for_scripts_then_run();
    }
});
