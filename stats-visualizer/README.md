# stats-visualizer

Steel extension: floating scoreboards (holograms) that show the top 5 players by wins per game type,
with their W/L ratio. Results and game types come from the `game-orchestrator` extension. Stats and
board locations are stored in the extension data directory.

**Status:** partly implemented. Done (tasks T001-T005): ranking, stats accumulation, TOML storage with
atomic saves, configuration, autosave and save on shutdown. Not done: boards (need host spec
`specs/006-text-holograms/`, not implemented yet), orchestrator results and game types (T007, T010, T011;
open decision on how `Draw` counts), the `/statsboard` command (T009). The spec is in
[`specs/001-stats-visualizer/`](specs/001-stats-visualizer/spec.md).

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

`/statsboard place|remove|list` is specified in
[`contracts/commands.md`](specs/001-stats-visualizer/contracts/commands.md) and not implemented yet. It will
need the host hologram bindings and the `game-orchestrator` extension (`depends: ["game-orchestrator"]`
once T007 lands).

## Layout

This extension lives in a Steel checkout next to the other extensions, so its crate reaches the API
through `../../steel-extension-api` and the orchestrator contract through `../game-orchestrator-api`. See
`specs/extensions-workflow.md` in the Steel checkout for the ownership rules.
