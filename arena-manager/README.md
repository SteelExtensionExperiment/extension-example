# arena-manager

Steel extension: arenas per game type, a match state machine, a world limit and cleanup.
Contract crate: [`arena-manager-api`](../arena-manager-api/src/lib.rs). Spec:
[`specs/001-arena-manager/`](specs/001-arena-manager/spec.md).

## Status

Implemented: `config.toml` and `arenas.toml`, the `/arena` commands (including `matches`), the arena and
world allocator, the match state machine, world creation and cleanup through `world-cloner`, player
transfers and lobby returns, the `arena-manager:match/v1` service and the `MatchFinished` /
`MatchFailed` actions. All of it is unit tested against a fake host (73 tests); **nothing has run on a
live server yet**. The open points are listed in `specs/001-arena-manager/plan.md`.

## Layout

Lives in `extension-example/` of a Steel checkout, so it reaches the API through
`../../steel-extension-api`. Other extensions' contracts are referenced by sibling path. Ownership
rules: `specs/extensions-workflow.md` in the Steel checkout.

## Configuration

`extensions/arena-manager/config.toml`; every key is optional and an invalid value stops the extension from starting.

| Key | Default | Meaning |
| --- | --- | --- |
| `max_worlds` | 4 | Most arena worlds existing or being created at once. At least 1. |
| `winner_return_delay_seconds` | 10 | Countdown before the last player standing returns to the lobby. |
| `player_ready_timeout_seconds` | 15 | How long to wait for a player after a domain switch. |

Arenas and the lobby are stored in `arenas.toml` in the extension data directory. It is written by
the commands (write to a temporary file, then rename). A corrupt file stops the extension from
starting, with the file and cause in the message.

## Commands

Each subcommand has its own permission node; without it the command is denied.

| Command | Permission |
| --- | --- |
| `/arena create <gametype> <arena>` | `arena.create` |
| `/arena remove <gametype> <arena>` | `arena.remove` |
| `/arena spawn set <gametype> <arena> <slot>` (player only) | `arena.spawn` |
| `/arena spawn remove <gametype> <arena> <slot>` | `arena.spawn` |
| `/arena spawn list <gametype> <arena>` | `arena.list` |
| `/arena template <gametype> <template>` | `arena.template` |
| `/arena list` | `arena.list` |
| `/arena matches` | `arena.matches` |
| `/arena lobby set <survival\|creative\|adventure\|spectator>` (player only) | `arena.lobby` |

Spawns are stored as coordinates only, without a world key: stand in a copy of the map when you set
them. An arena is complete with contiguous slots 1..N, N at least 2. `/arena remove` refuses an arena with a
running match and `/arena matches` lists matches (`arena.matches`). `/arena template` checks the name
against the `world-cloner` templates. `/arena create` creates the game type when it is new.

## Caller rules (service `arena-manager:match/v1`)

- Calls run on the server tick thread and never block.
- Emit `MatchFinished` / `MatchFailed` only after releasing every lock.
- Arena-manager polls its `world-cloner` `Operation`s on later ticks; callers never see them.
- A capacity rejection is retried by the caller; arena-manager has no queue.
- Requires a lobby (`/arena lobby set`), a template per game type and a Ready template.
