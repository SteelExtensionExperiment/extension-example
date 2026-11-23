# Contract: what stats-visualizer consumes from `game-orchestrator`

**Status: adopted (O2).** The orchestrator owns it in `game-orchestrator-api` (`../../../../game-orchestrator-api/src/lib.rs`, spec `../../../../game-orchestrator/specs/001-game-orchestrator/contracts/service.md`). This extension links only that crate and never copies its types.

| Need | Provided as |
| --- | --- |
| List of game types | Service `game-orchestrator:games/v1`, `game_types(server)`, looked up at use time (command validation, completion, flagging boards whose type disappeared). |
| Match result notification | Action `game-orchestrator:match_result/v1`; handler registered in `pre_init` with `add_action::<MatchResult>`. |
| Result payload | `MatchResult { game_type, participants: [{ uuid, name, outcome }] }`. |

## Properties relied on

- One result per finished match, every participant once, delivered on the tick thread; no extra locking contract beyond this extension's own `SyncMutex`.
- `game_type` names are stable across restarts, since `stats.toml` and `boards.toml` store them.
- The handler is `extern "C" fn(&MatchResult)` and keeps its state in a `static` (`OnceLock<SyncMutex<_>>`).

## Outcome handling

`Won` counts as a win, `Lost` as a loss. `Draw` ignored (neither) is proposed and needs the user's confirmation.

## Not needed

Kills, deaths, scores, match ids, timestamps, team structure: the extension stores none of it.
