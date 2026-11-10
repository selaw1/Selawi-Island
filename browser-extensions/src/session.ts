// Content script (isolated world) on https sites: forwards what
// session-main.ts sees (microphone/camera state and call controls the page
// offers) and what mic-main.ts sees (the page holds the microphone) to the
// background script, and passes the island's Mute / Leave back to the
// page. Runs in every frame, like call.ts.

import type {PageMessage, SessionCommand} from './model.js';

document.addEventListener('selawi-island-session', event => {
    const detail = (event as CustomEvent<unknown>).detail;
    if (typeof detail !== 'string' || detail.length > 1000)
        return;
    const message: PageMessage = {kind: 'session', raw: detail};
    chrome.runtime.sendMessage(message).catch(() => {
        // The extension was reloaded; nothing to do.
    });
});

let micActive = false;

function reportMic(active: boolean): void {
    if (active === micActive)
        return;
    micActive = active;
    const message: PageMessage = {kind: 'mic', active};
    chrome.runtime.sendMessage(message).catch(() => {});
}

document.addEventListener('selawi-island-mic', event => {
    reportMic((event as CustomEvent<unknown>).detail === true);
});

window.addEventListener('pagehide', () => {
    const message: PageMessage = {kind: 'session', raw: null};
    chrome.runtime.sendMessage(message).catch(() => {});
    reportMic(false);
});

chrome.runtime.onMessage.addListener((message: SessionCommand) => {
    if (message?.kind === 'session-action')
        document.dispatchEvent(new CustomEvent('selawi-island-session-action', {detail: message.action}));
});
