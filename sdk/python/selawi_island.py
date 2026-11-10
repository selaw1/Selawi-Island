"""Show your app's own events on Selawi Island (Plugins1 over D-Bus).

Needs PyGObject (python-gobject on Arch; installed with GNOME) and a GLib
main loop for button presses. Example::

    from gi.repository import GLib
    from selawi_island import Island

    island = Island({
        "manifest_version": 1,
        "id": "pomodoro",
        "name": "Pomodoro",
        "description": "Focus timer",
        "categories": ["custom"],
    })
    island.on_action(lambda event_id, action_id: print("pressed", action_id))
    island.show("timer", category="custom", persistent=True,
                payload={"type": "progress", "title": "Focus", "percent": 40},
                actions=[{"id": "pause", "label": "Pause"}])
    GLib.MainLoop().run()

The first time, the island asks the user to allow the app; until then
`show` does nothing (`allowed` is False). Events re-appear by themselves if
the island restarts. Events disappear when your program exits.
"""

import json

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

BUS_NAME = "com.selawi.Island"
PATH = "/com/selawi/Island"
INTERFACE = "com.selawi.Island.Plugins1"


class Island:
    def __init__(self, manifest):
        self.manifest = dict(manifest)
        self.manifest.setdefault("manifest_version", 1)
        self.allowed = False
        self._bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self._shown = {}
        self._callbacks = []
        self._bus.signal_subscribe(
            BUS_NAME, INTERFACE, "ActionInvoked", PATH, None,
            Gio.DBusSignalFlags.NONE, self._on_action)
        self._bus.signal_subscribe(
            BUS_NAME, INTERFACE, "Changed", PATH, None,
            Gio.DBusSignalFlags.NONE, self._on_changed)
        # (Re)register whenever the island appears, and show again what was
        # on screen.
        self._watch = Gio.bus_watch_name_on_connection(
            self._bus, BUS_NAME, Gio.BusNameWatcherFlags.NONE, self._on_appeared, None)

    # ---- public -------------------------------------------------------

    def register(self):
        """Registers now (also done automatically). Returns `allowed`."""
        try:
            reply = self._call("Register", GLib.Variant("(s)", (json.dumps(self.manifest),)))
        except GLib.Error:
            self.allowed = False
            return False
        self.allowed = bool(reply.unpack()[1])
        return self.allowed

    def show(self, event_id, payload, category, state=None, **fields):
        """Adds or updates an event. `fields`: persistent, ttl_ms, priority,
        group, actions. Returns True if the island took it."""
        state = state or ("updated" if event_id in self._shown else "started")
        event = {"id": event_id, "category": category, "state": state, "payload": payload}
        event.update(fields)
        if state in ("ended", "failed"):
            self._shown.pop(event_id, None)
        else:
            self._shown[event_id] = event
        return self._submit(event)

    def end(self, event_id, failed=False):
        """Finishes an event: it shows as done (or failed) for a moment."""
        event = self._shown.get(event_id)
        if not event:
            return False
        return self.show(event_id, event["payload"], event["category"],
                         state="failed" if failed else "ended",
                         **{k: v for k, v in event.items()
                            if k not in ("id", "category", "state", "payload")})

    def remove(self, event_id):
        """Removes an event at once."""
        self._shown.pop(event_id, None)
        try:
            return bool(self._call("Remove", GLib.Variant("(s)", (event_id,))).unpack()[0])
        except GLib.Error:
            return False

    def on_action(self, callback):
        """`callback(event_id, action_id)` when a button is pressed."""
        self._callbacks.append(callback)

    # ---- internals ----------------------------------------------------

    def _call(self, method, params):
        return self._bus.call_sync(BUS_NAME, PATH, INTERFACE, method, params, None,
                                   Gio.DBusCallFlags.NO_AUTO_START, 3000, None)

    def _submit(self, event):
        if not self.allowed:
            return False
        try:
            self._call("Submit", GLib.Variant("(s)", (json.dumps(event),)))
            return True
        except GLib.Error as error:
            if "NotAllowed" in str(error):
                self.allowed = False
            return False

    def _resend(self):
        for event in list(self._shown.values()):
            self._submit({**event, "state": "started"})

    def _on_appeared(self, *_args):
        if self.register():
            self._resend()

    def _on_changed(self, _bus, _sender, _path, _iface, _signal, params):
        # The user allowed or blocked the app (Settings or the question).
        try:
            plugins = json.loads(params.unpack()[0])
        except ValueError:
            return
        me = next((p for p in plugins if p.get("id") == self.manifest["id"]), None)
        was = self.allowed
        self.allowed = bool(me and me.get("allowed"))
        if self.allowed and not was:
            self._resend()

    def _on_action(self, _bus, _sender, _path, _iface, _signal, params):
        event_id, action_id = params.unpack()
        for callback in self._callbacks:
            callback(event_id, action_id)
