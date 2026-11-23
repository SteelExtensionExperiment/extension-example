# Required contract: what stats-visualizer needs from `game-orchestrator`

**Status: proposal.** The orchestrator owns this contract
(`specs/extensions-workflow.md` in the Steel checkout: the provider owns it, in its own `<name>-api` crate and
`specs/<spec>/contracts/`). The `game-orchestrator` repository has no spec yet (README only). This file
records the **minimum** this extension consumes so the orchestrator spec can adopt or reshape it. A
published contract changes only with the user's approval. This extension never copies the orchestrator's
types; it links to them by sibling path `../game-orchestrator-api`.

## What is needed

| Need | Why | Proposed shape |
| --- | --- | --- |
| List of game types | Validate `/statsboard place`, complete the `<gametype>` argument, flag boards whose type disappeared. | `game_types(&self, server: Server<'_>) -> ApiVec<ApiString>`: stable game type identifiers (the string the orchestrator uses everywhere). |
| Match result notification | Count wins and losses. | A way to register a listener that receives `MatchResult` once per finished match. |
| Result payload | One win or loss per participant, latest name. | `MatchResult { game_type: ApiString, participants: ApiVec<Participant> }`, `Participant { uuid: Uuid, name: ApiString, won: bool }`. |

## Required properties

- A result is delivered **once** per finished match, with every participant listed once. Players who
  left before the end are included or not by the orchestrator's own rule; stats counts what it receives.
- Delivery happens on the server tick thread, like other extension callbacks, so no locking contract is
  needed beyond the extension's own `SyncMutex`.
- `game_type` identifiers are stable across restarts, because they are stored in `stats.toml` and
  `boards.toml`.
- The listener registration follows the existing service pattern (`steel-extension-api/src/services.rs`,
  `extension-example/worldedit-api`): a versioned service key `game-orchestrator:<name>/v1`.

## Open for the orchestrator spec

1. Whether results are pushed to a registered listener (stabby callback object) or pulled from an event
   log. Pushing is assumed here.
2. Whether the contract also offers display names for game types. The board title uses the identifier
   until it does.
3. Whether `won: bool` is enough. Draws or ties are not counted by this extension; a draw would need
   a third state and a decision on whether it counts as a loss.

## Not needed

Kills, deaths, scores, match ids, timestamps, team structure: the extension stores none of it.
