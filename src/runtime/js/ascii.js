// Original Grain implementation of a small, synchronous Play-inspired API.
// This closure is evaluated before sketch code; capture trusted intrinsics here.
(() => {
    'use strict';
    const parse = JSON.parse;
    const stringify = JSON.stringify;
    const createObject = Object.create;
    const freeze = Object.freeze;
    const setPrototype = Object.setPrototypeOf;
    const keys = Object.keys;
    const isArray = Array.isArray;
    const isInteger = Number.isInteger;
    const apply = Reflect.apply;
    const tag = Object.prototype.toString;
    const slice = String.prototype.slice;
    const charCodeAt = String.prototype.charCodeAt;
    const parseIntSafe = Number.parseInt;
    const regexTest = RegExp.prototype.test;
    const NativeError = Error;
    const imul = Math.imul;
    const define = Object.defineProperty;
    const plain = () => createObject(null);
    const array = () => setPrototype([], null);
    const truncate = (s, n) => apply(slice, s, [0, n]);
    const fail = message => { throw new NativeError(message); };

    function errorObject(error) {
        const result = plain();
        result.message = 'ASCII sketch failed';
        result.line = null;
        result.column = null;
        result.stack = null;
        if (typeof error === 'string') result.message = truncate(error, 2048);
        else if (error && typeof error === 'object') {
            const message = error.message;
            const stack = error.stack;
            const line = error.lineNumber;
            const column = error.columnNumber;
            if (typeof message === 'string') result.message = truncate(message, 2048);
            if (typeof stack === 'string') result.stack = truncate(stack, 8192);
            if (isInteger(line) && line > 0) result.line = line;
            if (isInteger(column) && column > 0) result.column = column;
        }
        return result;
    }

    function seed(text) {
        let state = 2166136261;
        for (let i = 0; i < text.length; i++) {
            state = imul(state ^ apply(charCodeAt, text, [i]), 16777619) >>> 0;
        }
        define(Math, 'random', { value: () => {
            state = (imul(state, 1664525) + 1013904223) >>> 0;
            return state / 4294967296;
        }, writable: false, configurable: false });
        // The timeline belongs to the host, never the process wall clock.
        define(globalThis, 'Date', { value: undefined, writable: false, configurable: false });
    }

    function color(value, fallback) {
        if (value === undefined) return fallback;
        if (isArray(value) && value.length === 3) {
            const rgb = array();
            for (let i = 0; i < 3; i++) {
                const channel = value[i];
                if (!isInteger(channel) || channel < 0 || channel > 255) fail('RGB channels must be integers from 0 to 255');
                rgb[i] = channel;
            }
            return rgb;
        }
        if (typeof value === 'string' && apply(regexTest, /^#[0-9a-fA-F]{6}$/, [value])) {
            const rgb = array();
            for (let i = 0; i < 3; i++) rgb[i] = parseIntSafe(apply(slice, value, [1 + i * 2, 3 + i * 2]), 16);
            return rgb;
        }
        fail('ASCII colors require RGB byte arrays or #rrggbb; CSS colors and alpha are unsupported');
    }

    function normalize(value) {
        if (typeof value === 'string') {
            const object = plain(); object.char = value; value = object;
        }
        if (!value || typeof value !== 'object' || isArray(value)) fail('main must return a character or a cell object');
        const allowed = { char: true, color: true, backgroundColor: true };
        for (const key of keys(value)) {
            if (key !== 'char' && key !== 'color' && key !== 'backgroundColor') fail('Unsupported ASCII cell field: ' + key);
        }
        const symbol = value.char;
        if (typeof symbol !== 'string' || symbol.length === 0 || symbol.length > 2) fail('ASCII cells require one single-column character');
        const fg = color(value.color, [230, 230, 230]);
        const bg = color(value.backgroundColor, [0, 0, 0]);
        const cell = plain();
        cell.symbol = symbol; cell.r = fg[0]; cell.g = fg[1]; cell.b = fg[2];
        const background = array();
        background[0] = bg[0]; background[1] = bg[1]; background[2] = bg[2];
        cell.background = background;
        return cell;
    }

    function synchronous(fn, args, name) {
        if (fn === undefined) return undefined;
        if (typeof fn !== 'function' || apply(tag, fn, []) !== '[object Function]') fail(name + ' must be a synchronous function');
        const result = apply(fn, undefined, args);
        if (result && (typeof result === 'object' || typeof result === 'function') && typeof result.then === 'function') {
            fail(name + ' must not return a Promise or thenable');
        }
        return result;
    }

    function create(hooks) {
        if (hooks.settings !== undefined) fail('ASCII settings exports are unsupported; the host controls layout and playback');
        const boot = hooks.boot, pre = hooks.pre, main = hooks.main, post = hooks.post;
        if (typeof main !== 'function') fail('ASCII sketch requires a main function');
        const data = plain();
        let buffer = [], booted = false, previousCols = 0, previousRows = 0;
        const cursor = plain();
        cursor.x = 0; cursor.y = 0; cursor.pressed = false; cursor.available = false;
        cursor.p = freeze({ x: 0, y: 0, pressed: false });
        freeze(cursor);
        return encoded => {
            try {
                const request = parse(encoded);
                const context = request.context;
                context.cols = request.cols; context.rows = request.rows;
                context.seedText = request.seedText;
                context.metrics = freeze({ aspect: request.aspect });
                freeze(context.audio); freeze(context.params); freeze(context);
                if (previousCols !== context.cols || previousRows !== context.rows) {
                    buffer = [];
                    for (let i = 0; i < context.cols * context.rows; i++) buffer[i] = ' ';
                    previousCols = context.cols; previousRows = context.rows;
                }
                if (!booted) { synchronous(boot, [context, buffer, data], 'boot'); booted = true; }
                synchronous(pre, [context, cursor, buffer, data], 'pre');
                for (let y = 0; y < context.rows; y++) {
                    for (let x = 0; x < context.cols; x++) {
                        const index = y * context.cols + x;
                        const coord = freeze({ x, y, index });
                        const value = synchronous(main, [coord, context, cursor, buffer, data], 'main');
                        if (value !== undefined) buffer[index] = value;
                    }
                }
                synchronous(post, [context, cursor, buffer, data], 'post');
                if (buffer.length !== context.cols * context.rows) fail('ASCII hooks must preserve buffer length');
                const cells = array();
                for (let y = 0; y < context.rows; y++) {
                    const row = array();
                    for (let x = 0; x < context.cols; x++) row[x] = normalize(buffer[y * context.cols + x]);
                    cells[y] = row;
                }
                const frame = plain(); frame.cols = context.cols; frame.rows = context.rows; frame.cells = cells;
                const result = plain(); result.frame = frame;
                return stringify(result);
            } catch (error) {
                const result = plain(); result.error = errorObject(error);
                return stringify(result);
            }
        };
    }
    return { create, seed, formatError: error => stringify(errorObject(error)) };
})()
