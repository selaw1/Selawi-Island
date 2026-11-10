// The single-row content shown inside the pill when it is not expanded.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {Waves} from './waves.js';
import {label, setAccent, setIcon, setIndicators, setText} from './widgets.js';

// Layout: icon and title on the left; on the right the "+N" count, voice
// waves (microphone only) and the trailing value (timer, percent). The
// title stops before the right side and ends with "…" when too long.
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
            x_align: Clutter.ActorAlign.START,
            y_align: Clutter.ActorAlign.CENTER,
            x_expand: true,
            y_expand: true,
        });
        this._icon = new St.Icon({style_class: 'selawi-compact-icon', y_align: Clutter.ActorAlign.CENTER});
        this._title = label('selawi-compact-title');
        this._center.add_child(this._icon);
        this._center.add_child(this._title);

        this._right = new St.BoxLayout({
            style_class: 'selawi-compact-right',
            x_align: Clutter.ActorAlign.END,
            y_align: Clutter.ActorAlign.CENTER,
            x_expand: true,
            y_expand: true,
        });
        this._badge = label('selawi-badge');
        this._indicators = new St.BoxLayout({style_class: 'selawi-indicators', y_align: Clutter.ActorAlign.CENTER});
        this._indicators.visible = false;
        this._waves = new Waves();
        this._waves.visible = false;
        this._trailing = label('selawi-compact-trailing');
        this._right.add_child(this._badge);
        this._right.add_child(this._indicators);
        this._right.add_child(this._waves);
        this._right.add_child(this._trailing);

        this.add_child(this._center);
        this.add_child(this._right);
        this._style = '';
    }

    // Keeps the title clear of the right side.
    _updateClearance() {
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const right = this._right.get_preferred_width(-1)[1];
        const padRight = right > 0 ? Math.ceil((right + 10 * scale) / scale) : 0;
        const style = `padding-right: ${padRight}px;`;
        if (style !== this._style) {
            this._style = style;
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
            this._waves.visible = false;
            this._updateClearance();
            return;
        }
        this._icon.visible = true;
        const hasLogo = setIcon(this._icon, vm.iconName, vm.logo);
        if (hasLogo)
            this._icon.add_style_class_name('selawi-logo');
        else
            this._icon.remove_style_class_name('selawi-logo');
        setAccent(this._icon, vm.accent);
        setIndicators(this._indicators, vm.indicators, 'selawi-indicator');
        for (const icon of this._indicators.get_children())
            setAccent(icon, vm.accent);
        setAccent(this._trailing, vm.accent);
        setText(this._title, vm.title);
        setText(this._trailing, vm.trailing);
        setText(this._badge, total > 1 ? `+${total - 1}` : '');
        this._waves.visible = vm.waves;
        this._waves.setAccent(vm.accent);
        this._updateClearance();
    }

    /** Whether voice waves are on screen right now. */
    get showsWaves() {
        return this._waves.visible;
    }

    setLevel(level) {
        if (this._waves.visible)
            this._waves.setLevel(level);
    }
});
