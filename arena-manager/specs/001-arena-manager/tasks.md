---
description: "Task list for the Arena Manager extension (arenas, match flow, world limit, cleanup)"
---

# Tasks: Arena Manager extension

**Input**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md); world-cloner's `specs/001-world-cloner/`.

**Tests**: unit tests beside the code for the pure logic; native test location is open (O1).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1-US5 from spec.md
- **(deferred)**: waits for O2, the registry/orchestrator contract. Do not start, stub or guess it.
- Paths relative to `extension-example/`: `arena-manager/` is the extension crate (holding this spec) and `arena-manager-api/` its sibling contract crate, at `SteelExtensionMC/extension-example/`.

---

## Phase 1: Setup

- [x] T001 [P] Create `arena-manager-api/` with `Cargo.toml` (own `[workspace]`, path dependency `../../steel-extension-api`, lint table from `../worldedit-api/Cargo.toml`) and an empty `src/lib.rs`
- [x] T002 [P] Create `arena-manager/` with `Cargo.toml` (`cdylib` and `rlib`; dependencies `../arena-manager-api`, `../world-cloner-api`, `serde`, `toml`), `extension.toml`, `clippy.toml`, and `src/lib.rs` with `export_extension!` in the long form, `name: "arena-manager"`, `depends: ["world-cloner"]`

## Phase 2: User Story 1 - Arenas by command (P1) 🎯 MVP

**Independent Test**: create an arena, set two spawns, list, restart, list (spec US1).

- [x] T003 [P] [US1] `src/config.rs`: parse and validate `config.toml` (`max_worlds` at least 1, delays positive). Tests: defaults, each invalid value gives a message naming the key.
- [x] T004 [US1] `src/store.rs`: arena and lobby model, completeness (contiguous 1..N, N at least 2), load and write-then-rename save of `arenas.toml`, validation per data-model. Tests: round trip, duplicate names and slots rejected, gap makes an arena incomplete, corrupt file gives a message naming file and cause.
- [x] T005 [US1] `src/commands.rs`: `/arena create|remove|spawn set|spawn remove|spawn list|template|list|lobby set` with per-node permissions ([contracts/commands.md](contracts/commands.md)). Tests: syntax, permissions, errors. Game type validation against the registry is **(deferred)**; until then names are checked against saved game types only.

**Checkpoint**: an administrator defines arenas that survive a restart.

## Phase 3: Match rules (pure logic, P1)

- [x] T006 [P] [US2] `src/allocator.rs`: choose a free complete arena with slot count equal to the player count in an existing ready world; otherwise decide to create a world if `worlds < max_worlds` (worlds being created count), else reject with "no capacity". Tests: free arena reused, cap respected, two requests for the last arena (only one wins), incomplete arena ignored, slot-count mismatch.
- [x] T007 [P] [US3] `src/matches.rs`: match state machine ([data-model.md](data-model.md)). Inputs: placed, death, quit, tick. Outputs: outcomes. Tests: one death in two players ends with won and lost; simultaneous death in one drained batch is a draw; quit counts as a loss; three players, last standing wins; death before Running cancels (O9); winner countdown length and the point at which the arena is released.

## Phase 4: User Story 3 - Deaths and return (P1)

- [ ] T008 [US3] `src/lib.rs` and `src/transfer.rs`: subscribe death and quit (enqueue only), drain once per server tick (research R8), return the dead player to the lobby with the lobby game mode, run the winner countdown with `send_title` or `send_action_bar` each second, then move the winner. **Waits for research R6** for the respawn screen; may need a host spec.

## Phase 5: User Story 2 - Matches and worlds (P1)

**Independent Test**: limit 1, start one match (new world), second rejected, after the first ends a third succeeds (spec US2).

- [ ] T009 [US2] (deferred) Match request intake: validate game type and players, reserve an arena, return a rejection or a match id. Needs the registry and orchestrator types (O2).
- [ ] T010 [US2] `src/worlds.rs`: clone the game type's template as `arena<n>:map` in a new temporary domain through `world-cloner:clone/v1` (`cloner()` lookup at use time), poll the `Operation`, mark the world ready or fail the match. **Waits for world-cloner T006.** Tests: key and domain counter never reused; failure releases the arena.
- [ ] T011 [US2] `src/transfer.rs`: per-player steps from research R4 (switch domain, wait ready or timeout, teleport to the slot spawn, game mode, clear inventory, kit items). **Waits for world-cloner T006 and research R4 (teleport timing).** Kit type is **(deferred)**.

## Phase 6: User Story 5 - Cleanup (P2)

- [ ] T012 [US5] `src/worlds.rs`: when no arena of a world has a match, call world-cloner `remove_world`, poll, then `destroy_temporary_domain` and poll; log failures and retry next tick; count a world toward the cap until fully gone. **Waits for world-cloner T008 and research R7.**

## Phase 7: User Story 4 - Listing and service (P2)

- [ ] T013 [US4] (deferred) `src/service.rs` and `arena-manager-api`: start match, list matches, result listener, once O2 fixes the contract. Includes the `/arena matches` command.
- [ ] T014 [US3] (deferred) Deliver one result per finished match to the listener, with the draw state, once O2 is decided.

## Phase 8: Polish

- [x] T015 [P] `README.md` for both crates: layout, configuration, commands, caller rules (poll world-cloner operations; arena-manager's own service is documented once O2 is decided).
- [ ] T016 [P] `cargo clippy`, `cargo fmt --check`, `typos`, the manual check in [quickstart.md](quickstart.md). Decide O1 and add a native test if the decision calls for one.

---

## Dependencies & execution order

- **T001-T007** have no external dependency and can start now (T006 and T007 are pure).
- **T008** waits for R6. **T010-T012** wait for world-cloner tasks. **T009, T013, T014** wait for O2.
- **T005** needs T004; **T006** needs T004's model; **T010** needs T006; **T011** needs T007 and T010; **T012** needs T010.

## Implementation strategy

MVP: T001-T005, then T006-T007 so the rules are tested before any host call. T010-T012 follow when world-cloner lands. The service and results (T009, T013, T014) are designed together with the registry and orchestrator once the user fixes O2. Track host work in `specs/005-world-templates/tasks.md`.
