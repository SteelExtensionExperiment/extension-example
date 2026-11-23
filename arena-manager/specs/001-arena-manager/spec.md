# Feature Specification: Arena Manager extension

**Feature Branch**: `001-arena-manager`

**Created**: 2026-11-23

**Status**: Draft

**Input**: User description: "The arena manager coordinates domains, teleports players and reports wins and losses to the game orchestrator. It holds the spawn points of every arena per player slot, teleports players there, sets their game mode and inventory, and creates a new domain with a world (through the world-cloner extension) when every arena is full. A world can hold several arenas; all maps are identical, so an arena configuration is saved once per game type and applies to every spawned world. It provides an API to start a match for a game type and some players, and to list running matches. When a player dies, that player returns to the lobby; the survivor is teleported 10 seconds later with an on-screen countdown. Commands create an arena and add a spawn point (the sender's location) for a given player slot. The number of worlds is limited, and a world with no match left is deleted together with its empty domain."

Decisions taken before writing (asked of the user, rounds 1-3):

| # | Decision |
| --- | --- |
| D1 | **Arena** = one match at a time, with N ordered player slots and one spawn point per slot. N is configurable per arena, not fixed at 2. |
| D2 | **Arena type is the game type.** A game type has one arena configuration (a list of arenas) saved once; it applies to every world spawned for that game type. One world holds all arenas of its game type. |
| D3 | **Arenas and spawn points are created by commands** and persist across restarts. |
| D4 | **The kit** (game mode and items) is managed by the game orchestrator and arrives in each start-match request. Arena-manager applies it and stores none. |
| D5 | **One lobby location** (plus a game mode), set by command. Dead players return there at once; the winner follows after a countdown (default 10 s). |
| D6 | **Disconnect counts as a loss. Simultaneous death is a draw** (no winner), so a result has three outcomes: won, lost, draw. |
| D7 | **World limit** is configured. When it is reached and every arena is busy, a request is rejected at once; the game orchestrator queues and retries. Arena-manager has no queue. |
| D8 | **Cleanup:** a world with no running match is removed, then its emptied domain. |
| D9 | **Cross-extension dependency:** no `game-registry`. Arena-manager `depends: ["world-cloner"]` and never depends on the orchestrator. The orchestrator calls arena-manager's `arena-manager:match/v1` service; results travel back as the `MatchFinished` / `MatchFailed` hook actions defined in `arena-manager-api`, so no cycle exists. Game types are opaque names here and kits arrive per player in the start request. |

**Communication (O2, decided):** calls go down through the service `arena-manager:match/v1`; results go up
through hook actions (`specs/extension-communication.md` in the Steel checkout). The kit field of
`PlayerEntry` waits for host spec 007 (item snapshot), so the contract is final only after it.
Requirements marked **(deferred)** wait for that field or for host spec 005.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An administrator defines arenas (Priority: P1)

An administrator stands in a copy of the map, creates an arena for a game type, and stands at each player's
starting position and runs one command per slot. The arenas are saved and are still there after a restart.

**Why this priority**: nothing can start a match without at least one complete arena.

**Independent Test**: create an arena, add two spawn points, list it, restart the server, list it again.

**Acceptance Scenarios**:

1. **Given** a permitted player, **When** they run `/arena create <gametype> <arena>`, **Then** the arena exists for that game type with no spawn points and is listed as incomplete.
2. **Given** an arena, **When** they run `/arena spawn set <gametype> <arena> <slot>`, **Then** their current position and rotation are stored for that slot, replacing an earlier one.
3. **Given** an arena with contiguous slots 1..N, N at least 2, **When** it is listed, **Then** it is shown as complete with N slots.
4. **Given** saved arenas, **When** the server restarts, **Then** the same arenas and spawn points are loaded.
5. **Given** no permission, **When** a player runs any `/arena` subcommand, **Then** it is denied and nothing changes.
6. **Given** a game type name the registry does not know **(deferred)**, **When** `create` runs, **Then** it fails with a message naming the game type.

---

### User Story 2 - A match starts in a free arena, in a new world when needed (Priority: P1)

The orchestrator asks for a match of a game type for some players. Arena-manager reserves a free arena of that
game type in an existing world, or, when none is free and the world limit allows, asks world-cloner for a new
world in a new domain. When the world is ready it moves each player there, sets the kit and starts the match.

**Why this priority**: this is the extension's purpose.

**Independent Test**: with one complete arena and the limit at 1, start one match: it runs in a new world; start a second: it is rejected; end the first, start a second: it succeeds.

**Acceptance Scenarios**:

1. **Given** a free arena in a loaded world, **When** a match is requested, **Then** the arena is reserved at once and players are moved without creating a world.
2. **Given** no free arena and fewer worlds than the limit, **When** a match is requested, **Then** a world is cloned into a new domain, the arena is reserved immediately, and players are moved when the world is ready.
3. **Given** no free arena and the limit reached (worlds still being created count), **When** a match is requested, **Then** the request is rejected at once with "no capacity".
4. **Given** a game type with no complete arena, an unknown game type, a player who is offline or already in a match, or a request whose player count has no arena with that many slots, **When** a match is requested, **Then** it is rejected with a message naming the cause and nothing changes.
5. **Given** the world clone fails, **When** the operation reports it, **Then** the match is reported as failed with the diagnostic, the arena is released and no player was moved.
6. **Given** a match starts, **When** a player arrives, **Then** their game mode is the kit's, their inventory is cleared and the kit items given, and they stand at their slot's spawn point.

---

### User Story 3 - A match ends and the survivor returns after a countdown (Priority: P1)

Players fight. Each player who dies is sent to the lobby at once. When one player is left, that player sees a
countdown (10 s by default) and is then moved to the lobby. The result is reported once.

**Why this priority**: the orchestrator needs results to credit wins and losses.

**Independent Test**: start a two-player match, kill one player: the dead player is in the lobby, the survivor sees 10..1 and then lands in the lobby, and one result (one won, one lost) is delivered.

**Acceptance Scenarios**:

1. **Given** a running match, **When** a player dies, **Then** they are moved to the lobby position with the lobby game mode and are recorded as lost.
2. **Given** exactly one player remains alive, **When** the match ends, **Then** a countdown is shown on their screen each second, and after the configured delay they are moved to the lobby and recorded as won.
3. **Given** every remaining player dies in the same tick, **When** the tick is resolved, **Then** the result is a draw for them.
4. **Given** a player disconnects during a match, **When** the host reports it, **Then** they are recorded as lost and the match continues or ends as above.
5. **Given** a match ended, **When** the winner has left the arena, **Then** the arena is released, and its world is removed with its domain when no other arena in it has a match.
6. **Given** a result, **When** it is delivered **(deferred)**, **Then** it is delivered exactly once with every participant's outcome.

---

### User Story 4 - Running matches can be reviewed (Priority: P2)

The orchestrator (and an administrator by command) can list current matches: game type, arena, world, state and
players with their alive status.

**Independent Test**: start a match, call the listing, see it; end it, see it gone.

**Acceptance Scenarios**:

1. **Given** running and preparing matches, **When** listed, **Then** each shows its id, game type, arena, world key, state and its players with alive status.
2. **Given** no matches, **When** listed, **Then** the list is empty.

---

### User Story 5 - Worlds are limited and cleaned up (Priority: P2)

**Independent Test**: limit 2, start three matches at once (third rejected), end one, check its world and domain are gone.

**Acceptance Scenarios**:

1. **Given** `max_worlds` in the configuration, **When** worlds exist or are being created up to that number, **Then** no further world is created.
2. **Given** a world whose arenas are all idle, **When** the last match ends, **Then** world-cloner removes it, then its domain is destroyed when empty.
3. **Given** world removal fails, **When** reported, **Then** it is logged and retried on a later tick; the world is not counted as free.

---

### Edge Cases

- A player disconnects or dies while the world is still being cloned (before Running): the match is cancelled and reported as failed, others stay where they are **(O9, confirm)**.
- The server stops with matches running: RAM worlds vanish; nothing about matches is persisted.
- Two matches start the same tick for the last free arena: the arena is reserved at request time on the tick thread, so only one succeeds.
- An arena was edited (spawn replaced or removed) while a match runs in it: the running match keeps its positions; the edit applies to the next match.
- A dead player lands on the respawn screen: handling is host-dependent (research R6, O3).
- A game type's arenas have different slot counts: a request is matched to an arena whose slot count equals the player count **(O5, confirm)**.
- A world-cloner template for the game type is missing or Failed: the request is rejected with the template message.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Arena-manager MUST store, per game type, a list of arenas. Each arena has a unique name within the game type and an ordered list of spawn points (position, yaw, pitch) indexed by slot starting at 1.
- **FR-002**: Spawn positions MUST be stored as coordinates only, without a world key, because every world spawned for the game type is identical. They apply to every new world.
- **FR-003**: An arena is **complete** when its slots are contiguous from 1 and number at least 2. Only complete arenas are used.
- **FR-004**: Commands MUST exist to create and remove an arena, set, remove and list spawn points, set the lobby, and list matches (see [contracts/commands.md](contracts/commands.md)). Each is guarded by its own permission.
- **FR-005**: Arenas and the lobby MUST be saved to disk when a command changes them and loaded at `init`. A corrupt file MUST refuse to start the extension with a message naming the file and cause.
- **FR-006**: The configuration MUST provide `max_worlds` (default 4) and `winner_return_delay_seconds` (default 10), validated at `init`.
- **FR-007**: A match request MUST name a game type and a list of players with a kit **(deferred: shapes)**, and MUST be rejected synchronously for every cause listed in User Story 2, scenario 4, and for capacity.
- **FR-008**: Arena reservation and world creation MUST happen on the tick thread at request time, so two requests cannot take the same arena. A newly requested world MUST count toward `max_worlds` from the moment it is requested until it is removed.
- **FR-009**: New worlds MUST be created through the `world-cloner:clone/v1` service, as a clone of the game type's template in a **new temporary domain per world** **(O4, confirm)**. Every returned `Operation` MUST be polled to completion.
- **FR-010**: For each player, arena-manager MUST move them into the match world, set the kit's game mode, clear their inventory, give the kit items and place them at their slot's spawn point, tolerating the player not yet being resident after a domain switch (research R4).
- **FR-011**: Arena-manager MUST watch deaths and disconnects of match players on the tick thread, apply D5 and D6, and decide draws by resolving deaths once per tick.
- **FR-012**: The winner's countdown MUST show once per second on their screen for `winner_return_delay_seconds`, then the winner MUST be moved to the lobby. The delay is the only thing between the end of play and the arena being released.
- **FR-013**: Arena-manager MUST remove a world through world-cloner when none of its arenas has a match, and then destroy its domain when emptied **(research R7)**.
- **FR-014**: Arena-manager MUST provide a service to start a match, list matches, and deliver results to a listener **(deferred)**, and MUST NOT depend on the game orchestrator (D9).
- **FR-015**: Failures (clone, teleport, removal) MUST be reported with a message and logged; none may panic.
- **FR-016**: The extension MUST have the unit tests listed in [tasks.md](tasks.md) and the manual check in [quickstart.md](quickstart.md).

### Key Entities

- **Arena**: `{ game_type, name, spawns: [slot -> Spawn] }`, persisted.
- **Lobby**: `{ position, yaw, pitch, world key, game mode }`, persisted.
- **Match**: id, game type, arena, world, state, players with alive status; in memory only.
- **Arena world**: a cloned world and its domain, with the arenas inside it and which are busy.
- **Result**: per participant outcome won, lost or draw **(deferred: delivery)**.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An administrator defines a complete two-slot arena with three commands and it survives a restart.
- **SC-002**: With `max_worlds = 1` and one arena, the second concurrent request is rejected and a request after the first match ends succeeds.
- **SC-003**: In a two-player match, a death produces one result and the survivor sees the countdown and returns after the configured delay.
- **SC-004**: After the last match in a world ends, the world and its domain no longer exist.
- **SC-005**: Without permission, every `/arena` subcommand is denied.

## Assumptions

- **A1**: world-cloner and its host platform (`specs/005-world-templates/`, tasks T012 for clone, T018 for remove) are implemented first.
- **A2**: the extension sits in `extension-example/` of the Steel checkout (`../../steel-extension-api`).
- **A3**: kits and game types are provided by the orchestrator side (D4, D9); arena-manager stores none.
- **A4**: Out of scope: queuing (D7), kit storage, statistics, teams, spectators, rematches, hot reload of the configuration, persisting matches.
- **O1 (open)**: where the extension's native integration test lives (as in world-cloner O1).
- **O2 (decided)**: no `game-registry`; service `arena-manager:match/v1` plus actions `MatchFinished`/`MatchFailed`; `Outcome` is `Won | Lost | Draw`; draw handling for statistics is proposed as neither win nor loss (confirm). Still open: the kit field (host spec 007).
- **O3 (open)**: how a dead player is moved out of the respawn screen (research R6); may need a host change.
- **O4 (confirm)**: one new temporary domain per cloned world (D2 keeps all arenas of a game type in one world).
- **O5 (confirm)**: a request is matched to an arena whose slot count equals the number of players.
- **O6 (confirm)**: which world-cloner template a game type clones, and where that is configured. Proposed: `/arena template <gametype> <template>`, saved with the arenas and validated against the `templates()` listing.
- **O7 (confirm)**: with more than two slots, the last player standing wins and each earlier death returns to the lobby at once.
- **O8 (confirm)**: saving the small data file on the command that changes it is acceptable on the tick thread; otherwise it goes through a host job.
- **O9 (confirm)**: death or disconnect before the match is Running cancels it.
