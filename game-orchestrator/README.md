# game-orchestrator

Steel extension: Game orchestrator.

**Status:** in progress. Game types, queues, access control, kit choice, `/queue`, `/orchestrator`, persistence, and result forwarding (`MatchResult`) work. Spec: [`specs/001-game-orchestrator/`](specs/001-game-orchestrator/spec.md).

**Waiting on other work:**
- Forming matches from full queues, `kit save|load` and `/orchestrator match` need a kit per player in the match request (host spec 007).
- Registering signs from their text needs the world of a sign-edit event, which the host does not expose yet. Registered signs already work on click.

## Layout

This extension lives in `extension-example/` inside a Steel checkout (`SteelExtensionMC/extension-example/game-orchestrator/`), so its crates can reach
the API through `../../steel-extension-api`. Cross-extension contracts are referenced by sibling path,
for example `../world-cloner-api`. See `specs/extensions-workflow.md` in the Steel
checkout for the ownership rules.
