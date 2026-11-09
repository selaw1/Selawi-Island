// The single-row content shown inside the pill when it is not expanded.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {label, setAccent, setIcon, setText} from './widgets.js';

// Layout: the icon and title are centered in the pill as one group. The
// "+N" count sits at the left edge and the trailing value (timer, percent)
// at the right edge. The centered group keeps the same clearance on both
// sides, so it stays centered and never runs under either edge item.
export const CompactView = GObject.registerClass(
class CompactView extends St.Widget {
    constructor() {
        super({
            style_class: 'selawi-compact',
            layout_manager: new Clutter.BinLayout(),
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.START,
        });

        this._center = new St.BoxLayout({
            style_class: 'selawi-compact-center',
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
            x_expand: true,
            y_expand: true,
        });
        this._icon = new St.Icon({style_class: 'selawi-compact-icon', y_align: Clutter.ActorAlign.CENTER});
        this._title = label('selawi-compact-title');
        this._center.add_child(this._icon);
        this._center.add_child(this._title);

        this._badge = label('selawi-badge', {x_align: Clutter.ActorAlign.START, x_expand: true, y_expand: true});
        this._trailing = label('selawi-compact-trailing', {x_align: Clutter.ActorAlign.END, x_expand: true, y_expand: true});

        this.add_child(this._center);
        this.add_child(this._badge);
        this.add_child(this._trailing);
        this._clearance = -1;
    }

    _updateClearance() {
        const width = actor => (actor.visible ? actor.get_preferred_width(-1)[1] : 0);
        const side = Math.max(width(this._badge), width(this._trailing));
        const clearance = side > 0 ? Math.ceil(side) + 8 : 0;
        if (clearance !== this._clearance) {
            this._clearance = clearance;
            this._center.set_style(`padding: 0 ${clearance}px;`);
        }
    }

    /**
     * @param {object|null} vm - view model from view-model.js, or null when idle
     * @param {number} total - number of events
     */
    update(vm, total) {
        if (!vm) {
            setText(this._title, '');
            setText(this._trailing, '');
            setText(this._badge, '');
            this._icon.visible = false;
            this._updateClearance();
            return;
        }
        this._icon.visible = true;
        setIcon(this._icon, vm.iconName);
        setAccent(this._icon, vm.accent);
        setAccent(this._trailing, vm.accent);
        setText(this._title, vm.title);
        setText(this._trailing, vm.trailing);
        setText(this._badge, total > 1 ? `+${total - 1}` : '');
        this._updateClearance();
    }
});
