// Turns a stored event into what the island shows. Pure functions only
// (no GNOME imports), so this is unit-tested with Node.
//
// Rule: show only what the event says. Never invent battery levels, call
// mute state or anything else a source did not report.

import {isBrowserApp} from '../services/privacy-model.js';

export function formatClock(ms) {
    const total = Math.max(0, Math.floor(ms / 1000));
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = String(total % 60).padStart(2, '0');
    return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
}

function join(...parts) {
    return parts.filter(p => typeof p === 'string' && p.length > 0).join(' · ');
}

const CONFIDENCE_NOTE = {
    confirmed: '',
    probable: 'Probably in a call',
    audio_only: 'Audio activity',
    unknown: 'Unconfirmed',
};

const DEVICE_ICON = {
    headphones: 'audio-headphones-symbolic',
    speaker: 'audio-speakers-symbolic',
    keyboard: 'input-keyboard-symbolic',
    mouse: 'input-mouse-symbolic',
    phone: 'phone-symbolic',
};

const SEVERITY_ICON = {
    info: 'dialog-information-symbolic',
    warning: 'dialog-warning-symbolic',
    critical: 'dialog-error-symbolic',
};

/**
 * Better icons for system events from our own integrations (battery,
 * network, drives and warnings from the daemon; audio devices from the
 * extension's `gnome-audio`).
 * Returns null for anything else.
 */
function systemIcon(event) {
    const p = event.payload;
    if (event.source.integration === 'upower') {
        if (event.id === 'upower:warning')
            return p.severity === 'critical' ? 'battery-caution-symbolic' : 'battery-low-symbolic';
        const percent = Number.parseInt(p.detail ?? '', 10);
        const level = Number.isFinite(percent) ? Math.min(100, Math.max(0, Math.round(percent / 10) * 10)) : 100;
        switch (p.title) {
        case 'Charging':
            return level === 100 ? 'battery-level-100-charged-symbolic' : `battery-level-${level}-charging-symbolic`;
        case 'Plugged in':
        case 'Fully charged':
            return `battery-level-${level}-plugged-in-symbolic`;
        default:
            return `battery-level-${level}-symbolic`;
        }
    }
    if (event.source.integration === 'network') {
        if (event.id === 'network:portal')
            return 'network-wireless-acquiring-symbolic';
        if (event.id.startsWith('network:vpn:'))
            return p.detail === 'VPN disconnected' ? 'network-vpn-disconnected-symbolic' : 'network-vpn-symbolic';
        if (p.title === 'Offline')
            return 'network-offline-symbolic';
        return p.detail === 'Ethernet connected' ? 'network-wired-symbolic' : 'network-wireless-symbolic';
    }
    if (event.source.integration === 'storage')
        return p.detail === 'USB drive removed' ? 'media-removable-symbolic' : 'drive-harddisk-usb-symbolic';
    if (event.source.integration === 'health')
        return event.id === 'health:memory' ? 'computer-symbolic' : 'dialog-warning-symbolic';
    if (event.source.integration === 'gnome-audio') {
        if (event.id === 'audio:output')
            return 'audio-speakers-symbolic';
        if (event.id === 'audio:mic-mute' && p.title === 'Microphone muted')
            return 'microphone-disabled-symbolic';
        return 'audio-input-microphone-symbolic';
    }
    return null;
}

/**
 * Microphone / camera in use (from the extension's privacy source):
 * orange for the microphone only, green when the camera is on, a running
 * timer, and voice waves while the microphone is part of it.
 */
function describePrivacy(event, now, vm) {
    const p = event.payload;
    const camera = p.title.startsWith('Camera');
    const muted = p.title.endsWith('muted');
    const mic = p.title.includes('icrophone') && !muted;
    vm.accent = camera ? 'green' : 'orange';
    vm.iconName = camera ? 'camera-web-symbolic'
        : muted ? 'microphone-disabled-symbolic' : 'audio-input-microphone-symbolic';
    vm.subtitle = p.detail ?? '';
    const finished = isFinished(event);
    vm.trailing = formatClock((finished ? event.updated_at : now) - event.created_at);
    vm.live = !finished;
    vm.waves = mic && !finished;
}

const BROWSER_FAMILIES = {
    chromium: 'chrome', chrome: 'chrome', 'google-chrome': 'chrome', brave: 'chrome',
    vivaldi: 'chrome', opera: 'chrome', 'microsoft-edge': 'chrome', msedge: 'chrome',
    firefox: 'firefox', librewolf: 'firefox', waterfox: 'firefox', floorp: 'firefox',
};

/**
 * A browser reports only one media player to Linux (MPRIS). When the
 * browser extension reports that browser's tabs itself
 * (`browser:media:<family>:<tab>`), the browser's own MPRIS player is a
 * duplicate and is hidden. Other browsers' players stay.
 *
 * @param {object[]} events - ordered events
 * @returns {object[]} the list to show
 */
export function foldBrowserMedia(events) {
    const covered = new Set();
    for (const e of events) {
        const match = /^browser:media:(chrome|firefox):/.exec(e.id);
        if (match && !isFinished(e))
            covered.add(match[1]);
    }
    if (covered.size === 0)
        return events;
    return events.filter(e => {
        const match = /^mpris:([a-z-]+)/.exec(e.id);
        return !(match && covered.has(BROWSER_FAMILIES[match[1]]));
    });
}

/**
 * Playing media comes before paused media, so the island shows what is
 * actually playing. Media events keep their places in the list; only
 * their order among themselves changes (newest playing first, as before).
 *
 * @param {object[]} events - ordered events
 * @returns {object[]} the list to show
 */
export function playingFirst(events) {
    const isMedia = e => e.payload.type === 'media';
    const media = events.filter(isMedia);
    const playing = e => e.payload.playing === true && !isFinished(e);
    const sorted = [...media.filter(playing), ...media.filter(e => !playing(e))];
    let next = 0;
    return events.map(e => (isMedia(e) ? sorted[next++] : e));
}

/**
 * The browser extension says which site holds the microphone
 * (`browser:mic:<tab>`); Linux only knows "Google Chrome". These events
 * are never shown themselves: when the microphone bubble's app is a
 * browser, it takes the newest site's name and logo instead.
 *
 * @param {object[]} events - ordered events
 * @returns {object[]} the list to show
 */
export function foldBrowserMic(events) {
    const sites = events.filter(e => e.id.startsWith('browser:mic:'));
    if (sites.length === 0)
        return events;
    const site = sites.filter(e => !isFinished(e)).sort((a, b) => b.created_at - a.created_at)[0];
    return events.filter(e => !sites.includes(e)).map(e => {
        if (!site || e.id !== 'privacy:capture' || e.source.icon || !isBrowserApp(e.source.app))
            return e;
        const detail = e.payload.detail?.replace(e.source.app, site.source.app) ?? site.source.app;
        const source = {...e.source, app: site.source.app};
        if (site.source.icon)
            source.icon = site.source.icon;
        return {...e, source, payload: {...e.payload, detail}};
    });
}

/**
 * During a confirmed call (e.g. Google Meet through the browser
 * extension) the separate "Microphone / camera in use" event is folded
 * into the call: it is hidden, and the call shows the voice waves while
 * the microphone is live and the call is not muted.
 *
 * @param {object[]} events - ordered events
 * @returns {{events: object[], wavesFor: string|null}} the list to show,
 *   and the id of the call that should show waves (or null)
 */
export function foldMicIntoCalls(events) {
    const call = events.find(e => e.payload.type === 'call' &&
        e.payload.confidence === 'confirmed' && !isFinished(e));
    const mic = events.find(e => e.id === 'privacy:capture');
    if (!call || !mic)
        return {events, wavesFor: null};
    const title = mic.payload.title ?? '';
    const micLive = title.includes('icrophone') && !title.endsWith('muted') && !isFinished(mic);
    // The camera state moves into the call too, unless the call already
    // knows it (e.g. Meet reads its own camera button).
    const cameraOn = title.startsWith('Camera') && !isFinished(mic);
    const withCamera = cameraOn && call.payload.camera === undefined
        ? {...call, payload: {...call.payload, camera: true}}
        : call;
    return {
        events: events.filter(e => e !== mic).map(e => (e === call ? withCamera : e)),
        wavesFor: micLive && call.payload.muted !== true ? call.id : null,
    };
}

/** Current playback position, extrapolated while playing. */
export function mediaPosition(event, now) {
    const p = event.payload;
    if (p.position_ms === undefined)
        return null;
    let pos = p.position_ms;
    if (p.playing && !isFinished(event))
        pos += Math.max(0, now - event.updated_at);
    if (p.duration_ms !== undefined)
        pos = Math.min(pos, p.duration_ms);
    return pos;
}

function isFinished(event) {
    return event.state === 'ended' || event.state === 'failed' || event.state === 'expired';
}

/**
 * @param {object} event - a stored event (see docs/EVENT_SYSTEM.md)
 * @param {number} now - milliseconds since the epoch
 * @returns {{iconName: string, accent: string, title: string, subtitle: string,
 *   trailing: string, progress: number|null, live: boolean}}
 *   accent is one of: neutral, green, blue, pink, orange, red.
 *   progress is 0..1 or null. live means the view changes with time and
 *   must be refreshed every second while visible. waves means the view
 *   shows microphone voice bars. logo is the source's logo (or null);
 *   indicators are small status icon names shown next to the time.
 */
export function describe(event, now) {
    const p = event.payload;
    const app = event.source.app;
    const vm = {
        iconName: 'dialog-information-symbolic',
        accent: 'neutral',
        title: '',
        subtitle: '',
        trailing: '',
        progress: null,
        live: false,
        waves: false,
        // The app's or site's logo (source.icon), shown instead of iconName.
        logo: event.source.icon ?? null,
        // Small status icons next to the time, used when a logo takes the
        // main icon's place (camera on, muted, paused).
        indicators: [],
    };

    switch (p.type) {
    case 'call': {
        vm.iconName = 'call-start-symbolic';
        vm.title = p.title;
        if (event.state === 'ended' || event.state === 'failed') {
            vm.accent = 'neutral';
            vm.iconName = 'call-stop-symbolic';
            vm.subtitle = join(app, event.state === 'ended' ? 'Call ended' : 'Call failed');
            vm.trailing = formatClock(event.updated_at - event.created_at);
        } else {
            vm.accent = p.confidence === 'confirmed' ? 'green' : 'orange';
            vm.subtitle = join(app, CONFIDENCE_NOTE[p.confidence],
                p.camera === true ? 'Camera on' : '', p.muted === true ? 'Muted' : '');
            if (p.camera === true)
                vm.iconName = 'camera-web-symbolic';
            vm.trailing = formatClock(now - event.created_at);
            vm.live = true;
        }
        break;
    }
    case 'media': {
        vm.accent = 'pink';
        vm.iconName = p.playing ? 'media-playback-start-symbolic' : 'media-playback-pause-symbolic';
        vm.title = p.title;
        vm.subtitle = join(p.artist, p.album) || app;
        let pos = mediaPosition(event, now);
        // Some players (often browsers) report position 0 and no length
        // when they don't know; a frozen 0:00 would be misleading.
        if (pos === 0 && !p.duration_ms && !p.playing)
            pos = null;
        if (pos !== null && p.duration_ms)
            vm.progress = pos / p.duration_ms;
        if (pos !== null)
            vm.trailing = formatClock(pos);
        vm.live = p.playing && pos !== null && !isFinished(event);
        break;
    }
    case 'progress': {
        if (event.category === 'transfer')
            vm.iconName = 'folder-download-symbolic';
        else if (event.source.integration === 'pacman')
            vm.iconName = 'software-update-available-symbolic';
        else if (event.source.integration === 'cli' && event.category === 'custom')
            vm.iconName = 'system-run-symbolic';
        else
            vm.iconName = 'view-refresh-symbolic';
        vm.title = p.title;
        vm.subtitle = join(app, p.detail);
        if (event.state === 'failed') {
            vm.accent = 'red';
            vm.trailing = 'Failed';
        } else if (event.state === 'ended') {
            vm.accent = 'green';
            vm.iconName = 'object-select-symbolic';
            vm.trailing = 'Done';
            vm.progress = 1;
        } else {
            vm.accent = 'blue';
            if (p.percent !== undefined) {
                vm.progress = p.percent / 100;
                vm.trailing = `${Math.round(p.percent)}%`;
            } else if (event.category !== 'transfer') {
                // A task with no percentage (a command, a system update):
                // how long it has been running.
                vm.trailing = formatClock(now - event.created_at);
                vm.live = true;
            }
        }
        break;
    }
    case 'device': {
        vm.iconName = DEVICE_ICON[p.device_kind] ?? 'bluetooth-active-symbolic';
        vm.title = p.name;
        vm.subtitle = p.connected ? 'Connected' : 'Disconnected';
        vm.accent = p.connected ? 'blue' : 'neutral';
        if (p.battery_percent !== undefined)
            vm.trailing = `${p.battery_percent}%`;
        break;
    }
    case 'notification':
        vm.iconName = 'mail-unread-symbolic';
        vm.title = p.title;
        vm.subtitle = join(app, p.body);
        break;
    case 'system':
        vm.iconName = systemIcon(event) ?? SEVERITY_ICON[p.severity];
        vm.accent = p.severity === 'info' ? 'neutral' : p.severity === 'warning' ? 'orange' : 'red';
        vm.title = p.title;
        vm.subtitle = p.detail ?? app;
        break;
    }

    if (event.source.integration === 'gnome-privacy' && p.type === 'system')
        describePrivacy(event, now, vm);

    if (vm.logo) {
        if (p.type === 'call' && !isFinished(event)) {
            if (p.camera === true)
                vm.indicators.push('camera-web-symbolic');
            if (p.muted === true)
                vm.indicators.push('microphone-disabled-symbolic');
        } else if (event.source.integration === 'gnome-privacy' && p.type === 'system' && !isFinished(event)) {
            if (p.title.startsWith('Camera'))
                vm.indicators.push('camera-web-symbolic');
            if (p.title.endsWith('muted'))
                vm.indicators.push('microphone-disabled-symbolic');
        } else if (p.type === 'media' && !p.playing && !isFinished(event)) {
            vm.indicators.push('media-playback-pause-symbolic');
        }
    }

    if (vm.progress !== null)
        vm.progress = Math.min(1, Math.max(0, vm.progress));
    return vm;
}
