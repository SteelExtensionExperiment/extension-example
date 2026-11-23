# game-orchestrator

Steel extension: Game orchestrator.

**Status:** in progress. Game types, queues, access control, kit choice, `/queue`, `/orchestrator`, persistence, and result forwarding (`MatchResult`) work. Spec: [`specs/001-game-orchestrator/`](specs/001-game-orchestrator/spec.md).

**Waiting on other work:** forming matches from full queues and `/orchestrator match` need a kit per player in arena-manager's `PlayerEntry`. Everything else in the commands contract works, including `kit save|load` (host `InventorySnapshot`) and sign registration from sign text.
