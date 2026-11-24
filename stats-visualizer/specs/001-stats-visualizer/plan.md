# Implementation Plan: Stats Visualizer extension

**Branch**: `001-stats-visualizer` | **Date**: 2026-11-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-stats-visualizer/spec.md`

## Summary

Build the `stats-visualizer` extension. It receives match results from the game orchestrator, keeps
wins and losses per player and game type, renders the top 5 as floating text boards placed by an
administrator, and saves everything to its data directory at boot, on a timer and on stop. It provides no
service; it consumes the orchestrator's contract and the host's hologram bindings (spec 006).

## Technical Context

**Language/Version**: Rust (the Steel checkout's `rust-toolchain.toml`, nightly; found by Cargo because
this extension sits inside the checkout)

**Primary Dependencies**: `steel-extension-api` (path `../../steel-extension-api`, without the `host`
feature), `game-orchestrator-api` (path `../game-orchestrator-api`, **does not exist
yet**), `serde`, `toml`. No other crates.

**Storage**: `stats.toml` and `boards.toml` in the extension data directory, `config.toml` optional
([data-model.md](data-model.md)).

**Testing**: unit tests beside the code (FR-011) and the manual check in [quickstart.md](quickstart.md).
The native test location is open (O1).

**Target Platform**: Linux server, vanilla Java clients.

**Project Type**: native extension (cdylib), no contract crate of its own.

**Performance Goals**: none beyond the host's; work happens only per result and per autosave.

**Constraints**:
- No async, no threads. Autosave runs on the tick as in `essentials` (O2).
- No panics in production paths; every failure is a message to the sender or the log.
- State behind `SyncMutex` (`steel_utils::locks`) as bundled structs, one per file's data.

**Scale/Scope**: thousands of player records, tens of boards.

## Constitution Check

| Principle | Status | How |
| --- | --- | --- |
| I. Stable Native Boundary | Pass, one dependency open | Consumes stabby services only; the orchestrator contract it needs is unspecified (O3). |
| II. Modding-Grade Extensibility | Pass | Uses only the orchestrator's public service and the host's public bindings. |
| III. Vanilla Parity First | Pass | No gameplay logic; hologram vanilla parity is spec 006. |
| IV. Foundation Rigor | Pass | Nothing is stubbed: hologram and orchestrator tasks are blocked until their foundations exist. |
| V. Explicit Failure | Pass | Malformed results are rejected whole; corrupt files are moved aside; commands return messages. |
| VI. Concurrency & Dependencies | Pass | No threads, no async, no new crates beyond `serde` and `toml`. |
| VII. Verified Behavior | Pass, open point | Unit tests plus manual check; native test is O1. |

No violations.

## Project Structure

### Documentation (this feature)

```text
specs/001-stats-visualizer/
├── plan.md
├── spec.md
├── research.md
├── data-model.md
├── quickstart.md
├── tasks.md
├── contracts/
│   ├── commands.md
│   └── required-orchestrator.md
└── checklists/
    └── requirements.md
```

### Source Code

```text
stats-visualizer/
├── Cargo.toml
├── extension.toml
├── clippy.toml
└── src/
    ├── lib.rs          # NEW export_extension!, pre_init, init, shutdown
    ├── config.rs       # NEW config parsing and validation
    ├── stats.rs        # NEW Stats, PlayerRecord, applying a result
    ├── ranking.rs      # NEW ranking and ratio, pure functions
    ├── storage.rs      # NEW load, atomic save, .broken handling
    ├── boards.rs       # NEW Board, holograms, rendering and refresh
    ├── results.rs      # NEW orchestrator listener and result validation
    └── commands.rs     # NEW /statsboard
```

**Structure decision.** One module per responsibility. `ranking.rs` and `stats.rs` are pure and carry most
tests. Host calls (hologram, orchestrator, file system) sit behind small traits in the module that uses
them, so tests need no server.

## Layout requirement

This extension sits in `extension-example/` of a Steel checkout, as `SteelExtensionMC/extension-example/stats-visualizer/`.
Its crate reaches the API through `../../steel-extension-api`. The orchestrator contract is reached by
`../game-orchestrator-api`.

## Platform and cross-extension dependencies

| This task | Waits for | Status |
| --- | --- | --- |
| T006, T008, T009 (holograms) | Steel `specs/006-text-holograms/` bindings | Host spec 006 merged (`e22f2e44`); done. |
| T007, T010, T011 (orchestrator) | `game-orchestrator-api` | Done. `Draw` is ignored (user decision). |
| T001–T005 | Nothing | Done. |

### Platform gap: `<gametype>` completion

`/statsboard place <id> <gametype>` should complete game types from the orchestrator
(`complete_game_types`, `src/game_types.rs`), but `Games::game_types` needs a `Server<'_>` and a command
suggester receives none: `SuggestionContext` and `SuggestionRequest` expose only the partial input,
the player UUID and player names (`steel-extension-api/src/command/builder.rs`, `command/mod.rs`). Host fix
needed: a `Server` accessor on the suggestion context. Until then `<gametype>` is not completed; `place`
still validates the type. Completion of `remove <id>` works.

### Behavior notes

- A result is applied one tick after it arrives: the action handler has no `Server`, so validation runs in
  `scheduler::run_later(1, ..)`.
- Boards are restored one tick after `init` and one tick after a world loads, for the same reason.

## Complexity Tracking

Empty.
