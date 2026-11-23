# Contract: service `arena-manager:match/v1` — DRAFT, NOT PUBLISHED

**Status: draft.** The communication between arena-manager, the `game-registry` extension and the game orchestrator is not fixed; the user will work on it later (spec O2). This file records what arena-manager must offer so that discussion can start. It is not a published contract and nothing may build against it. A published contract changes only with the user's approval (`../../../specs/extensions-workflow.md` in the Steel checkout).

Arena-manager owns this service and will put its types in `arena-manager-api`, the sibling crate of this extension. It does **not** depend on the game orchestrator (spec D9). Game type and kit types are owned by the registry contract and referenced from there, never copied.

## Needs

| Need | Notes |
| --- | --- |
| Start a match | Input: game type, players, kit (game mode and items). Output: a match id, or a synchronous rejection (see below). The world may still be created afterwards; the match then reports a state. |
| List matches | Snapshot of every match: id, game type, arena, world key, state, players with alive status. |
| Receive results | A listener gets one result per finished match: id, game type, outcome per participant (won, lost, draw). |
| Failure report | A match that fails before running (clone error, player not ready) is reported with its message, not as win or loss. |

## Synchronous rejections (spec FR-007)

Unknown game type, no complete arena for the player count, a player offline, not ready or already in a match, no capacity (world limit reached and every arena busy), template Failed or Loading.

## Rules for callers

- A world request starts a world-cloner `Operation` owned by arena-manager; callers never see it.
- Calls run on the server tick thread, take a borrowed `Server<'_>` as other services do (`vault-api/src/lib.rs:130-132`), and never block.
- The orchestrator handles capacity rejections by queueing and retrying (spec D7).

## Open (O2)

1. Shape and ownership of game type and kit types (registry).
2. Push (listener registered with arena-manager) versus pull (event log) for results; the listener is the current assumption.
3. The draw state: the stats-visualizer proposal (`stats-visualizer/specs/001-stats-visualizer/contracts/required-orchestrator.md`) assumes `won: bool`; a draw needs a third state and a decision on whether it counts as a loss.
4. Whether `start_match` returns immediately with a pending state or completes after players are placed.
5. How the orchestrator learns a player left or the match was cancelled.

## Versioning

`/v1` changes only for incompatible layout changes; adding a method is `/v2`.
