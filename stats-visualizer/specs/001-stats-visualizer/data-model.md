# Data Model: Stats Visualizer extension

Host entities (holograms, worlds) are in the platform spec `specs/006-text-holograms/` in the Steel
checkout. Game types and results are defined by the orchestrator contract
([contracts/required-orchestrator.md](contracts/required-orchestrator.md)). This file covers only what
this extension stores. Everything is **(new)**.

## Files in the extension data directory

`ExtensionContext::data_directory()` (`steel-extension-api/src/bindings.rs:160`) is an existing absolute
directory. Both files are TOML, written atomically (temporary file, then rename; pattern in
`extension-example/essentials/src/storage.rs:236-244`).

### `stats.toml`

```toml
[[players]]
uuid = "3f5c3a52-6c35-4d79-9d60-0a8b3c7f12aa"   # canonical lowercase hyphenated; the key
name = "Alex"                                    # latest name seen in a result

[players.games.skywars]
wins = 12
losses = 4

[players.games.spleef]
wins = 3
losses = 9
```

Only name, UUID, wins and losses are stored (user requirement). Ratio, rank and board text are derived.
A game type table with `wins = 0` and `losses = 0` is never written.

### `boards.toml`

```toml
[[boards]]
id = "lobby_skywars"          # unique
world = "lobby:main"          # world key
x = 12.5
y = 71.0
z = -3.5
game_type = "skywars"         # an orchestrator game type
```

### `config.toml` (`extensions/stats-visualizer/config.toml`, optional)

```toml
autosave_interval_ticks = 1200   # default 1200 (60 s); 0 is rejected
```

Only `autosave_interval_ticks` is read. The top size (5) is a named constant, not configuration, and text
styling is fixed in code for this iteration (assumption A5).

## Runtime state

| Entity | Fields |
| --- | --- |
| `Stats` | `players: HashMap<Uuid, PlayerRecord>`, `dirty: bool`. Behind one `SyncMutex`. |
| `PlayerRecord` | `name: String`, `games: BTreeMap<String, GameRecord>`. |
| `GameRecord` | `wins: u32`, `losses: u32`. |
| `Boards` | `boards: Vec<Board>`, `dirty: bool`. Behind one `SyncMutex`. |
| `Board` | `id`, `world`, `position`, `game_type`, `hologram: Option<HologramId>`, `last_lines: Vec<String>`. |
| `RankingLine` | `rank: u8`, `name: String`, `wins: u32`, `ratio: f64`. Derived. |

`HologramId` is the host handle from spec 006. `last_lines` lets a refresh skip the host call when the
rendered lines did not change (FR-005).

## Rules

- **Ranking** (D6): filter players with `wins > 0` for the game type; sort by `wins` desc, then `ratio`
  desc, then `name` asc (ordinal, case-insensitive then case-sensitive for a stable total order); take 5.
- **Ratio** (D1): `losses == 0 → wins as f64`, else `wins as f64 / losses as f64`; formatted with two
  decimals.
- **Counters** saturate at `u32::MAX`, never wrap.
- **Result application** (FR-003): validate the whole result first (known game type, no duplicate UUID),
  then apply, then mark `dirty` and refresh boards of that game type.
- **Dirty flags** drive autosave: the periodic task writes a file only when its flag is set, then clears it.
