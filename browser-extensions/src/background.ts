// Background script: talks to the native host (which talks to the island),
// keeps track of Meet tabs, and follows browser downloads.

import {
    type BrowserFamily, type DownloadLike, type EventInput, type FromHost, type MediaCommand,
    type MediaState, type MeetCommand, type MeetState, type PageMessage, type SessionCommand, type SessionState,
    type State, type ToHost,
    NO_SESSION, downloadEvent, hasSession, mediaEvent, micEvent, meetEvent, meetTitle, parseSession, sessionCommand,
    siteName, webCallEvent,
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

// Events last sent, by id, so a logo that arrives later can be added.
const lastSent = new Map<string, {event: EventInput; tabId: number | null}>();

function publish(event: EventInput, tabId: number | null = null): void {
    if (tabId !== null) {
        const logo = logos.get(tabId);
        if (logo)
            event.source.icon = logo;
        else
            void fetchLogo(tabId);
    }
    if (event.state === 'ended' || event.state === 'failed')
        lastSent.delete(event.id);
    else
        lastSent.set(event.id, {event, tabId});
    send({type: 'event', event});
}

// ---- site logos -------------------------------------------------------------

// Tab → its site icon as a small PNG data URL. The icon is decoded and
// redrawn here, in the browser, so the island only ever gets a clean
// 64×64 PNG, never a file straight from a website.
const logos = new Map<number, string>();
const fetching = new Set<number>();
// Tabs whose icon changed while a lookup was running: look again after it.
const refetch = new Set<number>();
const LOGO_SIZE = 64;
const MAX_LOGO = 48 * 1024;

async function fetchLogo(tabId: number): Promise<void> {
    if (fetching.has(tabId)) {
        refetch.add(tabId);
        return;
    }
    fetching.add(tabId);
    try {
        const tab = await chrome.tabs.get(tabId);
        const url = tab.favIconUrl;
        if (!url || !/^(https:|data:image\/)/.test(url))
            return;
        const bitmap = await createImageBitmap(await (await fetch(url)).blob());
        const canvas = new OffscreenCanvas(LOGO_SIZE, LOGO_SIZE);
        const context = canvas.getContext('2d');
        if (!context)
            return;
        context.drawImage(bitmap, 0, 0, LOGO_SIZE, LOGO_SIZE);
        const png = new Uint8Array(await (await canvas.convertToBlob({type: 'image/png'})).arrayBuffer());
        let binary = '';
        for (let i = 0; i < png.length; i += 0x8000)
            binary += String.fromCharCode(...png.subarray(i, i + 0x8000));
        const logo = `data:image/png;base64,${btoa(binary)}`;
        if (logo.length > MAX_LOGO)
            return;
        logos.set(tabId, logo);
        // Add it to what this tab is already showing.
        for (const {event, tabId: owner} of [...lastSent.values()]) {
            if (owner === tabId)
                publish({...event, state: 'updated', source: {...event.source, icon: logo}}, tabId);
        }
    } catch {
        // No logo (unsupported image, closed tab…): the island shows its own icon.
    } finally {
        fetching.delete(tabId);
        if (refetch.delete(tabId) && !logos.has(tabId))
            void fetchLogo(tabId);
    }
}

chrome.tabs.onUpdated.addListener((tabId, change) => {
    if (change.favIconUrl === undefined)
        return;
    // The icon often loads after the first event went out: fetch it now.
    logos.delete(tabId);
    refetch.delete(tabId);
    if ([...lastSent.values()].some(sent => sent.tabId === tabId))
        void fetchLogo(tabId);
});

function onHostMessage(message: FromHost): void {
    if (message?.type !== 'action' || typeof message.id !== 'string')
        return;
    const meet = /^browser:meet:(\d+)$/.exec(message.id);
    if (meet) {
        const action = message.action;
        // Meet's own call controls first, its buttons on the page otherwise.
        if (sendSessionAction(Number(meet[1]), action))
            return;
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
    const call = /^browser:call:(\d+)$/.exec(message.id);
    if (call) {
        sendSessionAction(Number(call[1]), message.action);
        return;
    }
    const download = /^browser:download:(\d+)$/.exec(message.id);
    if (download && message.action === 'cancel')
        chrome.downloads.cancel(Number(download[1])).catch(() => {});
}

// ---- call controls a site reports (Media Session) -------------------------

// Tab → the frame that reports call controls, and what it reports.
const sessions = new Map<number, {frameId: number; state: SessionState}>();

function sessionOf(tabId: number): SessionState {
    return sessions.get(tabId)?.state ?? NO_SESSION;
}

/** Presses the page's own control for `action`. False when it has none. */
function sendSessionAction(tabId: number, action: string): boolean {
    const session = sessions.get(tabId);
    const command = session ? sessionCommand(session.state, action) : null;
    if (!session || !command)
        return false;
    const message: SessionCommand = {kind: 'session-action', action: command};
    chrome.tabs.sendMessage(tabId, message, {frameId: session.frameId}).catch(() => {});
    return true;
}

function onSession(tabId: number, frameId: number, state: SessionState): void {
    const current = sessions.get(tabId);
    if (hasSession(state))
        sessions.set(tabId, {frameId, state});
    else if (current?.frameId === frameId)
        sessions.delete(tabId);
    else
        return;
    // Show the change on a call this tab already has.
    const meet = calls.get(tabId);
    if (meet)
        publish(meetEvent(tabId, meet, 'updated', sessionOf(tabId)), tabId);
    const call = webCalls.get(tabId);
    if (call)
        publish(webCallEvent(tabId, call.site, 'updated', sessionOf(tabId)), tabId);
}

// ---- which site uses the microphone -----------------------------------------

// Tab → frames holding the microphone, and the site name.
const mics = new Map<number, {frames: Set<number>; site: string}>();

function onMic(tabId: number, frameId: number, active: boolean, tabUrl: string): void {
    const mic = mics.get(tabId);
    if (active) {
        if (mic) {
            mic.frames.add(frameId);
            return;
        }
        const site = siteName(tabUrl);
        mics.set(tabId, {frames: new Set([frameId]), site});
        publish(micEvent(tabId, site, 'started'), tabId);
        return;
    }
    if (!mic)
        return;
    mic.frames.delete(frameId);
    if (mic.frames.size === 0)
        endMic(tabId);
}

function endMic(tabId: number): void {
    const mic = mics.get(tabId);
    if (!mic)
        return;
    mics.delete(tabId);
    publish(micEvent(tabId, mic.site, 'ended'), tabId);
}

// ---- Google Meet ----------------------------------------------------------

const calls = new Map<number, MeetState>();

function endCall(tabId: number): void {
    const meet = calls.get(tabId);
    if (!meet)
        return;
    calls.delete(tabId);
    publish(meetEvent(tabId, meet, 'ended', sessionOf(tabId)), tabId);
}

// ---- media tabs -----------------------------------------------------------

const playing = new Map<number, MediaState>();

function endMedia(tabId: number): void {
    const media = playing.get(tabId);
    if (!media)
        return;
    playing.delete(tabId);
    publish(mediaEvent(BROWSER, tabId, media, 'ended'), tabId);
}

function onMedia(tabId: number, state: MediaState | null): void {
    if (!state) {
        endMedia(tabId);
        return;
    }
    const known = playing.has(tabId);
    playing.set(tabId, state);
    publish(mediaEvent(BROWSER, tabId, state, known ? 'updated' : 'started'), tabId);
}

// ---- calls on any site ------------------------------------------------------

// Tab → frames that are in a call, and the site name shown.
const webCalls = new Map<number, {frames: Set<number>; site: string}>();

function onWebCall(tabId: number, frameId: number, active: boolean, tabUrl: string): void {
    const call = webCalls.get(tabId);
    if (active) {
        if (call) {
            call.frames.add(frameId);
            return;
        }
        const site = siteName(tabUrl);
        webCalls.set(tabId, {frames: new Set([frameId]), site});
        publish(webCallEvent(tabId, site, 'started', sessionOf(tabId)), tabId);
        return;
    }
    if (!call)
        return;
    call.frames.delete(frameId);
    if (call.frames.size === 0)
        endWebCall(tabId);
}

function endWebCall(tabId: number): void {
    const call = webCalls.get(tabId);
    if (!call)
        return;
    webCalls.delete(tabId);
    publish(webCallEvent(tabId, call.site, 'ended', sessionOf(tabId)), tabId);
}

chrome.runtime.onMessage.addListener((message: PageMessage, sender) => {
    const tabId = sender.tab?.id;
    if (tabId === undefined)
        return;
    if (message?.kind === 'call') {
        onWebCall(tabId, sender.frameId ?? 0, message.active === true, sender.tab?.url ?? sender.url ?? '');
        return;
    }
    if (message?.kind === 'mic') {
        onMic(tabId, sender.frameId ?? 0, message.active === true, sender.tab?.url ?? sender.url ?? '');
        return;
    }
    if (message?.kind === 'session') {
        onSession(tabId, sender.frameId ?? 0, parseSession(typeof message.raw === 'string' ? message.raw : null));
        return;
    }
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
    publish(meetEvent(tabId, state, known ? 'updated' : 'started', sessionOf(tabId)), tabId);
});

chrome.tabs.onRemoved.addListener(tabId => {
    endCall(tabId);
    endMedia(tabId);
    endWebCall(tabId);
    endMic(tabId);
    logos.delete(tabId);
    refetch.delete(tabId);
    sessions.delete(tabId);
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
