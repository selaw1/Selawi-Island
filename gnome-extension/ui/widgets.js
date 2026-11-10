// Small St helpers shared by the compact and expanded views.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
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

const PNG_DATA_PREFIX = 'data:image/png;base64,';
const logoCache = new Map();

/**
 * A GIcon for `source.icon`: an installed app's own icon ("app:spotify",
 * looked up like GNOME's dock does) or a small PNG from the browser
 * extension. Null when there is none. Cached, so a refresh every second
 * does not decode it again.
 */
export function logoGicon(logo) {
    if (!logo)
        return null;
    if (logoCache.has(logo))
        return logoCache.get(logo);
    let gicon = null;
    try {
        if (logo.startsWith('app:')) {
            const id = logo.slice(4);
            const apps = Shell.AppSystem.get_default();
            const app = apps.lookup_app(`${id}.desktop`) ?? apps.lookup_heuristic_basename(`${id}.desktop`);
            gicon = app?.get_icon() ?? null;
        } else if (logo.startsWith(PNG_DATA_PREFIX)) {
            gicon = Gio.BytesIcon.new(new GLib.Bytes(GLib.base64_decode(logo.slice(PNG_DATA_PREFIX.length))));
        }
    } catch {
        gicon = null;
    }
    logoCache.set(logo, gicon);
    if (logoCache.size > 64)
        logoCache.delete(logoCache.keys().next().value);
    return gicon;
}

/** Shows the logo if there is one, otherwise the symbolic icon `name`. */
export function setIcon(icon, name, logo = null) {
    const gicon = logoGicon(logo);
    const key = gicon ? `logo:${logo}` : name;
    if (icon._selawiKey === key)
        return Boolean(gicon);
    icon._selawiKey = key;
    if (gicon)
        icon.gicon = gicon;
    else
        icon.icon_name = name;
    return Boolean(gicon);
}

/**
 * Keeps a box of small status icons in step with `names`, reusing actors.
 */
export function setIndicators(box, names, styleClass) {
    const icons = box.get_children();
    names.forEach((name, i) => {
        let icon = icons[i];
        if (!icon) {
            icon = new St.Icon({style_class: styleClass, y_align: Clutter.ActorAlign.CENTER});
            box.add_child(icon);
        }
        setIcon(icon, name);
    });
    for (const extra of icons.slice(names.length))
        extra.destroy();
    box.visible = names.length > 0;
}
