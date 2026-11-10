// What the microphone/camera event says. Pure (no GNOME imports) so it is
// unit-tested with Node; privacy-source.js feeds it what the mixer and
// camera monitor report.

export const PRIVACY_EVENT_ID = 'privacy:capture';

/** "Google Chrome input" → "Google Chrome" (apps name their audio connection). */
export function appName(raw) {
    const name = String(raw ?? '').replace(/\s+(input|output|recording|capture)$/i, '').trim();
    return name.slice(0, 60) || 'An app';
}

const BROWSER_NAME = /\b(chrome|chromium|brave|firefox|edge|vivaldi|opera|librewolf|waterfox|floorp|zen)\b/i;

/**
 * Whether a recording app is a web browser. A browser's own logo says
 * nothing about which site uses the microphone, so it gets none; the
 * browser extension names the site instead.
 */
export function isBrowserApp(...names) {
    return names.some(name => BROWSER_NAME.test(String(name ?? '').replace(/[._-]/g, ' ')));
}

/**
 * `source.icon` for an installed app, from its desktop file id
 * ("com.slack.Slack.desktop" → "app:com.slack.Slack"). Null for anything else.
 */
export function appIcon(desktopId) {
    const id = String(desktopId ?? '').trim().replace(/\.desktop$/, '');
    return id && id.length <= 128 && /^[A-Za-z0-9._-]+$/.test(id) ? `app:${id}` : null;
}

/**
 * @param {object} state
 * @param {string[]} state.apps - apps recording from the microphone
 * @param {boolean} state.camera - a camera is in use
 * @param {boolean} state.muted - the microphone is muted (nobody hears you)
 * @param {string|null} [state.icon] - the first app's logo (`app:<id>`)
 * @returns {object|null} event input without `state`, or null when nothing is in use
 */
export function privacyInput({apps, camera, muted, icon = null}) {
    const mic = apps.length > 0;
    if (!mic && !camera)
        return null;
    const micMuted = mic && muted;
    let title;
    if (camera && mic)
        title = micMuted ? 'Camera and microphone muted' : 'Camera and microphone';
    else if (camera)
        title = 'Camera';
    else
        title = micMuted ? 'Microphone muted' : 'Microphone';
    return {
        id: PRIVACY_EVENT_ID,
        source: mic && icon
            ? {app: apps[0], integration: 'gnome-privacy', icon}
            : {app: apps[0] ?? 'Camera', integration: 'gnome-privacy'},
        category: 'call',
        priority: 'call',
        persistent: true,
        payload: {type: 'system', title, detail: mic ? apps.join(', ') : 'In use', severity: 'info'},
        actions: mic ? [micMuted ? {id: 'unmute', label: 'Unmute'} : {id: 'mute', label: 'Mute'}] : [],
    };
}
