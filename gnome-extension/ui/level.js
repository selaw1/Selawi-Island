// Microphone loudness → wave bars. Pure functions (no GNOME imports), so
// they are unit-tested with Node. Only loudness is ever read; no sound is
// kept.

export const BAR_COUNT = 5;
/** Quieter than this (dBFS) counts as silence. */
export const SILENCE_DB = -50;
const MIN_BAR = 0.18;
const SHAPE = [0.55, 0.85, 1, 0.8, 0.6];

/**
 * Loudest channel's RMS in dB from a GStreamer `level` message, given as
 * the structure's text, e.g. "level, …, rms=(double){ -42.1, -40.3 };" or
 * "rms=(GValueArray)< -42.1 >". Returns null if there is none.
 */
export function parseLevelMessage(text) {
    const match = /rms=\([^)]*\)\s*[{<]([^}>]*)[}>]/.exec(String(text));
    if (!match)
        return null;
    const values = match[1].split(',').map(v => Number.parseFloat(v)).filter(Number.isFinite);
    return values.length ? Math.max(...values) : null;
}

/** 0 (silent) … 1 (loud) from dBFS. */
export function loudness(db) {
    if (!Number.isFinite(db))
        return 0;
    const x = Math.min(1, Math.max(0, (db - SILENCE_DB) / -SILENCE_DB));
    // A little boost so normal speech moves the bars clearly.
    return Math.sqrt(x);
}

/** Rises at once, falls gently, so the bars don't flicker. */
export function smooth(previous, next) {
    return next >= previous ? next : previous * 0.55 + next * 0.45;
}

/**
 * Heights (0…1) for the bars. `phase` changes every update so the bars
 * ripple while you talk; at silence they all sit at the minimum.
 */
export function barHeights(level, phase) {
    return SHAPE.map((shape, i) => {
        const ripple = 0.75 + 0.25 * Math.sin(phase * 2.1 + i * 1.3);
        return MIN_BAR + (1 - MIN_BAR) * Math.min(1, level * shape * ripple);
    });
}
