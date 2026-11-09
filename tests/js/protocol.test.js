import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

import {validateInput} from '../../gnome-extension/services/protocol.js';

const cases = JSON.parse(readFileSync(new URL('../fixtures/protocol-cases.json', import.meta.url)));

test('accepts every shared valid case', () => {
    for (const event of cases.valid) {
        const r = validateInput(event);
        assert.ok(r.ok, `${event.id}: ${r.error}`);
    }
});

test('rejects every shared invalid case', () => {
    for (const {why, event} of cases.invalid)
        assert.equal(validateInput(event).ok, false, why);
});

test('never throws on garbage', () => {
    for (const junk of [null, 42, 'x', [], {}, {payload: null}, {source: []}])
        assert.equal(validateInput(junk).ok, false);
});

test('output is a copy without undefined fields', () => {
    const raw = structuredClone(cases.valid[0]);
    const r = validateInput(raw);
    assert.ok(r.ok);
    raw.payload.title = 'changed';
    assert.equal(r.input.payload.title, 'New message');
    assert.ok(!('body' in r.input.payload));
});
