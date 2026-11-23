# game-orchestrator

Steel extension: Game orchestrator.

**Status:** planning. Nothing is implemented. The spec is in [`specs/001-game-orchestrator/`](specs/001-game-orchestrator/spec.md). The connection to arena-manager and stats-visualizer is not fixed yet (open point O2); cycles are accepted in the plan.

## Layout

This extension lives in `extension-example/` inside a Steel checkout (`SteelExtensionMC/extension-example/game-orchestrator/`), so its crates can reach
the API through `../../steel-extension-api`. Cross-extension contracts are referenced by sibling path,
for example `../world-cloner-api`. See `specs/extensions-workflow.md` in the Steel
checkout for the ownership rules.
