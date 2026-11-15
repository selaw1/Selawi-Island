# Claude Code in the island

The island can show what Claude Code is doing on your computer: a bubble
per session with the project folder, what it is doing right now, when it
needs you, and when it is done.

| Claude Code | The island |
|---|---|
| You send a prompt | Orange bubble: “my-app · Working”, with a running timer |
| It uses a tool | “Editing main.rs”, “Reading README.md”, “Running a command”, “Searching the code”, “Reading a web page”, “Using github” (an MCP server)… |
| It asks for your approval | What it asks for (“Run: npm test”, “Edit: src/api/route.ts”, “Fetch: docs.example.com”) with **Allow** and **Deny**, a question mark, and the island opens by itself |
| It asks you a question | The question itself, the same way, without buttons (answer it in Claude Code) |
| You reject what it asked for | The bubble goes about a minute later, when Claude Code reports it is waiting for your next message (it reports nothing at the moment you reject). Your next message shows it again |
| The turn finishes | Green: “Done · 4:12” (how long the turn took), then it goes |
| The turn stops on an error | Red: “Usage limit reached”, “Service unavailable”, “Sign-in needed”, “Billing problem” or “Stopped by an error” |
| You close the session | An unfinished bubble goes |

Several sessions show as separate bubbles, one per session.

**Logo:** if the Claude app is installed (an entry named “Claude” in your
app grid), the bubble shows its logo, and the question mark appears as a
small icon next to the timer. Without it, the bubble shows a gear while
working and a question mark while waiting. No logo file is part of this
project: the island uses the icon of the app you installed.

**What is shown:** while Claude Code works, only the project folder's
name, the kind of tool and a file's name. While it waits for your
approval, also what it asks for: the command's first line, the file (from
the project folder), the site's name, the search, or its question, cut to
one line of 80 characters. Turn that off in Settings → Integrations →
Claude Code → **Show what Claude asks for** (for example before sharing
your screen): it then says only “Needs your approval”. Your prompts and
file contents are never shown, and nothing is saved or logged.

## Allow and Deny

Press **Allow** or **Deny** in the island, or answer in Claude Code as
usual: whichever comes first counts.

- **Allow** lets the tool run; **Deny** refuses it and tells Claude it was
  denied from the island, so it can try something else.
- If you answer in Claude Code instead, the island's buttons go.
- The island waits up to 110 seconds; then its buttons go and Claude
  Code's own prompt decides.
- Questions and plans have no buttons: they need more than a yes, so you
  answer them in Claude Code.
- Any program running as you can press island buttons (as with every
  island button). Such a program could run commands itself anyway, so
  this gives it nothing new.

## Turning it on and off

The first `selawi-island-update` that includes this asks once:
“Show Claude Code progress in the island? [Y/n]”. Your answer is
remembered (`~/.config/selawi-island/agent-hooks`). Change it any time:

```bash
selawi-island agent-hooks on
selawi-island agent-hooks off
selawi-island agent-hooks status
```

There is also a switch in Settings → Integrations → **Claude Code**: off,
the island ignores Claude Code's messages without changing any files.

Restart open Claude Code sessions after turning it on or off.

## What it changes in Claude Code's settings

`on` adds the island's own hooks to `~/.claude/settings.json` (or
`$CLAUDE_CONFIG_DIR/settings.json`) for these Claude Code events:
`UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`,
`Notification`, `Stop`, `StopFailure` and `SessionEnd`. Each runs:

```text
'/usr/bin/selawi-island' agent-hook claude 2>/dev/null || true
```

- **Only its own entries.** Anything else in the file (other hooks,
  permissions, MCP servers, the order of keys) stays as it was.
- **A backup first:** `settings.json.bak` next to it, whenever the file
  changes.
- **Safe to repeat.** Every update refreshes the entries without adding
  duplicates.
- **Never a broken file.** If the file is not valid JSON, nothing is
  changed and it says why.
- **Never in Claude Code's way.** The hook always succeeds and gives up
  after 3 seconds; only for an approval does it wait for your Allow /
  Deny (at most 110 seconds, its entry allows 120) and print the answer.
  If the island is not running, turned off or not installed, it does
  nothing.
- `off` removes the entries again. Removing the package
  (`sudo pacman -R selawi-island`) removes them for logged-in users too.

## Where it works

It works wherever Claude Code runs **on your computer** and runs its
hooks:

- **VS Code extension:** works (checked: working, approval with the logo
  and the question mark, back to working after approving, done).
- **Terminal** (`claude`), including VS Code's built-in terminal: runs
  the same hooks.
- **Claude desktop app:** uses the same settings file; not checked yet
  (docs/TESTING.md).
- **Cloud sessions** (Claude Code on the web, cloud sessions in the app)
  run on another machine and cannot reach your island.

Other agents can show their work with `selawi-island run -- COMMAND`
(running, then done or failed) or through the plugin interface
([PLUGINS.md](PLUGINS.md)).
