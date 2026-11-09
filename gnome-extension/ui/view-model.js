// Turns a stored event into what the island shows. Pure functions only
// (no GNOME imports), so this is unit-tested with Node.
//
// Rule: show only what the event says. Never invent battery levels, call
// mute state or anything else a source did not report.

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
 *   must be refreshed every second while visible.
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
            vm.subtitle = join(app, CONFIDENCE_NOTE[p.confidence]);
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
        const pos = mediaPosition(event, now);
        if (pos !== null && p.duration_ms)
            vm.progress = pos / p.duration_ms;
        if (pos !== null)
            vm.trailing = formatClock(pos);
        vm.live = p.playing && pos !== null && !isFinished(event);
        break;
    }
    case 'progress': {
        vm.iconName = event.category === 'transfer'
            ? 'folder-download-symbolic' : 'emblem-synchronizing-symbolic';
        vm.title = p.title;
        vm.subtitle = join(app, p.detail);
        if (event.state === 'failed') {
            vm.accent = 'red';
            vm.trailing = 'Failed';
        } else if (event.state === 'ended') {
            vm.accent = 'green';
            vm.iconName = 'emblem-ok-symbolic';
            vm.trailing = 'Done';
            vm.progress = 1;
        } else {
            vm.accent = 'blue';
            if (p.percent !== undefined) {
                vm.progress = p.percent / 100;
                vm.trailing = `${Math.round(p.percent)}%`;
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
        vm.iconName = 'preferences-system-notifications-symbolic';
        vm.title = p.title;
        vm.subtitle = join(app, p.body);
        break;
    case 'system':
        vm.iconName = SEVERITY_ICON[p.severity];
        vm.accent = p.severity === 'info' ? 'neutral' : p.severity === 'warning' ? 'orange' : 'red';
        vm.title = p.title;
        vm.subtitle = p.detail ?? app;
        break;
    }

    if (vm.progress !== null)
        vm.progress = Math.min(1, Math.max(0, vm.progress));
    return vm;
}
