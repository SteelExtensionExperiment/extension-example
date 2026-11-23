# Quickstart: Stats Visualizer extension

Run from the **Steel checkout** (`SteelExtensionMC/`), where this extension sits at `extension-example/stats-visualizer/`.

## Prerequisites

- Host spec 006 hologram bindings are merged into the checkout you build from
  ([plan.md](plan.md#platform-and-cross-extension-dependencies)).
- The `game-orchestrator` extension is built and loaded, and can report game types and match results.
  Until it exists, only the automated checks below apply.

## Automated checks

```sh
cargo test --manifest-path extension-example/stats-visualizer/Cargo.toml
cargo clippy --manifest-path extension-example/stats-visualizer/Cargo.toml
cargo fmt --manifest-path extension-example/stats-visualizer/Cargo.toml --check
typos extension-example/stats-visualizer
```

A native test for this extension is decision O1 (research R3).

## Install by hand

```sh
python3 package-extension.py extension-example/stats-visualizer
```

Optional `extensions/stats-visualizer/config.toml`:

```toml
autosave_interval_ticks = 1200
```

Restart Steel after building or changing configuration. Native extensions stay loaded until exit.

## Manual check in a real client

1. Start Steel, `/op` yourself from the console. Play or simulate matches in two game types so several
   players have wins and losses.
2. Stand in the lobby and run `/statsboard place lobby_a <gametype>`. A floating board appears at your
   position with the title and up to five ranked lines.
3. Compare the lines with the stored counts: order is wins, then W/L, then name; a player with 0 losses
   shows wins as the ratio.
4. Finish another match of that game type. The board changes without any command. A board of the other
   game type does not change.
5. `/statsboard list` shows the board with world and position. `/statsboard place lobby_a <gametype>`
   again fails with "already exists"; an unknown game type fails with `unknown game type <name>`.
6. Wait one autosave interval, then check `extensions/stats-visualizer/stats.toml` and `boards.toml`
   contain the latest counts and the board.
7. Stop the server cleanly and start it again. The board is back at the same position with the same lines.
8. `/statsboard remove lobby_a` removes the hologram. Restart: it does not return.

## Failure checks

- Replace `stats.toml` with garbage: the server starts, `stats.toml.broken` exists, the log names it, and
  the extension starts empty (the garbage file is not overwritten before it is moved).
- A player without `statsboard.place` runs `/statsboard place ...`: denied, nothing changes.
- A board whose world is not loaded at boot stays in `boards.toml` and returns when the world loads.
- Kill the server process between autosaves: changes since the last autosave are lost (documented, D4).
