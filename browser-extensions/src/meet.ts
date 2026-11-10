// Content script for meet.google.com. Reads only two things from Meet's
// page: whether the "leave call" button exists (you are in a call) and the
// page title (the meeting name). It can press the leave button when asked.
//
// Microphone and camera state, and Mute / Unmute, come from Meet's own
// Media Session call controls (session-main.ts), like on any other site.
//
// Meet's page uses Material icon names inside its buttons (call_end).
// If Meet changes its page, detection stops (nothing wrong is shown)
// until this file is updated.

import type {MeetCommand, MeetState, PageMessage} from './model.js';

const LEAVE_ICONS = ['call_end'];
const CHECK_MS = 1000;

function iconNames(button: Element): string[] {
    const names: string[] = [];
    for (const icon of button.querySelectorAll('i, span, svg')) {
        const text = icon.textContent?.trim();
        if (text && /^[a-z_]+$/.test(text))
            names.push(text);
    }
    return names;
}

function leaveButton(): HTMLElement | null {
    for (const button of document.querySelectorAll<HTMLElement>('button, [role="button"]')) {
        if (iconNames(button).some(name => LEAVE_ICONS.includes(name)))
            return button;
    }
    for (const button of document.querySelectorAll<HTMLElement>('button[aria-label], [role="button"][aria-label]')) {
        if (/leave call/i.test(button.getAttribute('aria-label') ?? ''))
            return button;
    }
    return null;
}

function read(): MeetState {
    return {inCall: leaveButton() !== null, title: document.title};
}

let last = '';

function report(state: MeetState): void {
    const key = JSON.stringify(state);
    if (key === last)
        return;
    last = key;
    const message: PageMessage = {kind: 'meet', state};
    chrome.runtime.sendMessage(message).catch(() => {
        // The extension was reloaded; nothing to do.
    });
}

setInterval(() => report(read()), CHECK_MS);
window.addEventListener('pagehide', () => report({inCall: false, title: document.title}));

// Used only if Meet does not offer "hang up" through its call controls.
chrome.runtime.onMessage.addListener((message: MeetCommand) => {
    if (message?.kind === 'meet-action' && message.action === 'leave')
        leaveButton()?.click();
});
