# Command-line client: `selawi-island`

Sends events to the daemon. Works from a terminal or any script.

```bash
selawi-island --help
```

## Notifications

```bash
selawi-island notify "Build passed"
```

```bash
selawi-island notify "Backup done" --body "120 GB in 14 min" --app Backup --ttl 15
```

| Option | Meaning |
|---|---|
| `--body TEXT` | Second line |
| `--app NAME` | App name shown (default `Terminal`) |
| `--ttl SECONDS` | How long it stays (default 8) |
| `--id ID` | Reuse an id to replace an earlier notification |
| `--priority TIER` | `critical`, `call`, `device`, `notification`, `media`, `transfer`, `reminder`, `metric` |

## Progress

`progress start` prints the task id; pass it to `update` and `finish`.

```bash
ID=$(selawi-island progress start --title "Running tests" --percent 0)
```

```bash
selawi-island progress update --id "$ID" --percent 65 --detail "65 of 100"
```

```bash
selawi-island progress finish --id "$ID" --status success
```

- `start`: `--title` (required), `--detail`, `--percent` (0–100, leave out
  if unknown), `--app`, `--id`, `--ttl` (seconds without an update before it
  disappears; default 3600, so a crashed script can't leave it forever).
- `update`: change any of `--title`, `--detail`, `--percent`; the rest is kept.
- `finish`: `--status success|failure` (default success), `--detail`. It
  stays visible a few seconds, then goes.

Example: wrap any command.

```bash
ID=$(selawi-island progress start --title "cargo build")
cargo build && selawi-island progress finish --id "$ID" || selawi-island progress finish --id "$ID" --status failure
```

## Other commands

| Command | Meaning |
|---|---|
| `selawi-island status` | Is the daemon running? Version and event count |
| `selawi-island list` | Current events as JSON |
| `selawi-island dismiss ID` | Remove an event |
| `selawi-island send JSON` | Send a raw protocol v1 event input (`-` reads stdin) |

Exit code is 0 on success and 1 on any error, with the reason on stderr.
