#!/usr/bin/env bash
# End-to-end: daemon + CLI (+ the extension's DaemonFeed under GJS when
# available) on a private D-Bus session bus. Run via scripts/test.sh, or:
#   dbus-run-session -- tests/integration/dbus-smoke.sh
set -euo pipefail
cd "$(dirname "$0")/../.."

BIN=./target/debug
CLI="$BIN/selawi-island"
WORK="$(mktemp -d)"
trap 'kill "$DAEMON_PID" 2>/dev/null || true; rm -rf "$WORK"' EXIT
fail() { echo "FAIL: $*"; exit 1; }

if "$CLI" status >/dev/null 2>&1; then fail "status should fail without a daemon"; fi

"$BIN/selawi-islandd" --state-file "$WORK/state/events.json" 2>"$WORK/daemon.log" &
DAEMON_PID=$!
for _ in $(seq 50); do "$CLI" status >/dev/null 2>&1 && break; sleep 0.1; done
"$CLI" status | grep -q "protocol 1" || fail "daemon did not start"
sleep 0.5
INTEGRATIONS=$("$CLI" integrations)
for id in mpris upower bluez notifications network storage health; do
    echo "$INTEGRATIONS" | grep -q "^$id " || fail "integration $id not listed"
done
echo "$INTEGRATIONS" | grep -q "^mpris .*active" || fail "mpris should be active on the session bus"
echo "$INTEGRATIONS" | grep -q "^health .*disabled" || fail "health must be off by default"

"$CLI" notify "Hello" --id n1 --ttl 1
ID=$("$CLI" progress start --title "Tests" --percent 10 --ttl 120)
"$CLI" progress update --id "$ID" --percent 65
"$CLI" list | grep -q '"percent":65.0' || fail "progress update not applied"
"$CLI" progress finish --id "$ID"
"$CLI" list | grep -q '"state":"ended"' || fail "finish not applied"
if "$CLI" progress update --id "$ID" --percent 70 2>/dev/null; then fail "update after finish accepted"; fi
if "$CLI" send '{"id":"x","exec":"rm"}' 2>/dev/null; then fail "invalid event accepted"; fi
sleep 1.2
"$CLI" list | grep -q '"id":"n1"' && fail "notification did not expire"

# selawi-island run: the command's own output and exit code, Running while
# it runs, then Done / Failed (arguments never shown).
"$CLI" run -- sh -c 'echo built; exit 0' | grep -q built || fail "run did not pass output through"
"$CLI" list | grep -q '"title":"sh","detail":"Done · 0:00"' || fail "run did not finish as Done"
set +e; "$CLI" run --title "Tests" -- sh -c 'exit 3'; code=$?; set -e
[ "$code" = 3 ] || fail "run did not return the command's exit code ($code)"
"$CLI" list | grep -q '"title":"Tests","detail":"Failed (exit 3) · 0:00"' || fail "failed run not shown"
"$CLI" run -- sh -c "$CLI list > $WORK/during.json"
grep -q '"detail":"Running"' "$WORK/during.json" || fail "run not shown while running"
set +e; "$CLI" run -- /no/such/program 2>/dev/null; code=$?; set -e
[ "$code" = 127 ] || fail "missing program should exit 127 ($code)"

# pacman hooks, delivered to this test session (the real hook finds the
# logged-in users and runs as each of them).
printf 'firefox\nmesa\n' | "$CLI" pacman-hook --this-session pre install
"$CLI" list | grep -q '"id":"pacman:transaction".*"title":"Updating 2 packages"' || fail "pacman start not shown"
"$CLI" pacman-hook --this-session post install
"$CLI" list | grep -q '"title":"Updated 2 packages"' || fail "pacman finish not shown"
printf 'bad name;rm\n' | "$CLI" pacman-hook --this-session pre remove
"$CLI" list | grep -q 'Removing' && fail "an invalid package name was shown"
gdbus call --session --dest com.selawi.Island --object-path /com/selawi/Island \
    --method com.selawi.Island.Configuration1.Set '{"integrations":{"pacman":false}}' >/dev/null
printf 'spotify\n' | "$CLI" pacman-hook --this-session pre remove
"$CLI" list | grep -q 'Removing spotify' && fail "pacman events shown while turned off"
gdbus call --session --dest com.selawi.Island --object-path /com/selawi/Island \
    --method com.selawi.Island.Configuration1.Set '{}' >/dev/null

# Claude Code hooks: what each hook shows. Prompts, commands and file
# contents are never shown, and a hook never fails.
hook() { printf '%s' "$1" | "$CLI" agent-hook claude || fail "agent-hook exited with an error"; }
S='"session_id":"abc-123","cwd":"/home/user/code/my-app"'
hook "{$S,\"hook_event_name\":\"UserPromptSubmit\",\"prompt\":\"my secret plan\"}"
"$CLI" list | grep -q '"id":"agents:claude:abc-123".*"title":"my-app","detail":"Working"' || fail "Claude Code turn not shown"
hook "{$S,\"hook_event_name\":\"PreToolUse\",\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"/home/user/code/my-app/src/main.rs\",\"new_string\":\"hidden text\"}}"
"$CLI" list | grep -q '"detail":"Editing main.rs"' || fail "Claude Code tool not shown"
hook "{$S,\"hook_event_name\":\"PreToolUse\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"echo topsecret\"}}"
hook "{$S,\"hook_event_name\":\"PermissionRequest\",\"tool_name\":\"Bash\"}"
"$CLI" list | grep -q '"category":"reminder".*"detail":"Needs your approval"' || fail "waiting for approval not shown"
hook "{$S,\"hook_event_name\":\"PostToolUse\",\"tool_name\":\"Bash\",\"tool_input\":{}}"
"$CLI" list | grep -q '"category":"custom".*"detail":"Running a command"' || fail "back to working after approval"
"$CLI" list | grep -q 'secret\|hidden text' && fail "a prompt, command or file contents were shown"
hook "{$S,\"hook_event_name\":\"Stop\"}"
"$CLI" list | grep -q '"id":"agents:claude:abc-123".*"state":"ended".*"detail":"Done · 0:00"' || fail "finished turn not shown as Done"
R='"session_id":"reject-1","cwd":"/tmp/rejected"'
hook "{$R,\"hook_event_name\":\"UserPromptSubmit\"}"
hook "{$R,\"hook_event_name\":\"PermissionRequest\",\"tool_name\":\"Bash\"}"
hook "{$R,\"hook_event_name\":\"Notification\",\"notification_type\":\"idle_prompt\"}"
"$CLI" list | grep -q 'agents:claude:reject-1' && fail "a rejected tool's bubble stayed after Claude Code went idle"
hook "{$R,\"hook_event_name\":\"PreToolUse\",\"tool_name\":\"Read\",\"tool_input\":{}}"
"$CLI" list | grep -q '"id":"agents:claude:reject-1"' || fail "after going idle, the next tool call should show again"
hook "{$S,\"hook_event_name\":\"Notification\",\"notification_type\":\"idle_prompt\"}"
"$CLI" list | grep -q '"id":"agents:claude:abc-123".*"state":"ended"' || fail "idle after a finished turn should keep Done"
printf 'not json' | "$CLI" agent-hook claude || fail "agent-hook failed on bad input"
mkdir -p "$WORK/data/applications"
printf '[Desktop Entry]\nType=Application\nName=Claude\nIcon=claude\nExec=claude-desktop\n' > "$WORK/data/applications/claude-desktop.desktop"
printf '{"session_id":"logo-1","hook_event_name":"UserPromptSubmit"}' |
    XDG_DATA_HOME="$WORK/data" XDG_DATA_DIRS="$WORK/none" "$CLI" agent-hook claude
printf '{"session_id":"logo-2","hook_event_name":"UserPromptSubmit"}' |
    XDG_DATA_HOME="$WORK/none" XDG_DATA_DIRS="$WORK/none" "$CLI" agent-hook claude
icon_of() { "$CLI" list | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const e=JSON.parse(s).find(e=>e.id===process.argv[1]);console.log(e?.source.icon ?? "none")})' "$1"; }
[ "$(icon_of agents:claude:logo-1)" = app:claude-desktop ] || fail "the installed Claude app's logo was not used"
[ "$(icon_of agents:claude:logo-2)" = none ] || fail "a logo was sent without the Claude app"
"$CLI" agent-hook claude </dev/null || fail "agent-hook failed on empty input"
T='"session_id":"other-1","cwd":"/tmp/other"'
hook "{$T,\"hook_event_name\":\"UserPromptSubmit\"}"
hook "{$T,\"hook_event_name\":\"SessionEnd\",\"reason\":\"prompt_input_exit\"}"
"$CLI" list | grep -q 'agents:claude:other-1' && fail "a closed session's bubble stayed"
gdbus call --session --dest com.selawi.Island --object-path /com/selawi/Island \
    --method com.selawi.Island.Configuration1.Set '{"integrations":{"agents":false}}' >/dev/null
hook '{"session_id":"off-1","hook_event_name":"UserPromptSubmit"}'
"$CLI" list | grep -q 'agents:claude:off-1' && fail "Claude Code shown while turned off"
gdbus call --session --dest com.selawi.Island --object-path /com/selawi/Island \
    --method com.selawi.Island.Configuration1.Set '{}' >/dev/null

# Claude Code settings: the island's hooks are added and removed without
# touching anything else, with a backup, and never in a file it can't read.
H="$WORK/home"
mkdir -p "$H/.claude"
SETTINGS="$H/.claude/settings.json"
printf '{"model":"opus","hooks":{"Stop":[{"hooks":[{"type":"command","command":"notify-send done"}]}]}}\n' > "$SETTINGS"
as_home() { env -u CLAUDE_CONFIG_DIR HOME="$H" XDG_CONFIG_HOME="$H/.config" "$CLI" "$@"; }
[ "$(as_home agent-hooks status --choice)" = unset ] || fail "agent hooks should start unset"
as_home agent-hooks on >/dev/null || fail "agent-hooks on failed"
grep -q 'agent-hook claude' "$SETTINGS" || fail "hooks not added"
grep -q 'notify-send done' "$SETTINGS" || fail "an existing hook was lost"
grep -q '"model": "opus"' "$SETTINGS" || fail "an existing setting was lost"
[ -f "$H/.claude/settings.json.bak" ] || fail "no backup made"
[ "$(as_home agent-hooks status --choice)" = on ] || fail "answer not remembered"
cp "$SETTINGS" "$WORK/once.json"
as_home agent-hooks on >/dev/null
cmp -s "$SETTINGS" "$WORK/once.json" || fail "turning on twice changed the file"
as_home agent-hooks off >/dev/null || fail "agent-hooks off failed"
grep -q 'agent-hook' "$SETTINGS" && fail "hooks not removed"
grep -q 'notify-send done' "$SETTINGS" || fail "removing lost another hook"
[ "$(as_home agent-hooks status --choice)" = off ] || fail "off not remembered"
printf '{ broken' > "$SETTINGS"
if as_home agent-hooks on 2>/dev/null; then fail "a broken settings file was changed"; fi
[ "$(cat "$SETTINGS")" = '{ broken' ] || fail "a broken settings file was rewritten"
rm -rf "$H/.claude"
as_home agent-hooks on >/dev/null || fail "agent-hooks on without a settings file"
grep -q 'agent-hook claude' "$SETTINGS" || fail "settings file not created"
as_home agent-hooks off --keep-choice >/dev/null
grep -q 'agent-hook' "$SETTINGS" && fail "package removal left the hooks"
[ "$(as_home agent-hooks status --choice)" = on ] || fail "package removal forgot the answer"

# Persistence: a persistent event survives a restart.
"$CLI" send '{"id":"call","source":{"app":"Slack","integration":"test"},"category":"call","state":"started","persistent":true,"payload":{"type":"call","title":"Standup","confidence":"confirmed"}}'
kill "$DAEMON_PID"; wait "$DAEMON_PID" 2>/dev/null || true
"$BIN/selawi-islandd" --state-file "$WORK/state/events.json" 2>>"$WORK/daemon.log" &
DAEMON_PID=$!
for _ in $(seq 50); do "$CLI" status >/dev/null 2>&1 && break; sleep 0.1; done
"$CLI" list | grep -q '"id":"call"' || fail "persistent event not restored"
"$CLI" dismiss call

if command -v gjs >/dev/null; then
    gjs -m tests/integration/daemon-feed.gjs.js "$CLI" || fail "DaemonFeed check failed"
else
    echo "(gjs not installed: skipped the DaemonFeed check)"
fi
echo "D-Bus smoke test passed."
