// The single-row content shown inside the pill when it is not expanded.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {label, setAccent, setIcon, setText} from './widgets.js';

export const CompactView = GObject.registerClass(
class CompactView extends St.BoxLayout {
    constructor() {
        super({
            style_class: 'selawi-compact',
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.START,
        });

        this._icon = new St.Icon({style_class: 'selawi-compact-icon', y_align: Clutter.ActorAlign.CENTER});
        this._title = label('selawi-compact-title', {x_expand: true});
        this._badge = label('selawi-badge');
        this._trailing = label('selawi-compact-trailing');

        this.add_child(this._icon);
        this.add_child(this._title);
        this.add_child(this._badge);
        this.add_child(this._trailing);
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
            return;
        }
        this._icon.visible = true;
        setIcon(this._icon, vm.iconName);
        setAccent(this._icon, vm.accent);
        setAccent(this._trailing, vm.accent);
        setText(this._title, vm.title);
        setText(this._trailing, vm.trailing);
        setText(this._badge, total > 1 ? `+${total - 1}` : '');
    }
});
