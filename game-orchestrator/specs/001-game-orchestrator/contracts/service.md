# Contract: service `game-orchestrator:games/v1`

**Status: decided (O2).** Types live in `game-orchestrator-api` (`../../../../game-orchestrator-api/src/lib.rs`). A published contract changes only with the user's approval (`specs/extensions-workflow.md` in the Steel checkout).

The orchestrator owns the contract it **provides** and never copies the one it **consumes** from arena-manager (`../../../../arena-manager/specs/001-arena-manager/contracts/service.md`).

## Provided

| Item | Shape |
| --- | --- |
| Service `game-orchestrator:games/v1` | `game_types(server) -> ApiVec<ApiString>`: stable names, as in `gametypes.toml`, enabled or not. |
| Action `game-orchestrator:match_result/v1` | `MatchResult { game_type, participants: [{ uuid, name, outcome }] }`. `Outcome` is re-exported from `arena-manager-api`. |

`MatchResult` is emitted once per finished match with every participant once, on the tick thread, after the orchestrator dropped its own locks. It is never emitted for failed matches. Stats-visualizer counts `Won` as a win and `Lost` as a loss; ignoring `Draw` is proposed (confirm).

## Consumed (from arena-manager)

| Need | How |
| --- | --- |
| Start a match | `arena_manager_api::matches()?.start_match(..)` at use time; a kit per player once host spec 007 exists. Capacity rejections are retried; other rejections are reported to the players. |
| Results | Handler for action `MatchFinished`, registered in `pre_init`; it maps the result and emits `MatchResult`. |
| Failures | Handler for `MatchFailed`; players are told, the queue entry is released, no result is emitted. |
| List matches | `matches()` for `/orchestrator status`. |

Declared in `export_extension!` as `depends: ["arena-manager"]`.

## Rules

- Calls run on the tick thread and never block (spec FR-013).
- Handlers cannot capture state; the orchestrator keeps its runtime state in a `static` `OnceLock<SyncMutex<_>>`.
- Never emit while holding that mutex: stats and the next `start_match` may re-enter.

## Versioning

`/v1` changes only for incompatible layout changes; adding a method is `/v2`.
