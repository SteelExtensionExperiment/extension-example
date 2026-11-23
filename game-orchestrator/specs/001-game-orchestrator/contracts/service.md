# Contract: service `game-orchestrator:games/v1` — DRAFT, NOT PUBLISHED

**Status: draft.** How the orchestrator, arena-manager and stats-visualizer connect is not fixed; the user will decide later and accepts cyclic dependencies in the plan (spec D12, O2). This file records what the orchestrator offers and needs so that discussion can start. Nothing may build against it. A published contract changes only with the user's approval (`specs/extensions-workflow.md` in the Steel checkout).

The orchestrator owns the contract it **provides** and puts its types in `game-orchestrator-api`. The contract it **consumes** from arena-manager is owned by arena-manager (`../../../../arena-manager/specs/001-arena-manager/contracts/service.md`) and is never copied here.

## Provided (needs of stats-visualizer)

These match the proposal in `../../../../stats-visualizer/specs/001-stats-visualizer/contracts/required-orchestrator.md`.

| Need | Shape (draft) |
| --- | --- |
| List of game types | `game_types(&self, server: Server<'_>) -> ApiVec<ApiString>`: stable names, as in `gametypes.toml`. |
| Result listener | Registration of a callback receiving one `MatchResult` per finished match. |
| Result payload | `MatchResult { game_type, participants: [{ uuid, name, outcome }] }` with `outcome: Won | Lost | Draw` (O5). Stats' proposal has `won: bool`; the draw state needs a decision (counts as neither, proposed). |

Properties: a result is delivered once per match with every participant once; delivery is on the tick thread; game type names are stable across restarts.

## Consumed (from arena-manager)

| Need | Notes |
| --- | --- |
| Start a match | Input: game type, players, **a kit per player** (R9; arena-manager's draft assumes one kit per request). Output: a match id or a synchronous rejection (unknown game type, no complete arena for N, player offline or in a match, no capacity, template not ready). |
| Result | One result per finished match with an outcome per participant. |
| Failure | A match that failed before running is reported with a message, not as won or lost. |
| List matches | Used by `/orchestrator status`. |

How the orchestrator learns about results (push to a registered listener versus pulling an event log) is O2 and shared with arena-manager's open points.

## Rules for callers

- Calls run on the tick thread and never block (spec FR-013).
- A capacity rejection is retried by the orchestrator; other callers must not rely on a queue in arena-manager (its D7).
- A consumer links to this contract by sibling path and never copies it.

## Dependency note (D12, O2)

The host rejects dependency cycles, soft ones included (R5). Arena-manager and the orchestrator need each other, and arena-manager's D9 proposed a `game-registry` crate to break that. The user has deferred the decision; this draft assumes only that the two can reach each other's service at use time.

## Versioning

`/v1` changes only for incompatible layout changes; adding a method is `/v2`.
