#!/usr/bin/env python3
"""Plays the browser against the real native messaging host and daemon.

Run inside a D-Bus session with the daemon running (see browser-host.sh).
"""
import json
import struct
import subprocess
import sys
import time

BIN = sys.argv[1]


def frame(obj):
    body = json.dumps(obj).encode()
    return struct.pack("=I", len(body)) + body


def daemon(*args):
    return subprocess.run([f"{BIN}/selawi-island", *args], capture_output=True, text=True).stdout


def fail(msg):
    print(f"FAIL: {msg}")
    sys.exit(1)


meet = {
    "id": "browser:meet:7",
    "source": {"app": "Google Meet", "integration": "browser"},
    "category": "call", "state": "started", "persistent": True,
    "payload": {"type": "call", "title": "Weekly sync", "confidence": "confirmed", "muted": False},
    "actions": [{"id": "mute", "label": "Mute"}, {"id": "leave", "label": "Leave"}],
}
spoof = dict(meet, id="mpris:spotify")

host = subprocess.Popen([f"{BIN}/selawi-island-browser-host"], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
host.stdin.write(frame({"type": "hello", "version": 1, "browser": "chrome"}))
host.stdin.write(frame({"type": "event", "event": meet}))
host.stdin.write(frame({"type": "event", "event": spoof}))
host.stdin.write(frame({"type": "run", "cmd": "rm -rf ~"}))
host.stdin.flush()
time.sleep(0.5)

events = json.loads(daemon("list"))
ids = [e["id"] for e in events]
if "browser:meet:7" not in ids:
    fail(f"Meet call not shown: {ids}")
if "mpris:spotify" in ids:
    fail("an event posing as another integration got through")
print("ok: Meet call reached the island; spoofed and unknown messages refused")

# Press Mute in the island: it must come back to the "browser".
subprocess.run(["gdbus", "call", "--session", "--dest", "com.selawi.Island", "--object-path",
                "/com/selawi/Island", "--method", "com.selawi.Island.Events1.InvokeAction",
                "browser:meet:7", "mute"], capture_output=True, check=True)
length = struct.unpack("=I", host.stdout.read(4))[0]
reply = json.loads(host.stdout.read(length))
if reply != {"type": "action", "id": "browser:meet:7", "action": "mute"}:
    fail(f"unexpected reply {reply}")
print("ok: Mute pressed in the island reached the browser")

# The browser closes: the call must end.
host.stdin.close()
host.wait(timeout=5)
time.sleep(0.3)
state = [e["state"] for e in json.loads(daemon("list")) if e["id"] == "browser:meet:7"]
if state != ["ended"]:
    fail(f"call not ended after the browser closed: {state}")
print("ok: browser closed, call ended")
