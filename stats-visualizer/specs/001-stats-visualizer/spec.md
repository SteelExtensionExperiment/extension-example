# Feature Specification: Stats Visualizer extension

**Feature Branch**: `001-stats-visualizer`

**Created**: 2026-11-23

**Status**: Draft

**Input**: User description: "Stats visualizer extension for SteelExtensionMC. It displays stats as a scoreboard that floats over blocks. The data is about the wins and losses of different game modes. A board shows the 5 players with the most wins (absolute) and, per player, the current ratio. The data comes from the game orchestrator; this extension only displays it. A command places a board at a position for a game type; game types come from the game orchestrator. The extension saves only player name, UUID, wins and losses, plus the location and game type of every board. It loads them at boot and saves them when the server stops."

This is the **extension** half. Two things it needs are not built yet and are prerequisites:

- The floating text (hologram) capability is host work, specified in the Steel checkout as
  `specs/006-text-holograms/`.
- The game types and match results come from the `game-orchestrator` extension. Its contract is **not
  specified yet**; what this extension needs from it is written down in
  [contracts/required-orchestrator.md](contracts/required-orchestrator.md) and is a proposal until the
  orchestrator's spec adopts it.

Decisions taken before writing (asked of the user, rounds 1–2):

| # | Decision |
| --- | --- |
| D1 | The ratio column is **W/L** (wins ÷ losses), two decimals. With 0 losses it shows the wins as the ratio. Kills and deaths are not stored, so no K/D exists. |
| D2 | The **orchestrator owns the contract**: it provides the game types and a match-result listener; this extension depends on it and the orchestrator knows nothing about stats. |
| D3 | Floating boards are built on a **new host capability** (spec 006), not on entity spawning from the extension. |
| D4 | Data is loaded at `init`, saved on clean server stop, saved periodically while changed (interval in configuration), and saved when a board is placed or removed. |
| D5 | Commands: `/statsboard place <id> <gametype> [<x> <y> <z> [<world>]]`, `remove <id>`, `list`, each with its own permission. |
| D6 | Ranking: wins descending, then W/L descending, then name ascending. Top 5. |
| D7 | A board refreshes when a match result changes its game type's ranking lines; nothing runs per tick. |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A board shows the top 5 for a game type (Priority: P1)

A player walks up to a floating board in the lobby. It shows the game type as its title and up to five
lines: rank, player name, total wins and W/L ratio.

**Why this priority**: it is the visible product; everything else feeds it.

**Independent Test**: record results for six players in one game type, place a board for it, and read the
board text: five lines in the order of D6.

**Acceptance Scenarios**:

1. **Given** six players with different wins in `skywars`, **When** a `skywars` board renders, **Then** it shows the five with most wins, best first, each with wins and W/L.
2. **Given** two players with equal wins, **When** the board renders, **Then** the better W/L ranks first, and equal W/L is ordered by name.
3. **Given** a player with wins and 0 losses, **When** the board renders, **Then** the ratio equals the wins (`5 wins, 0 losses → 5.00`).
4. **Given** fewer than five players with at least one win, **When** the board renders, **Then** only those players are listed and no empty rank lines appear.
5. **Given** a board for a game type nobody has won yet, **When** it renders, **Then** it shows the title and a "no wins yet" line.

---

### User Story 2 - Results from the orchestrator update the stats (Priority: P1)

A match ends. The game orchestrator reports the game type and, for each participant, whether they won or
lost. The extension adds one win or one loss per participant and updates every board of that game type.

**Why this priority**: without it no board ever shows data.

**Independent Test**: feed one result to the extension's listener and check the stored counts and that
only boards of that game type changed.

**Acceptance Scenarios**:

1. **Given** a result with two winners and three losers, **When** it is received, **Then** each winner gains a win and each loser a loss for that game type.
2. **Given** a player who changed their name, **When** a result carries the new name, **Then** the stored name is replaced; the UUID stays the key.
3. **Given** boards for `skywars` and `spleef`, **When** a `skywars` result arrives, **Then** only `skywars` boards re-render, and only if their lines changed.
4. **Given** a result for a game type the orchestrator does not list, **When** it is received, **Then** it is rejected with a log line and nothing is stored.

---

### User Story 3 - Stats and boards survive a restart (Priority: P1)

The server stops and starts again. Every player's wins and losses and every board are still there.

**Why this priority**: a leaderboard that resets is useless.

**Independent Test**: record results, place a board, stop the server cleanly, start it, and compare the
board text and `/statsboard list` before and after.

**Acceptance Scenarios**:

1. **Given** recorded stats and placed boards, **When** the server stops cleanly and starts, **Then** the same counts are loaded and each board is shown again at its position.
2. **Given** changed stats, **When** the configured autosave interval passes, **Then** the files on disk contain the change without a server stop.
3. **Given** an unreadable or corrupt data file at boot, **When** the extension initializes, **Then** the file is moved to `<name>.toml.broken`, an error names it, and the extension starts empty instead of overwriting it.
4. **Given** no data files, **When** the extension first starts, **Then** it starts empty and creates them on the first save.

---

### User Story 4 - Administrators place and remove boards (Priority: P2)

An administrator stands where a board should float, runs `/statsboard place lobby_skywars skywars`, and
the board appears. They can list and remove boards.

**Why this priority**: the only way to create a board; P2 because stats work without a board.

**Independent Test**: run each subcommand with and without permission from a player and the console.

**Acceptance Scenarios**:

1. **Given** no permission, **When** a subcommand is run, **Then** it is denied and nothing changes.
2. **Given** a player, **When** `place <id> <gametype>` omits coordinates, **Then** the board is placed at the player's position in the player's world.
3. **Given** the console, **When** `place` omits coordinates, **Then** it fails with a message saying the console must give `<x> <y> <z> <world>`.
4. **Given** a game type the orchestrator does not list, **When** `place` runs, **Then** it fails with `unknown game type <name>` and creates nothing.
5. **Given** an id already in use, **When** `place` runs, **Then** it fails and changes nothing.
6. **Given** `remove <id>`, **When** the id exists, **Then** the hologram disappears and the board is no longer saved; otherwise `unknown board <id>`.
7. **Given** the argument `<gametype>`, **When** the sender asks for completion, **Then** the orchestrator's game types are suggested; for `<id>` of `remove`, existing board ids.

---

### Edge Cases

- A board's world is not loaded (never existed, or a runtime world that was removed): the board stays saved
  and inactive; it is shown again when that world loads (`on_world_load`). It is not deleted.
- A board's chunk is unloaded: the host restores the hologram when the chunk loads (spec 006).
- A board's game type is no longer listed by the orchestrator: the board stays and shows its last stored
  ranking; `/statsboard list` marks it `unknown game type`. It is not deleted.
- The orchestrator is not loaded: the extension does not start (hard dependency, D2).
- A result lists the same UUID twice or a UUID both as winner and loser: the whole result is rejected and
  logged; no partial update.
- Two boards for the same game type: both update.
- Server crash between autosaves: changes since the last autosave are lost (D4, documented).
- A game type name that is not a valid file or table key: stored under its exact name; no assumption about
  characters is made beyond what the orchestrator contract defines.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The extension MUST hold, per player UUID, the latest known name and per game type a win count and a loss count, and nothing else about the player.
- **FR-002**: The extension MUST hold, per board, a unique id, a world key, a position and a game type.
- **FR-003**: On a received result, the extension MUST add one win to every winner and one loss to every loser for that game type, update names, and reject a malformed result entirely.
- **FR-004**: A board MUST show a title, and for ranks 1–5 the player name, total wins and the W/L ratio, using the ordering of D6 and the ratio rule of D1.
- **FR-005**: A board MUST refresh only after a result for its game type or when it is created or restored; it MUST NOT do per-tick work.
- **FR-006**: The extension MUST load its data at `init` and write it on clean server stop, on each autosave interval when data changed, and on board placement or removal. Writes MUST be atomic (temporary file, then rename).
- **FR-007**: A corrupt data file MUST be moved aside and reported, never overwritten silently.
- **FR-008**: The extension MUST provide `/statsboard place|remove|list`, each with its own permission, with completion for game types and board ids.
- **FR-009**: The extension MUST validate a game type against the orchestrator at `place` time and MUST reject unknown ones.
- **FR-010**: The extension MUST declare a dependency on `game-orchestrator` in the long `export_extension!` form.
- **FR-011**: The extension MUST have the unit tests listed in [tasks.md](tasks.md) and the manual check in [quickstart.md](quickstart.md).

### Key Entities

- **Player record**: `{ uuid, name, games: { <game type>: { wins, losses } } }`.
- **Board**: `{ id, world, x, y, z, game_type }` plus a runtime hologram handle.
- **Ranking line**: `{ rank, name, wins, ratio }`, derived, never stored.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After a result is received, every board of its game type shows the new ranking in the same tick the result is processed.
- **SC-002**: After a clean stop and start, a board's text is identical to before the stop.
- **SC-003**: With no result and no command, the extension does no work per tick besides the autosave check.
- **SC-004**: Without permission, every `/statsboard` subcommand is denied.

## Assumptions

- **A1**: host spec 006 (text holograms) is implemented before any task that creates a hologram (see [tasks.md](tasks.md)).
- **A2**: the orchestrator adopts the contract in [contracts/required-orchestrator.md](contracts/required-orchestrator.md) or a form of it. Until then every task that needs it is blocked.
- **A3**: this extension sits at `SteelExtensionMC/extension-example/stats-visualizer/`, so `../../steel-extension-api` resolves.
- **A4 (confirm)**: players with 0 wins in a game type are not ranked, because the board ranks by wins; they still keep their stored losses.
- **A5 (confirm)**: board text layout is `Title`, then `#1 Name  12 wins  3.00 W/L`-style lines; exact styling is configuration, not contract.
- **A6**: out of scope: K/D, kills, deaths, per-period stats (weekly, monthly), per-player lookup commands, a combined cross-game board, stats for players who never appear in a result, a web view.
- **O1 (open)**: location of the native integration test, as in the world cloner (research R3).
- **O2 (open)**: autosave writes synchronously on the server tick, as `essentials` does. Confirm this is acceptable for the expected file size (a few thousand players).
- **O3 (open)**: the orchestrator contract shape (R2) is a proposal; the orchestrator spec owns the final form.
