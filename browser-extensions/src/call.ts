// Content script (isolated world) on https sites: forwards "in a call
// yes/no" from rtc-main.ts to the background script. Runs in every frame,
// because some sites run their call inside a frame.

import type {PageMessage} from './model.js';

let inCall = false;

function report(active: boolean): void {
    if (active === inCall)
        return;
    inCall = active;
    const message: PageMessage = {kind: 'call', active};
    chrome.runtime.sendMessage(message).catch(() => {
        // The extension was reloaded; nothing to do.
    });
}

document.addEventListener('selawi-island-call', event => {
    report((event as CustomEvent<unknown>).detail === true);
});
window.addEventListener('pagehide', () => report(false));
