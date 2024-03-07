


const DEMO_TYPING_SPEED = 50;
let events_for_demo = [];
let next_typing_at = 0;


function key(name) {
    events_for_demo.push({
        key: name,
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        preventDefault: function () {
        }
    });
}

function alt_up(count) {
    for (let i = 0; i < count; i++) {
        events_for_demo.push({
            key: 'ArrowUp',
            ctrlKey: false,
            altKey: true,
            shiftKey: true,
            preventDefault: function () {
            }
        });
        wait(500);
    }
    events_for_demo.push({
        key: 'doKeyUp',
        ctrlKey: false,
        altKey: true,
        shiftKey: false,
        preventDefault: function () {
        },
    })
}


function shift_key(name) {
    events_for_demo.push({
        key: name,
        ctrlKey: false,
        altKey: false,
        shiftKey: true,
        preventDefault: function () {
        }
    });
}

function text(str) {
    for (let i = 0; i < str.length; i++) {
        key(str[i]);
    }
}

function wait(howmuch) {
    for (let i = 0; i < howmuch / DEMO_TYPING_SPEED; i++) {
        key('Unknown');
    }
}


function demo_sum_selection() {
    text('SUM will appear in a green box');
    key('Enter');
    text('  12 + 3*4 + (26 * 48 / 98 - 44)');
    key('Enter');
    text('1M');
    key('Enter');
    text('2e12');
    key('ArrowUp');
    key('ArrowUp');
    key('Home');
    for (let i = 0; i < 2; i++) {
        key('ArrowRight');
    }
    for (let i = 0; i < 6; i++) {
        shift_key('ArrowRight');
    }
    wait(1000);
    shift_key('ArrowRight');
    shift_key('ArrowRight');
    wait(1000);
    for (let i = 0; i < 10; i++) {
        key('ArrowRight');
    }
    for (let i = 0; i < 6; i++) {
        shift_key('ArrowRight');
    }
    wait(1000);
    shift_key('ArrowRight');
    wait(1000);
    shift_key('ArrowDown');
    wait(1000);
    shift_key('ArrowDown');
    start_demo();
}

function demo_earth_circumference() {
    text('Earth\'s circumference is around 40k km');
    key('Enter');
    key('Enter');
    text('so travelling constatnly with 80 km/h');
    key('Enter');
    text('you need ');
    alt_up(3);
    text(' / ');
    alt_up(1);
    text(' to go around');
    key('Enter');
    key('Enter');
    text('which is ');
    alt_up(2);
    text(' in days');
    start_demo();
}

function start_demo() {
    requestAnimationFrame(showcase_tick);
}

function showcase_tick(now) {
    if (next_typing_at > now) {
        requestAnimationFrame(showcase_tick);
        return;
    } else if (events_for_demo.length === 0 ) {
        return;
    }
    next_typing_at = now + DEMO_TYPING_SPEED;

    const event = events_for_demo[0];
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
    events_for_demo.shift();
    requestAnimationFrame(showcase_tick);
}