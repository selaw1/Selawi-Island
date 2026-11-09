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
}

export interface ProgressPayload {
    type: 'progress';
    title: string;
    detail?: string;
    percent?: number;
}

export interface EventInput {
    id: string;
    source: {app: string; integration: 'browser'};
    category: 'call' | 'transfer';
    state: State;
    persistent: boolean;
    priority?: 'call';
    ttl_ms?: number;
    payload: CallPayload | ProgressPayload;
    actions: Action[];
}

/** What the Meet content script sees on the page. */
export interface MeetState {
    inCall: boolean;
    title: string;
    /** null when the page does not say. */
    muted: boolean | null;
}

/** Messages between content scripts and the background script. */
export type PageMessage = {kind: 'meet'; state: MeetState};
export type MeetCommand = {kind: 'meet-action'; action: 'mute' | 'unmute' | 'leave'};

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
