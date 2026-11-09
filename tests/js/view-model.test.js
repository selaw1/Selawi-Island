import {test} from 'node:test';
import assert from 'node:assert/strict';

import {describe, formatClock} from '../../gnome-extension/ui/view-model.js';

const base = {source: {app: 'App', integration: 'test'}, created_at: 0, updated_at: 0, actions: []};

test('formatClock', () => {
    assert.equal(formatClock(0), '0:00');
    assert.equal(formatClock(65_000), '1:05');
    assert.equal(formatClock(3_725_000), '1:02:05');
    assert.equal(formatClock(-5), '0:00');
});

test('active call shows a live duration', () => {
    const vm = describe({...base, category: 'call', state: 'started',
        payload: {type: 'call', title: 'Standup', confidence: 'confirmed'}}, 90_000);
    assert.equal(vm.trailing, '1:30');
    assert.equal(vm.live, true);
    assert.equal(vm.accent, 'green');
});

test('unconfirmed call is not presented as fact', () => {
    const vm = describe({...base, category: 'call', state: 'started',
        payload: {type: 'call', title: 'Tab', confidence: 'audio_only'}}, 0);
    assert.match(vm.subtitle, /Audio activity/);
    assert.notEqual(vm.accent, 'green');
});

test('device without battery shows no battery', () => {
    const vm = describe({...base, category: 'device', state: 'started',
        payload: {type: 'device', name: 'Buds', connected: true}}, 0);
    assert.equal(vm.trailing, '');
});

test('progress percent, done and failed', () => {
    const p = {type: 'progress', title: 'f', percent: 41.6};
    assert.equal(describe({...base, category: 'transfer', state: 'updated', payload: p}, 0).trailing, '42%');
    const done = describe({...base, category: 'transfer', state: 'ended', payload: p}, 0);
    assert.equal(done.trailing, 'Done');
    assert.equal(done.progress, 1);
    assert.equal(describe({...base, category: 'transfer', state: 'failed', payload: p}, 0).accent, 'red');
    assert.equal(describe({...base, category: 'transfer', state: 'updated',
        payload: {type: 'progress', title: 'f'}}, 0).progress, null);
});

test('media position extrapolates only while playing', () => {
    const ev = playing => ({...base, category: 'media', state: 'updated', updated_at: 1000,
        payload: {type: 'media', title: 'S', playing, position_ms: 10_000, duration_ms: 20_000}});
    assert.equal(describe(ev(true), 6000).trailing, '0:15');
    assert.equal(describe(ev(false), 6000).trailing, '0:10');
    assert.equal(describe(ev(true), 999_999).progress, 1);
});

test('paused media with unknown position shows no frozen 0:00', () => {
    const ev = {...base, category: 'media', state: 'updated',
        payload: {type: 'media', title: 'Clip', playing: false, position_ms: 0}};
    assert.equal(describe(ev, 5000).trailing, '');
    assert.equal(describe(ev, 5000).progress, null);
    const known = {...ev, payload: {...ev.payload, duration_ms: 60_000}};
    assert.equal(describe(known, 5000).trailing, '0:00', 'with a length, 0:00 is real');
});
