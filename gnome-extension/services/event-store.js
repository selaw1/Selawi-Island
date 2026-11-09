// UI-side event store used while there is no daemon.
//
// Follows the same rules as core/src/store.rs (lifecycle, deduplication,
// priority order, expiry, linger after finishing, dismissal, capacity).
// Once the daemon exists it becomes the authority and the extension will
// receive ordered snapshots instead; the island itself does not change.
//
// No GNOME imports: time is passed in, so this is unit-tested with Node.

import {DEFAULT_PRIORITY, isTerminal, priorityRank, validateInput} from './protocol.js';

const MAX_REMEMBERED_DISMISSALS = 256;

export const DEFAULT_CONFIG = Object.freeze({
    defaultTtlMs: 8000,
    finishedLingerMs: 4000,
    maxEvents: 64,
    categoryPriority: {},
});

function sameJson(a, b) {
    return JSON.stringify(a) === JSON.stringify(b);
}

export class EventStore {
    constructor(config = {}) {
        this._config = {...DEFAULT_CONFIG, ...config};
        this._events = new Map(); // id -> {event, removeAt}
        this._dismissed = [];
    }

    get size() {
        return this._events.size;
    }

    get(id) {
        return this._events.get(id)?.event ?? null;
    }

    /**
     * Validates and applies one raw input.
     *
     * @param {unknown} raw - untrusted event input
     * @param {number} now - milliseconds since the epoch
     * @returns {{ok: boolean, changed: boolean, error?: string}}
     */
    apply(raw, now) {
        const result = validateInput(raw);
        if (!result.ok)
            return {ok: false, changed: false, error: result.error};
        const input = result.input;

        const dismissedAt = this._dismissed.indexOf(input.id);
        if (dismissedAt !== -1) {
            if (input.state !== 'started') {
                if (isTerminal(input.state))
                    this._dismissed.splice(dismissedAt, 1);
                return {ok: true, changed: false};
            }
            this._dismissed.splice(dismissedAt, 1);
        }

        const stored = this._events.get(input.id);
        const current = stored?.event.state ?? null;
        const priority = this._config.categoryPriority[input.category] ??
            input.priority ?? DEFAULT_PRIORITY[input.category];

        if (current === null) {
            if (input.state === 'expired')
                return {ok: false, changed: false, error: 'cannot expire an event that is not active'};
            this._insert(input, priority, now);
            return {ok: true, changed: true};
        }
        if (isTerminal(current)) {
            if (input.state !== 'started')
                return {ok: false, changed: false, error: 'event has already finished'};
            this._insert(input, priority, now);
            return {ok: true, changed: true};
        }
        if (input.state === 'expired') {
            this._events.delete(input.id);
            return {ok: true, changed: true};
        }
        return {ok: true, changed: this._update(stored, input, priority, now)};
    }

    dismiss(id) {
        const stored = this._events.get(id);
        if (!stored)
            return false;
        this._events.delete(id);
        if (!isTerminal(stored.event.state)) {
            if (this._dismissed.length >= MAX_REMEMBERED_DISMISSALS)
                this._dismissed.shift();
            this._dismissed.push(id);
        }
        return true;
    }

    /** Removes everything whose deadline has passed. Returns true if anything changed. */
    tick(now) {
        let changed = false;
        for (const [id, stored] of this._events) {
            const deadline = this._deadlineOf(stored);
            if (deadline !== null && deadline <= now) {
                this._events.delete(id);
                changed = true;
            }
        }
        return changed;
    }

    /** Next time tick() has work to do, or null. */
    nextDeadline() {
        let next = null;
        for (const stored of this._events.values()) {
            const d = this._deadlineOf(stored);
            if (d !== null && (next === null || d < next))
                next = d;
        }
        return next;
    }

    /** Events in display order: most important first, then newest first. */
    ordered() {
        return [...this._events.values()]
            .map(s => s.event)
            .sort((a, b) =>
                priorityRank(a.priority) - priorityRank(b.priority) ||
                b.created_at - a.created_at ||
                (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    }

    _deadlineOf(stored) {
        return stored.removeAt ?? stored.event.expires_at ?? null;
    }

    _expiresAt(input, now) {
        if (input.persistent || isTerminal(input.state))
            return null;
        return now + (input.ttl_ms ?? this._config.defaultTtlMs);
    }

    _insert(input, priority, now) {
        const event = {
            id: input.id,
            source: input.source,
            category: input.category,
            state: input.state,
            priority,
            persistent: input.persistent,
            created_at: now,
            updated_at: now,
            expires_at: this._expiresAt(input, now),
            group: input.group ?? null,
            payload: input.payload,
            actions: input.actions,
        };
        const removeAt = isTerminal(event.state) ? now + this._config.finishedLingerMs : null;
        this._events.set(event.id, {event, removeAt});
        this._enforceCapacity(event.id);
    }

    _update(stored, input, priority, now) {
        const event = stored.event;
        const expiresAt = this._expiresAt(input, now);
        const samePhase = event.state === input.state ||
            (!isTerminal(event.state) && !isTerminal(input.state));
        const unchanged = samePhase &&
            sameJson(event.source, input.source) &&
            event.category === input.category &&
            event.priority === priority &&
            event.persistent === input.persistent &&
            event.group === (input.group ?? null) &&
            sameJson(event.payload, input.payload) &&
            sameJson(event.actions, input.actions);
        if (unchanged) {
            if (event.expires_at !== null)
                event.expires_at = expiresAt;
            return false;
        }
        // Replace the object so consumers can compare by identity.
        stored.event = {
            ...event,
            source: input.source,
            category: input.category,
            state: input.state,
            priority,
            persistent: input.persistent,
            group: input.group ?? null,
            payload: input.payload,
            actions: input.actions,
            updated_at: now,
            expires_at: expiresAt,
        };
        if (isTerminal(input.state))
            stored.removeAt = now + this._config.finishedLingerMs;
        return true;
    }

    _enforceCapacity(keepId) {
        while (this._events.size > this._config.maxEvents) {
            let victim = null;
            for (const {event} of this._events.values()) {
                if (event.id === keepId)
                    continue;
                if (victim === null || evictBefore(event, victim))
                    victim = event;
            }
            if (victim === null)
                return;
            this._events.delete(victim.id);
        }
    }
}

// Non-persistent first, then least important, then least recently updated.
function evictBefore(a, b) {
    if (a.persistent !== b.persistent)
        return !a.persistent;
    const pa = priorityRank(a.priority), pb = priorityRank(b.priority);
    if (pa !== pb)
        return pa > pb;
    return a.updated_at < b.updated_at;
}
