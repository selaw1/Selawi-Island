import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

import {EventStore} from '../../gnome-extension/services/event-store.js';

const src = {app: 'Test', integration: 'test'};
const note = (id, title, extra = {}) => ({id, source: src, category: 'notification', state: 'started',
    ttl_ms: 5000, payload: {type: 'notification', title}, ...extra});
const dl = (id, state, percent) => ({id, source: src, category: 'transfer', state, persistent: true,
    payload: {type: 'progress', title: 'file.iso', percent}});
const call = id => ({id, source: {app: 'Slack', integration: 'test'}, category: 'call', state: 'started',
    persistent: true, payload: {type: 'call', title: 'Standup', confidence: 'confirmed'}});
const media = id => ({id, source: src, category: 'media', state: 'started', persistent: true,
    payload: {type: 'media', title: 'Song', playing: true}});
const ids = s => s.ordered().map(e => e.id);

test('stamps and expires non-persistent events', () => {
    const s = new EventStore();
    assert.deepEqual(s.apply(note('n', 'Hi'), 1000), {ok: true, changed: true});
    assert.equal(s.get('n').expires_at, 6000);
    assert.equal(s.nextDeadline(), 6000);
    assert.equal(s.tick(5999), false);
    assert.equal(s.tick(6000), true);
    assert.equal(s.size, 0);
});

test('rejects invalid input without changing state', () => {
    const s = new EventStore();
    const r = s.apply({id: 'bad id'}, 0);
    assert.equal(r.ok, false);
    assert.equal(s.size, 0);
});

test('deduplicates identical updates', () => {
    const s = new EventStore();
    s.apply(dl('d', 'started', 10), 1000);
    assert.equal(s.apply(dl('d', 'updated', 10), 2000).changed, false);
    assert.equal(s.get('d').updated_at, 1000);
    assert.equal(s.apply(dl('d', 'updated', 20), 3000).changed, true);
    assert.equal(s.get('d').created_at, 1000);
});

test('finished events linger, stale updates are rejected', () => {
    const s = new EventStore({finishedLingerMs: 3000});
    s.apply(dl('d', 'started', 0), 0);
    s.apply(dl('d', 'ended', 100), 10000);
    assert.equal(s.get('d').state, 'ended');
    assert.equal(s.apply(dl('d', 'updated', 50), 10001).ok, false);
    assert.equal(s.nextDeadline(), 13000);
    s.tick(13000);
    assert.equal(s.size, 0);
});

test('a notification never displaces an active call', () => {
    const s = new EventStore();
    s.apply(call('call'), 0);
    s.apply(media('music'), 1);
    s.apply(note('n', 'Mail'), 2);
    s.apply(dl('d', 'started', 0), 3);
    assert.deepEqual(ids(s), ['call', 'n', 'music', 'd']);
});

test('updates do not reorder events of equal priority', () => {
    const s = new EventStore();
    s.apply(note('old', 'A'), 0);
    s.apply(note('new', 'B'), 10);
    s.apply(note('old', 'A2', {state: 'updated'}), 20);
    assert.deepEqual(ids(s), ['new', 'old']);
});

test('dismissed events ignore updates until restarted', () => {
    const s = new EventStore();
    s.apply(dl('d', 'started', 0), 0);
    assert.equal(s.dismiss('d'), true);
    assert.equal(s.apply(dl('d', 'updated', 50), 1).changed, false);
    assert.equal(s.size, 0);
    assert.equal(s.apply(dl('d', 'started', 0), 2).changed, true);
});

test('user priority override wins', () => {
    const s = new EventStore({categoryPriority: {media: 'critical'}});
    s.apply(call('call'), 0);
    s.apply({...media('music'), priority: 'metric'}, 1);
    assert.deepEqual(ids(s), ['music', 'call']);
});

test('capacity stays bounded under a flood', () => {
    const s = new EventStore();
    s.apply(call('call'), 0);
    for (let i = 0; i < 5000; i++)
        s.apply(note(`n${i}`, 'spam'), i);
    assert.equal(s.size, 64);
    assert.ok(s.get('call'), 'persistent call survives');
});

test('demo scenario plays cleanly twice', () => {
    const scenario = JSON.parse(readFileSync(
        new URL('../../gnome-extension/data/demo-scenario.json', import.meta.url)));
    const s = new EventStore();
    let now = 0;
    for (let round = 0; round < 2; round++) {
        for (const step of scenario.steps) {
            now += step.delay_ms;
            s.tick(now);
            const r = s.apply(step.event, now);
            assert.ok(r.ok, `${step.event.id}: ${r.error}`);
        }
    }
});
