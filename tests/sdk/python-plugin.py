"""End to end: the Python helper against a real daemon (run by
tests/sdk/python-plugin.sh on a private D-Bus session)."""
import json
import subprocess
import sys
import time

sys.path.insert(0, "sdk/python")
from gi.repository import GLib  # noqa: E402
from selawi_island import Island  # noqa: E402

BIN = sys.argv[1]


def gdbus(iface, method, *args):
    out = subprocess.run(["gdbus", "call", "--session", "--dest", "com.selawi.Island",
                          "--object-path", "/com/selawi/Island", "--method", f"{iface}.{method}", *args],
                         capture_output=True, text=True, check=True).stdout
    return out


def events():
    return json.loads(subprocess.run([f"{BIN}/selawi-island", "list"], capture_output=True, text=True,
                                     check=True).stdout)


def pump(seconds):
    context = GLib.MainContext.default()
    end = time.time() + seconds
    while time.time() < end:
        context.iteration(False)
        time.sleep(0.01)


def ok(cond, msg):
    if not cond:
        print(f"FAIL: {msg}")
        sys.exit(1)
    print(f"ok: {msg}")


island = Island({"id": "pomodoro", "name": "Pomodoro", "description": "Focus timer",
                 "categories": ["custom"]})
pressed = []
island.on_action(lambda event_id, action_id: pressed.append((event_id, action_id)))
pump(0.5)
ok(island.allowed is False, "not allowed before the user says so")
ok(any(e["id"] == "plugins:request:pomodoro" for e in events()), "the island asks the user")
ok(island.show("timer", category="custom", persistent=True,
               payload={"type": "progress", "title": "Focus", "percent": 10}) is False,
   "show does nothing until allowed")

gdbus("com.selawi.Island.Events1", "InvokeAction", "plugins:request:pomodoro", "allow")
pump(0.5)
ok(island.allowed is True, "pressing Allow on the island reaches the app")
ok(any(e["id"] == "plugin:pomodoro:timer" and e["source"]["app"] == "Pomodoro" for e in events()),
   "the event shown before approval appears once allowed")
island.show("timer", category="custom", persistent=True,
            payload={"type": "progress", "title": "Focus", "percent": 50},
            actions=[{"id": "pause", "label": "Pause"}])
ok(any(e["id"] == "plugin:pomodoro:timer" and e["payload"].get("percent") == 50 for e in events()),
   "updates show")
gdbus("com.selawi.Island.Events1", "InvokeAction", "plugin:pomodoro:timer", "pause")
pump(0.5)
ok(pressed == [("timer", "pause")], "the button press reaches the app with its own id")
island.end("timer")
ok(any(e["id"] == "plugin:pomodoro:timer" and e["state"] == "ended" for e in events()), "end shows Done")
