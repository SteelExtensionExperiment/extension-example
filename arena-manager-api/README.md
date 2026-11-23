# arena-manager-api

Contract of the `arena-manager` extension. Calls go down through the service
`arena-manager:match/v1` (`start_match`, `matches`, `game_types`, `can_host`); results go up as the
hook actions `MatchFinished` and `MatchFailed`, so listeners need no dependency on arena-manager.

`PlayerEntry` has no kit field yet. It is added with host spec 007 (item snapshot), so `/v1` is not
final until then. A published contract changes only with the user's approval.
Details: `arena-manager/specs/001-arena-manager/contracts/service.md`.
