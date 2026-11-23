---
description: "Task list for the World Cloner extension (configuration, startup loading, service, commands)"
---

# Tasks: World Cloner extension

**Input**: Design documents from `specs/001-world-cloner/` in `extension-example/world-cloner/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md), and the host
platform spec `specs/005-world-templates/` in the Steel checkout (the "platform" tasks below).

**Tests**: unit tests beside the code (FR-010); the native test location is open (O1, research R2).

**Organization**: by user story. Tasks that need host bindings name the platform task they wait for.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US3 from spec.md
- Paths are relative to `extension-example/`: `world-cloner/` is the extension crate (holding this spec) and
  `world-cloner-api/` its sibling contract crate. Both sit at `SteelExtensionMC/extension-example/`, so
  `../../steel-extension-api` resolves.

---

## Phase 1: Setup

- [ ] T001 [P] Create `world-cloner-api/` with `Cargo.toml` (own `[workspace]`, path dependency `../../steel-extension-api`, lint table copied from `../worldedit-api/Cargo.toml`) and an empty `src/lib.rs`
- [x] T002 [P] Create `world-cloner/` with `Cargo.toml` (`[lib] crate-type = ["cdylib", "rlib"]`, dependencies on `../world-cloner-api`, `serde`, `toml`), `extension.toml` (`name = "world-cloner"`, `[targets]` with `libworld_cloner.so` and the other platform names), `clippy.toml`, and `src/lib.rs` with `export_extension!` in the long form declaring `name: "world-cloner"`, `depends: []`

---

## Phase 2: User Story 1 - Templates load at startup (Priority: P1) 🎯 MVP

**Goal**: configured templates load in the background and report their state.

**Independent Test**: one valid and one missing template; the server starts, one Ready, one Failed (spec US1).

- [x] T003 [P] [US1] Add `world-cloner/src/config.rs` (data-model) with parsing and validation per FR-002, and `init` wiring that stores the validated configuration and refuses to start on error. Tests: duplicate names, bad characters, an absolute path, a `..` path, an unknown game mode and a defaulted game mode each give the expected result and a message naming the entry. No host needed.
- [ ] T004 [US1] Add startup loading in `world-cloner/src/lib.rs` and `src/startup.rs`: on the first server tick call the platform `load_world_template` for each template (host key `world-cloner:<name>`), keep the `Operation`s, poll them on later ticks once, and log each result. **Waits for platform T007 and T012.** Test: key building and the once-only result handling, with the host call behind a small seam so the logic is unit-testable.

**Checkpoint**: US1 works; a server owner can see templates become Ready or Failed.

---

## Phase 3: User Story 2 - Service for other extensions (Priority: P1)

**Goal**: another extension can clone, create and remove worlds.

**Independent Test**: a consumer requests one clone, one generated world and one removal and polls each to completion (spec US2).

- [ ] T005 [US2] Write the contract in `world-cloner-api/src/lib.rs` (data-model, [contracts/service.md](contracts/service.md)): `CLONER`, `TemplateInfo`, `CloneRequest`, `CreateRequest`, the `WorldCloner` trait, `DynWorldCloner` and `cloner()`. **Waits for platform T012** (types re-exported from `steel_extension_api::runtime`). Test: the key constant and a stabby layout check, as `../worldedit-api/src/lib.rs:81-97` does.
- [ ] T006 [US2] Implement `templates` and `clone_world` in `world-cloner/src/service.rs`, register the service in `init` with `services::register`, and map names to host keys, fill the template's default game mode, and fail immediately for unknown, Loading or Failed templates (FR-006). **Waits for platform T012.** Tests: the request mapping, the default game mode and each failure message, with the host call behind the same seam as T004. This is the first place a service method returns an `Operation`; the first real consumer or the manual check in [quickstart.md](quickstart.md) proves it (research R1).
- [ ] T007 [US2] Add the `create_world` service method in `world-cloner/src/service.rs`. **Waits for platform T015.** Test: the request is passed through with the full `WorldSettings`.
- [ ] T008 [US2] Add the `remove_world` service method in `world-cloner/src/service.rs`. **Waits for platform T018.** Test: the call is passed through.

**Checkpoint**: the service is complete; the arena manager and game orchestrator can be implemented against it.

---

## Phase 4: User Story 3 - Admin commands (Priority: P2)

**Goal**: manual use of the service.

**Independent Test**: run each subcommand with and without permission (spec US3).

- [ ] T009 [US3] (tree and argument parsing done in `src/commands.rs`, not registered; handlers and reporter blocked on T006-T008) Add `world-cloner/src/commands.rs` with `/worldcloner templates|clone|create|remove` ([contracts/commands.md](contracts/commands.md)), template-name suggestions, and the pending-operation reporter that messages the sender with `Server::message` and logs. Tests: syntax, permissions, suggestions, and the report path for success and failure.

---

## Phase 5: Polish & cross-cutting

- [ ] T010 [P] Write `world-cloner/README.md` and `world-cloner-api/README.md`: layout requirement (`SteelExtensionMC/extension-example/world-cloner/`), configuration, service, commands, and the rule that callers poll every `Operation`.
- [ ] T011 [P] Run `cargo clippy` and `cargo fmt --check` for both crates, `typos`, and the manual check in [quickstart.md](quickstart.md). Decide O1 (research R2) and add the native test if the decision calls for one.

---

## Dependencies & execution order

- **T001, T002, T003** have no host dependency and can start now.
- **T004, T005, T006** wait for platform T012 (and T004 for T007). **T007** waits for platform T015,
  **T008** for platform T018.
- **T006** needs **T005**; **T009** needs T006–T008 for the subcommands whose methods exist (it can land
  per subcommand).
- **T010, T011** follow.

## Implementation strategy

**MVP**: T001–T004. Then T005–T006 so the arena manager can start against the service. T007–T008 and
T009 complete the surface. Track the platform tasks in `specs/005-world-templates/tasks.md`; this list
only schedules the extension side.
