// Content script for media sites (YouTube, YouTube Music, SoundCloud).
// Reads what the site publishes for the media keys (navigator.mediaSession)
// and the page's own media element and controls. Each tab is reported on
// its own, so several tabs can play at once.
//
// During a YouTube ad (the player is marked "ad-showing") the last real
// title is kept and the ad's own timer is hidden.

import type {MediaAction, MediaCommand, MediaState, PageMessage} from './model.js';

interface Site {
    name: string;
    hosts: RegExp;
    next?: string;
    previous?: string;
    playButton?: string;
    /** Elapsed / total time shown on the page, when there is no media element. */
    elapsed?: string;
    total?: string;
    adShowing?: () => boolean;
}

const SITES: Site[] = [
    {
        name: 'YouTube Music',
        hosts: /^music\.youtube\.com$/,
        next: '.next-button',
        previous: '.previous-button',
        playButton: '#play-pause-button',
        adShowing: () => document.querySelector('.html5-video-player.ad-showing') !== null,
    },
    {
        name: 'YouTube',
        hosts: /^(www\.|m\.)?youtube\.com$/,
        next: '.ytp-next-button',
        previous: '.ytp-prev-button',
        playButton: '.ytp-play-button',
        adShowing: () => document.querySelector('.html5-video-player.ad-showing') !== null,
    },
    {
        name: 'SoundCloud',
        hosts: /^soundcloud\.com$/,
        next: '.skipControl__next',
        previous: '.skipControl__previous',
        playButton: '.playControls__play',
        elapsed: '.playbackTimeline__timePassed span[aria-hidden="true"]',
        total: '.playbackTimeline__duration span[aria-hidden="true"]',
    },
];

const CHECK_MS = 1000;
/** A seek of more than this is reported, so the island's timer stays right. */
const DRIFT_MS = 3000;

const site = SITES.find(s => s.hosts.test(location.hostname)) ?? null;

function clickable(selector: string | undefined): HTMLElement | null {
    if (!selector)
        return null;
    const el = document.querySelector<HTMLElement>(selector);
    if (!el || el.hasAttribute('disabled') || el.getAttribute('aria-disabled') === 'true')
        return null;
    // Hidden controls (e.g. no playlist) don't count.
    return el.offsetParent !== null || el.getClientRects().length > 0 ? el : null;
}

// The playing element, else the one that has played, else any loaded one
// (a paused clip back at 0:00 is still the tab's media).
function mediaElement(): HTMLMediaElement | null {
    const all = [...document.querySelectorAll<HTMLMediaElement>('video, audio')];
    return all.find(m => !m.paused) ??
        all.find(m => m.currentTime > 0) ??
        all.find(m => m.readyState > 0 && Number.isFinite(m.duration) && m.duration > 0) ??
        null;
}

function clockMs(selector: string | undefined): number | null {
    if (!selector)
        return null;
    const text = document.querySelector(selector)?.textContent ?? '';
    const match = /^\s*(?:(\d+):)?(\d{1,2}):(\d{2})\s*$/.exec(text);
    if (!match)
        return null;
    const [, h, m, s] = match;
    return ((Number(h ?? 0) * 60 + Number(m)) * 60 + Number(s)) * 1000;
}

// The last real (non-ad) title, kept while an ad plays.
let lastReal: {title: string; artist: string; album: string} | null = null;

function read(): MediaState | null {
    if (!site)
        return null;
    const metadata = navigator.mediaSession?.metadata ?? null;
    const element = mediaElement();
    const sessionState = navigator.mediaSession?.playbackState;
    if (!metadata && !element)
        return null;

    const playing = element ? !element.paused : sessionState === 'playing';
    if (!playing && !element && sessionState !== 'paused')
        return null;

    const ad = site.adShowing?.() ?? false;
    let title = metadata?.title ?? '';
    let artist = metadata?.artist ?? '';
    let album = metadata?.album ?? '';
    if (ad && lastReal) {
        ({title, artist, album} = lastReal);
    } else if (!ad && title) {
        lastReal = {title, artist, album};
    }
    if (!title)
        title = document.title.replace(/\s+-\s+(YouTube|YouTube Music|SoundCloud)\s*$/i, '').replace(/^\(\d+\)\s*/, '');

    let positionMs: number | null = null;
    let durationMs: number | null = null;
    if (!ad) {
        if (element && Number.isFinite(element.duration) && element.duration > 0) {
            positionMs = element.currentTime * 1000;
            durationMs = element.duration * 1000;
        } else {
            positionMs = clockMs(site.elapsed);
            durationMs = clockMs(site.total);
        }
    }
    return {
        site: site.name,
        title,
        artist,
        album,
        playing,
        positionMs,
        durationMs,
        canNext: clickable(site.next) !== null,
        canPrevious: clickable(site.previous) !== null,
    };
}

let last: MediaState | null = null;
let lastAt = 0;

function changed(now: MediaState | null): boolean {
    if (!now || !last)
        return now !== last;
    const same = now.title === last.title && now.artist === last.artist && now.album === last.album &&
        now.playing === last.playing && now.canNext === last.canNext && now.canPrevious === last.canPrevious &&
        (now.durationMs === null) === (last.durationMs === null);
    if (!same)
        return true;
    // Only a seek (not normal playback) needs the position sent again.
    if (now.positionMs === null || last.positionMs === null)
        return now.positionMs !== last.positionMs;
    const expected = last.positionMs + (last.playing ? Date.now() - lastAt : 0);
    return Math.abs(now.positionMs - expected) > DRIFT_MS;
}

function report(force = false): void {
    const state = read();
    if (!force && !changed(state))
        return;
    last = state;
    lastAt = Date.now();
    const message: PageMessage = {kind: 'media', state};
    chrome.runtime.sendMessage(message).catch(() => {
        // The extension was reloaded; nothing to do.
    });
}

function act(action: MediaAction): void {
    if (!site)
        return;
    if (action === 'play_pause') {
        const element = mediaElement() ?? document.querySelector<HTMLMediaElement>('video, audio');
        if (element) {
            if (element.paused)
                void element.play().catch(() => clickable(site.playButton)?.click());
            else
                element.pause();
        } else {
            clickable(site.playButton)?.click();
        }
    } else {
        clickable(action === 'next' ? site.next : site.previous)?.click();
    }
    setTimeout(() => report(true), 400);
}

if (site) {
    setInterval(() => report(), CHECK_MS);
    for (const type of ['play', 'pause', 'ended', 'seeked'])
        document.addEventListener(type, () => report(), true);
    window.addEventListener('pagehide', () => {
        const message: PageMessage = {kind: 'media', state: null};
        chrome.runtime.sendMessage(message).catch(() => {});
    });
    chrome.runtime.onMessage.addListener((message: MediaCommand) => {
        if (message?.kind === 'media-action')
            act(message.action);
    });
}
