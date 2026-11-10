// Tests the browser extension's pure logic (compiled by
// scripts/build-browser.sh) and checks that every event it builds is valid
// for the island's own protocol validator.
import {test} from 'node:test';
import assert from 'node:assert/strict';

import {
    downloadEvent, fileName, formatBytes,
} from '../../browser-extensions/build/js/model.js';
import {validateInput} from '../../gnome-extension/services/protocol.js';

const valid = event => {
    const r = validateInput(event);
    assert.ok(r.ok, r.error);
};

test('downloads: progress, done, cancelled, only the file name', () => {
    const item = {id: 3, filename: '/home/yousef/Downloads/archlinux-x86_64.iso', bytesReceived: 512 * 1024 * 1024,
        totalBytes: 1200 * 1024 * 1024, state: 'in_progress'};
    const running = downloadEvent(item, 'started');
    assert.equal(running.id, 'browser:download:3');
    assert.equal(running.payload.title, 'archlinux-x86_64.iso');
    assert.equal(running.payload.detail, '512 MB of 1.2 GB');
    assert.equal(running.payload.percent, 42.7);
    assert.deepEqual(running.actions.map(a => a.id), ['cancel']);
    assert.equal(running.persistent, true);
    valid(running);

    const done = downloadEvent({...item, state: 'complete'}, 'ended');
    assert.equal(done.payload.percent, 100);
    assert.equal(done.persistent, false);
    valid(done);

    const cancelled = downloadEvent({...item, state: 'interrupted', error: 'USER_CANCELED'}, 'failed');
    assert.equal(cancelled.payload.detail, 'Cancelled');
    valid(cancelled);

    const unknownSize = downloadEvent({...item, totalBytes: 0}, 'updated');
    assert.equal(unknownSize.payload.percent, undefined);
    assert.equal(unknownSize.payload.detail, '512 MB');
    valid(unknownSize);
});

test('helpers', () => {
    assert.equal(formatBytes(0), '0 B');
    assert.equal(formatBytes(1536), '1.5 KB');
    assert.equal(formatBytes(-1), '');
    assert.equal(fileName('C:\\Users\\me\\file.zip'), 'file.zip');
    assert.equal(fileName(''), 'Download');
    assert.equal(fileName('/tmp/a\u0007b.txt'), 'a b.txt');
});

test('media tabs: controls the page offers, paused expires, valid for the island', async () => {
    const {mediaEvent, PAUSED_TTL_MS} = await import('../../browser-extensions/build/js/model.js');
    const state = {site: 'YouTube', title: 'Midnight City', artist: 'M83', album: '', playing: true,
        positionMs: 61_500.4, durationMs: 243_000, canNext: true, canPrevious: false};
    const playing = mediaEvent('chrome', 12, state, 'started');
    assert.equal(playing.id, 'browser:media:chrome:12');
    assert.equal(playing.source.app, 'YouTube');
    assert.equal(playing.persistent, true);
    assert.deepEqual(playing.payload, {type: 'media', title: 'Midnight City', artist: 'M83',
        playing: true, position_ms: 61_500, duration_ms: 243_000});
    assert.deepEqual(playing.actions.map(a => a.id), ['play_pause', 'next'], 'no Previous when the page has none');
    assert.equal(playing.actions[0].label, 'Pause');
    valid(playing);

    const paused = mediaEvent('firefox', 3, {...state, playing: false, canPrevious: true}, 'updated');
    assert.equal(paused.persistent, false);
    assert.equal(paused.ttl_ms, PAUSED_TTL_MS);
    assert.deepEqual(paused.actions.map(a => a.label), ['Previous', 'Play', 'Next']);
    valid(paused);

    const ad = mediaEvent('chrome', 12, {...state, positionMs: null, durationMs: null}, 'updated');
    assert.equal('position_ms' in ad.payload, false, 'no timer during an ad');
    valid(ad);

    const ended = mediaEvent('chrome', 12, state, 'ended');
    assert.deepEqual(ended.actions, []);
    assert.equal(ended.payload.playing, false);
    valid(ended);
});

test('calls on any site: the site name, a timer, no buttons', async () => {
    const {siteName, webCallEvent} = await import('../../browser-extensions/build/js/model.js');
    assert.equal(siteName('https://app.slack.com/huddle/T1/C2'), 'Slack');
    assert.equal(siteName('https://teams.microsoft.com/v2/'), 'Microsoft Teams');
    assert.equal(siteName('https://teams.live.com/v2/'), 'Microsoft Teams');
    assert.equal(siteName('https://app.zoom.us/wc/123/join'), 'Zoom');
    assert.equal(siteName('https://discord.com/channels/1/2'), 'Discord');
    assert.equal(siteName('https://web.whatsapp.com/'), 'WhatsApp');
    assert.equal(siteName('https://www.call.example.org/room'), 'call.example.org', 'unknown sites by address');
    assert.equal(siteName('not a url'), 'Web call');

    const call = webCallEvent(9, 'Slack', 'started');
    assert.equal(call.id, 'browser:call:9');
    assert.equal(call.payload.title, 'Slack');
    assert.equal(call.payload.confidence, 'confirmed');
    assert.deepEqual(call.actions, []);
    valid(call);
    valid(webCallEvent(9, 'Slack', 'ended'));
});

const checked = event => {
    valid(event);
    return event;
};

test('call controls a site reports (Media Session)', async () => {
    const {NO_SESSION, parseSession, sessionCommand, siteName: siteNameOf, webCallEvent} = await import('../../browser-extensions/build/js/model.js');
    // Bad or hostile input is ignored.
    assert.deepEqual(parseSession(null), NO_SESSION);
    assert.deepEqual(parseSession('not json'), NO_SESSION);
    assert.deepEqual(parseSession('{"mic":"yes","actions":["hangup","rm -rf"]}'), {mic: null, camera: null, actions: ['hangup']});

    const session = parseSession('{"mic":true,"camera":false,"actions":["togglemicrophone","hangup"]}');
    assert.equal(sessionCommand(session, 'mute'), 'togglemicrophone');
    assert.equal(sessionCommand(session, 'unmute'), null, 'already unmuted: nothing to press');
    assert.equal(sessionCommand(session, 'leave'), 'hangup');
    assert.equal(sessionCommand(NO_SESSION, 'leave'), null);

    const call = checked(webCallEvent(5, 'Slack', 'started', session));
    assert.equal(call.payload.muted, false);
    assert.equal(call.payload.camera, false);
    assert.deepEqual(call.actions.map(a => a.id), ['mute', 'leave']);
    assert.equal(siteNameOf('https://meet.google.com/abc-defg-hij'), 'Google Meet');
    assert.equal(siteNameOf('https://chat.selawii.com/call/1'), 'Selawigram');
    const plain = checked(webCallEvent(5, 'Slack', 'started'));
    assert.deepEqual(plain.actions, [], 'a site that reports nothing: no buttons');
    assert.equal(plain.payload.muted, undefined);
    assert.deepEqual(checked(webCallEvent(5, 'Slack', 'ended', session)).actions, []);
});
