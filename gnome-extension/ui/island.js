// The Dynamic Island itself: a black pill in the top chrome that morphs
// between idle, compact and expanded.
//
// Rendering rules:
// - Actors are created once; refreshes only change text, icons and size.
// - Size and position are eased together, so the pill stays centered and
//   any new target interrupts the running animation smoothly.
// - GNOME's "reduce animations" setting is honoured by Clutter's ease().

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {CompactView} from './compact-view.js';
import {ExpandedView} from './expanded-view.js';
import {MonitorManager} from './monitor-manager.js';
import {describe} from './view-model.js';

const MORPH_MS = 420;
const FADE_MS = 180;
const HOVER_GROW = 10;
const MAX_RADIUS = 30;
const LEAVE_COLLAPSE_MS = 1200;
const TICK_MS = 1000;

const Mode = Object.freeze({IDLE: 'idle', COMPACT: 'compact', EXPANDED: 'expanded'});

export class Island {
    constructor(settings, logger) {
        this._settings = settings;
        this._log = logger;
        this._feed = null;
        this._mode = Mode.IDLE;
        this._expandedBy = null; // 'user' | 'auto'
        this._selectedId = null;
        this._seen = new Map(); // id -> created_at, to spot new events
        this._target = null;
        this._collapseTimer = 0;
        this._tickTimer = 0;

        this._monitors = new MonitorManager(settings);
        this._monitors.connectObject('changed', () => this._relayout(false), this);

        this._pill = new St.Widget({
            style_class: 'selawi-island',
            reactive: true,
            track_hover: true,
            can_focus: true,
            accessible_name: 'Selawi Island',
            layout_manager: new Clutter.BinLayout(),
        });
        this._clip = new St.Widget({
            layout_manager: new Clutter.BinLayout(),
            clip_to_allocation: true,
            x_expand: true,
            y_expand: true,
        });
        this._compact = new CompactView();
        this._expanded = new ExpandedView();
        this._expanded.opacity = 0;
        this._expanded.visible = false;
        this._clip.add_child(this._compact);
        this._clip.add_child(this._expanded);
        this._pill.add_child(this._clip);

        this._pill.connect('notify::hover', () => this._onHoverChanged());

        // GNOME 51 input: gestures and key controllers, not event signals.
        this._click = new Clutter.ClickGesture({required_button: Clutter.BUTTON_PRIMARY});
        this._click.connect('recognize', () => this._onClick());
        this._pill.add_action(this._click);
        this._keys = new Clutter.KeyController();
        this._keys.connect('key-press', () => this._onKeyPress());
        this._pill.add_action(this._keys);
        this._pill.connect('notify::height', () => this._updateRadius());

        this._expanded.connect('action-activated', (_v, id, actionId) =>
            this._feed?.invokeAction(id, actionId));
        this._expanded.connect('dismiss-requested', (_v, id) => this._feed?.dismiss(id));
        this._expanded.connect('event-selected', (_v, id) => {
            this._selectedId = id;
            this._refresh();
        });

        this._settings.connectObject('changed', (_s, key) => {
            if (key !== 'simulation-enabled')
                this._relayout(true);
        }, this);

        // GNOME 51 accepts only trackFullscreen and affectsStruts here; a
        // visible reactive actor gets pointer input automatically. Fullscreen
        // is handled by us so we also control visibility when idle.
        Main.layoutManager.addTopChrome(this._pill, {trackFullscreen: false});
        // Keyboard access: Ctrl+Alt+Tab focuses the island, like the top bar.
        // Removed automatically when the pill is destroyed.
        Main.ctrlAltTabManager.addGroup(this._pill, 'Selawi Island', 'dialog-information-symbolic');

        // Place it without animation first, so it doesn't fly in from 0,0.
        this._relayout(false);
        this._refresh();
    }

    setFeed(feed) {
        this._feed?.disconnectObject(this);
        this._feed = feed;
        this._feed?.connectObject('changed', () => this._refresh(), this);
        this._selectedId = null;
        this._seen.clear();
        this._refresh();
    }

    destroy() {
        this._clearCollapseTimer();
        this._stopTicking();
        this._feed?.disconnectObject(this);
        this._feed = null;
        this._settings.disconnectObject(this);
        this._monitors.disconnectObject(this);
        this._monitors.destroy();
        // Removes it from the chrome and untracks it.
        this._pill.destroy();
        this._pill = null;
    }

    // ---- state -----------------------------------------------------------

    _events() {
        return this._feed?.events() ?? [];
    }

    _focusedEvent(events) {
        if (this._selectedId) {
            const selected = events.find(e => e.id === this._selectedId);
            if (selected)
                return selected;
            this._selectedId = null;
        }
        return events[0] ?? null;
    }

    _refresh() {
        const events = this._events();

        // A new event (or a restarted one) that becomes the most important
        // briefly expands the island. Lower-priority arrivals stay quiet,
        // so nothing interrupts an active call.
        const top = events[0];
        const isNewTop = top && this._seen.get(top.id) !== top.created_at;
        this._seen = new Map(events.map(e => [e.id, e.created_at]));

        if (events.length === 0) {
            this._selectedId = null;
            this._expandedBy = null;
            this._mode = Mode.IDLE;
        } else if (this._mode === Mode.IDLE) {
            this._mode = Mode.COMPACT;
        }

        if (isNewTop && this._settings.get_boolean('auto-expand') &&
            (this._mode === Mode.COMPACT || this._expandedBy === 'auto') && !this._pill.hover) {
            this._selectedId = null;
            this._setMode(Mode.EXPANDED, 'auto');
            this._scheduleCollapse(this._settings.get_int('auto-expand-ms'));
            return;
        }

        this._render(events);
        this._relayout(true);
    }

    _render(events = this._events()) {
        const now = Date.now();
        const focused = this._focusedEvent(events);
        const focusedVm = focused ? describe(focused, now) : null;
        let live = focusedVm?.live ?? false;

        this._compact.update(focusedVm, events.length);

        if (this._mode === Mode.EXPANDED) {
            const others = events.filter(e => e !== focused).map(e => {
                const vm = describe(e, now);
                live ||= vm.live;
                return {event: e, vm};
            });
            this._expanded.update(focused, focusedVm, others, this._duration(MORPH_MS));
        }

        if (live)
            this._startTicking();
        else
            this._stopTicking();
    }

    _setMode(mode, by = null) {
        this._clearCollapseTimer();
        if (mode === Mode.EXPANDED && this._events().length === 0)
            return;
        this._mode = mode;
        this._expandedBy = mode === Mode.EXPANDED ? by : null;
        if (mode !== Mode.EXPANDED)
            this._selectedId = null;
        this._render();
        this._relayout(true);
    }

    _toggle() {
        if (this._mode === Mode.EXPANDED)
            this._setMode(Mode.COMPACT);
        else if (this._mode === Mode.COMPACT)
            this._setMode(Mode.EXPANDED, 'user');
    }

    // ---- geometry and animation -----------------------------------------

    _duration(ms) {
        const scale = this._settings.get_double('animation-duration-scale');
        return Math.round(ms * Math.min(3, Math.max(0, scale)));
    }

    _size() {
        const s = this._monitors.scale;
        const height = this._settings.get_int('compact-height') * s;
        const grow = this._pill.hover && this._mode === Mode.COMPACT ? HOVER_GROW * s : 0;
        switch (this._mode) {
        case Mode.EXPANDED: {
            const width = this._settings.get_int('expanded-width') * s;
            const [, natural] = this._expanded.get_preferred_height(width);
            return {width, height: Math.max(height * 2, natural)};
        }
        case Mode.COMPACT:
            return {width: this._settings.get_int('compact-width') * s + grow, height: height + grow / 4};
        default:
            return {width: this._settings.get_int('idle-width') * s, height};
        }
    }

    _relayout(animate) {
        const area = this._monitors.area();
        if (!area || !this._pill)
            return;

        const s = this._monitors.scale;
        this._compact.set_size(this._settings.get_int('compact-width') * s,
            this._settings.get_int('compact-height') * s);
        this._expanded.width = this._settings.get_int('expanded-width') * s;
        // Measure the expanded view only while it is shown (fades in below).
        if (this._mode === Mode.EXPANDED)
            this._expanded.show();

        const hiddenWhenIdle = this._mode === Mode.IDLE && !this._settings.get_boolean('show-when-idle');
        const hidden = hiddenWhenIdle || this._monitors.fullscreenBlocked;

        const {width, height} = this._size();
        const target = {
            x: Math.round(area.x + (area.width - width) / 2),
            y: area.y,
            width: Math.round(width),
            height: Math.round(height),
        };
        const duration = animate ? this._duration(MORPH_MS) : 0;
        const changed = !this._target || Object.keys(target).some(k => target[k] !== this._target[k]);
        this._target = target;

        if (changed) {
            // Same duration and curve for position and size keeps it centered.
            this._pill.ease({
                ...target,
                duration,
                mode: Clutter.AnimationMode.EASE_OUT_EXPO,
            });
        }

        this._fadeContent(animate);
        this._setHidden(hidden, animate);
    }

    _fadeContent(animate) {
        const expanded = this._mode === Mode.EXPANDED;
        const compactOpacity = this._mode === Mode.COMPACT ? 255 : 0;
        this._ease(this._compact, compactOpacity, animate, expanded ? 0 : FADE_MS / 2);

        if (expanded)
            this._expanded.show();
        this._ease(this._expanded, expanded ? 255 : 0, animate, expanded ? FADE_MS / 2 : 0, () => {
            if (this._mode !== Mode.EXPANDED)
                this._expanded.hide();
        });
    }

    _ease(actor, opacity, animate, delay, onComplete) {
        if (actor.opacity === opacity && !actor.get_transition('opacity')) {
            onComplete?.();
            return;
        }
        actor.ease({
            opacity,
            duration: animate ? this._duration(FADE_MS) : 0,
            delay: animate ? this._duration(delay) : 0,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: isFinished => {
                if (isFinished)
                    onComplete?.();
            },
        });
    }

    _setHidden(hidden, animate) {
        if (hidden === this._hidden)
            return;
        this._hidden = hidden;
        if (!hidden)
            this._pill.show();
        this._pill.ease({
            opacity: hidden ? 0 : 255,
            duration: animate ? this._duration(FADE_MS) : 0,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: isFinished => {
                // Hide only at the end so the input region is released.
                if (isFinished && this._hidden)
                    this._pill.hide();
            },
        });
    }

    _updateRadius() {
        const radius = Math.min(this._pill.height / 2, MAX_RADIUS * this._monitors.scale);
        this._pill.set_style(`border-radius: ${Math.round(radius)}px;`);
    }

    // ---- interaction -----------------------------------------------------

    _onHoverChanged() {
        if (this._pill.hover) {
            this._clearCollapseTimer();
        } else if (this._mode === Mode.EXPANDED) {
            const wait = this._expandedBy === 'auto' && this._autoDeadline
                ? Math.max(LEAVE_COLLAPSE_MS, this._autoDeadline - Date.now())
                : LEAVE_COLLAPSE_MS;
            this._scheduleCollapse(wait);
        }
        this._relayout(true);
    }

    _onClick() {
        // Clicks on action buttons and event rows inside the pill belong to
        // those buttons, not to the expand/collapse toggle.
        const event = Clutter.get_current_event();
        if (event) {
            const [x, y] = event.get_coords();
            let actor = global.stage.get_actor_at_pos(Clutter.PickMode.REACTIVE, x, y);
            for (; actor && actor !== this._pill; actor = actor.get_parent()) {
                if (actor instanceof St.Button)
                    return;
            }
        }
        this._toggle();
    }

    _onKeyPress() {
        const focused = this._focusedEvent(this._events());
        const [, symbol] = this._keys.get_key();
        switch (symbol) {
        case Clutter.KEY_Return:
        case Clutter.KEY_KP_Enter:
        case Clutter.KEY_space:
            this._toggle();
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Escape:
            if (this._mode === Mode.EXPANDED) {
                this._setMode(Mode.COMPACT);
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        case Clutter.KEY_Delete:
            if (focused) {
                this._feed?.dismiss(focused.id);
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        default:
            return Clutter.EVENT_PROPAGATE;
        }
    }

    _scheduleCollapse(ms) {
        this._clearCollapseTimer();
        if (this._expandedBy === 'auto')
            this._autoDeadline = Date.now() + ms;
        this._collapseTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            this._collapseTimer = 0;
            // Never collapse under the pointer; leaving will reschedule.
            if (this._mode === Mode.EXPANDED && !this._pill.hover)
                this._setMode(Mode.COMPACT);
            return GLib.SOURCE_REMOVE;
        });
    }

    _clearCollapseTimer() {
        if (this._collapseTimer) {
            GLib.source_remove(this._collapseTimer);
            this._collapseTimer = 0;
        }
        this._autoDeadline = 0;
    }

    // Runs only while something visible changes with time (call duration,
    // playing media). Otherwise the island is fully event-driven.
    _startTicking() {
        if (this._tickTimer)
            return;
        this._tickTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, TICK_MS, () => {
            this._render();
            if (this._mode === Mode.EXPANDED)
                this._relayout(true);
            return this._tickTimer ? GLib.SOURCE_CONTINUE : GLib.SOURCE_REMOVE;
        });
    }

    _stopTicking() {
        if (this._tickTimer) {
            GLib.source_remove(this._tickTimer);
            this._tickTimer = 0;
        }
    }
}
