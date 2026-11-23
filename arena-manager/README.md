# arena-manager

Steel extension: arenas per game type, a match state machine, a world limit and cleanup.
Contract crate: [`arena-manager-api`](../arena-manager-api/src/lib.rs). Spec:
[`specs/001-arena-manager/`](specs/001-arena-manager/spec.md).

## Status

Implemented and unit tested: `config.toml`, `arenas.toml` storage and validation, the `/arena`
commands, the arena and world allocator (`allocator.rs`) and the match state machine
(`matches.rs`).

Not implemented yet, each waiting on something outside this crate (see `specs/001-arena-manager/tasks.md`):

- Starting matches, the death and quit handling in the live server, and moving players
  (T008-T011): the world-cloner service, the host's death-screen behavior (research R6) and a tick-rate source.
- World cleanup (T012): world-cloner remove service.
- The `arena-manager:match/v1` service and the `MatchFinished` / `MatchFailed` actions (T009, T013,
  T014): `PlayerEntry.kit` needs host spec 007. The contract is not final until then.

The `world-cloner-api` dependency is not in `Cargo.toml` yet because that crate does not exist yet. The extension
declares `depends: ["world-cloner"]`, so the host will not load it without that extension.

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
| `/arena lobby set <survival\|creative\|adventure\|spectator>` (player only) | `arena.lobby` |

Spawns are stored as coordinates only, without a world key: stand in a copy of the map when you set
them. An arena is complete with contiguous slots 1..N, N at least 2. `/arena remove` does not yet
refuse an arena that has a running match, and there is no `/arena matches` yet (no match runtime).
`/arena create` creates the game type when it is new, and game types and templates are not validated against anything yet.

## Caller rules (for the future service)

- Calls run on the server tick thread and never block.
- Emit `MatchFinished` / `MatchFailed` only after releasing every lock.
- World-cloner `Operation`s are polled on later ticks, never waited on.
