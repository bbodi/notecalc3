let fuzzing_is_running = false;
let had_error = false;
let next_event_tick = 0;
const EVENT_INTERVAL_MS = 0;

const CHARACTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
const NUMBERS = '0123456789';
const OPERATORS = '+-*/%[]()^$=';
const NAVIGATION_EVENTS = ['ArrowRight', 'ArrowLeft', 'ArrowUp', 'ArrowDown',
    'Home', 'End', 'PageUp', 'PageDown'];
const CHARACTERS_LENGTH = CHARACTERS.length;

let chance_enter = 0;
let chance_char = 0;
let chance_backspace = 0;
let chance_del = 0;
let chance_navigation = 0;
let chance_ctrl_a = 0;
let chance_operator = 0;
let chance_number = 0;
let chance_select_multiple = 0;
let chance_undo = 0;
let chance_tab = 0;
let chance_insert_lineref = 0;

let event_history = [];

function random_float(min, max) {
    return Math.random() * (max - min) + min;
}

function init() {
    if (had_error) {
        return;
    }
    let max = 1.0;

    function rnd_eql() {
        const rnd = random_float(0.0, max);
        max -= rnd;
        return rnd;
    }

    event_history.length = 0;
    chance_char = rnd_eql();
    chance_number = rnd_eql();
    chance_operator = rnd_eql();
    chance_backspace = rnd_eql();
    chance_del = rnd_eql();
    chance_navigation = rnd_eql();
    chance_enter = rnd_eql();
    chance_ctrl_a = rnd_eql();
    chance_select_multiple = rnd_eql();
    chance_undo = rnd_eql();
    chance_tab = rnd_eql();
    chance_insert_lineref = rnd_eql();

}

function toggle_fuzzing() {
    fuzzing_is_running = !fuzzing_is_running;
    if (fuzzing_is_running) {
        if (had_error) {
            on_tab_close(tab_index_counter - 1, true);
            add_tab_and_switch_to_it('');
        }
        init();
        requestAnimationFrame(fuzzy_tick);
    }
}

function generate_random_events() {
    let rnd = Math.random();
    let acc = 0;
    let events = [];

    function hit(chance, rnd) {
        acc += chance;
        return rnd < acc;
    }

    function keypress(name) {
        events.push({
            key: name,
            ctrlKey: false,
            altKey: false,
            shiftKey: false,
            preventDefault: function () {
            }
        });
    }

    if (hit(chance_enter, rnd)) {
        keypress('Enter');
    } else if (hit(chance_char, rnd)) {
        keypress(CHARACTERS.charAt(Math.floor(Math.random() * CHARACTERS_LENGTH)));
    } else if (hit(chance_backspace, rnd)) {
        keypress('Backspace');
    } else if (hit(chance_del, rnd)) {
        keypress('Delete');
    } else if (hit(chance_navigation, rnd)) {
        keypress(NAVIGATION_EVENTS[Math.floor(Math.random() * NAVIGATION_EVENTS.length)]);
    } else if (hit(chance_ctrl_a, rnd)) {
        events.push({
            key: 'a',
            ctrlKey: true,
            altKey: false,
            shiftKey: false,
            preventDefault: function () {
            },
        });
    } else if (hit(chance_operator, rnd)) {
        keypress(OPERATORS[Math.floor(Math.random() * OPERATORS.length)]);
    } else if (hit(chance_number, rnd)) {
        keypress(NUMBERS[Math.floor(Math.random() * NUMBERS.length)]);
    } else if (hit(chance_select_multiple, rnd)) {
        let dir = NAVIGATION_EVENTS[Math.floor(Math.random() * NAVIGATION_EVENTS.length)];
        let step = Math.floor(Math.random() * 20);
        for (let i = 0; i < step; ++i) {
            events.push({
                key: dir,
                ctrlKey: false,
                altKey: false,
                shiftKey: true,
                preventDefault: function () {
                },
            });
        }
    } else if (hit(chance_undo, rnd)) {
        events.push({
            key: 'z',
            ctrlKey: true,
            altKey: false,
            shiftKey: Math.random() > 0.5, // undo or redo
            preventDefault: function () {
            },
        });
    } else if (hit(chance_tab, rnd)) {
        keypress('Tab');
    } else if (hit(chance_insert_lineref, rnd)) {
        let step = Math.floor(Math.random() * 10);
        for (let i = 0; i < step; ++i) {
            events.push({
                key: 'ArrowUp',
                ctrlKey: false,
                altKey: true,
                shiftKey: false,
                preventDefault: function () {
                },
            });
        }
        events.push({
            key: 'doKeyUp',
            ctrlKey: false,
            altKey: true,
            shiftKey: false,
            preventDefault: function () {
            },
        });
    }
    return events;
}

function simulate_input() {
    if (!fuzzing_is_running) {
        return;
    }
    let events;
    if (had_error) {
        events = [...event_history];
        event_history.length = 0;
    } else {
        events = generate_random_events()
    }
    for (let i = 0; i < events.length; i++) {
        const event = events[i];
        let content_before_event = wasm_bindgen.get_plain_content(app_ptr);
        const cursor_before_event = wasm_bindgen.get_cursor(app_ptr);
        const top_of_undo_stack = wasm_bindgen.get_top_of_undo_stack(app_ptr);
        try {
            if (had_error) {
                console.log(JSON.stringify(event));
                console.log(content_before_event);
            }
            event_history.push(event);
            if (event.key === 'doKeyUp') {
                doKeyUp({
                    key: 'Alt',
                    ctrlKey: false,
                    altKey: false,
                    shiftKey: false,
                    preventDefault: function () {
                    },
                });
            } else {
                doKeyDown(event);
            }
        } catch (err) {
            console.log(content_before_event);
            console.log(cursor_before_event);
            console.log(top_of_undo_stack);
            console.log(event);

            const element = document.createElement('pre');
            element.textContent = JSON.stringify(err) + '\n------content--------\n' +
                content_before_event + '\n---cursor------------\n' +
                cursor_before_event + '\n----stack---------------\n' +
                top_of_undo_stack + '\n------event-------------\n' +
                JSON.stringify(event) +
                '\n\n\n' +
                event_history_to_rust_code(event_history);
            const before = document.getElementsByTagName('nav')[0];
            document.getElementsByTagName('body')[0].insertBefore(element, before);
            fuzzing_is_running = false;
            debugger;
            events.length = 0;
            return false;
        }
        content_before_event = null;
        if (event_history.length > 1000) {
            console.log("FUZZING RESTART");
            // remove the tab
            on_tab_close(tab_index_counter - 1, true);
            add_tab_and_switch_to_it('');
            // restart fuzzing
            init();
        }
    }
    events.length = 0;
}

function event_history_to_rust_code(event_history) {
    let str = '';
    event_history.forEach(function (event) {
        let modif = '';
        let input = '';
        if (event.ctrlKey) {
            if (event.shiftKey) {
                modif = 'InputModifiers::ctrl_shift()';
            } else {
                modif = 'InputModifiers::ctrl()';
            }
        } else if (event.shiftKey) {
            modif = 'InputModifiers::shift()';
        } else if (event.altKey) {
            modif = 'InputModifiers::alt()';
        } else {
            modif = 'InputModifiers::none()';
        }
        if (event.key === 'Enter' || event.key === 'Backspace'
            || event.key === 'Home'
            || event.key === 'End'
            || event.key === 'PageUp'
            || event.key === 'PageDown'
            || event.key === 'Tab') {
            input = 'EditorInputEvent::' + event.key;
        } else if (event.key === 'Delete') {
            input = 'EditorInputEvent::Del';
        } else if (event.key === 'ArrowUp') {
            input = 'EditorInputEvent::Up';
        } else if (event.key === 'ArrowDown') {
            input = 'EditorInputEvent::Down';
        } else if (event.key === 'ArrowLeft') {
            input = 'EditorInputEvent::Left';
        } else if (event.key === 'ArrowRight') {
            input = 'EditorInputEvent::Right';
        } else {
            input = "EditorInputEvent::Char('" + event.key + "')";
        }
        str += 'test.input(' + input + ', ' + modif + ');\n';
    });
    return str;
}

function fuzzy_tick(now) {
    if (next_event_tick > now) {
        if (fuzzing_is_running) {
            requestAnimationFrame(fuzzy_tick);
        }
        return;
    }
    next_event_tick = now + EVENT_INTERVAL_MS;
    for (let i = 0; i < 100; ++i) {
        if (simulate_input() === false) {
            return;
        }
    }

    if (fuzzing_is_running) {
        requestAnimationFrame(fuzzy_tick);
    }
}