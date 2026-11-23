---
description: "Task list for the Stats Visualizer extension (ranking, storage, boards, orchestrator results, commands)"
---

# Tasks: Stats Visualizer extension

**Input**: Design documents from `specs/001-stats-visualizer/` in `extension-example/stats-visualizer/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md), host spec
`specs/006-text-holograms/` (Steel checkout) and the orchestrator contract
([contracts/required-orchestrator.md](contracts/required-orchestrator.md)).

**Tests**: unit tests beside the code (FR-011); native test location is open (O1, research R3).

**Organization**: by user story. Tasks that need host holograms or the orchestrator name what they wait for.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US4 from spec.md
- Paths are relative to the extension crate, `SteelExtensionMC/extension-example/stats-visualizer/`.

---

## Phase 1: Setup

- [x] T001 Create `Cargo.toml` (own `[workspace]`, `[lib] crate-type = ["cdylib", "rlib"]`, path dependency `../../steel-extension-api`, `serde`, `toml`, lint table copied from `../worldedit/Cargo.toml`), `extension.toml` (`name = "stats-visualizer"`, `[targets]` for each platform library name), `clippy.toml`, and `src/lib.rs` with `export_extension!` in the long form (`name: "stats-visualizer"`, `depends: []` for now). **Do not add the orchestrator dependency yet; its crate does not exist (T007).**

---

## Phase 2: Foundational (no host or orchestrator needed)

- [x] T002 [P] [US1] Add `src/ranking.rs`: ranking per D6 and ratio per D1 as pure functions over a plain record list, plus the named top-size constant. Tests: ties on wins fall to W/L then name; 0 losses shows wins as the ratio; players with 0 wins are excluded; fewer than five entries; a huge count does not overflow; two-decimal formatting.
- [x] T003 [P] [US2] Add `src/stats.rs`: `Stats`, `PlayerRecord`, `GameRecord` (data-model) and `Stats::apply(result)` taking a plain result struct defined in this crate (mapped from the orchestrator type later, T007). Tests: wins and losses added per participant; name replaced; counters saturate; a duplicate UUID or a UUID in both groups rejects the whole result and changes nothing; dirty flag set only on success.
- [x] T004 [US3] Add `src/storage.rs`: load `stats.toml` and `boards.toml`, atomic save (temporary file, then rename), corrupt file moved to `*.toml.broken` with an error, missing file is an empty state. Tests: round trip; unreadable file moved aside and original bytes preserved; a failed rename leaves the old file intact; zero-count game tables are not written.
- [x] T005 [US3] Add `src/config.rs` and wire `lib.rs`: parse `config.toml` (`autosave_interval_ticks`, reject 0), load storage in `init`, `hooks.enable_scheduler()` plus `scheduler::run_repeating` saving only dirty files, and `hooks.on_shutdown` saving both (pattern `../../extension-example/essentials/src/lib.rs:27,44-51`). Tests: config defaults and the zero rejection; the dirty-flag decision of the autosave and shutdown paths behind a fake storage.

**Checkpoint**: stats and boards load, accumulate and save without any host hologram or orchestrator.

---

## Phase 3: User Story 1 - Boards show the top 5 (Priority: P1) 🎯 MVP

**Goal**: a board shows the ranking as floating text.

**Independent Test**: record results for six players, place a board, read its lines (spec US1).

- [x] T006 [US1] Add `src/boards.rs`: `Board`, text rendering from ranking lines (title, `#rank Name wins W/L`, empty "no wins yet" line), create, update and remove through the host hologram bindings behind a trait, and `last_lines` to skip unchanged refreshes (FR-005). **Waits for spec 006 T007 (create, update, remove bindings).** Tests with a fake hologram host: rendering of full, partial and empty rankings; an unchanged ranking makes no host call; a changed one makes exactly one update; creation failure keeps the board saved and inactive.

**Checkpoint**: a board renders and updates from stored stats.

---

## Phase 4: User Story 2 - Results from the orchestrator (Priority: P1)

**Goal**: match results change stats and boards.

**Independent Test**: deliver a result and see counts and boards change (spec US2).

- [x] T007 [US2] Add the orchestrator dependency (`Cargo.toml` path dependency and `depends: ["game-orchestrator"]` in `export_extension!`) and `src/results.rs`: register the listener, map the orchestrator result to the crate's plain struct (T003), apply it, mark dirty, refresh boards of that game type only. **Waits for the orchestrator spec and its `game-orchestrator-api` crate (O3).** Tests: only boards of the result's game type refresh; a rejected result changes neither stats nor boards.
- [ ] T008 [US3] Restore boards: on `init` and `on_world_load`, create each saved board's hologram if its world is loaded; on `on_world_unload`, mark boards of that world inactive (the host drops the holograms). A board whose world never appears stays saved. **Waits for spec 006 T007 (create, update, remove bindings).** Tests with fakes: boards of a loading world come back; others are untouched; a refused creation is logged once.

---

## Phase 5: User Story 4 - Commands (Priority: P2)

**Goal**: administrators place, remove and list boards.

**Independent Test**: each subcommand with and without permission, player and console (spec US4).

- [ ] T009 [US4] Add `src/commands.rs`: `/statsboard place|remove|list` ([contracts/commands.md](contracts/commands.md)) with permissions, id validation, player position and console coordinates, save on place and remove, completion of board ids. **Waits for spec 006 T007 (create, update, remove bindings).** Tests: syntax, permissions, id rules, duplicate id, console without coordinates, remove of an unknown id.
- [ ] T010 [US4] Validate the game type against the orchestrator in `place` and complete `<gametype>` from its list. **Waits for the orchestrator contract (T007).** Tests with a fake game type source: unknown type rejected, known type accepted, completion lists the source's types.
- [x] T011 [US2] Reject a result for an unlisted game type with a log line, and mark boards of a vanished game type `unknown game type` in `list`. **Waits for T007.** Tests: both paths.

---

## Phase 6: Polish & cross-cutting

- [ ] T012 [P] Update `README.md`: status, layout requirement, configuration, files written, commands and permissions, dependency on the orchestrator and host spec 006.
- [ ] T013 [P] Run `cargo clippy` and `cargo fmt --check`, `typos stats-visualizer`, and the manual check in [quickstart.md](quickstart.md). Decide O1 (research R3) and O2 (spec) and add the native test if the decision calls for one.

---

## Dependencies & execution order

- **T001** first. **T002, T003** are parallel and need only T001; **T004** needs T003's types; **T005** needs T004.
- **T006** needs T002 and spec 006. **T007** needs T003 and the orchestrator. **T008** needs T004 and T006.
- **T009** needs T004 and T006. **T010, T011** need T007 (and T009 for T010).
- **T012, T013** follow.

## Implementation strategy

**Start now:** T001–T005. They are independent of both blocked dependencies and produce ranking, stats,
storage and persistence with full unit tests. **MVP** (a visible board with real data) is T006–T008 and
needs spec 006 and the orchestrator contract. T009–T011 complete the admin surface. Track the host work in
`specs/006-text-holograms/tasks.md` and the orchestrator work in its own extension; this list only
schedules the extension side.
