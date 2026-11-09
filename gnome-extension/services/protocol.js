// Event protocol v1 validation for the extension.
//
// Mirrors core/src/events.rs. This module must stay free of GNOME imports so
// it can be unit-tested with Node (see tests/js). Every event that enters
// the island passes through validateInput() first.

export const PROTOCOL_VERSION = 1;

export const CATEGORIES = ['call', 'media', 'transfer', 'device', 'notification',
    'system', 'reminder', 'metric', 'custom'];
export const STATES = ['started', 'updated', 'ended', 'failed', 'expired'];
// Most important first. Index = rank.
export const PRIORITIES = ['critical', 'call', 'device', 'notification', 'media',
    'transfer', 'reminder', 'metric'];
const CONFIDENCES = ['confirmed', 'probable', 'audio_only', 'unknown'];
const SEVERITIES = ['info', 'warning', 'critical'];
const DEVICE_KINDS = ['headphones', 'speaker', 'keyboard', 'mouse', 'phone', 'other'];

const MAX_ID_LEN = 128;
const MAX_NAME_LEN = 64;
const MAX_TITLE_LEN = 256;
const MAX_TEXT_LEN = 1024;
const MAX_LABEL_LEN = 32;
const MAX_ACTIONS = 4;
export const MAX_TTL_MS = 24 * 60 * 60 * 1000;

const TOKEN_RE = /^[A-Za-z0-9._:-]+$/;
// Control characters other than newline.
const CONTROL_RE = /[\u0000-\u0009\u000b-\u001f\u007f-\u009f]/;

export const DEFAULT_PRIORITY = {
    call: 'call',
    device: 'device',
    notification: 'notification',
    system: 'notification',
    custom: 'notification',
    media: 'media',
    transfer: 'transfer',
    reminder: 'reminder',
    metric: 'metric',
};

export function isTerminal(state) {
    return state === 'ended' || state === 'failed' || state === 'expired';
}

export function priorityRank(priority) {
    return PRIORITIES.indexOf(priority);
}

class ProtocolError extends Error {}

function fail(message) {
    throw new ProtocolError(message);
}

function isObject(v) {
    return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function checkKeys(obj, allowed, where) {
    for (const key of Object.keys(obj)) {
        if (!allowed.includes(key))
            fail(`${where}: unknown field "${key}"`);
    }
}

function token(v, max, field) {
    if (typeof v !== 'string' || v.length === 0 || v.length > max || !TOKEN_RE.test(v))
        fail(`${field}: invalid identifier`);
    return v;
}

function text(v, max, field) {
    if (typeof v !== 'string' || v.trim().length === 0)
        fail(`${field}: must be a non-empty string`);
    if ([...v].length > max)
        fail(`${field}: is too long`);
    if (CONTROL_RE.test(v))
        fail(`${field}: contains control characters`);
    return v;
}

function optText(v, max, field) {
    return v === undefined || v === null ? undefined : text(v, max, field);
}

function oneOf(v, allowed, field) {
    if (!allowed.includes(v))
        fail(`${field}: must be one of ${allowed.join(', ')}`);
    return v;
}

function bool(v, field) {
    if (typeof v !== 'boolean')
        fail(`${field}: must be a boolean`);
    return v;
}

function optUint(v, field, max = Number.MAX_SAFE_INTEGER) {
    if (v === undefined || v === null)
        return undefined;
    if (!Number.isInteger(v) || v < 0 || v > max)
        fail(`${field}: out of range`);
    return v;
}

function payload(raw, category) {
    if (!isObject(raw))
        fail('payload: must be an object');
    const p = {type: raw.type};
    switch (raw.type) {
    case 'call':
        checkKeys(raw, ['type', 'title', 'confidence'], 'payload');
        if (category !== 'call')
            fail('payload type does not match category');
        p.title = text(raw.title, MAX_TITLE_LEN, 'payload.title');
        p.confidence = oneOf(raw.confidence, CONFIDENCES, 'payload.confidence');
        break;
    case 'media':
        checkKeys(raw, ['type', 'title', 'artist', 'album', 'playing',
            'position_ms', 'duration_ms'], 'payload');
        if (category !== 'media')
            fail('payload type does not match category');
        p.title = text(raw.title, MAX_TITLE_LEN, 'payload.title');
        p.artist = optText(raw.artist, MAX_TITLE_LEN, 'payload.artist');
        p.album = optText(raw.album, MAX_TITLE_LEN, 'payload.album');
        p.playing = bool(raw.playing, 'payload.playing');
        p.position_ms = optUint(raw.position_ms, 'payload.position_ms');
        p.duration_ms = optUint(raw.duration_ms, 'payload.duration_ms');
        break;
    case 'progress':
        checkKeys(raw, ['type', 'title', 'detail', 'percent'], 'payload');
        p.title = text(raw.title, MAX_TITLE_LEN, 'payload.title');
        p.detail = optText(raw.detail, MAX_TEXT_LEN, 'payload.detail');
        if (raw.percent !== undefined && raw.percent !== null) {
            if (typeof raw.percent !== 'number' || !(raw.percent >= 0 && raw.percent <= 100))
                fail('payload.percent: out of range');
            p.percent = raw.percent;
        }
        break;
    case 'device':
        checkKeys(raw, ['type', 'name', 'device_kind', 'connected', 'battery_percent'], 'payload');
        if (category !== 'device')
            fail('payload type does not match category');
        p.name = text(raw.name, MAX_TITLE_LEN, 'payload.name');
        if (raw.device_kind !== undefined)
            p.device_kind = oneOf(raw.device_kind, DEVICE_KINDS, 'payload.device_kind');
        p.connected = bool(raw.connected, 'payload.connected');
        p.battery_percent = optUint(raw.battery_percent, 'payload.battery_percent', 100);
        break;
    case 'notification':
        checkKeys(raw, ['type', 'title', 'body'], 'payload');
        p.title = text(raw.title, MAX_TITLE_LEN, 'payload.title');
        p.body = optText(raw.body, MAX_TEXT_LEN, 'payload.body');
        break;
    case 'system':
        checkKeys(raw, ['type', 'title', 'detail', 'severity'], 'payload');
        p.title = text(raw.title, MAX_TITLE_LEN, 'payload.title');
        p.detail = optText(raw.detail, MAX_TEXT_LEN, 'payload.detail');
        p.severity = oneOf(raw.severity, SEVERITIES, 'payload.severity');
        break;
    default:
        fail('payload.type: unknown payload type');
    }
    for (const key of Object.keys(p)) {
        if (p[key] === undefined)
            delete p[key];
    }
    return p;
}

function actions(raw) {
    if (raw === undefined)
        return [];
    if (!Array.isArray(raw))
        fail('actions: must be an array');
    if (raw.length > MAX_ACTIONS)
        fail(`actions: at most ${MAX_ACTIONS} allowed`);
    const seen = new Set();
    return raw.map(a => {
        if (!isObject(a))
            fail('actions: each action must be an object');
        checkKeys(a, ['id', 'label'], 'action');
        const id = token(a.id, MAX_NAME_LEN, 'actions.id');
        if (seen.has(id))
            fail(`actions: duplicate id "${id}"`);
        seen.add(id);
        return {id, label: text(a.label, MAX_LABEL_LEN, 'actions.label')};
    });
}

/**
 * Validates and copies one event input. Returns {ok: true, input} or
 * {ok: false, error}. Never throws.
 *
 * @param {unknown} raw - untrusted parsed JSON
 * @returns {{ok: true, input: object} | {ok: false, error: string}}
 */
export function validateInput(raw) {
    try {
        if (!isObject(raw))
            fail('event: must be an object');
        checkKeys(raw, ['id', 'source', 'category', 'state', 'priority', 'persistent',
            'ttl_ms', 'group', 'payload', 'actions'], 'event');
        if (!isObject(raw.source))
            fail('source: must be an object');
        checkKeys(raw.source, ['app', 'integration'], 'source');
        const category = oneOf(raw.category, CATEGORIES, 'category');
        const input = {
            id: token(raw.id, MAX_ID_LEN, 'id'),
            source: {
                app: text(raw.source.app, MAX_NAME_LEN, 'source.app'),
                integration: text(raw.source.integration, MAX_NAME_LEN, 'source.integration'),
            },
            category,
            state: oneOf(raw.state, STATES, 'state'),
            persistent: raw.persistent === undefined ? false : bool(raw.persistent, 'persistent'),
            payload: payload(raw.payload, category),
            actions: actions(raw.actions),
        };
        if (raw.priority !== undefined)
            input.priority = oneOf(raw.priority, PRIORITIES, 'priority');
        if (raw.ttl_ms !== undefined) {
            const ttl = optUint(raw.ttl_ms, 'ttl_ms', MAX_TTL_MS);
            if (ttl === 0)
                fail('ttl_ms: out of range');
            input.ttl_ms = ttl;
        }
        if (raw.group !== undefined)
            input.group = token(raw.group, MAX_ID_LEN, 'group');
        return {ok: true, input};
    } catch (e) {
        if (e instanceof ProtocolError)
            return {ok: false, error: e.message};
        throw e;
    }
}

const STORED_ONLY_KEYS = ['priority', 'created_at', 'updated_at', 'expires_at'];

/**
 * Validates one stored event as sent by the daemon (an event input plus
 * resolved priority and timestamps). Returns {ok: true, event} with
 * `expires_at` and `group` normalized to null when absent, or
 * {ok: false, error}. Never throws.
 *
 * @param {unknown} raw - untrusted parsed JSON
 * @returns {{ok: true, event: object} | {ok: false, error: string}}
 */
export function validateEvent(raw) {
    if (!isObject(raw))
        return {ok: false, error: 'event: must be an object'};
    const inputPart = {};
    for (const [key, value] of Object.entries(raw)) {
        if (!STORED_ONLY_KEYS.includes(key))
            inputPart[key] = value;
    }
    const result = validateInput(inputPart);
    if (!result.ok)
        return result;
    if (!PRIORITIES.includes(raw.priority))
        return {ok: false, error: 'priority: missing or unknown'};
    const isTime = v => Number.isSafeInteger(v) && v >= 0;
    if (!isTime(raw.created_at) || !isTime(raw.updated_at))
        return {ok: false, error: 'created_at/updated_at: must be timestamps'};
    if (raw.expires_at !== undefined && raw.expires_at !== null && !isTime(raw.expires_at))
        return {ok: false, error: 'expires_at: must be a timestamp'};
    return {
        ok: true,
        event: {
            ...result.input,
            priority: raw.priority,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            expires_at: raw.expires_at ?? null,
            group: result.input.group ?? null,
        },
    };
}

/**
 * Parses the JSON event list the daemon sends. Invalid entries are dropped
 * and reported, so one bad event cannot blank the island.
 *
 * @param {string} json
 * @returns {{events: object[], errors: string[]}}
 */
export function parseEventList(json) {
    let raw;
    try {
        raw = JSON.parse(json);
    } catch (e) {
        return {events: [], errors: [`list: ${e.message}`]};
    }
    if (!Array.isArray(raw))
        return {events: [], errors: ['list: must be an array']};
    const events = [];
    const errors = [];
    for (const item of raw) {
        const r = validateEvent(item);
        if (r.ok)
            events.push(r.event);
        else
            errors.push(r.error);
    }
    return {events, errors};
}
