import {test} from 'node:test';
import assert from 'node:assert/strict';

import {
    ANCHORS, anchorSpot, customSpot, fractionsFor, placePill, positionFromDrop,
} from '../../gnome-extension/ui/placement.js';

const mon = {x: 0, y: 0, width: 1920, height: 1080};
const opts = {topInset: 34, margin: 6};
const at = (anchor, w, h, o = opts) => placePill(anchorSpot(anchor, mon, o), mon, w, h);

test('top center sits below the top bar and grows downward', () => {
    assert.deepEqual(at('top-center', 320, 36), {x: 800, y: 40});
    assert.deepEqual(at('top-center', 440, 200), {x: 740, y: 40});
});

test('corners keep the margin and grow away from their edges', () => {
    assert.deepEqual(at('top-left', 320, 36), {x: 6, y: 40});
    assert.deepEqual(at('top-right', 320, 36), {x: 1920 - 6 - 320, y: 40});
    assert.deepEqual(at('bottom-left', 320, 36), {x: 6, y: 1080 - 6 - 36});
    assert.deepEqual(at('bottom-right', 440, 200), {x: 1920 - 6 - 440, y: 1080 - 6 - 200});
});

test('bottom center grows upward', () => {
    const small = at('bottom-center', 320, 36);
    const big = at('bottom-center', 440, 200);
    assert.equal(small.y + 36, big.y + 200);
});

test('center stays centered on both axes', () => {
    assert.deepEqual(at('center', 200, 100), {x: 860, y: 490});
});

test('the top bar inset applies only to top anchors', () => {
    assert.equal(at('bottom-center', 100, 10).y, 1080 - 6 - 10);
    assert.equal(at('top-center', 100, 10, {topInset: 0, margin: 6}).y, 6);
});

test('custom default uses saved fractions', () => {
    const custom = {fx: 0.25, fy: 0.5};
    assert.deepEqual(placePill(anchorSpot('custom', mon, {...opts, custom}), mon, 100, 20), {x: 430, y: 540});
});

test('unknown anchor falls back to top center', () => {
    assert.deepEqual(at('nonsense', 320, 36), at('top-center', 320, 36));
    assert.ok(ANCHORS.includes('custom'));
});

test('never leaves the monitor, even when expanding near an edge', () => {
    assert.deepEqual(placePill(customSpot(mon, {fx: 1, fy: 1}), mon, 400, 200), {x: 1520, y: 880});
    assert.deepEqual(placePill(customSpot(mon, {fx: 0, fy: 0}), mon, 400, 200), {x: 0, y: 0});
});

test('works on a second monitor', () => {
    const right = {x: 1920, y: 0, width: 2560, height: 1440};
    assert.deepEqual(placePill(anchorSpot('top-center', right, {margin: 6}), right, 120, 12), {x: 3140, y: 6});
});

test('fractions round-trip', () => {
    const f = fractionsFor(mon, 100, 500, 320);
    assert.deepEqual(placePill(customSpot(mon, f), mon, 320, 36), {x: 100, y: 500});
});

test('drop near the default spot snaps back, wherever the default is', () => {
    const topCenter = anchorSpot('top-center', mon, opts);
    assert.equal(positionFromDrop(mon, topCenter, 820, 50, 320, 36), null);
    const bottomRight = anchorSpot('bottom-right', mon, opts);
    assert.equal(positionFromDrop(mon, bottomRight, 1580, 1030, 320, 36), null);
    assert.notEqual(positionFromDrop(mon, bottomRight, 820, 50, 320, 36), null);
});

test('drop elsewhere is saved as fractions', () => {
    const pos = positionFromDrop(mon, anchorSpot('top-center', mon, opts), 100, 500, 320, 36);
    assert.deepEqual(pos, {fx: 260 / 1920, fy: 500 / 1080});
});

test('snap distance follows the scale factor', () => {
    const spot = anchorSpot('top-center', mon, opts);
    assert.notEqual(positionFromDrop(mon, spot, 860, 40, 320, 36, 1), null);
    assert.equal(positionFromDrop(mon, spot, 860, 40, 320, 36, 2), null);
});

test('a dragged spot survives a monitor change (unplugged, smaller, rotated)', () => {
    const big = {x: 1920, y: 0, width: 3840, height: 2160};
    // Dragged near the bottom-right of a 4K screen on the right…
    const f = fractionsFor(big, 3400 + 1920, 2000, 320);
    // …that screen is unplugged: the same place on the laptop screen.
    const laptop = {x: 0, y: 0, width: 1366, height: 768};
    const p = placePill(customSpot(laptop, f), laptop, 320, 36);
    assert.ok(p.x >= 0 && p.x + 320 <= 1366 && p.y >= 0 && p.y + 36 <= 768, `inside the laptop screen: ${JSON.stringify(p)}`);
    // A portrait (rotated) screen narrower than the expanded island.
    const portrait = {x: 0, y: 0, width: 400, height: 1280};
    const q = placePill(customSpot(portrait, f), portrait, 440, 200);
    assert.equal(q.x, 0, 'wider than the screen: starts at its left edge');
    assert.ok(q.y + 200 <= 1280);
});

test('monitors left of or above the primary (negative coordinates)', () => {
    const left = {x: -2560, y: -200, width: 2560, height: 1440};
    const p = placePill(anchorSpot('top-center', left, {margin: 6}), left, 120, 12);
    assert.deepEqual(p, {x: -1340, y: -194});
    const f = fractionsFor(left, -2000, 300, 320);
    assert.deepEqual(placePill(customSpot(left, f), left, 320, 36), {x: -2000, y: 300});
});
