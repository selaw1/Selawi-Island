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
import {anchorSpot, customSpot, placePill, positionFromDrop} from './placement.js';
import {smooth} from './level.js';
import {describe, foldBrowserMedia, foldBrowserMic, foldMicIntoCalls, playingFirst} from './view-model.js';

const MORPH_MS = 420;
const FADE_MS = 180;
const HOVER_GROW = 10;
const MAX_RADIUS = 30;
const LEAVE_COLLAPSE_MS = 1200;
const TICK_MS = 1000;

const Mode = Object.freeze({IDLE: 'idle', COMPACT: 'compact', EXPANDED: 'expanded'});

export class Island {
    /**
     * @param {Gio.Settings} settings
     * @param {object} logger
     * @param {() => void} openPreferences - called on right-click
     */
    constructor(settings, logger, openPreferences) {
        this._settings = settings;
        this._log = logger;
        this._openPreferences = openPreferences;
        this._feed = null;
        this._mode = Mode.IDLE;
        this._expandedBy = null; // 'user' | 'auto'
        this._selectedId = null;
        this._seen = new Map(); // id -> created_at, to spot new events
        this._target = null;
        this._collapseTimer = 0;
        this._tickTimer = 0;
        this._drag = null; // {startX, startY, pointerX, pointerY} while dragging
        this._level = 0;
        this._wavesWanted = false;
        /** Called with true/false when voice waves appear on or leave the screen. */
        this.onWavesWanted = null;

        this._monitors = new MonitorManager(settings);
        this._monitors.connectObject('changed', () => {
            if (!this._savingDrop)
                this._relayout(false);
        }, 'clock-changed', () => {
            if (this._mode === Mode.IDLE)
                this._relayout(true);
        }, this);

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
        this._rightClick = new Clutter.ClickGesture({required_button: Clutter.BUTTON_SECONDARY});
        this._rightClick.connect('recognize', () => this._openPreferences?.());
        this._pill.add_action(this._rightClick);
        // Dragging starts only after the pointer moves 16 px, so clicks are
        // unaffected; once it starts it cancels the click.
        this._pan = new Clutter.PanGesture({required_button: Clutter.BUTTON_PRIMARY});
        this._pan.connect('recognize', () => this._onDragBegin());
        this._pan.connect('pan-update', () => this._onDragUpdate());
        this._pan.connect('end', () => this._onDragEnd());
        this._pan.connect('cancel', () => this._onDragCancel());
        this._pill.add_action(this._pan);
        this._pan.enabled = settings.get_boolean('allow-drag');
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
            if (key === 'allow-drag')
                this._pan.enabled = this._settings.get_boolean('allow-drag');
            else if (key !== 'simulation-enabled' && !this._savingDrop)
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
        this.onWavesWanted = null;
        this._releaseGrab();
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

    // During a confirmed call the "microphone in use" event is shown as
    // part of the call (foldMicIntoCalls), and a browser's own media
    // player is hidden while the extension shows its tabs (foldBrowserMedia).
    // Playing media is shown before paused media (playingFirst), and the
    // microphone bubble names the site when a browser records (foldBrowserMic).
    _events() {
        const folded = foldMicIntoCalls(foldBrowserMic(playingFirst(foldBrowserMedia(this._feed?.events() ?? []))));
        this._wavesFor = folded.wavesFor;
        return folded.events;
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
            // A user-opened island stays open and says "Nothing to show".
            if (!(this._mode === Mode.EXPANDED && this._expandedBy === 'user')) {
                this._expandedBy = null;
                this._mode = Mode.IDLE;
            }
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
        if (focusedVm && focused.id === this._wavesFor)
            focusedVm.waves = true;
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
        this._updateWavesWanted();
    }

    /** Microphone loudness 0…1 from the level meter. */
    setLevel(level) {
        this._level = smooth(this._level, level);
        if (this._mode === Mode.COMPACT)
            this._compact.setLevel(this._level);
        else if (this._mode === Mode.EXPANDED)
            this._expanded.setLevel(this._level);
    }

    // Waves are measured only while they are actually on screen.
    _updateWavesWanted() {
        const visibleView = this._mode === Mode.COMPACT ? this._compact
            : this._mode === Mode.EXPANDED ? this._expanded : null;
        const wanted = Boolean(visibleView?.showsWaves) && !this._hidden;
        if (wanted === this._wavesWanted)
            return;
        this._wavesWanted = wanted;
        if (!wanted)
            this._level = 0;
        this.onWavesWanted?.(wanted);
    }

    _setMode(mode, by = null) {
        this._clearCollapseTimer();
        this._mode = mode;
        this._expandedBy = mode === Mode.EXPANDED ? by : null;
        if (mode !== Mode.EXPANDED)
            this._selectedId = null;
        this._render();
        this._relayout(true);
    }

    _toggle() {
        if (this._mode === Mode.EXPANDED)
            this._collapse();
        else
            this._setMode(Mode.EXPANDED, 'user');
    }

    _collapse() {
        this._setMode(this._events().length > 0 ? Mode.COMPACT : Mode.IDLE);
    }

    // ---- geometry and animation -----------------------------------------

    _duration(ms) {
        const scale = this._settings.get_double('animation-duration-scale');
        return Math.round(ms * Math.min(3, Math.max(0, scale)));
    }

    _size() {
        const s = this._monitors.scale;
        const height = this._settings.get_int('compact-height') * s;
        const hovered = this._pill.hover && !this._drag;
        const grow = hovered && this._mode !== Mode.EXPANDED ? HOVER_GROW * s : 0;
        switch (this._mode) {
        case Mode.EXPANDED: {
            const width = this._settings.get_int('expanded-width') * s;
            const [, natural] = this._expanded.get_preferred_height(width);
            return {width, height: Math.max(height * 2, natural)};
        }
        case Mode.COMPACT:
            return {width: this._settings.get_int('compact-width') * s + grow, height: height + grow / 4};
        default: {
            // Slim when idle; grows to normal height under the pointer so it
            // is easy to click.
            const idleHeight = Math.min(height, this._settings.get_int('idle-height') * s);
            const clock = this._settings.get_boolean('idle-width-match-clock') ? this._monitors.clockWidth : null;
            const idleWidth = clock ?? this._settings.get_int('idle-width') * s;
            return {width: idleWidth + grow, height: hovered ? height : idleHeight};
        }
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
        const {x, y} = placePill(this._spot(area), area, width, height);
        const target = {x, y, width: Math.round(width), height: Math.round(height)};
        const duration = animate ? this._duration(MORPH_MS) : 0;
        const changed = !this._target || Object.keys(target).some(k => target[k] !== this._target[k]);
        this._target = target;

        if (changed) {
            // Same duration and curve for position and size keeps it centered.
            // While dragging, the pointer owns the position.
            const props = this._drag ? {width: target.width, height: target.height} : target;
            this._pill.ease({
                ...props,
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
        this._updateWavesWanted();
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
                this._collapse();
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

    // ---- dragging --------------------------------------------------------

    /** The default spot chosen in preferences. */
    _defaultSpot(area) {
        return anchorSpot(this._settings.get_string('default-anchor'), area, {
            topInset: area.topInset,
            margin: area.margin,
            custom: {
                fx: this._settings.get_double('default-x'),
                fy: this._settings.get_double('default-y'),
            },
        });
    }

    /** Where the island is now: the dragged position, or the default. */
    _spot(area) {
        if (!this._settings.get_boolean('custom-position'))
            return this._defaultSpot(area);
        return customSpot(area, {
            fx: this._settings.get_double('position-x'),
            fy: this._settings.get_double('position-y'),
        });
    }

    _onDragBegin() {
        const pointer = this._pan.get_centroid_abs();
        // Hold the pointer for the whole drag, like GNOME's own sliders.
        // Without this, Wayland hands the pointer to the window underneath
        // as soon as it leaves the pill and the drag is cancelled.
        this._grab = global.stage.grab(this._pill);
        this._pill.remove_transition('x');
        this._pill.remove_transition('y');
        this._drag = {startX: this._pill.x, startY: this._pill.y, pointerX: pointer.x, pointerY: pointer.y};
        this._clearCollapseTimer();
        this._pill.add_style_pseudo_class('dragging');
        this._relayout(true);
    }

    _onDragUpdate() {
        if (!this._drag)
            return;
        const pointer = this._pan.get_centroid_abs();
        this._pill.set_position(
            Math.round(this._drag.startX + pointer.x - this._drag.pointerX),
            Math.round(this._drag.startY + pointer.y - this._drag.pointerY));
    }

    _releaseGrab() {
        this._grab?.dismiss();
        this._grab = null;
    }

    _onDragEnd() {
        this._releaseGrab();
        if (!this._drag)
            return;
        this._drag = null;
        this._pill.remove_style_pseudo_class('dragging');

        // The monitor it was dropped on becomes the island's monitor.
        const index = this._monitors.monitorIndexFor(this._pill);
        const monitor = this._monitors.monitorAt(index);
        if (monitor) {
            // Write all keys at once, without relayouts in between.
            this._savingDrop = true;
            this._settings.delay();
            this._settings.set_int('monitor-index', index === this._monitors.primaryIndex ? -1 : index);
            const area = this._monitors.area();
            const pos = area && positionFromDrop(area, this._defaultSpot(area),
                this._pill.x, this._pill.y, this._pill.width, this._pill.height, this._monitors.scale);
            if (pos) {
                this._settings.set_double('position-x', pos.fx);
                this._settings.set_double('position-y', pos.fy);
            }
            this._settings.set_boolean('custom-position', pos !== null);
            this._settings.apply();
            this._savingDrop = false;
        }
        this._target = null; // force a fresh ease from where it was dropped
        this._relayout(true);
        if (this._mode === Mode.EXPANDED && !this._pill.hover)
            this._scheduleCollapse(LEAVE_COLLAPSE_MS);
    }

    _onDragCancel() {
        this._releaseGrab();
        if (!this._drag)
            return;
        this._drag = null;
        this._pill.remove_style_pseudo_class('dragging');
        this._target = null;
        this._relayout(true);
    }

    _scheduleCollapse(ms) {
        this._clearCollapseTimer();
        if (this._expandedBy === 'auto')
            this._autoDeadline = Date.now() + ms;
        this._collapseTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            this._collapseTimer = 0;
            // Never collapse under the pointer; leaving will reschedule.
            if (this._mode === Mode.EXPANDED && !this._pill.hover && !this._drag)
                this._collapse();
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
