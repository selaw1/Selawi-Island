// Small animated bars that follow microphone loudness.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Graphene from 'gi://Graphene';
import St from 'gi://St';

import {BAR_COUNT, barHeights} from './level.js';
import {setAccent} from './widgets.js';

export const Waves = GObject.registerClass(
class Waves extends St.BoxLayout {
    constructor() {
        super({style_class: 'selawi-waves', y_align: Clutter.ActorAlign.CENTER});
        this._bars = [];
        for (let i = 0; i < BAR_COUNT; i++) {
            const bar = new St.Widget({
                style_class: 'selawi-wave-bar',
                y_align: Clutter.ActorAlign.CENTER,
                pivot_point: new Graphene.Point({x: 0.5, y: 0.5}),
            });
            this._bars.push(bar);
            this.add_child(bar);
        }
        this._phase = 0;
        this.setLevel(0);
    }

    setAccent(accent) {
        for (const bar of this._bars)
            setAccent(bar, accent);
    }

    /** @param {number} level - 0…1 */
    setLevel(level) {
        this._phase += 1;
        const heights = barHeights(level, this._phase);
        this._bars.forEach((bar, i) => {
            // Scaling only: no relayout, cheap to draw.
            bar.ease({
                scale_y: heights[i],
                duration: 70,
                mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            });
        });
    }
});
