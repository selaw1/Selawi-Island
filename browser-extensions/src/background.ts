// Background script: talks to the native host (which talks to the island),
// keeps track of Meet tabs, and follows browser downloads.

import {
    type DownloadLike, type EventInput, type FromHost, type MeetCommand, type MeetState,
    type PageMessage, type State, type ToHost,
    downloadEvent, meetEvent, meetTitle,
} from './model.js';

const HOST = 'io.selawi.island';
const BROWSER: 'chrome' | 'firefox' = typeof (globalThis as {browser?: unknown}).browser === 'undefined' ? 'chrome' : 'firefox';

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

chrome.runtime.onMessage.addListener((message: PageMessage, sender) => {
    const tabId = sender.tab?.id;
    if (message?.kind !== 'meet' || tabId === undefined)
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

chrome.tabs.onRemoved.addListener(tabId => endCall(tabId));

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
