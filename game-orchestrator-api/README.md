# game-orchestrator-api

Contract of the `game-orchestrator` extension. The service `game-orchestrator:games/v1` lists game
types (`game_types`). Results go up as the hook action `MatchResult`
(`game-orchestrator:match_result/v1`), emitted once per finished match with every participant, so
consumers such as stats-visualizer link only this crate. `Outcome` is re-exported from
`arena-manager-api`.

Details: `game-orchestrator/specs/001-game-orchestrator/contracts/service.md`.
