// The single-row content shown inside the pill when it is not expanded.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {label, setAccent, setIcon, setText} from './widgets.js';

// Layout: the "+N" count sits at the left edge and the trailing value
// (timer, percent) at the right edge. The icon and title are centered when
// they fit between the two with equal clearance; when the title is too
// long for that, they start at the left instead, so the title gets all the
// room up to the trailing value.
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
        this._centered = true;
        this._style = '';
    }

    _updateClearance() {
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const gap = 8 * scale;
        const width = actor => (actor.visible ? actor.get_preferred_width(-1)[1] : 0);
        const left = width(this._badge) > 0 ? width(this._badge) + gap : 0;
        const right = width(this._trailing) > 0 ? width(this._trailing) + gap : 0;
        const side = Math.max(left, right);

        const available = this.width - this.get_theme_node().get_horizontal_padding();
        const content = this._center.get_preferred_width(-1)[1] -
            this._center.get_theme_node().get_horizontal_padding();
        // A little hysteresis so a ticking timer doesn't flip the layout.
        const centeredRoom = available - 2 * side;
        const centered = this._centered ? content <= centeredRoom : content <= centeredRoom - 12 * scale;

        const [padLeft, padRight] = centered ? [side, side] : [left, right];
        // St scales CSS pixels by the scale factor; these are stage pixels.
        const style = `padding: 0 ${Math.ceil(padRight / scale)}px 0 ${Math.ceil(padLeft / scale)}px;`;
        if (centered !== this._centered || style !== this._style) {
            this._centered = centered;
            this._style = style;
            this._center.x_align = centered ? Clutter.ActorAlign.CENTER : Clutter.ActorAlign.START;
            this._center.set_style(style);
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
