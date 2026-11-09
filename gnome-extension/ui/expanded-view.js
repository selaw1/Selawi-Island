// The detailed content shown when the island is expanded: the focused
// event with progress and actions, plus a short list of the other events.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Graphene from 'gi://Graphene';
import St from 'gi://St';

import {Waves} from './waves.js';
import {label, setAccent, setIcon, setText} from './widgets.js';

const MAX_OTHER_ROWS = 3;

export const ExpandedView = GObject.registerClass({
    Signals: {
        'action-activated': {param_types: [GObject.TYPE_STRING, GObject.TYPE_STRING]},
        'dismiss-requested': {param_types: [GObject.TYPE_STRING]},
        'event-selected': {param_types: [GObject.TYPE_STRING]},
    },
}, class ExpandedView extends St.BoxLayout {
    constructor() {
        super({
            style_class: 'selawi-expanded',
            orientation: Clutter.Orientation.VERTICAL,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.START,
        });

        // Shown instead of everything else when there are no events.
        this._empty = label('selawi-empty', {text: 'Nothing to show', x_align: Clutter.ActorAlign.CENTER});
        this.add_child(this._empty);

        // Header: icon tile, title + subtitle, trailing value.
        const header = new St.BoxLayout({style_class: 'selawi-expanded-header'});
        this._header = header;
        this._iconTile = new St.Bin({style_class: 'selawi-icon-tile', y_align: Clutter.ActorAlign.CENTER});
        this._icon = new St.Icon({style_class: 'selawi-expanded-icon'});
        this._iconTile.set_child(this._icon);
        const text = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._title = label('selawi-title');
        this._subtitle = label('selawi-subtitle');
        text.add_child(this._title);
        text.add_child(this._subtitle);
        this._trailing = label('selawi-trailing');
        this._waves = new Waves();
        this._waves.visible = false;
        header.add_child(this._iconTile);
        header.add_child(text);
        header.add_child(this._waves);
        header.add_child(this._trailing);
        this.add_child(header);

        // Progress bar. The fill spans the track and is scaled horizontally,
        // so it can be eased without relayout.
        this._track = new St.Widget({
            style_class: 'selawi-progress-track',
            layout_manager: new Clutter.BinLayout(),
            x_expand: true,
        });
        this._fill = new St.Widget({
            style_class: 'selawi-progress-fill',
            x_expand: true,
            y_expand: true,
            scale_x: 0,
            pivot_point: new Graphene.Point({x: 0, y: 0.5}),
        });
        this._track.add_child(this._fill);
        this.add_child(this._track);

        this._actions = new St.BoxLayout({style_class: 'selawi-actions'});
        this.add_child(this._actions);
        this._actionsKey = null;

        this._others = new St.BoxLayout({
            style_class: 'selawi-others',
            orientation: Clutter.Orientation.VERTICAL,
        });
        this.add_child(this._others);
        this._otherRows = [];
        this._othersKey = null;

        this._eventId = null;
        this._progress = null;
    }

    /**
     * @param {object|null} event - the focused event
     * @param {object|null} vm - its view model
     * @param {Array<{event: object, vm: object}>} others - remaining events
     * @param {number} animationMs - duration for the progress bar ease
     */
    update(event, vm, others, animationMs) {
        this._eventId = event?.id ?? null;
        const empty = !event || !vm;
        this._empty.visible = empty;
        this._header.visible = !empty;
        this._actions.visible = !empty;
        if (empty) {
            this._track.visible = false;
            this._waves.visible = false;
            this._rebuildActions([]);
            this._rebuildOthers([]);
            return;
        }

        setIcon(this._icon, vm.iconName);
        setAccent(this._iconTile, vm.accent);
        setText(this._title, vm.title);
        setText(this._subtitle, vm.subtitle);
        setText(this._trailing, vm.trailing);
        setAccent(this._trailing, vm.accent);
        this._waves.visible = vm.waves;
        this._waves.setAccent(vm.accent);
        this._updateProgress(vm, animationMs);
        this._rebuildActions(event.actions);
        this._rebuildOthers(others);
    }

    /** Whether voice waves are on screen right now. */
    get showsWaves() {
        return this._waves.visible;
    }

    setLevel(level) {
        if (this._waves.visible)
            this._waves.setLevel(level);
    }

    _updateProgress(vm, animationMs) {
        setAccent(this._fill, vm.accent);
        this._track.visible = vm.progress !== null;
        if (vm.progress === null || vm.progress === this._progress)
            return;
        const first = this._progress === null;
        this._progress = vm.progress;
        this._fill.remove_transition('scale-x');
        if (first || vm.live) {
            // Live media position moves every second: no easing needed.
            this._fill.scale_x = vm.progress;
        } else {
            this._fill.ease({
                scale_x: vm.progress,
                duration: animationMs,
                mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            });
        }
    }

    _rebuildActions(actions) {
        const key = JSON.stringify([this._eventId, actions]);
        if (key === this._actionsKey)
            return;
        this._actionsKey = key;
        this._actions.destroy_all_children();
        if (!this._eventId)
            return;
        const eventId = this._eventId;
        for (const action of actions) {
            this._actions.add_child(this._button(action.label, 'selawi-action', () =>
                this.emit('action-activated', eventId, action.id)));
        }
        this._actions.add_child(this._button('Dismiss', 'selawi-action selawi-action-dismiss', () =>
            this.emit('dismiss-requested', eventId)));
    }

    _button(text, styleClass, onClick) {
        const button = new St.Button({
            style_class: styleClass,
            label: text,
            can_focus: true,
            x_expand: true,
        });
        button.connect('clicked', onClick);
        return button;
    }

    _rebuildOthers(others) {
        const shown = others.slice(0, MAX_OTHER_ROWS);
        const key = shown.map(o => o.event.id).join('\n') + `|${others.length}`;
        if (key !== this._othersKey) {
            this._othersKey = key;
            this._others.destroy_all_children();
            this._otherRows = shown.map(o => this._makeRow(o.event.id));
            for (const row of this._otherRows)
                this._others.add_child(row.button);
            if (others.length > shown.length) {
                this._others.add_child(label('selawi-more', {
                    text: `${others.length - shown.length} more`,
                }));
            }
        }
        this._others.visible = shown.length > 0;
        shown.forEach((o, i) => {
            const row = this._otherRows[i];
            setIcon(row.icon, o.vm.iconName);
            setAccent(row.icon, o.vm.accent);
            setText(row.title, o.vm.title);
            setText(row.trailing, o.vm.trailing);
        });
    }

    _makeRow(eventId) {
        const box = new St.BoxLayout({style_class: 'selawi-other-row-box', x_expand: true});
        const icon = new St.Icon({style_class: 'selawi-other-icon', y_align: Clutter.ActorAlign.CENTER});
        const title = label('selawi-other-title', {x_expand: true});
        const trailing = label('selawi-other-trailing');
        box.add_child(icon);
        box.add_child(title);
        box.add_child(trailing);
        const button = new St.Button({
            style_class: 'selawi-other-row',
            child: box,
            can_focus: true,
            x_expand: true,
        });
        button.connect('clicked', () => this.emit('event-selected', eventId));
        return {button, icon, title, trailing};
    }
});
