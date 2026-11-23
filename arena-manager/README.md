# arena-manager

Steel extension: Arena manager.

**Status:** planning. Nothing is implemented. The spec is in [`specs/001-arena-manager/`](specs/001-arena-manager/spec.md). The contract with the game registry and orchestrator is not fixed yet (open point O2).

## Layout

This extension lives in `extension-example/` inside a Steel checkout (`SteelExtensionMC/extension-example/arena-manager/`), so its crates can reach
the API through `../../steel-extension-api`. Cross-extension contracts are referenced by sibling path,
for example `../world-cloner-api`. See `specs/extensions-workflow.md` in the Steel
checkout for the ownership rules.
