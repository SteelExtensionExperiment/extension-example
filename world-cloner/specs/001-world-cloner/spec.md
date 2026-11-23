# Feature Specification: World Cloner extension

**Feature Branch**: `001-world-cloner`

**Created**: 2026-11-23

**Status**: Draft

**Input**: User description: "World cloner extension for SteelExtensionMC. Load worlds from disk storage one time into memory and copy them an unlimited number of times, with RAM-only storage, to spawn the same world multiple times for a minigame server. The arena manager uses it when an arena is full: it spawns a new domain with the new world, or adds the world to an existing domain. The result is a ready, loaded world in the specified domain with the specified name."

This is the **extension** half. The host capability it uses (read-only templates, copy-on-write RAM
worlds, runtime world creation and removal, and the bindings that expose them) is specified in the
Steel checkout as `specs/005-world-templates/` and is a prerequisite. Decision numbers below match
that spec; the ones owned here are D1, D7 (default game mode) and D13.

Decisions taken before writing (asked of the user, rounds 1–7):

| # | Decision |
| --- | --- |
| D1 | **Templates** are standalone Steel-saved world folders that the running server does not use. All are declared in this extension's configuration and loaded into RAM once at startup. |
| D7 | **Default game mode** of a clone comes from the per-template configuration, because it is not stored in a world folder. The caller of the service can override it per request. |
| D12 | **A template that fails to load** is marked Failed and does not stop the others (host behavior; this extension reports it). |
| D13 | **Contract:** a stabby service `world-cloner:clone/v1` that returns host `Operation`s for the caller to poll, plus admin commands. The arena manager and game orchestrator consume it. |

All other decisions (D2–D6, D8–D11: what a template and a clone contain, targets, clocks, removal,
reconnect, portals) are host behavior described in the platform spec.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Templates load at startup and report their state (Priority: P1)

A server owner lists saved arena worlds in the extension's configuration and starts the server. Each
listed folder is loaded into memory in the background. The log says which templates are ready and why
any other failed, and the server starts normally either way.

**Why this priority**: nothing else works until at least one template is loaded, and a bad folder must
never stop a server.

**Independent Test**: configure one valid template and one with a missing folder, start the server, read
the log, and list templates with the command: one Ready, one Failed with a message.

**Acceptance Scenarios**:

1. **Given** a valid configuration, **When** the server starts, **Then** every template is loaded without delaying any tick and its state becomes Ready.
2. **Given** a template whose folder is missing or invalid, **When** the server starts, **Then** the server still starts, the log names the template and the cause, and that template is Failed while the others are Ready.
3. **Given** an invalid configuration (duplicate names, absolute or escaping paths, unknown game mode), **When** the extension initializes, **Then** it refuses to start with a message naming the entry.
4. **Given** a loaded template, **When** the server runs and stops, **Then** the template folder is unchanged.

---

### User Story 2 - Other extensions request worlds through the service (Priority: P1)

The arena manager looks up `world-cloner:clone/v1`, asks for a clone of `skywars_a` as `minigames:arena_12`,
and receives an `Operation`. It polls the operation on later ticks and gets the loaded world's key. It can
also ask for a generated world, or remove a world it created.

**Why this priority**: this is how the feature reaches the arena manager and game orchestrator.

**Independent Test**: a test consumer requests one clone, one generated world and one removal, polls each
operation to completion, and checks the results.

**Acceptance Scenarios**:

1. **Given** the extension is loaded, **When** a consumer looks up the service, **Then** it gets it and can list templates with their state and failure message.
2. **Given** a Ready template, **When** a consumer requests a clone for a new or existing domain, **Then** it receives an `Operation` that completes with the world key.
3. **Given** a request that leaves the game mode unset, **When** the clone is created, **Then** the template's configured default game mode is used.
4. **Given** a Failed or Loading template, **When** a clone is requested, **Then** the call fails immediately with that template's message, and other templates still clone.
5. **Given** an unknown template name, **When** a clone is requested, **Then** the call fails with `unknown template <name>`.
6. **Given** a consumer that requests removal of a world, **When** the host accepts it, **Then** it receives an `Operation` that completes once the world is retired.

---

### User Story 3 - Admin commands (Priority: P2)

An administrator lists templates and clones, creates or removes a world by hand to try an arena without
writing a second extension.

**Why this priority**: the service is usable without commands, but a command makes the feature checkable
and operable by humans.

**Independent Test**: run each subcommand once with and without permission from the console and a player.

**Acceptance Scenarios**:

1. **Given** no permission, **When** a player runs a `/worldcloner` subcommand, **Then** it is denied and nothing changes.
2. **Given** a command that starts a world operation, **When** the operation finishes, **Then** the sender is told the result and the log records it.
3. **Given** a template name argument, **When** the sender asks for completion, **Then** configured template names are suggested.

---

### Edge Cases

- A template listed in the configuration whose folder does not exist: Failed, server starts.
- The same template requested for clones while another clone is still being created: independent requests.
- The consumer never polls an `Operation` it received: the result stays in the host table for the life of the process (documented in the contract).
- A consumer requests a clone before the first server tick has started template loading: the template reports Loading, so the call fails with "template is still loading".
- The extension is installed on a host without the platform bindings: it fails to load with the host's binding error; it does not partly work.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The extension configuration MUST list templates by `name`, relative `path` and `default_game_mode` (default `survival`).
- **FR-002**: At `init`, the extension MUST reject duplicate names, names with characters outside letters, digits, `_` and `-`, absolute paths, paths that escape the data directory, and unknown game modes, with a message naming the entry.
- **FR-003**: On the first server tick, the extension MUST start loading every configured template through the host, without blocking the tick, and MUST log each result once.
- **FR-004**: The extension MUST NOT hold template data; the host registry is the source of truth, read through the host status call.
- **FR-005**: The `world-cloner-api` crate MUST define the service `world-cloner:clone/v1` with: list templates with state, clone, create generated world, and remove world. Each method taking a borrowed `Server<'_>` and returning an `Operation` (or the list) to the caller.
- **FR-006**: The service MUST map a bare template name to its host key, fill the template's default game mode when a request leaves it unset, and fail immediately with a clear message for unknown, Loading or Failed templates.
- **FR-007**: The service MUST document that every returned `Operation` must be polled to completion by its caller.
- **FR-008**: The extension MUST provide `/worldcloner templates`, `clone`, `create` and `remove`, each guarded by its own permission, and MUST report the final result of each started operation to the sender and the log.
- **FR-009**: The extension MUST declare no dependencies and MUST be consumable with `depends` or `soft_depends` by other extensions.
- **FR-010**: The extension MUST have the unit tests listed in [tasks.md](tasks.md), and its manual check in [quickstart.md](quickstart.md).

### Key Entities

- **Template configuration**: `{ name, path, default_game_mode }`, one per listed world folder.
- **Service**: `WorldCloner`, the versioned contract other extensions call.
- **Pending operation**: an `Operation` started by a command, tracked with the sender so the result can be reported.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With one valid and one invalid template configured, the server starts, one template is Ready and one Failed with a message in the log and in `/worldcloner templates`.
- **SC-002**: A consumer can clone, create and remove worlds through the service, polling each operation to completion.
- **SC-003**: Every invalid configuration entry in FR-002 is rejected with a message that names the entry.
- **SC-004**: Without permission, every `/worldcloner` subcommand is denied.

## Assumptions

- **A1**: the host platform spec (`specs/005-world-templates/` in the Steel checkout) is implemented first, at least the bindings task for each service method (see [tasks.md](tasks.md)).
- **A2**: the extension sits in `extension-example/` of the Steel checkout, so `../../steel-extension-api` resolves, as for the other examples. The README states the layout.
- **A3**: the default game mode is not stored in a world folder, so it lives in this extension's configuration (D7).
- **A4**: out of scope: capturing a template from a running world, persistent on-disk copies, hot reload of the configuration, command override flags for seed, spawn, game mode and difficulty (the service carries them).
- **O1 (open)**: where the extension's native integration test lives. The existing harness is `steel-extension-api/tests/common` in the Steel checkout, which loads example libraries from `../extension-example/<crate>`. See [research R2](research.md). Decision needed from the user before the Polish phase.
