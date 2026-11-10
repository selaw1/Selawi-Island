// Texts for the Status page and the "Copy report" button. Pure (no GNOME
// imports) so it is unit-tested with Node. The report holds versions and
// statuses only: never events, notifications, settings values, or details
// such as a Wi-Fi network's or a device's name.

// GNOME Shell's ExtensionState (js/misc/extensionUtils.js, GNOME 51).
const EXTENSION_STATES = {
    1: 'On',
    2: 'Off',
    3: 'Error',
    4: 'Not compatible with this GNOME version',
    5: 'Downloading',
    6: 'Off',
    7: 'Turning off',
    8: 'Turning on',
    99: 'Not installed',
};

/**
 * @param {object|null} info - GetExtensionInfo result (unpacked), or null
 *   when GNOME Shell could not be asked
 * @returns {{text: string, ok: boolean, canTurnOn: boolean}}
 */
export function extensionStatus(info) {
    if (!info || Object.keys(info).length === 0)
        return {text: 'Not installed, or GNOME Shell is not running', ok: false, canTurnOn: false};
    const state = Number(info.state);
    let text = EXTENSION_STATES[state] ?? `Unknown state (${state})`;
    if (state === 3 && info.error)
        text = `Error: ${String(info.error).slice(0, 300)}`;
    return {text, ok: state === 1, canTurnOn: state === 2 || state === 6};
}

/** "Arch Linux" from /etc/os-release. */
export function osName(osRelease) {
    const line = String(osRelease ?? '').split('\n').find(l => l.startsWith('PRETTY_NAME='));
    return line ? line.slice('PRETTY_NAME='.length).replace(/^"|"$/g, '').trim() : 'Unknown';
}

export const STATUS_LABELS = {
    active: 'Active', unavailable: 'Unavailable', failed: 'Failed', disabled: 'Off', starting: 'Starting',
};

/** "Media players (MPRIS)" → "Media players". */
export function integrationTitle(info) {
    return String(info?.name ?? info?.id ?? '').replace(/\s*\([^)]*\)$/, '');
}

/** "Active — battery at 64%". */
export function integrationText(info) {
    if (!info)
        return '';
    const status = STATUS_LABELS[info.status] ?? info.status;
    return info.detail ? `${status} — ${info.detail}` : status;
}

/**
 * The text copied by "Copy report".
 *
 * @param {object} r
 * @param {string} r.os
 * @param {string|null} r.shellVersion
 * @param {object|null} r.daemon - {version, protocol, events} or null when not running
 * @param {object|null} r.extension - GetExtensionInfo result, or null
 * @param {string[]} r.extensionErrors
 * @param {object[]} r.integrations - Integrations1.List rows
 */
export function buildReport({os, shellVersion, daemon, extension, extensionErrors, integrations}) {
    const lines = ['Selawi Island report', ''];
    lines.push(`System: ${os}`);
    lines.push(`GNOME Shell: ${shellVersion ?? 'not running'}`);
    lines.push(daemon
        ? `Service: running, version ${daemon.version}, protocol ${daemon.protocol}, ${daemon.events} event(s)`
        : 'Service: not running');
    const ext = extensionStatus(extension);
    lines.push(`Extension: ${ext.text}${extension?.version !== undefined ? ` (version ${extension.version})` : ''}`);
    for (const error of extensionErrors ?? [])
        lines.push(`  error: ${String(error).slice(0, 300)}`);
    if (integrations?.length) {
        lines.push('Sources:');
        // The state only: details can name a Wi-Fi network or a device.
        for (const i of integrations)
            lines.push(`  ${i.id}: ${STATUS_LABELS[i.status] ?? i.status}`);
    }
    return `${lines.join('\n')}\n`;
}
