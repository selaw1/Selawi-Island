// What the extension tells the island. Pure functions (no browser APIs),
// unit-tested with Node from the compiled output. The shapes follow event
// protocol v1 (docs/EVENT_SYSTEM.md); the native host checks them again.

export type State = 'started' | 'updated' | 'ended' | 'failed';

export interface Action {
    id: string;
    label: string;
}

export interface CallPayload {
    type: 'call';
    title: string;
    confidence: 'confirmed';
    muted?: boolean;
    camera?: boolean;
}

export interface MediaPayload {
    type: 'media';
    title: string;
    artist?: string;
    album?: string;
    playing: boolean;
    position_ms?: number;
    duration_ms?: number;
}

export interface ProgressPayload {
    type: 'progress';
    title: string;
    detail?: string;
    percent?: number;
}

export interface EventInput {
    id: string;
    source: {app: string; integration: 'browser'; icon?: string};
    category: 'call' | 'transfer' | 'media';
    state: State;
    persistent: boolean;
    priority?: 'call';
    ttl_ms?: number;
    payload: CallPayload | ProgressPayload | MediaPayload;
    actions: Action[];
}

/** What the Meet content script sees on the page. */
export interface MeetState {
    inCall: boolean;
    title: string;
    /** null when the page does not say. */
    muted: boolean | null;
    /** Camera on? null when the page does not say. */
    camera: boolean | null;
}

/** What a media content script sees in its tab. */
export interface MediaState {
    /** Site name shown as the app, e.g. "YouTube". */
    site: string;
    title: string;
    artist: string;
    album: string;
    playing: boolean;
    positionMs: number | null;
    durationMs: number | null;
    canNext: boolean;
    canPrevious: boolean;
}

export type MediaAction = 'play_pause' | 'next' | 'previous';

/** Messages between content scripts and the background script. */
export type PageMessage =
    | {kind: 'meet'; state: MeetState}
    | {kind: 'media'; state: MediaState | null}
    | {kind: 'call'; active: boolean};
export type MeetCommand = {kind: 'meet-action'; action: 'mute' | 'unmute' | 'leave'};
export type MediaCommand = {kind: 'media-action'; action: MediaAction};

/** Messages between the background script and the native host. */
export type ToHost =
    | {type: 'hello'; version: 1; browser: 'chrome' | 'firefox'}
    | {type: 'event'; event: EventInput};
export type FromHost = {type: 'action'; id: string; action: string};

const MAX_TITLE = 256;

function clean(text: string, max = MAX_TITLE): string {
    return text.replace(/[\u0000-\u001f\u007f-\u009f]/g, ' ').replace(/\s+/g, ' ').trim().slice(0, max);
}

/** "Meet - Weekly sync" → "Weekly sync"; the meeting code if that is all there is. */
export function meetTitle(documentTitle: string): string {
    const title = clean(documentTitle.replace(/^\(\d+\)\s*/, '').replace(/^(Google\s+)?Meet\s*[-–—:]\s*/i, ''));
    return title && !/^(Google\s+)?Meet$/i.test(title) ? title : 'Google Meet';
}

export function meetEventId(tabId: number): string {
    return `browser:meet:${tabId}`;
}

export function meetEvent(tabId: number, meet: MeetState, state: State): EventInput {
    const live = state !== 'ended' && state !== 'failed';
    const actions: Action[] = [];
    if (live && meet.muted === true)
        actions.push({id: 'unmute', label: 'Unmute'});
    else if (live && meet.muted === false)
        actions.push({id: 'mute', label: 'Mute'});
    if (live)
        actions.push({id: 'leave', label: 'Leave'});
    const payload: CallPayload = {type: 'call', title: clean(meet.title) || 'Google Meet', confidence: 'confirmed'};
    if (meet.muted !== null)
        payload.muted = meet.muted;
    if (meet.camera !== null)
        payload.camera = meet.camera;
    return {
        id: meetEventId(tabId),
        source: {app: 'Google Meet', integration: 'browser'},
        category: 'call',
        state,
        persistent: true,
        priority: 'call',
        payload,
        actions,
    };
}

/** The parts of a browser download item that matter here. */
export interface DownloadLike {
    id: number;
    filename: string;
    bytesReceived: number;
    totalBytes: number;
    state: 'in_progress' | 'interrupted' | 'complete';
    error?: string;
}

export function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes < 0)
        return '';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit++;
    }
    return `${value >= 10 || unit === 0 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`;
}

/** Only the file's name is shown, never its folder or the address it came from. */
export function fileName(path: string): string {
    const parts = path.split(/[\\/]/);
    return clean(parts[parts.length - 1] ?? '') || 'Download';
}

export function downloadEventId(downloadId: number): string {
    return `browser:download:${downloadId}`;
}

export function downloadEvent(item: DownloadLike, state: State): EventInput {
    const total = item.totalBytes > 0 ? item.totalBytes : 0;
    const payload: ProgressPayload = {type: 'progress', title: fileName(item.filename)};
    if (state === 'failed') {
        payload.detail = item.error === 'USER_CANCELED' ? 'Cancelled' : 'Download failed';
    } else if (state === 'ended') {
        payload.detail = total ? `Downloaded · ${formatBytes(total)}` : 'Downloaded';
        payload.percent = 100;
    } else {
        payload.detail = total
            ? `${formatBytes(item.bytesReceived)} of ${formatBytes(total)}`
            : formatBytes(item.bytesReceived);
        if (total)
            payload.percent = Math.min(100, Math.round((item.bytesReceived * 1000) / total) / 10);
    }
    return {
        id: downloadEventId(item.id),
        source: {app: 'Downloads', integration: 'browser'},
        category: 'transfer',
        state,
        persistent: state === 'started' || state === 'updated',
        payload,
        actions: state === 'started' || state === 'updated' ? [{id: 'cancel', label: 'Cancel'}] : [],
    };
}

/** A paused tab disappears this long after its last change (as for MPRIS). */
export const PAUSED_TTL_MS = 5 * 60 * 1000;

export type BrowserFamily = 'chrome' | 'firefox';

export function mediaEventId(family: BrowserFamily, tabId: number): string {
    return `browser:media:${family}:${tabId}`;
}

function millis(value: number | null): number | undefined {
    return value !== null && Number.isFinite(value) && value >= 0 ? Math.round(value) : undefined;
}

export function mediaEvent(family: BrowserFamily, tabId: number, media: MediaState, state: State): EventInput {
    const live = state === 'started' || state === 'updated';
    const payload: MediaPayload = {
        type: 'media',
        title: clean(media.title) || clean(media.site) || 'Media',
        playing: live && media.playing,
    };
    const artist = clean(media.artist);
    const album = clean(media.album);
    if (artist)
        payload.artist = artist;
    if (album)
        payload.album = album;
    const duration = millis(media.durationMs);
    const position = millis(media.positionMs);
    if (duration !== undefined && duration > 0)
        payload.duration_ms = duration;
    if (position !== undefined)
        payload.position_ms = duration ? Math.min(position, duration) : position;

    const actions: Action[] = [];
    if (live) {
        if (media.canPrevious)
            actions.push({id: 'previous', label: 'Previous'});
        actions.push({id: 'play_pause', label: media.playing ? 'Pause' : 'Play'});
        if (media.canNext)
            actions.push({id: 'next', label: 'Next'});
    }
    const event: EventInput = {
        id: mediaEventId(family, tabId),
        source: {app: clean(media.site, 64) || 'Browser', integration: 'browser'},
        category: 'media',
        state,
        persistent: live && media.playing,
        payload,
        actions,
    };
    if (live && !media.playing)
        event.ttl_ms = PAUSED_TTL_MS;
    return event;
}

// ---- calls on any site ------------------------------------------------------

const SITE_NAMES: Array<[RegExp, string]> = [
    [/(^|\.)slack\.com$/, 'Slack'],
    [/(^|\.)teams\.(microsoft|live)\.com$/, 'Microsoft Teams'],
    [/(^|\.)teams\.cloud\.microsoft$/, 'Microsoft Teams'],
    [/(^|\.)zoom\.(us|com)$/, 'Zoom'],
    [/(^|\.)discord\.com$/, 'Discord'],
    [/^web\.whatsapp\.com$/, 'WhatsApp'],
    [/^web\.telegram\.org$/, 'Telegram'],
    [/(^|\.)messenger\.com$/, 'Messenger'],
    [/(^|\.)webex\.com$/, 'Webex'],
    [/(^|\.)jit\.si$/, 'Jitsi'],
    [/(^|\.)whereby\.com$/, 'Whereby'],
    [/^meet\.google\.com$/, 'Google Meet'],
];

/** "https://app.slack.com/huddle/…" → "Slack"; unknown sites by their address. */
export function siteName(url: string): string {
    let host = '';
    try {
        host = new URL(url).hostname.toLowerCase();
    } catch {
        return 'Web call';
    }
    for (const [pattern, name] of SITE_NAMES) {
        if (pattern.test(host))
            return name;
    }
    return clean(host.replace(/^www\./, ''), 64) || 'Web call';
}

export function webCallEventId(tabId: number): string {
    return `browser:call:${tabId}`;
}

/** A call on any site: the site's name and a timer, no buttons. */
export function webCallEvent(tabId: number, site: string, state: State): EventInput {
    return {
        id: webCallEventId(tabId),
        source: {app: 'Call', integration: 'browser'},
        category: 'call',
        state,
        persistent: true,
        priority: 'call',
        payload: {type: 'call', title: clean(site) || 'Web call', confidence: 'confirmed'},
        actions: [],
    };
}
