# Event system — protocol v1

Everything the island shows is an **event**. Sources send an **event
input**; the engine validates it, stamps times, resolves priority and expiry,
and stores an **event**. JSON is used only at boundaries; inside Rust the
types are fully typed (`core/src/events.rs`).

## Event input (what a source sends)

| Field | Type | Required | Notes |
|---|---|---|---|
| `id` | string | yes | 1–128 chars of `A-Z a-z 0-9 . _ : -`. Same id = same event. |
| `source.app` | string | yes | Display name, ≤ 64 chars |
| `source.integration` | string | yes | e.g. `mpris`, `simulator` |
| `category` | enum | yes | `call` `media` `transfer` `device` `notification` `system` `reminder` `metric` `custom` |
| `state` | enum | yes | `started` `updated` `ended` `failed` `expired` |
| `priority` | enum | no | Overrides the category default |
| `persistent` | bool | no (false) | Stays until it ends or is dismissed |
| `ttl_ms` | int | no | 1 ms – 24 h. Lifetime of non-persistent events; refreshed on update |
| `group` | string | no | Same rules as `id`; groups related events |
| `payload` | object | yes | Typed, see below |
| `actions` | array | no | ≤ 4 of `{id, label}`; ids unique, labels ≤ 32 chars |

Unknown fields are rejected everywhere. Text must be non-empty, within its
length limit (titles 256, bodies 1024) and contain no control characters
except newline.

## Payloads (`payload.type`)

| Type | Fields | Allowed category |
|---|---|---|
| `call` | `title`, `confidence` (`confirmed` `probable` `audio_only` `unknown`), `muted?` (only when the source knows) | `call` |
| `media` | `title`, `artist?`, `album?`, `playing`, `position_ms?`, `duration_ms?` | `media` |
| `progress` | `title`, `detail?`, `percent?` (0–100) | any |
| `device` | `name`, `device_kind?`, `connected`, `battery_percent?` (0–100) | `device` |
| `notification` | `title`, `body?` | any |
| `system` | `title`, `detail?`, `severity` (`info` `warning` `critical`) | any |

`position_ms` is the position at the time of the update; the UI advances
it while `playing`. Optional fields are omitted when unknown — never guessed.

## Stored event (what the UI receives)

The input fields plus `priority` (resolved), `created_at`, `updated_at`,
`expires_at?` (ms since epoch).

## Lifecycle

| Stored \ Received | started | updated | ended / failed | expired |
|---|---|---|---|---|
| none | create | create (resync after restart) | create finished | rejected |
| started / updated | update (duplicate start) | update | finish | remove |
| ended / failed | restart as new | rejected (stale) | rejected | rejected |

- Identical repeats change nothing (deduplication) and do not bump `updated_at`.
- Finished events stay visible for 4 s (`finished_linger_ms`), then go.
- Non-persistent events expire `ttl_ms` (default 8 s) after their last update.
- A dismissed active event ignores further updates until the source sends
  `started` again.

## Priority

Most important first: `critical`, `call`, `device`, `notification`,
`media`, `transfer`, `reminder`, `metric`.

Category defaults: `system` and `custom` use `notification`; every other
category uses the tier with the same name.

Resolution order: user override for the category → source's `priority` →
category default. Display order: priority, then newest `created_at` first
(updates never reorder, which prevents flicker). Lower-priority events never
take the island from a higher one, and nothing is dropped because another
event arrived; they stay in the expanded list.

## Limits

At most 64 stored events. When full, the least important, oldest
non-persistent event is evicted first.

## Recovery

`EventStore::snapshot()` returns active persistent events;
`EventStore::restore()` re-validates and reloads them. The daemon will use
this across restarts (Phase 3).
