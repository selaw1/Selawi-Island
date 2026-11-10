// Content script (isolated world) on https sites: forwards what
// session-main.ts sees (microphone/camera state and call controls the page
// offers) to the background script, and passes the island's Mute / Leave
// back to the page. Runs in every frame, like call.ts.

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

window.addEventListener('pagehide', () => {
    const message: PageMessage = {kind: 'session', raw: null};
    chrome.runtime.sendMessage(message).catch(() => {});
});

chrome.runtime.onMessage.addListener((message: SessionCommand) => {
    if (message?.kind === 'session-action')
        document.dispatchEvent(new CustomEvent('selawi-island-session-action', {detail: message.action}));
});
