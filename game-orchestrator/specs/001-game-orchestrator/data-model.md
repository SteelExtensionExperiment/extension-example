# Data Model: Game Orchestrator extension

Host entities (`PlayerId`, `GameMode`, `BlockPos`, `ItemSpec`) come from `steel_extension_api`. Types that cross the extension boundary are `#[stabby::stabby]`. Everything here is **(new)**. Files are TOML in `ExtensionContext::data_directory()`, written atomically (temporary file, then rename; pattern `extension-example/essentials/src/storage.rs:236-244`). A corrupt file refuses to start the extension with its name and cause (FR-014).

## Persisted: `gametypes.toml`

```toml
[[game_types]]
name = "skywars"        # unique; the identifier used everywhere (stats, arena-manager)
players = 2             # N, at least 2
enabled = true

[[game_types.kits]]
name = "knight"         # unique within the game type
format = 1              # InventorySnapshot::format
data = "0a0b..."        # InventorySnapshot::data as lowercase hex, opaque binary NBT
```

A kit is the host's `InventorySnapshot` of the administrator (slots 0-40, selected hotbar slot, game mode; host spec 007). The bytes are stored as produced and never interpreted here.

Validation at load: unique game type names, unique kit names per game type, `players >= 2`, kit `data` is valid hex. The host validates the snapshot itself when it is restored.

## Persisted: `signs.toml`

```toml
next_id = 1             # last number handed out; only grows, so an id is never reused
[[signs]]
id = "queue-1"          # assigned, never reused: queue-<n> or kit-<n>
world = "lobby:main"
x = 10
y = 65
z = -4
kind = "queue"          # queue | kit
game_type = "skywars"
kit = ""                # kit signs only
```

## Persisted: `choices.toml` (O6)

```toml
[[choices]]
uuid = "3f5c3a52-6c35-4d79-9d60-0a8b3c7f12aa"
game_type = "skywars"
kit = "knight"
```

## Persisted: `state.toml` (O7)

```toml
open = false            # queueing open (default false on first start)
```

The session's closing time is not persisted: a restart ends a timed session and leaves the state as stored.

## Configuration: `config.toml` (optional)

| Key | Default | Meaning |
| --- | --- | --- |
| `retry_interval_ticks` | 40 | Ticks between retries of a request rejected for capacity. At least 1. |
| `max_session_hours` | 24 | Upper bound for `/orchestrator open <hours>`. At least 1. |

Sign headers (`[Queue]`, `[Kit]`) are constants in code. The relation between config and command-made game types is O3; proposed: only `gametypes.toml`.

## Runtime state (memory only)

| Entity | Fields |
| --- | --- |
| `Orchestrator` | `store: Store`, `config`, `access: Access`, `queues: Queues`, `in_match: Map<PlayerId, MatchRef>`, `pending: Vec<PendingRequest>`. |
| `Access` | `open: bool`, `closes_at_tick: Option<u64>`, `session_task: Option<TaskId>`. |
| `Queues` | `by_game_type: Map<name, VecDeque<PlayerId>>`, `of_player: Map<PlayerId, name>`. The two maps are one struct so they cannot disagree (rule below). |
| `PendingRequest` | `game_type`, `players: Vec<(PlayerId, kit name)>`, `attempts`, `next_try_tick`. Players in it are in neither a queue nor a match until accepted. |
| `MatchRef` | `{ match id, game type }`. |
| `Request` | `{ game_type, players: [{ id, uuid, name, kit }] }` where `kit` is the stored kit value; the final wire shape is O2. |

## Rules

- **One queue per player** (D3): `of_player` has at most one entry per player; join removes the old entry and queue entry first.
- **Formation** (D4, FR-006): after any change to a queue, while `len >= N` pop N from the front into a request, in game type name order across queues.
- **Refusal order for join**: in a match; queueing closed; game type unknown; game type disabled; no valid kit. The first match is the message.
- **Capacity retry** (FR-007): a `PendingRequest` holds its players out of the queues; if a player quits, the request is rejected as a whole and the others re-queue at the front.
- **Re-queue at the front** keeps the original order of the returned players.
- **Disable, remove or close**: clears the affected queues with a message; pending requests of a removed game type are dropped the same way.
- **Kit validity**: a choice is valid when the kit still exists in the game type; a game type with one kit needs no choice.
- **Result** (US6): arena-manager's result frees every participant (`in_match` entry removed) and is forwarded once.
