import {test} from 'node:test';
import assert from 'node:assert/strict';

import {privacyInput} from '../../gnome-extension/services/privacy-model.js';
import {validateInput} from '../../gnome-extension/services/protocol.js';

const valid = input => validateInput({...input, state: 'started'}).ok;

test('nothing in use, nothing shown', () => {
    assert.equal(privacyInput({apps: [], camera: false, muted: false}), null);
});

test('microphone offers Mute, muted offers Unmute', () => {
    const on = privacyInput({apps: ['Google Chrome'], camera: false, muted: false});
    assert.equal(on.payload.title, 'Microphone');
    assert.deepEqual(on.actions, [{id: 'mute', label: 'Mute'}]);
    assert.ok(valid(on));
    const off = privacyInput({apps: ['Google Chrome'], camera: false, muted: true});
    assert.equal(off.payload.title, 'Microphone muted');
    assert.deepEqual(off.actions, [{id: 'unmute', label: 'Unmute'}]);
    assert.ok(valid(off));
});

test('camera variants', () => {
    assert.equal(privacyInput({apps: ['Zoom'], camera: true, muted: false}).payload.title, 'Camera and microphone');
    assert.equal(privacyInput({apps: ['Zoom'], camera: true, muted: true}).payload.title, 'Camera and microphone muted');
    const cam = privacyInput({apps: [], camera: true, muted: true});
    assert.equal(cam.payload.title, 'Camera', 'no microphone, nothing to mute');
    assert.deepEqual(cam.actions, []);
    assert.equal(cam.payload.detail, 'In use');
    assert.ok(valid(cam));
});

test('several apps are listed', () => {
    const input = privacyInput({apps: ['Slack', 'Google Chrome'], camera: false, muted: false});
    assert.equal(input.payload.detail, 'Slack, Google Chrome');
    assert.equal(input.source.app, 'Slack');
});

test('app names drop the "input" suffix apps give their audio connection', async () => {
    const {appName} = await import('../../gnome-extension/services/privacy-model.js');
    assert.equal(appName('Google Chrome input'), 'Google Chrome');
    assert.equal(appName('Chromium input'), 'Chromium');
    assert.equal(appName('Slack'), 'Slack');
    assert.equal(appName(''), 'An app');
    assert.equal(appName(null), 'An app');
});

test('the recording app\'s logo', async () => {
    const {appIcon} = await import('../../gnome-extension/services/privacy-model.js');
    assert.equal(appIcon('com.slack.Slack.desktop'), 'app:com.slack.Slack');
    assert.equal(appIcon('zoom'), 'app:zoom');
    assert.equal(appIcon(null), null);
    assert.equal(appIcon('../evil'), null);
    const withLogo = privacyInput({apps: ['Slack'], camera: false, muted: false, icon: 'app:com.slack.Slack'});
    assert.equal(withLogo.source.icon, 'app:com.slack.Slack');
    assert.equal(withLogo.payload.title, 'Microphone', 'still orange "Microphone", not a call');
    const camOnly = privacyInput({apps: [], camera: true, muted: false, icon: 'app:x'});
    assert.equal(camOnly.source.icon, undefined, 'no app logo for the camera alone');
});

test('browsers get no app logo', async () => {
    const {isBrowserApp} = await import('../../gnome-extension/services/privacy-model.js');
    assert.ok(isBrowserApp('Google Chrome'));
    assert.ok(isBrowserApp(null, 'org.mozilla.firefox'));
    assert.ok(isBrowserApp('', '', 'brave-browser'));
    assert.ok(!isBrowserApp('Slack', 'com.slack.Slack'));
    assert.ok(!isBrowserApp('Zoom'));
});
