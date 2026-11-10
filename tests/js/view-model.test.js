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

test('battery and audio events get matching icons', () => {
    const sys = (id, integration, title, detail, severity = 'info') => describe({
        ...base, id, category: 'system', state: 'started', source: {app: 'X', integration},
        payload: {type: 'system', title, detail, severity},
    }, 0).iconName;
    assert.equal(sys('upower:power', 'upower', 'Charging', '64%'), 'battery-level-60-charging-symbolic');
    assert.equal(sys('upower:power', 'upower', 'Charging', '100%'), 'battery-level-100-charged-symbolic');
    assert.equal(sys('upower:power', 'upower', 'Plugged in', '100%'), 'battery-level-100-plugged-in-symbolic');
    assert.equal(sys('upower:power', 'upower', 'On battery', '47%'), 'battery-level-50-symbolic');
    assert.equal(sys('upower:warning', 'upower', 'Battery low', '9% remaining', 'warning'), 'battery-low-symbolic');
    assert.equal(sys('upower:warning', 'upower', 'Battery critical', '3%', 'critical'), 'battery-caution-symbolic');
    assert.equal(sys('audio:output', 'gnome-audio', 'Galaxy Buds', 'Audio output'), 'audio-speakers-symbolic');
    assert.equal(sys('audio:mic-mute', 'gnome-audio', 'Microphone muted', 'Built-in'), 'microphone-disabled-symbolic');
    assert.equal(sys('x', 'cli', 'Disk full', '', 'warning'), 'dialog-warning-symbolic', 'others unchanged');
});

test('network, drive and warning icons', () => {
    const icon = (id, integration, title, detail) => describe({
        ...base, id, category: 'system', state: 'started', source: {app: 'X', integration},
        payload: {type: 'system', title, detail, severity: 'info'},
    }, 0).iconName;
    assert.equal(icon('network:primary', 'network', 'Home', 'Wi-Fi connected'), 'network-wireless-symbolic');
    assert.equal(icon('network:primary', 'network', 'Office', 'Ethernet connected'), 'network-wired-symbolic');
    assert.equal(icon('network:primary', 'network', 'Offline', 'No network connection'), 'network-offline-symbolic');
    assert.equal(icon('network:vpn:Work', 'network', 'Work', 'VPN connected'), 'network-vpn-symbolic');
    assert.equal(icon('network:vpn:Work', 'network', 'Work', 'VPN disconnected'), 'network-vpn-disconnected-symbolic');
    assert.equal(icon('storage:x', 'storage', 'SanDisk', 'USB drive connected'), 'drive-harddisk-usb-symbolic');
    assert.equal(icon('storage:x', 'storage', 'SanDisk', 'USB drive removed'), 'media-removable-symbolic');
    assert.equal(icon('health:temperature', 'health', 'High temperature', 'CPU at 93 °C'), 'dialog-warning-symbolic');
});

test('microphone is orange with waves, camera is green, both run a timer', () => {
    const ev = (title, state = 'started') => describe({
        ...base, id: 'privacy:capture', category: 'call', state, created_at: 0, updated_at: 0,
        source: {app: 'Google Chrome', integration: 'gnome-privacy'},
        payload: {type: 'system', title, detail: 'Google Chrome', severity: 'info'},
    }, 192_000);
    const mic = ev('Microphone');
    assert.equal(mic.accent, 'orange');
    assert.equal(mic.iconName, 'audio-input-microphone-symbolic');
    assert.equal(mic.trailing, '3:12');
    assert.equal(mic.subtitle, 'Google Chrome');
    assert.ok(mic.live && mic.waves);
    const both = ev('Camera and microphone');
    assert.equal(both.accent, 'green');
    assert.equal(both.iconName, 'camera-web-symbolic');
    assert.ok(both.waves);
    const cam = ev('Camera');
    assert.equal(cam.accent, 'green');
    assert.equal(cam.waves, false, 'no microphone, no waves');
    const ended = ev('Microphone', 'ended');
    assert.equal(ended.waves, false);
    assert.equal(ended.live, false);
    assert.equal(describe({...base, category: 'notification', state: 'started',
        payload: {type: 'notification', title: 'x'}}, 0).waves, false);
});

test('muted microphone: no waves, crossed-out icon, still timed', () => {
    const vm = describe({
        ...base, id: 'privacy:capture', category: 'call', state: 'updated', created_at: 0, updated_at: 0,
        source: {app: 'Slack', integration: 'gnome-privacy'},
        payload: {type: 'system', title: 'Microphone muted', detail: 'Slack', severity: 'info'},
    }, 65_000);
    assert.equal(vm.waves, false);
    assert.equal(vm.iconName, 'microphone-disabled-symbolic');
    assert.equal(vm.accent, 'orange');
    assert.equal(vm.trailing, '1:05');
    assert.equal(vm.live, true);
});

test('a confirmed call absorbs the microphone event and takes its waves', async () => {
    const {foldMicIntoCalls} = await import('../../gnome-extension/ui/view-model.js');
    const call = (muted, state = 'started') => ({...base, id: 'browser:call:1', category: 'call', state,
        source: {app: 'Google Meet', integration: 'browser'},
        payload: {type: 'call', title: 'Weekly sync', confidence: 'confirmed', muted}});
    const mic = (title = 'Microphone') => ({...base, id: 'privacy:capture', category: 'call', state: 'started',
        source: {app: 'Google Chrome', integration: 'gnome-privacy'},
        payload: {type: 'system', title, detail: 'Google Chrome', severity: 'info'}});
    const note = {...base, id: 'n', category: 'notification', state: 'started',
        payload: {type: 'notification', title: 'x'}};

    let r = foldMicIntoCalls([call(false), mic(), note]);
    assert.deepEqual(r.events.map(e => e.id), ['browser:call:1', 'n']);
    assert.equal(r.wavesFor, 'browser:call:1');
    assert.equal(foldMicIntoCalls([call(true), mic()]).wavesFor, null, 'muted in Meet: no waves');
    assert.equal(foldMicIntoCalls([call(false), mic('Microphone muted')]).wavesFor, null, 'muted by the island: no waves');
    r = foldMicIntoCalls([mic(), note]);
    assert.equal(r.events.length, 2, 'no call: the microphone event stays');
    r = foldMicIntoCalls([call(false, 'ended'), mic()]);
    assert.equal(r.events.length, 2, 'ended call: the microphone event shows again');

    const muted = describe(call(true), 60_000);
    assert.match(muted.subtitle, /Muted/);
    assert.doesNotMatch(describe(call(undefined), 0).subtitle, /Muted/, 'unknown mute state is not shown');
});

test('browser tabs from the extension hide that browser\'s own MPRIS player only', async () => {
    const {foldBrowserMedia} = await import('../../gnome-extension/ui/view-model.js');
    const ev = (id, state = 'started') => ({...base, id, category: 'media', state,
        payload: {type: 'media', title: id, playing: true}});
    const list = [
        ev('browser:media:chrome:12'), ev('browser:media:chrome:14'),
        ev('mpris:chromium.instance4512'), ev('mpris:firefox.instance_1_9'), ev('mpris:spotify'),
    ];
    assert.deepEqual(foldBrowserMedia(list).map(e => e.id),
        ['browser:media:chrome:12', 'browser:media:chrome:14', 'mpris:firefox.instance_1_9', 'mpris:spotify']);
    assert.equal(foldBrowserMedia([ev('mpris:chromium.instance1')]).length, 1, 'no extension tabs: MPRIS stays');
    assert.equal(foldBrowserMedia([ev('browser:media:chrome:1', 'ended'), ev('mpris:chromium.instance1')]).length, 2,
        'ended tab: MPRIS shows again');
});

test('camera in a call: from the call itself, or carried over from the camera bubble', async () => {
    const {foldMicIntoCalls} = await import('../../gnome-extension/ui/view-model.js');
    const call = camera => ({...base, id: 'browser:call:3', category: 'call', state: 'started',
        source: {app: 'Call', integration: 'browser'},
        payload: {type: 'call', title: 'Slack', confidence: 'confirmed', camera}});
    const capture = title => ({...base, id: 'privacy:capture', category: 'call', state: 'started',
        source: {app: 'Google Chrome', integration: 'gnome-privacy'},
        payload: {type: 'system', title, detail: 'Google Chrome', severity: 'info'}});

    const on = describe(call(true), 0);
    assert.equal(on.iconName, 'camera-web-symbolic');
    assert.match(on.subtitle, /Camera on/);
    assert.doesNotMatch(describe(call(false), 0).subtitle, /Camera/);

    const folded = foldMicIntoCalls([call(undefined), capture('Camera and microphone')]);
    assert.equal(folded.events.length, 1);
    assert.equal(folded.events[0].payload.camera, true, 'camera carried into the call');
    assert.equal(folded.wavesFor, 'browser:call:3');
    const known = foldMicIntoCalls([call(false), capture('Camera and microphone')]);
    assert.equal(known.events[0].payload.camera, false, 'the call\'s own answer wins');
});

test('logos replace the main icon; camera, muted and paused move next to the time', () => {
    const meet = describe({...base, id: 'browser:call:1', category: 'call', state: 'started',
        source: {app: 'Google Meet', integration: 'browser', icon: 'data:image/png;base64,AAAA'},
        payload: {type: 'call', title: 'Standup', confidence: 'confirmed', camera: true, muted: true}}, 0);
    assert.equal(meet.logo, 'data:image/png;base64,AAAA');
    assert.deepEqual(meet.indicators, ['camera-web-symbolic', 'microphone-disabled-symbolic']);

    const paused = describe({...base, category: 'media', state: 'updated',
        source: {app: 'Spotify', integration: 'mpris', icon: 'app:spotify'},
        payload: {type: 'media', title: 'Song', playing: false}}, 0);
    assert.deepEqual(paused.indicators, ['media-playback-pause-symbolic']);

    const noLogo = describe({...base, category: 'call', state: 'started',
        payload: {type: 'call', title: 'x', confidence: 'confirmed', camera: true}}, 0);
    assert.equal(noLogo.logo, null);
    assert.deepEqual(noLogo.indicators, [], 'without a logo the main icon already shows the camera');
    assert.equal(noLogo.iconName, 'camera-web-symbolic');
});

test('playing media is shown before paused media', async () => {
    const {playingFirst} = await import('../../gnome-extension/ui/view-model.js');
    const media = (id, playing) => ({...base, id, state: 'updated', payload: {type: 'media', title: id, playing}});
    const call = {...base, id: 'call', state: 'started', payload: {type: 'call', title: 'x', confidence: 'confirmed'}};
    // Ordered newest first: the paused one started last.
    const ids = list => playingFirst(list).map(e => e.id);
    assert.deepEqual(ids([media('paused', false), media('playing', true)]), ['playing', 'paused']);
    assert.deepEqual(ids([media('newer', true), media('older', true)]), ['newer', 'older'], 'both playing: newest first');
    assert.deepEqual(ids([call, media('paused', false), media('playing', true)]), ['call', 'playing', 'paused'],
        'other events keep their place');
    assert.deepEqual(ids([media('a', false), media('b', false)]), ['a', 'b'], 'both paused: unchanged');
});

test('microphone bubble with an app logo', () => {
    const mic = (title) => ({...base, id: 'privacy:capture', state: 'started',
        source: {app: 'Slack', integration: 'gnome-privacy', icon: 'app:com.slack.Slack'},
        payload: {type: 'system', title, detail: 'Slack', severity: 'info'}});
    const on = describe(mic('Microphone'), 0);
    assert.equal(on.logo, 'app:com.slack.Slack');
    assert.equal(on.accent, 'orange');
    assert.deepEqual(on.indicators, []);
    assert.deepEqual(describe(mic('Microphone muted'), 0).indicators, ['microphone-disabled-symbolic']);
    assert.deepEqual(describe(mic('Camera and microphone'), 0).indicators, ['camera-web-symbolic']);
});

test('the microphone bubble names the site when a browser records', async () => {
    const {foldBrowserMic} = await import('../../gnome-extension/ui/view-model.js');
    const capture = {...base, id: 'privacy:capture', state: 'started',
        source: {app: 'Google Chrome', integration: 'gnome-privacy'},
        payload: {type: 'system', title: 'Microphone', detail: 'Google Chrome', severity: 'info'}};
    const site = {...base, id: 'browser:mic:7', state: 'started', created_at: 5,
        source: {app: 'selawigram.com', integration: 'browser', icon: 'data:image/png;base64,AAAA'},
        payload: {type: 'system', title: 'Microphone', detail: 'selawigram.com', severity: 'info'}};
    const out = foldBrowserMic([capture, site]);
    assert.equal(out.length, 1, 'the site hint is never shown on its own');
    assert.equal(out[0].source.app, 'selawigram.com');
    assert.equal(out[0].source.icon, 'data:image/png;base64,AAAA');
    assert.equal(out[0].payload.detail, 'selawigram.com');
    const slack = {...capture, source: {app: 'Slack', integration: 'gnome-privacy', icon: 'app:slack'}};
    assert.equal(foldBrowserMic([slack, site])[0].source.app, 'Slack', 'a desktop app keeps its own name');
    assert.deepEqual(foldBrowserMic([site]), [], 'no mic bubble: nothing shown');
});
