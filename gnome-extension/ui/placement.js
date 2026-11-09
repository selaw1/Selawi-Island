// Where the pill goes on a monitor. Pure functions (no GNOME imports) so
// they are unit-tested with Node.
//
// A "spot" is a reference point plus how the pill hangs from it:
// alignX/alignY of 0 means the pill's left/top edge is at the point,
// 0.5 its center, 1 its right/bottom edge. That way a pill anchored at the
// bottom grows upward and one anchored at the top grows downward.
//
// Custom positions (dragged, or a custom default) are stored as fractions
// of the monitor: fx is the pill's horizontal center, fy its top edge.
// Fractions survive resolution and scale changes better than pixels.

/** Distance (logical px) within which a drop snaps back to the default spot. */
export const SNAP_DISTANCE = 40;

export const ANCHORS = Object.freeze([
    'top-center', 'top-left', 'top-right',
    'center',
    'bottom-center', 'bottom-left', 'bottom-right',
    'custom',
]);

function clamp(v, lo, hi) {
    return Math.min(Math.max(v, lo), Math.max(lo, hi));
}

/**
 * The spot for a named anchor.
 *
 * @param {string} anchor - one of ANCHORS (unknown values mean top-center)
 * @param {{x: number, y: number, width: number, height: number}} monitor
 * @param {object} opts
 * @param {number} opts.topInset - space taken by the top bar (0 if none)
 * @param {number} opts.margin - gap from the screen edge
 * @param {{fx: number, fy: number}} [opts.custom] - used by 'custom'
 * @returns {{x: number, y: number, alignX: number, alignY: number}}
 */
export function anchorSpot(anchor, monitor, {topInset = 0, margin = 0, custom = null} = {}) {
    if (anchor === 'custom' && custom)
        return customSpot(monitor, custom);

    const [vertical, horizontal = 'center'] = anchor === 'center'
        ? ['center', 'center'] : String(anchor).split('-');

    let x = monitor.x + monitor.width / 2, alignX = 0.5;
    if (horizontal === 'left') {
        x = monitor.x + margin;
        alignX = 0;
    } else if (horizontal === 'right') {
        x = monitor.x + monitor.width - margin;
        alignX = 1;
    }

    let y = monitor.y + topInset + margin, alignY = 0;
    if (vertical === 'center') {
        y = monitor.y + monitor.height / 2;
        alignY = 0.5;
    } else if (vertical === 'bottom') {
        y = monitor.y + monitor.height - margin;
        alignY = 1;
    }
    return {x, y, alignX, alignY};
}

/** Spot for a position stored as monitor fractions. */
export function customSpot(monitor, {fx, fy}) {
    return {
        x: monitor.x + clamp(fx, 0, 1) * monitor.width,
        y: monitor.y + clamp(fy, 0, 1) * monitor.height,
        alignX: 0.5,
        alignY: 0,
    };
}

/**
 * Top-left corner for a pill of this size at this spot, kept fully on the
 * monitor.
 *
 * @returns {{x: number, y: number}}
 */
export function placePill(spot, monitor, width, height) {
    return {
        x: Math.round(clamp(spot.x - spot.alignX * width, monitor.x, monitor.x + monitor.width - width)),
        y: Math.round(clamp(spot.y - spot.alignY * height, monitor.y, monitor.y + monitor.height - height)),
    };
}

/** Monitor fractions for a pill whose top-left corner is at (x, y). */
export function fractionsFor(monitor, x, y, width) {
    return {
        fx: clamp((x + width / 2 - monitor.x) / monitor.width, 0, 1),
        fy: clamp((y - monitor.y) / monitor.height, 0, 1),
    };
}

/**
 * Converts a dropped pill into a saved position, or null when it was
 * dropped close enough to the default spot to snap back there.
 *
 * @returns {{fx: number, fy: number} | null}
 */
export function positionFromDrop(monitor, defaultSpot, x, y, width, height, scale = 1) {
    const home = placePill(defaultSpot, monitor, width, height);
    if (Math.hypot(x - home.x, y - home.y) <= SNAP_DISTANCE * scale)
        return null;
    return fractionsFor(monitor, x, y, width);
}
