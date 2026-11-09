import {test} from 'node:test';
import assert from 'node:assert/strict';

import {BAR_COUNT, barHeights, loudness, parseLevelMessage, smooth} from '../../gnome-extension/ui/level.js';

test('parses both GStreamer serializations', () => {
    assert.equal(parseLevelMessage('level, endtime=(guint64)1, rms=(double){ -42.5, -40.25 }, decay=(double){ -1 };'), -40.25);
    assert.equal(parseLevelMessage('level, peak=(GValueArray)< -3 >, rms=(GValueArray)< -61.2 >;'), -61.2);
    assert.equal(parseLevelMessage('level, peak=(double){ -3 };'), null);
    assert.equal(parseLevelMessage('rms=(double){ -inf }'), null);
});

test('loudness maps silence to 0 and full scale to 1', () => {
    assert.equal(loudness(-90), 0);
    assert.equal(loudness(-Infinity), 0);
    assert.equal(loudness(0), 1);
    assert.ok(loudness(-20) > 0.7 && loudness(-20) < 0.8);
});

test('smoothing rises instantly and falls gently', () => {
    assert.equal(smooth(0.2, 0.9), 0.9);
    const fallen = smooth(0.9, 0);
    assert.ok(fallen > 0.3 && fallen < 0.9);
});

test('bars are flat when silent and taller when loud', () => {
    const quiet = barHeights(0, 3);
    assert.equal(quiet.length, BAR_COUNT);
    assert.ok(quiet.every(h => h === quiet[0]), 'silence is flat');
    const loud = barHeights(1, 3);
    assert.ok(loud.every((h, i) => h > quiet[i]));
    assert.ok(loud.every(h => h <= 1));
});
