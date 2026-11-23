---
description: "Task list for the Game Orchestrator extension (queues, kits, signs, tournament control, results)"
---

# Tasks: Game Orchestrator extension

**Input**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md).

**Tests**: unit tests beside the code for the pure logic; native test location is open (O10).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1-US6 from spec.md
- **(deferred)**: waits for O2 (connection and contracts with arena-manager and stats-visualizer). Do not start, stub or guess it.
- Paths relative to `extension-example/`: `game-orchestrator/` is the extension crate (holding this spec) and `game-orchestrator-api/` its sibling contract crate, at `SteelExtensionMC/extension-example/`.

---

## Phase 1: Setup

- [x] T001 [P] Create `game-orchestrator-api/` with `Cargo.toml` (own `[workspace]`, path dependency `../../steel-extension-api`, lint table from `../worldedit-api/Cargo.toml`) and an empty `src/lib.rs`
- [x] T002 [P] Create `game-orchestrator/` with `Cargo.toml` (`cdylib` and `rlib`; dependencies `../game-orchestrator-api`, `serde`, `toml`), `extension.toml`, `clippy.toml`, and `src/lib.rs` with `export_extension!` in the long form, `name: "game-orchestrator"`. Dependencies on arena-manager are added with O2.

## Phase 2: Pure rules (P1)

- [x] T003 [P] [US1] `src/queues.rs`: `Queues` with the one-queue-per-player rule, join (moving), leave, front re-queue, and formation of requests of N in game type order. Tests: join moves between queues; never two entries per player; formation takes the first N and leaves the rest in order; two game types complete in one change; quit removes; re-queue keeps order; N changed re-evaluates.
- [x] T004 [P] [US4] `src/access.rs`: open/closed, session closing tick, enabled flags, and the join check in the order of data-model rules. Tests: each refusal and its precedence; timed session expires; `open` without hours; disable and close list the players to clear.
- [x] T005 [P] [US2] `src/kits.rs` (model only): kit, slot rules, kit choice resolution (single-kit auto select, removed kit invalid). Tests: valid and invalid choices, duplicate slots rejected, game type with one kit.
- [x] T006 [P] [US2] `src/signs.rs` (parser and registry only): parse `[Queue]`/`[Kit]` lines, validate game type and kit, id assignment never reused. Tests: valid and invalid texts, unknown names, header case, stale registry entries.
- [x] T007 [P] `src/config.rs`: parse and validate `config.toml` (`retry_interval_ticks` at least 1, `max_session_hours` at least 1). Tests: defaults, each invalid value names its key.

## Phase 3: Storage and commands (P1) 🎯 MVP

**Independent Test**: create a game type, list it, restart, list it again (spec US3 scenario 4, kits excluded until O1).

- [x] T009 [US3] `src/store.rs`: `gametypes.toml`, `signs.toml`, `choices.toml`, `state.toml` model, load, atomic save, validation per data-model. Tests: round trip, duplicates and bad slots rejected, corrupt file gives a message naming file and cause, missing files start empty.
- [x] T010 [US3] (kit completion only for game types: the host suggester cannot read earlier arguments) `src/commands.rs`: `/queue` and `/orchestrator gametype|open|close|status|queues|sign|kit list` with permission nodes and errors per [contracts/commands.md](contracts/commands.md); `match` is added with T013. Tests: syntax, permissions, error messages, player-only commands.
- [x] T011 [US1] `src/lib.rs`: wire join, quit and interact events to the pure modules; session expiry through `scheduler::run_later`; clear queues on close, disable and remove with messages. Tests: handler logic through a small host seam.

## Phase 4: Kits (P1)

- [x] T008 [US3] `src/kits.rs` (host part): snapshot slots 0-40 and game mode from the administrator, apply a kit back for `kit save`, `kit load`. Done with host spec 007's `InventorySnapshot` (O1 decided). Tests after O1: round trip of every readable field; empty slots; armor and off hand.
- [x] T012 [US2] `src/signs.rs` (host part): subscribe sign-edit (validate, register, rewrite lines) and interact (queue and kit signs act as the command and kit choice); `sign list|remove`. Done: sign-edit registration uses `SignChangeEvent::world()`; a click checks that the block key still ends in `_sign`; editing a registered sign to other text unregisters it.

## Phase 5: Matches and results (deferred)

- [x] T013 [US1][US4] `src/arena.rs`: build a request with a kit per player, call arena-manager, handle synchronous rejection (capacity retry through `scheduler::run_repeating` every `retry_interval_ticks`, other causes re-queue at the front), `/orchestrator match`. Tests (pure part): retry and re-queue decisions.
- [x] T014 [US6] `src/service.rs`: receive arena-manager results, free players, fan out to listeners, provide `game_types`. Needs O2 and the stats-visualizer draw decision (O5).
- [x] T015 [US5] `/orchestrator status` running-match listing and `/queue` details that need match data.

## Phase 6: Polish

- [x] T016 [P] `README.md` for both crates: layout, configuration, commands, signs, caller rules.
- [~] T017 [P] (clippy, fmt and unit tests pass; the manual check and native test (O10) are not run) `cargo clippy`, `cargo fmt --check`, `typos`, the manual check in [quickstart.md](quickstart.md). Decide O10 and add a native test if the decision calls for one.

---

## Dependencies & execution order

- **T001-T007, T009-T011** have no external dependency and can start now (T003-T007 are pure).
- All tasks are unblocked; T017 still needs the manual check and the O10 native test decision.
- **T009** needs the models from T003-T006; **T010** needs T009; **T011** needs T003, T004, T010.

## Implementation strategy

MVP: T001-T007, then T009-T011, so queues, access and game types are tested and working before any host call to another extension. Kits and signs follow when O1 and R3 are settled; arena and results (T013-T015) are designed together with arena-manager and stats-visualizer when the user fixes O2. Until then a match request can only be checked through a test seam, which the user must approve if a temporary caller is wanted.
