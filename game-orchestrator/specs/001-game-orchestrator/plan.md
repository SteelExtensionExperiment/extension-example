# Implementation Plan: Game Orchestrator extension

**Branch**: `001-game-orchestrator` | **Date**: 2026-11-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-game-orchestrator/spec.md`

## Summary

Build the `game-orchestrator` extension and its `game-orchestrator-api` contract crate. The extension stores
game types, kits, signs and kit choices; keeps one queue per game type with one queue per player; forms a
match request when a queue reaches the game type's player count; controls access through an open/closed
switch, a timed session, per-game-type enable flags and on-behalf matches; and receives match results from
arena-manager, forwarding them to listeners such as stats-visualizer. How it connects to arena-manager and
stats-visualizer is deferred (O2); cycles are accepted in this plan (D12).

## Technical Context

**Language/Version**: Rust (the Steel checkout's `rust-toolchain.toml`, nightly; found by Cargo because this extension sits inside the checkout)

**Primary Dependencies**: `steel-extension-api` (path `../../steel-extension-api`, no `host` feature), `serde`, `toml`. The arena-manager contract (`../arena-manager-api`) is added when O2 is decided. No other crates.

**Storage**: `gametypes.toml`, `signs.toml`, `choices.toml`, `state.toml` and `config.toml` in the extension data directory ([data-model.md](data-model.md)). Queues and matches are memory only.

**Testing**: unit tests beside the code for the pure logic (queue rules, matchmaking, access rules, kit and sign parsing, persistence) behind small host seams; the manual check in [quickstart.md](quickstart.md). Native test location is open (O10).

**Target Platform**: Linux server, vanilla Java clients.

**Project Type**: native extension (cdylib) plus a contract crate.

**Performance Goals**: none beyond the host's. Work happens on joins, leaves, results, retries and the session timer.

**Constraints**:
- No async, no threads. Handlers for join/quit/interact/sign edit run on the tick thread; session expiry and capacity retries use `scheduler::run_later`/`run_repeating`.
- Nothing waits for another extension. A rejected request is answered synchronously by arena-manager (its D7).
- No panics in production paths.
- State is bundled in one struct per file's data behind a `SyncMutex` (`steel_utils::locks`).

**Scale/Scope**: a handful of game types, tens of kits, hundreds of players queueing over some hours.

## Constitution Check

| Principle | Status | How |
| --- | --- | --- |
| I. Stable Native Boundary | Pass, with an open item | Contract is stabby under a versioned key; its shape is deferred (O2), so nothing is published. |
| II. Modding-Grade Extensibility | Pass | Others use only the service; sign and file formats are private. |
| III. Vanilla Parity First | Pass | No gameplay logic. Inventory snapshot fidelity is research R1 (O1). |
| IV. Foundation Rigor | Pass, with an open item | Kits wait for O1 rather than shipping a lossy snapshot silently; arena calls wait for O2. |
| V. Explicit Failure | Pass | Every refusal and failure carries a message; corrupt files refuse to start. |
| VI. Concurrency & Dependencies | Pass | No threads or async; no crates beyond `serde` and `toml`. |
| VII. Verified Behavior | Pass, open point | Unit tests plus manual check; native test is O10. |

No violations.

## Project Structure

### Documentation (this feature)

```text
specs/001-game-orchestrator/
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
game-orchestrator-api/
├── Cargo.toml
└── src/lib.rs               # NEW, draft until O2 is decided

game-orchestrator/
├── Cargo.toml
├── extension.toml
├── clippy.toml
└── src/
    ├── lib.rs               # NEW export_extension!, init, event subscriptions
    ├── config.rs            # NEW config.toml parsing and validation
    ├── store.rs             # NEW game types, kits, signs, choices, state: load and atomic save
    ├── kits.rs              # NEW kit model, snapshot from and apply to an inventory
    ├── queues.rs            # NEW queues, one-queue-per-player rule, matchmaking (pure)
    ├── access.rs            # NEW open/closed, session, enabled flags, join checks (pure)
    ├── signs.rs             # NEW sign text parsing, registry, interact handling
    ├── commands.rs          # NEW /queue and /orchestrator
    ├── arena.rs             # NEW calls to arena-manager, retry on capacity (deferred, O2)
    └── service.rs           # NEW listener registration and result fan-out (deferred, O2)
```

**Structure decision.** Rules are pure modules without host types so they are unit-testable (`queues`, `access`, parsers in `signs`, models in `kits`). Host calls sit in `kits.rs` (slot access), `signs.rs` (events), `arena.rs` and `service.rs` behind small seams.

## Layout requirement

This extension sits in `extension-example/` of a Steel checkout, as `SteelExtensionMC/extension-example/game-orchestrator/` (crate and this spec) with its contract crate beside it as `extension-example/game-orchestrator-api/`. Crates reach the API through `../../steel-extension-api`. The arena-manager contract is reached by `../arena-manager-api` once it exists.

## Platform and cross-extension dependencies

| This task | Waits for |
| --- | --- |
| T008 (kit snapshot and apply) | O1: user decision on lossy items; possibly a new host spec (next free number 007) |
| T012 (sign interaction) | research R3: confirm a way to verify the block is a sign and to learn when it is removed (O9) |
| T013, T014, T015 (arena, results, service) | O2: connection and contract with arena-manager and the listener type for stats-visualizer |
| T001-T007, T009-T011 | Nothing |

## Complexity Tracking

Empty.
