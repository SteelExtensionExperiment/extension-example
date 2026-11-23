# arena-manager-api

Contract of the `arena-manager` extension. Calls go down through the service
`arena-manager:match/v1` (`start_match`, `matches`, `game_types`, `can_host`); results go up as the
hook actions `MatchFinished` and `MatchFailed`, so listeners need no dependency on arena-manager.

`PlayerEntry` carries the kit as an `InventorySnapshot` (host spec 007). `/v1` is final.
Details: `arena-manager/specs/001-arena-manager/contracts/service.md`.
