# game-orchestrator

Steel extension: the central control of a PvP tournament. Players queue per game type by command or sign, choose a kit per game type, and when a queue is full the orchestrator asks arena-manager for a match. Results are forwarded as `MatchResult` for stats-visualizer.

Spec, plan and contracts: [`specs/001-game-orchestrator/`](specs/001-game-orchestrator/spec.md). The contract crate is the sibling `../game-orchestrator-api`; this extension depends on `arena-manager` and `../arena-manager-api`.

## Layout

The crate sits two levels below a Steel checkout so `../../steel-extension-api` resolves. Rules are host-free modules (`queues`, `access`, `kits`, `signs`, `config`, `store`, `orchestrator`) with unit tests; `commands`, `arena`, `service`, `host` and `lib.rs` are the host glue.

## Configuration

`config.toml` (optional, in the extension's directory):

| Key | Default | Meaning |
| --- | --- | --- |
| `retry_interval_ticks` | 40 | Ticks between retries of a request rejected for capacity. |
| `max_session_hours` | 24 | Upper bound for `/orchestrator open <hours>`. |

Game types, kits, signs, kit choices and the open/closed state are saved by commands in `gametypes.toml`, `signs.toml`, `choices.toml` and `state.toml`. A corrupt file stops the extension from starting and the message names it. Queueing is closed after the first start.

## Commands

Players (`orchestrator.queue`, allowed by default): `/queue`, `/queue join <gametype>`, `/queue leave`.

Administrators, each subcommand with its own node: `/orchestrator gametype create|players|enable|disable|remove|list` (`orchestrator.gametype`, `list` is `orchestrator.list`), `kit save|load|remove` (`orchestrator.kit`), `kit list`, `status`, `queues` (`orchestrator.list`), `open [hours]`, `close` (`orchestrator.session`), `match <gametype> <player>...` (`orchestrator.match`), `sign list|remove <id>` (`orchestrator.sign`).

`kit save` stores the administrator's whole inventory and game mode as the host's `InventorySnapshot`; `kit load` replaces them. To edit a kit: load it, change it (also with `/invsee`), save it again.

## Signs

Write `[Queue]` and the game type, or `[Kit]`, the game type and the kit, on the first lines of a sign. It is registered by world and position (permission `orchestrator.sign`) and rewritten to plain text. Right-clicking acts as `/queue join` or as choosing the kit. Editing a registered sign to other text unregisters it.

## Rules for callers

- Match formation, retries and results run on the tick thread and never block.
- Consumers read results through the action `game-orchestrator:match_result/v1`, emitted once per finished match after the orchestrator released its lock. The service `game-orchestrator:games/v1` lists game types.
- A request rejected with the text `no capacity` is retried every `retry_interval_ticks`; any other rejection re-queues the players at the front and tells them.
