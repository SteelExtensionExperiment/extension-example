# Implementation Plan: Arena Manager extension

**Branch**: `001-arena-manager` | **Date**: 2026-11-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-arena-manager/spec.md`

## Summary

Build the `arena-manager` extension and its `arena-manager-api` contract crate. The extension stores arenas and a lobby per game type, reserves an arena for each match request, obtains a world from `world-cloner:clone/v1` when none is free (within `max_worlds`), moves players in and applies the kit, tracks deaths and disconnects on the server tick, returns players to the lobby, reports results and removes idle worlds and their domains. The contract with the game registry and orchestrator is deferred (O2).

## Technical Context

**Language/Version**: Rust (the Steel checkout's `rust-toolchain.toml`; found by Cargo because this extension sits inside the checkout)

**Primary Dependencies**: `steel-extension-api` (path `../../steel-extension-api`, no `host` feature), `world-cloner-api` (path `../world-cloner-api`), `serde`, `toml`. Later `game-registry`'s API crate (O2). No new third-party crates.

**Storage**: `arenas.toml` (lobby and arenas, written on command) and `config.toml` in the extension data directory. Matches are memory only.

**Testing**: unit tests beside the code for the pure logic (arena allocation, the match state machine, persistence, command parsing, validation) behind a small host seam; the manual check in [quickstart.md](quickstart.md). Native test location is open (O1, research R10).

**Target Platform**: Linux server, vanilla Java clients.

**Project Type**: native extension (cdylib) plus a contract crate.

**Performance Goals**: none beyond the host's. Per tick it drains a death queue and advances a few matches.

**Constraints**:
- No async, no threads of its own. Death and quit handlers only enqueue; the tick resolves (research R8).
- Nothing waits for an `Operation`; all are polled on later ticks.
- No panics in production paths.

**Scale/Scope**: a handful of game types, tens of simultaneous matches at most.

## Constitution Check

| Principle | Status | How |
| --- | --- | --- |
| I. Stable Native Boundary | Pass, with an open item | The contract is stabby under a versioned key; its shape is deferred (O2), so nothing is published. |
| II. Modding-Grade Extensibility | Pass | Others use only the service; world-cloner keys and domain naming stay private. |
| III. Vanilla Parity First | Pass, open item | No gameplay logic here. Death and respawn handling relies on host behavior (R6). |
| IV. Foundation Rigor | Pass | No stubs. Tasks needing the registry or a host change wait; they are not faked. |
| V. Explicit Failure | Pass | Every rejection and failure carries a message; a corrupt data file refuses to start. |
| VI. Concurrency & Dependencies | Pass | No threads or async; one `SyncMutex` for the death queue; no new third-party crates. |
| VII. Verified Behavior | Pass, open point | Unit tests plus manual check; native test is O1. |

No violations.

## Project Structure

### Documentation (this feature)

```text
specs/001-arena-manager/
├── plan.md
├── spec.md
├── research.md
├── data-model.md
├── quickstart.md
├── tasks.md
├── contracts/
│   ├── service.md        # DRAFT, not published (O2)
│   └── commands.md
└── checklists/
    └── requirements.md
```

### Source Code

```text
arena-manager-api/
├── Cargo.toml
└── src/lib.rs               # NEW, draft until O2 is decided

arena-manager/
├── Cargo.toml
├── extension.toml
├── clippy.toml
└── src/
    ├── lib.rs               # NEW export_extension!, init, tick, event subscriptions
    ├── config.rs            # NEW config.toml parsing and validation
    ├── store.rs             # NEW arenas, lobby, arenas.toml load and save
    ├── commands.rs          # NEW /arena
    ├── allocator.rs         # NEW arena reservation, world cap, cleanup decisions (pure)
    ├── matches.rs           # NEW match state machine, deaths, draws, countdown (pure)
    ├── worlds.rs            # NEW world-cloner calls and Operation polling
    ├── transfer.rs          # NEW per-player move steps (R4)
    └── service.rs           # NEW match service (deferred, O2)
```

**Structure decision.** The decisions (allocation, match rules) are pure modules with no host types, so they are unit-testable. Host calls sit in `worlds.rs` and `transfer.rs` behind a small seam.

## Layout requirement

This extension sits in `extension-example/` of a Steel checkout, as `SteelExtensionMC/extension-example/arena-manager/` (crate and this spec) with its contract crate beside it as `extension-example/arena-manager-api/`. Crates reach the API through `../../steel-extension-api`. It depends on `world-cloner-api` by `../world-cloner-api`, and on the registry's API crate once it exists (`../<registry-api>`, O2).

## Platform dependencies

| This task | Waits for |
| --- | --- |
| T010, T011 | world-cloner T006 (clone service), which waits for host spec 005 T012 |
| T012 | world-cloner T008 (remove service), which waits for host spec 005 T018; and research R7 |
| T009, T013, T014 | O2 (registry and orchestrator contract) |
| T008 death handling for the respawn screen | R6: may need a host spec (next free number is 007) |

T001-T007 need none of these.

## Complexity Tracking

Empty.

## Decisions and status (2026-11-24, relayed by the coordinator session)

Implemented and unit tested: T001-T007, T015.

Decisions:

- **Dead players (R6):** the host refuses a world change while the player is dead (`steel-core/src/server/world_changes.rs:380`, "cannot change worlds while dead"). A death marks the player Lost at once. The move to the lobby happens in the `RespawnEvent` handler with `select_world` (lobby world key) then `teleport_to`. If `RespawnEvent` does not fire in a state where the player can be moved, record a host gap (next free spec number 008) and stop; do not work around it.
- **Tick rate:** `const TICKS_PER_SECOND: u32 = 20`, private to the extension, as in spleef. The host exposes none. `matches::Match::new` takes it as an argument.
- **Draw:** neither win nor loss for statistics; stats ignores it. A death and a quit in the same drained batch resolve as Draw for the death and Lost for the quit.
- **T008 is on hold** until world-cloner T010 lands, so death handling can be tested end to end against a real match.
- `MatchFinished` is emitted when the winner countdown ends, together with the arena release.
- The allocator reuses only Ready worlds; worlds being created count toward `max_worlds`.

Remaining blockers:

| Tasks | Waits for |
| --- | --- |
| T008, T010-T012 | `world-cloner-api` and the world-cloner service (host spec 005 T012 and T018). The `world-cloner-api` path dependency is not in `Cargo.toml` yet. |
| T009, T013, T014 (the parts that create a world) | `world-cloner-api`. The kit field is done: `PlayerEntry.kit: InventorySnapshot` (host spec 007, merged), so `arena-manager:match/v1` is final. |
| T016 | The manual check in `quickstart.md` needs the above; native test location (O1) is still open. |
| `/arena remove` in-use check, `/arena matches` | Match runtime (T009/T010). |
