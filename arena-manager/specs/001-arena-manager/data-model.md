# Data Model: Arena Manager extension

Host entities (`Operation`, `Location`, `PlayerId`, `GameMode`) come from `steel_extension_api`. World clones come from `world-cloner` (`../../../world-cloner/specs/001-world-cloner/contracts/service.md`). Types that cross the extension boundary are `#[stabby::stabby]`. Items are **(new)**.

## Persisted: `extensions/arena-manager/arenas.toml` (new)

```toml
[lobby]
world = "lobby:main"      # world key at the time of /arena lobby set
x = 0.5
y = 64.0
z = 0.5
yaw = 0.0
pitch = 0.0
game_mode = "adventure"   # survival | creative | adventure | spectator

[[game_types]]
name = "skywars"          # identifier from the registry (O2)
template = "skywars_a"    # world-cloner template name (O6, confirm)

[[game_types.arenas]]
name = "north"            # unique within the game type
[[game_types.arenas.spawns]]
slot = 1
x = 10.5
y = 70.0
z = -3.5
yaw = 90.0
pitch = 0.0
```

Validation at load: unique game type names, unique arena names per game type, unique slots per arena, valid game mode. Completeness (contiguous slots 1..N, N at least 2) is derived, not stored. A bad file refuses to start the extension with the file and cause (FR-005).

## Configuration: `extensions/arena-manager/config.toml` (new)

| Key | Default | Meaning |
| --- | --- | --- |
| `max_worlds` | 4 | Most arena worlds existing or being created at once. At least 1. |
| `winner_return_delay_seconds` | 10 | Countdown before the last player standing returns to the lobby. |
| `player_ready_timeout_seconds` | 15 | How long to wait for a player to become ready after a domain switch (R4). Cancels the match when exceeded (O9). |

## Runtime state (new, memory only)

| Entity | Fields |
| --- | --- |
| `ArenaManager` | `store: ArenaStore`, `config`, `worlds: Vec<ArenaWorld>`, `matches: Vec<Match>`, `next_match_id`, `next_domain_id`, `deaths: SyncMutex<Vec<PlayerEvent>>`. |
| `ArenaWorld` | `key` (`arena<n>:map`), `domain`, `game_type`, `state: Creating(Operation) | Ready | Removing(Operation) | DestroyingDomain(Operation)`, `busy: Set<arena name>`. |
| `Match` | `id`, `game_type`, `arena`, `world`, `state: Preparing | Starting | Running | Ending { remaining_ticks } | Finished`, `players: Vec<MatchPlayer>`. |
| `MatchPlayer` | `{ id: PlayerId, uuid, name, slot, step: Pending | Switching | Ready | Placed, alive: bool, outcome: Option<Outcome> }`. |
| `Outcome` | `Won | Lost | Draw`. |

World keys and domain names use a counter that is never reused in the process (the host requires a never-used domain name for a new temporary domain: `specs/005-world-templates/contracts/host-api.md`, `clone_world`).

Every `Operation` held by a world or a match belongs to this extension and is polled to completion.

## Match state machine

```text
Preparing ──world ready──▶ Starting ──all players placed──▶ Running
    │                         │                                │ one alive left, or none
    │ clone failed            │ player not ready / left        ▼
    ▼                         ▼                            Ending (winner countdown)
  Failed ◀──────────────── Failed                              │ delay elapsed, winner moved
                                                               ▼
                                                           Finished ─▶ arena released
```

## `arena-manager-api` (draft, deferred O2)

Superseded by the published contract in `arena-manager-api` and [contracts/service.md](contracts/service.md).

| Item | Draft shape |
| --- | --- |
| `MATCHES` | `"arena-manager:match/v1"` |
| `MatchId` | `u64` |
| `MatchInfo` | `{ id, game_type, arena, world, state, players: [{ uuid, name, alive }] }` |
| `Outcome` | `Won | Lost | Draw` |
| `MatchResult` | `{ id, game_type, participants: [{ uuid, name, outcome }] }` |
| `start_match`, `matches`, `subscribe_results` | see [contracts/service.md](contracts/service.md); request fields for game type and kit are owned by the registry contract |
