// Content script for meet.google.com. Reads only Meet's own call
// controls: whether the "leave call" button exists (you are in a call),
// the microphone button's muted state, and the page title. It can press
// the microphone and leave buttons when asked by the island.
//
// Meet's page uses Material icon names inside its buttons (call_end,
// mic, mic_off) and a data-is-muted attribute; those are what this
// relies on. If Meet changes its page, detection stops (nothing wrong is
// shown) until this file is updated.

import type {MeetCommand, MeetState, PageMessage} from './model.js';

const LEAVE_ICONS = ['call_end'];
const MIC_ICONS = ['mic', 'mic_off', 'mic_none'];
const CAMERA_ICONS = ['videocam', 'videocam_off'];
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

function findButton(icons: string[], label: RegExp): HTMLElement | null {
    for (const button of document.querySelectorAll<HTMLElement>('button, [role="button"]')) {
        if (iconNames(button).some(name => icons.includes(name)))
            return button;
    }
    for (const button of document.querySelectorAll<HTMLElement>('button[aria-label], [role="button"][aria-label]')) {
        if (label.test(button.getAttribute('aria-label') ?? ''))
            return button;
    }
    return null;
}

function leaveButton(): HTMLElement | null {
    return findButton(LEAVE_ICONS, /leave call/i);
}

// The in-call microphone and camera toggles carry data-is-muted.
function toggle(icons: string[], label: RegExp): HTMLElement | null {
    for (const button of document.querySelectorAll<HTMLElement>('[data-is-muted]')) {
        if (iconNames(button).some(name => icons.includes(name)) || label.test(button.getAttribute('aria-label') ?? ''))
            return button;
    }
    return null;
}

function micButton(): HTMLElement | null {
    return toggle(MIC_ICONS, /microphone/i);
}

function cameraButton(): HTMLElement | null {
    return toggle(CAMERA_ICONS, /camera/i);
}

function flag(button: HTMLElement | null): boolean | null {
    const value = button?.getAttribute('data-is-muted');
    return value === 'true' ? true : value === 'false' ? false : null;
}

function read(): MeetState {
    const inCall = leaveButton() !== null;
    const cameraOff = inCall ? flag(cameraButton()) : null;
    return {
        inCall,
        title: document.title,
        muted: inCall ? flag(micButton()) : null,
        camera: cameraOff === null ? null : !cameraOff,
    };
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
window.addEventListener('pagehide', () => report({inCall: false, title: document.title, muted: null, camera: null}));

chrome.runtime.onMessage.addListener((message: MeetCommand) => {
    if (message?.kind !== 'meet-action')
        return;
    if (message.action === 'leave') {
        leaveButton()?.click();
        return;
    }
    const state = read();
    const wantMuted = message.action === 'mute';
    if (state.muted !== null && state.muted !== wantMuted)
        micButton()?.click();
    setTimeout(() => report(read()), 300);
});
