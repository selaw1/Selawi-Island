// What the microphone/camera event says. Pure (no GNOME imports) so it is
// unit-tested with Node; privacy-source.js feeds it what the mixer and
// camera monitor report.

export const PRIVACY_EVENT_ID = 'privacy:capture';

/**
 * @param {object} state
 * @param {string[]} state.apps - apps recording from the microphone
 * @param {boolean} state.camera - a camera is in use
 * @param {boolean} state.muted - those apps can't be heard (muted)
 * @returns {object|null} event input without `state`, or null when nothing is in use
 */
export function privacyInput({apps, camera, muted}) {
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
        source: {app: apps[0] ?? 'Camera', integration: 'gnome-privacy'},
        category: 'call',
        priority: 'call',
        persistent: true,
        payload: {type: 'system', title, detail: mic ? apps.join(', ') : 'In use', severity: 'info'},
        actions: mic ? [micMuted ? {id: 'unmute', label: 'Unmute'} : {id: 'mute', label: 'Mute'}] : [],
    };
}
