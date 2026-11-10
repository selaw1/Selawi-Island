// Background script: talks to the native host (which talks to the island),
// keeps track of Meet tabs, and follows browser downloads.

import {
    type BrowserFamily, type DownloadLike, type EventInput, type FromHost, type MediaCommand,
    type MediaState, type MeetCommand, type MeetState, type PageMessage, type State, type ToHost,
    downloadEvent, mediaEvent, meetEvent, meetTitle,
} from './model.js';

const HOST = 'io.selawi.island';
// Newer Chrome also has a `browser` object, so look at the extension's own
// address instead: moz-extension:// only exists in Firefox.
const BROWSER: BrowserFamily = chrome.runtime.getURL('').startsWith('moz-extension:') ? 'firefox' : 'chrome';

// ---- connection to the island -------------------------------------------

let port: chrome.runtime.Port | null = null;

function connect(): chrome.runtime.Port | null {
    if (port)
        return port;
    try {
        port = chrome.runtime.connectNative(HOST);
    } catch {
        return null;
    }
    port.onMessage.addListener((message: FromHost) => onHostMessage(message));
    port.onDisconnect.addListener(() => {
        port = null;
    });
    send({type: 'hello', version: 1, browser: BROWSER});
    return port;
}

function send(message: ToHost): void {
    connect()?.postMessage(message);
}

function publish(event: EventInput): void {
    send({type: 'event', event});
}

function onHostMessage(message: FromHost): void {
    if (message?.type !== 'action' || typeof message.id !== 'string')
        return;
    const meet = /^browser:meet:(\d+)$/.exec(message.id);
    if (meet) {
        const action = message.action;
        if (action === 'mute' || action === 'unmute' || action === 'leave') {
            const command: MeetCommand = {kind: 'meet-action', action};
            chrome.tabs.sendMessage(Number(meet[1]), command).catch(() => {});
        }
        return;
    }
    const media = /^browser:media:(?:chrome|firefox):(\d+)$/.exec(message.id);
    if (media) {
        const action = message.action;
        if (action === 'play_pause' || action === 'next' || action === 'previous') {
            const command: MediaCommand = {kind: 'media-action', action};
            chrome.tabs.sendMessage(Number(media[1]), command).catch(() => {});
        }
        return;
    }
    const download = /^browser:download:(\d+)$/.exec(message.id);
    if (download && message.action === 'cancel')
        chrome.downloads.cancel(Number(download[1])).catch(() => {});
}

// ---- Google Meet ----------------------------------------------------------

const calls = new Map<number, MeetState>();

function endCall(tabId: number): void {
    const meet = calls.get(tabId);
    if (!meet)
        return;
    calls.delete(tabId);
    publish(meetEvent(tabId, meet, 'ended'));
}

// ---- media tabs -----------------------------------------------------------

const playing = new Map<number, MediaState>();

function endMedia(tabId: number): void {
    const media = playing.get(tabId);
    if (!media)
        return;
    playing.delete(tabId);
    publish(mediaEvent(BROWSER, tabId, media, 'ended'));
}

function onMedia(tabId: number, state: MediaState | null): void {
    if (!state) {
        endMedia(tabId);
        return;
    }
    const known = playing.has(tabId);
    playing.set(tabId, state);
    publish(mediaEvent(BROWSER, tabId, state, known ? 'updated' : 'started'));
}

chrome.runtime.onMessage.addListener((message: PageMessage, sender) => {
    const tabId = sender.tab?.id;
    if (tabId === undefined)
        return;
    if (message?.kind === 'media') {
        onMedia(tabId, message.state);
        return;
    }
    if (message?.kind !== 'meet')
        return;
    const state: MeetState = {...message.state, title: meetTitle(String(message.state.title ?? ''))};
    if (!state.inCall) {
        endCall(tabId);
        return;
    }
    const known = calls.has(tabId);
    calls.set(tabId, state);
    publish(meetEvent(tabId, state, known ? 'updated' : 'started'));
});

chrome.tabs.onRemoved.addListener(tabId => {
    endCall(tabId);
    endMedia(tabId);
});

// ---- downloads ------------------------------------------------------------

const POLL_MS = 1000;
const active = new Set<number>();
let poll: ReturnType<typeof setInterval> | null = null;

function asDownload(item: chrome.downloads.DownloadItem): DownloadLike {
    return {
        id: item.id,
        filename: item.filename,
        bytesReceived: item.bytesReceived,
        totalBytes: item.totalBytes,
        state: item.state as DownloadLike['state'],
        error: item.error,
    };
}

async function refresh(): Promise<void> {
    const running = await chrome.downloads.search({state: 'in_progress'});
    for (const item of running) {
        const state: State = active.has(item.id) ? 'updated' : 'started';
        active.add(item.id);
        publish(downloadEvent(asDownload(item), state));
    }
    if (active.size === 0 && poll) {
        clearInterval(poll);
        poll = null;
    }
}

// Progress is read once a second, and only while something downloads.
function startPolling(): void {
    if (!poll)
        poll = setInterval(() => void refresh(), POLL_MS);
    void refresh();
}

chrome.downloads.onCreated.addListener(item => {
    if (item.state !== 'in_progress')
        return;
    // Shown at once, so even a quick download appears before it finishes.
    active.add(item.id);
    publish(downloadEvent(asDownload(item), 'started'));
    startPolling();
});

chrome.downloads.onChanged.addListener(async delta => {
    const finished = delta.state?.current === 'complete' || delta.state?.current === 'interrupted';
    if (!finished || !active.has(delta.id))
        return;
    active.delete(delta.id);
    const [item] = await chrome.downloads.search({id: delta.id});
    if (item)
        publish(downloadEvent(asDownload(item), delta.state?.current === 'complete' ? 'ended' : 'failed'));
});

startPolling();
