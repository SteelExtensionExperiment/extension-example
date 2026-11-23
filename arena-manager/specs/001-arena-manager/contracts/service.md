# Contract: service `arena-manager:match/v1`

**Status: final (O2 decided, kit field added).** Types live in `arena-manager-api` (sibling crate, `../../../../arena-manager-api/src/lib.rs`). `PlayerEntry { uuid, kit: InventorySnapshot }`: the kit is the host's item snapshot (spec 007, `steel_extension_api::inventory_snapshot`), captured by the caller with `Server::inventory_snapshot`. Arena-manager restores it with `Server::restore_inventory` when it places the player; the snapshot carries the game mode, and the restore fails, leaving the player unchanged, if a game mode change event cancels it. A published contract changes only with the user's approval (`specs/extensions-workflow.md` in the Steel checkout).

Arena-manager provides the service and emits the actions. It does **not** depend on the orchestrator or link its crate. Game types are opaque names.

## Service (down: callers pull)

| Method | Behavior |
| --- | --- |
| `start_match(server, StartRequest) -> ApiResult<MatchId>` | Synchronous rejection: unknown game type, no complete arena for the player count, a player offline, not ready or already in a match, no capacity (world limit reached and every arena busy), template Failed or Loading. Otherwise returns the id; the world may still be created afterwards. |
| `matches(server) -> ApiVec<MatchInfo>` | Snapshot: id, game type, arena, world key, state, players with alive flag. |
| `game_types(server) -> ApiVec<ApiString>` | Game types that have at least one arena. |
| `can_host(server, game_type, players) -> bool` | Whether a complete arena for that player count exists. |

## Actions (up: arena-manager emits, anyone listens)

| Action tag | Payload | When |
| --- | --- | --- |
| `arena-manager:match_finished/v1` | `MatchFinished { id, game_type, participants: [{ uuid, name, outcome }] }` | Once per match that ran to an end. `Outcome` is `Won \| Lost \| Draw`. Every participant once. How a player who left is reported is open (confirm with the user). |
| `arena-manager:match_failed/v1` | `MatchFailed { id, game_type, message }` | Once per match that failed before running (clone error, player not ready). Not a win or a loss. |

## Rules

- Calls run on the server tick thread, take a borrowed `Server<'_>` and never block.
- **Emit after releasing every lock.** `hooks::do_action` is synchronous; a listener may call `start_match` again.
- Listeners register in `pre_init` with `Hooks::add_action::<MatchFinished>(handler).register()`. The handler is `extern "C" fn(&MatchFinished)` and cannot capture, so it keeps its state in a `static` (`OnceLock<SyncMutex<_>>`).
- A world request starts a world-cloner `Operation` owned by arena-manager; callers never see it.
- A capacity rejection is retried by the orchestrator; arena-manager has no queue (D7).

## Versioning

`/v1` changes only for incompatible layout changes; adding a method is `/v2`.
