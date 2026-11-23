# stats-visualizer

Steel extension: floating scoreboards (holograms) that show the top 5 players by wins per game type,
with their W/L ratio. Results and game types come from the `game-orchestrator` extension. Stats and
board locations are stored in the extension data directory.

**Status:** planning. Nothing is implemented. The spec is in
[`specs/001-stats-visualizer/`](specs/001-stats-visualizer/spec.md); it needs the host spec
`specs/006-text-holograms/` (Steel checkout) and the orchestrator contract it proposes in
[`contracts/required-orchestrator.md`](specs/001-stats-visualizer/contracts/required-orchestrator.md).

## Layout

This extension lives in `extension-example/` inside a Steel checkout (`SteelExtensionMC/extension-example/stats-visualizer/`), so its crates can reach
the API through `../../steel-extension-api`. Cross-extension contracts are referenced by sibling path,
for example `../world-cloner-api`. See `specs/extensions-workflow.md` in the Steel
checkout for the ownership rules.
