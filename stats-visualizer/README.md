# stats-visualizer

Steel extension: floating scoreboards (holograms) that show the top 5 players by wins per game type,
with their W/L ratio. Results and game types come from the `game-orchestrator` extension. Stats and
board locations are stored in the extension data directory.

**Status:** implemented (tasks T001-T009, T011, and T010 except completion). Match results update stats,
boards show the top 5 as holograms and refresh after each result, and `/statsboard place|remove|list` manages
boards. Known gap: `<gametype>` is validated but not tab-completed, because a command suggester gets no
`Server` (host change needed, see `specs/001-stats-visualizer/plan.md`). Not yet run against a live server.
A `Draw` outcome is ignored (neither win nor loss).

## Configuration

`config.toml` in the extension configuration (optional):

```toml
autosave_interval_ticks = 1200   # default 1200 (60 s); 0 is rejected
```

Unknown keys are rejected.

## Files written

In the extension data directory: `stats.toml` (player UUID, name, wins and losses per game type) and
`boards.toml` (board id, world, position, game type). Writes are atomic (temporary file, then rename).
An unreadable or invalid file is moved to `<name>.toml.broken`, an error is logged, and the extension
starts empty instead of overwriting it. Changed data is saved on the autosave interval and on clean
server stop; a crash loses changes since the last autosave.

## Commands and dependencies

| Command | Permission |
| --- | --- |
| `/statsboard place <id> <gametype> [<x> <y> <z> [<world>]]` | `statsboard.place` |
| `/statsboard remove <id>` | `statsboard.remove` |
| `/statsboard list` | `statsboard.list` |

Details in [`contracts/commands.md`](specs/001-stats-visualizer/contracts/commands.md). Depends on the
`game-orchestrator` extension and on the host text holograms (spec 006). Results are applied one tick after
they arrive, and boards come back one tick after init or a world load.

## Layout

This extension lives in a Steel checkout next to the other extensions, so its crate reaches the API
through `../../steel-extension-api` and the orchestrator contract through `../game-orchestrator-api`. See
`specs/extensions-workflow.md` in the Steel checkout for the ownership rules.
