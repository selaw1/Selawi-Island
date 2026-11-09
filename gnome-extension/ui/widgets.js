// Small St helpers shared by the compact and expanded views.

import Clutter from 'gi://Clutter';
import Pango from 'gi://Pango';
import St from 'gi://St';

export const ACCENTS = ['neutral', 'green', 'blue', 'pink', 'orange', 'red'];

export function label(styleClass, params = {}) {
    const l = new St.Label({style_class: styleClass, y_align: Clutter.ActorAlign.CENTER, ...params});
    l.clutter_text.ellipsize = Pango.EllipsizeMode.END;
    return l;
}

/** Swaps the accent-* style class on an actor. */
export function setAccent(actor, accent) {
    for (const a of ACCENTS) {
        if (a !== accent)
            actor.remove_style_class_name(`accent-${a}`);
    }
    actor.add_style_class_name(`accent-${accent}`);
}

/** Only touches the actor when the text really changed (avoids relayouts). */
export function setText(lbl, text) {
    if (lbl.text !== text)
        lbl.text = text;
    lbl.visible = text.length > 0;
}

export function setIcon(icon, name) {
    if (icon.icon_name !== name)
        icon.icon_name = name;
}
