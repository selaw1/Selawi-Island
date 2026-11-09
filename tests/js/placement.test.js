import {test} from 'node:test';
import assert from 'node:assert/strict';

import {placePill, positionFromDrop} from '../../gnome-extension/ui/placement.js';

const mon = {x: 0, y: 0, width: 1920, height: 1080};

test('default is top center', () => {
    assert.deepEqual(placePill(mon, 40, null, 320, 36), {x: 800, y: 40});
});

test('custom position keeps the center, so the pill grows around it', () => {
    const custom = {fx: 0.25, fy: 0.5};
    assert.deepEqual(placePill(mon, 40, custom, 100, 20), {x: 430, y: 540});
    assert.deepEqual(placePill(mon, 40, custom, 400, 200), {x: 280, y: 540});
});

test('never leaves the monitor, even when expanding near an edge', () => {
    assert.deepEqual(placePill(mon, 40, {fx: 1, fy: 1}, 400, 200), {x: 1520, y: 880});
    assert.deepEqual(placePill(mon, 40, {fx: 0, fy: 0}, 400, 200), {x: 0, y: 0});
});

test('works on a second monitor', () => {
    const right = {x: 1920, y: 0, width: 2560, height: 1440};
    assert.deepEqual(placePill(right, 6, null, 120, 12), {x: 3140, y: 6});
});

test('drop near the default spot snaps back', () => {
    assert.equal(positionFromDrop(mon, 40, 800 + 20, 40 + 10, 320), null);
});

test('drop elsewhere is saved as fractions that round-trip', () => {
    const pos = positionFromDrop(mon, 40, 100, 500, 320);
    assert.deepEqual(pos, {fx: 260 / 1920, fy: 500 / 1080});
    assert.deepEqual(placePill(mon, 40, pos, 320, 36), {x: 100, y: 500});
});

test('snap distance follows the scale factor', () => {
    assert.notEqual(positionFromDrop(mon, 40, 800 + 60, 40, 320, 1), null);
    assert.equal(positionFromDrop(mon, 40, 800 + 60, 40, 320, 2), null);
});
