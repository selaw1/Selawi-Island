// Where the pill goes on a monitor. Pure functions (no GNOME imports) so
// they are unit-tested with Node.
//
// A custom position is stored as fractions of the monitor: fx is the
// pill's horizontal center, fy its top edge. Fractions survive resolution
// and scale changes better than pixels.

/** Distance (logical px) within which a drop snaps back to the default spot. */
export const SNAP_DISTANCE = 40;

function clamp(v, lo, hi) {
    return Math.min(Math.max(v, lo), Math.max(lo, hi));
}

/**
 * @param {{x: number, y: number, width: number, height: number}} monitor
 * @param {number} defaultTop - y of the default (top-center) spot
 * @param {{fx: number, fy: number} | null} custom - saved position, or null for default
 * @param {number} width - pill width
 * @param {number} height - pill height
 * @returns {{x: number, y: number}} top-left corner, kept fully on the monitor
 */
export function placePill(monitor, defaultTop, custom, width, height) {
    const centerX = custom ? monitor.x + custom.fx * monitor.width : monitor.x + monitor.width / 2;
    const top = custom ? monitor.y + custom.fy * monitor.height : defaultTop;
    return {
        x: Math.round(clamp(centerX - width / 2, monitor.x, monitor.x + monitor.width - width)),
        y: Math.round(clamp(top, monitor.y, monitor.y + monitor.height - height)),
    };
}

/**
 * Converts a dropped pill into a saved position, or null when it was
 * dropped close enough to the default spot to snap back there.
 *
 * @returns {{fx: number, fy: number} | null}
 */
export function positionFromDrop(monitor, defaultTop, pillX, pillY, pillWidth, scale = 1) {
    const centerX = pillX + pillWidth / 2;
    const defaultCenterX = monitor.x + monitor.width / 2;
    if (Math.hypot(centerX - defaultCenterX, pillY - defaultTop) <= SNAP_DISTANCE * scale)
        return null;
    return {
        fx: clamp((centerX - monitor.x) / monitor.width, 0, 1),
        fy: clamp((pillY - monitor.y) / monitor.height, 0, 1),
    };
}
