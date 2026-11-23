# Feature Specification: Game Orchestrator extension

**Feature Branch**: `001-game-orchestrator`

**Created**: 2026-11-23

**Status**: Draft

**Input**: User description: "The game orchestrator is the mastermind. A player requests a game by command or by clicking a sign in the lobby, and can be in only one queue. Each game type has a number of players, set by command or config; when that many players are in its queue, the orchestrator requests a new arena from the arena manager with these players. Game types can be created by command or config, and kits are added to a game type. Kits are built by taking a snapshot of an administrator's inventory and are easy to modify with `/invsee`. Players can have different kits per game type and choose one with a sign; without a chosen kit a warning is shown. The orchestrator also manages whether players may join a game, and can request a game on behalf of players, so it is the central management of a PvP tournament running for some hours."

Decisions taken before writing (asked of the user, rounds 1-2, plus the restated dependency rule):

| # | Decision |
| --- | --- |
| D1 | **The orchestrator is the master.** It owns game types, kits, queues, access control and the tournament session. It asks arena-manager to run a match and receives the result. It forwards results to stats-visualizer. |
| D2 | A player requests a game by **command** (`/queue join <gametype>`) or by **right-clicking a queue sign** in the lobby. |
| D3 | A player is in **at most one queue**. Joining another game type **moves** them to the new queue. |
| D4 | Each **game type has a player count N**, set by command or config. When its queue holds N players the orchestrator requests a match for the first N queued players (oldest first). |
| D5 | Game types are **created by command or config**; **kits** are added to a game type (a game type has several). |
| D6 | **A kit is a snapshot of an administrator's inventory.** Edit flow: load a kit into the administrator's inventory, change it (directly or with `/invsee`), save it again. |
| D7 | **Kits are chosen per player and per game type** at a **kit sign**. The chosen kit is applied when the match starts. **Without a chosen kit, joining the queue is blocked with a warning.** A game type with exactly one kit selects it automatically. |
| D8 | **Signs are registered by their text** (`[Queue]` or `[Kit]` plus game type, and kit name) through the sign-edit event; positions persist. |
| D9 | **Access control**: a global open/closed switch, a per-game-type enable/disable switch, and an administrator command that **requests a match on behalf of named players**. |
| D10 | **Tournament session**: the administrator can open queueing for a number of hours; it closes by itself. Running matches finish. |
| D11 | **Queue status command** for players (own queue, position, kit) and for administrators (all queues). |
| D12 | **Cross-extension dependencies may be cyclic in this plan.** The orchestrator and arena-manager need each other (start-match requests one way, results the other). How they are connected is a future decision (O2); it does not block this spec. |

**Not in scope now** (the user: "maybe more comes later, but for now that should be everything"): brackets,
rounds, Elo or rankings, teams or parties, spectators, scheduled tournaments, persisting queues or matches,
a web view.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Players queue by command or sign and a match forms (Priority: P1)

A player has a kit chosen for a game type and joins its queue by command or by right-clicking its queue
sign. When N players are queued, the orchestrator requests a match with the first N and they leave the queue.

**Why this priority**: it is the purpose of the extension.

**Independent Test**: set `skywars` to 2 players, give two players a kit each, have both join: one match request with both players and their kits is made, and the queue is empty.

**Acceptance Scenarios**:

1. **Given** a game type with N=2 and an empty queue, **When** one player joins, **Then** they are queued at position 1 and told so.
2. **Given** one player queued, **When** a second joins, **Then** one match request with both players and their chosen kits is made and both leave the queue.
3. **Given** a queued player, **When** they join another game type, **Then** they leave the first queue and enter the second, and are told.
4. **Given** a queued player, **When** they run `/queue leave`, disconnect or enter a match, **Then** they leave the queue.
5. **Given** a player who is in a running match, **When** they try to join a queue, **Then** it is refused with a message.
6. **Given** a queue sign, **When** a player right-clicks it, **Then** the effect is that of `/queue join <gametype>`.
7. **Given** a request is rejected for "no capacity" (arena-manager D7), **When** it happens, **Then** the players keep their place at the front of the queue and the request is retried later; they are not removed.
8. **Given** a request is rejected for a cause other than capacity or fails after acceptance, **When** it is reported, **Then** the affected players are told the message and are re-queued at the front, except a player who left.

---

### User Story 2 - Players choose a kit per game type at a sign (Priority: P1)

A player right-clicks a kit sign for a game type and kit. That kit is now their choice for that game type.
The choice is what they get in matches of that game type until they choose another.

**Why this priority**: a match cannot start without a kit for every player.

**Independent Test**: right-click the `[Kit] skywars knight` sign, run `/queue join skywars`: no warning and the match request carries `knight` for this player. Without the click: the join is blocked.

**Acceptance Scenarios**:

1. **Given** a kit sign for `skywars`/`knight`, **When** a player right-clicks it, **Then** their kit for `skywars` is `knight` and they are told.
2. **Given** a player with no kit chosen for a game type that has several, **When** they join its queue, **Then** it is refused with a warning naming the kit sign step.
3. **Given** a game type with exactly one kit, **When** a player joins without choosing, **Then** that kit is used.
4. **Given** a chosen kit that was removed, **When** the player joins, **Then** it counts as no kit chosen.
5. **Given** a player queued with kit A, **When** they click a sign for kit B of the same game type, **Then** the kit changes for the match request; their queue place is unchanged **(O6, confirm)**.
6. **Given** different players choose different kits, **When** a match forms, **Then** each player receives their own kit.

---

### User Story 3 - Administrators define game types and kits (Priority: P1)

An administrator creates a game type with a player count and builds its kits by taking a snapshot of their
own inventory. A saved kit can be loaded back into the administrator's inventory, changed (also with
`/invsee` on a helper player) and saved again.

**Why this priority**: nothing can be queued until a game type with at least one kit exists.

**Independent Test**: `/orchestrator gametype create skywars 2`, fill the inventory, `/orchestrator kit save skywars knight`, clear inventory, `kit load skywars knight`: the inventory is restored; restart: the kit is still there.

**Acceptance Scenarios**:

1. **Given** a permitted administrator, **When** they create a game type with name and N at least 2, **Then** it exists, enabled, with no kits.
2. **Given** an inventory, **When** `kit save <gametype> <kit>` runs, **Then** the kit stores every slot (hotbar, main, armor, off hand) and the administrator's game mode, replacing an earlier kit of that name.
3. **Given** a saved kit, **When** `kit load` runs, **Then** the administrator's inventory and game mode are replaced by the kit, after a warning that this clears the inventory **(O4)**.
4. **Given** game types and kits, **When** the server restarts, **Then** they are loaded unchanged.
5. **Given** the game types and kits are described in the configuration file, **When** the extension starts, **Then** they are loaded exactly as if created by command **(O3, confirm how the two sources relate)**.
6. **Given** no permission, **When** any `/orchestrator` subcommand runs, **Then** it is denied and nothing changes.
7. **Given** a game type that has queued players or a running match, **When** an administrator removes it, **Then** its queue is cleared with a message and running matches finish.

---

### User Story 4 - The tournament is controlled centrally (Priority: P1)

An administrator opens the tournament for a number of hours, can disable single game types, close the queue
for everybody and request a match on behalf of named players. When the session ends queueing closes.

**Why this priority**: the user's purpose is running a PvP tournament for some hours.

**Independent Test**: `/orchestrator open 2`, check status shows the closing time; `/orchestrator close`: a join is refused; `/orchestrator match skywars A B`: a match request is made for A and B without queueing.

**Acceptance Scenarios**:

1. **Given** queueing is closed, **When** a player joins by command or sign, **Then** it is refused with a message and nothing changes.
2. **Given** `open <hours>`, **When** the time elapses, **Then** queueing closes, queued players are told and removed, running matches finish.
3. **Given** `open` without hours, **When** it runs, **Then** queueing is open until `close` **(O7, confirm)**.
4. **Given** a game type disabled, **When** a player joins its queue, **Then** it is refused; players already queued are removed with a message.
5. **Given** `match <gametype> <player>...` with exactly N valid players, **When** it runs, **Then** one match request is made on their behalf, skipping the queue and removing them from any queue, using each player's chosen kit **(O8, confirm: needs a chosen kit; bypasses open/closed, not disabled)**.
6. **Given** the status command, **When** an administrator runs it, **Then** it shows open or closed, time left, each game type with enabled, N, kits and queue size, and running matches.

---

### User Story 5 - Players see their queue status (Priority: P2)

**Independent Test**: join a queue, run `/queue`, see game type, position, N and the chosen kit.

**Acceptance Scenarios**:

1. **Given** a queued player, **When** they run `/queue`, **Then** it shows game type, position, players needed and kit.
2. **Given** a player not queued, **When** they run `/queue`, **Then** it says so and lists enabled game types with queue sizes.
3. **Given** an administrator, **When** they run `/orchestrator queues`, **Then** every queue with player names in order is shown.

---

### User Story 6 - Results reach consumers (Priority: P2)

When arena-manager reports a finished match, the orchestrator delivers one result to registered listeners,
including stats-visualizer, and frees the players to queue again.

**Independent Test**: finish a two-player match: one result with won/lost is delivered to a test listener once; the players can queue again.

**Acceptance Scenarios**:

1. **Given** a result from arena-manager, **When** received, **Then** it is delivered once to every listener with each participant's outcome (won, lost or draw) **(O5: draw state vs stats' `won: bool`)**.
2. **Given** a result, **When** received, **Then** the players are no longer in a match.
3. **Given** a match that failed before it ran, **When** reported, **Then** no result is delivered and US1 scenario 8 applies.

---

### Edge Cases

- A queued player disconnects: removed at once; no match is formed with them.
- The N-th player joins in the same tick as another leaves: the queue is resolved on the tick thread, one change at a time, so a match never has fewer than N players.
- Two game types complete in the same tick: both requests are made, in game type order.
- A player is queued while the tournament closes or their game type is disabled: removed with a message.
- The queue is longer than N: only the first N leave; the rest stay in order.
- Capacity is full: retried on a configured interval; players are never lost.
- The server stops: queues and matches are not persisted (out of scope); game types, kits, signs, kit choices (O6) and open/closed state **(O7, confirm)** are.
- A registered sign whose game type or kit no longer exists: right-click answers with a message naming it.
- A sign is written with an unknown game type or kit: the edit is refused with a message and the sign is not registered.
- A `kit save` while the administrator is in creative or spectator: the game mode is stored as it is.
- `/invsee` is a host command and unchanged; the orchestrator never calls it (research R7).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The orchestrator MUST store game types, each with a unique name, a player count N of at least 2, an enabled flag and an ordered list of kits, and MUST persist them.
- **FR-002**: Game types MUST be creatable and changeable by command and loadable from the configuration (O3).
- **FR-003**: A kit MUST store the item stacks of slots 0 to 40 and a game mode, and MUST be savable from, and loadable into, an administrator's inventory.
- **FR-004**: A player MUST be in at most one queue; joining another moves them (D3). A player in a running match MUST NOT be able to queue.
- **FR-005**: Joining MUST be refused when queueing is closed, the game type is disabled or unknown, the player has no valid kit for it, or the player is in a match. Each refusal MUST carry a message naming the cause.
- **FR-006**: When a queue holds N players the orchestrator MUST request a match for the first N, with each player's chosen kit, remove them from the queue and mark them in a match.
- **FR-007**: A rejection for capacity MUST NOT remove players; the request MUST be retried every `retry_interval_ticks`. Another rejection or failure MUST tell the players and re-queue them at the front (US1-8).
- **FR-008**: The kit sign and queue sign MUST be registered from sign text and persisted by position; right-click MUST act as the command (D2, D7, D8).
- **FR-009**: Kit choices MUST be stored per player UUID and game type **(O6: memory or persisted)**.
- **FR-010**: The orchestrator MUST provide a global open/closed switch, a timed session, a per-game-type enabled flag and an on-behalf match command (D9, D10).
- **FR-011**: The orchestrator MUST provide `/queue join|leave` and `/queue` status for players, and `/orchestrator` administration with a permission node per subcommand.
- **FR-012**: The orchestrator MUST receive match results and deliver each once to listeners, with won, lost or draw per participant, and free the players (US6).
- **FR-013**: All state changes (queues, signs, kit choices, results) MUST happen on the tick thread; nothing waits or sleeps for another extension. Timed session expiry and retries use the host scheduler.
- **FR-014**: Failures MUST be reported with a message and logged; none may panic. A corrupt data file MUST refuse to start the extension with a message naming file and cause.
- **FR-015**: The extension MUST have the unit tests listed in [tasks.md](tasks.md) and the manual check in [quickstart.md](quickstart.md).

### Key Entities

- **Game type**: `{ name, players, enabled, kits[] }`, persisted.
- **Kit**: `{ name, game_mode, slots[0..=40] }`, persisted per game type.
- **Queue**: ordered player ids for one game type, memory only.
- **Player state**: not queued, queued for a game type, in a match.
- **Kit choice**: `(player UUID, game type) -> kit name`.
- **Sign**: `{ world key, position, kind: queue | kit, game type, kit? }`, persisted.
- **Session**: open/closed and the closing tick.
- **Match request / result**: shapes are owned by the contracts, deferred (O2).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With N=2 and two players with kits, joining by command or sign produces exactly one match request carrying both kits, and the queue is empty afterwards.
- **SC-002**: A player is never in two queues, nor queued while in a match, in any sequence of joins, leaves, quits and results.
- **SC-003**: A kit saved from an inventory and loaded back reproduces every slot the host can read, and survives a restart.
- **SC-004**: With queueing closed or a game type disabled, every join is refused and no match request is made.
- **SC-005**: Without permission, every `/orchestrator` subcommand is denied.
- **SC-006**: A finished match produces exactly one delivered result.

## Assumptions

- **A1**: arena-manager provides the start-match, list and result path (its O2); until that is fixed, the tasks that call it wait (see [tasks.md](tasks.md)).
- **A2**: stats-visualizer consumes this extension's result listener; this extension knows nothing about stats (stats D2).
- **A3**: this extension sits at `SteelExtensionMC/extension-example/game-orchestrator/`, so `../../steel-extension-api` resolves.
- **A4**: Kits only need what the host can read and write per slot today, unless O1 is decided otherwise.
- **A5**: Out of scope: brackets, rounds, Elo, teams, parties, spectators, scheduled tournaments, queue persistence, hot reload of the configuration.
- **O1 (open, blocks kits)**: the extension API's item snapshot is **lossy** (research R1): potions, enchanted-book enchantments, custom model data, attribute modifiers, banners, firework and similar components are lost, so a kit of such items would not equal the administrator's inventory. Options: accept the loss, or specify a host item-snapshot capability as host spec 007. Needs the user's decision.
- **O2 (open, user decides later)**: how the orchestrator, arena-manager and stats-visualizer connect: cyclic dependency (D12), the `game-registry` split from arena-manager D9, service shapes, per-player kit in the start request, result delivery and the draw state.
- **O3 (confirm)**: how config and commands relate. Proposed: one persisted `gametypes.toml` that both commands write and an administrator may edit while the server is stopped; `config.toml` holds only global settings; no hot reload.
- **O4 (confirm)**: `kit load` replaces the administrator's inventory without a confirmation prompt, after a one-line warning in the reply.
- **O5 (confirm)**: results carry won, lost or draw; arena-manager already has three states, stats-visualizer's proposal has `won: bool`. A draw counts as neither (proposed).
- **O6 (confirm)**: kit choices are persisted per UUID; clicking another kit sign while queued changes the kit without losing the queue place.
- **O7 (confirm)**: the open/closed state is persisted; `open` without hours stays open until `close`; the default after first start is closed.
- **O8 (confirm)**: `match` on behalf bypasses the open/closed switch and the queue, not the disabled flag, and needs each player to have a chosen kit.
- **O9 (confirm)**: `/orchestrator sign list` and `sign remove <id>` exist to unregister signs (text registration alone cannot).
- **O10 (open)**: the native integration test location, as in the other extensions.
