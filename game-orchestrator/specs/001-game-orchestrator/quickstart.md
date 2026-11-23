# Quickstart: Game Orchestrator extension

Run from the **Steel checkout** (`SteelExtensionMC/`), where this extension sits at `extension-example/game-orchestrator/`.

## Prerequisites

- The extension built and installed. Steps 1-6 need nothing else.
- Steps 7-10 need arena-manager and the connection decided in O2; until then they cannot be checked and need a temporary test caller that the user approves.
- Kit steps (3-4) need O1 decided.

## Automated checks

```sh
cargo test --manifest-path extension-example/game-orchestrator-api/Cargo.toml
cargo test --manifest-path extension-example/game-orchestrator/Cargo.toml
cargo clippy --manifest-path extension-example/game-orchestrator/Cargo.toml
cargo fmt --manifest-path extension-example/game-orchestrator/Cargo.toml --check
typos extension-example/game-orchestrator extension-example/game-orchestrator-api
```

A native test for this extension is O10 (research R10).

## Install by hand

```sh
python3 package-extension.py extension-example/game-orchestrator
```

Optional `extensions/game-orchestrator/config.toml`:

```toml
retry_interval_ticks = 40
max_session_hours = 24
```

Restart Steel after building or changing configuration. Native extensions stay loaded until exit.

## Manual check in a real client (two players, A is an administrator)

1. `/orchestrator gametype create skywars 2`, `/orchestrator gametype list`: shows `skywars`, N=2, enabled, no kits.
2. Restart; `gametype list` shows it again.
3. (O1) Fill A's inventory, `/orchestrator kit save skywars knight`, clear the inventory, `/orchestrator kit load skywars knight`: the inventory returns. Change an item with `/invsee <helper>` on a helper wearing the kit, save again.
4. (O1) Add a second kit `archer`.
5. Place a sign, write `[Kit]`, `skywars`, `knight`: it is rewritten and registered. Write `[Queue]`, `skywars` on another. `[Queue]`, `nothing` is refused with a message.
6. `/orchestrator open 1`; `/orchestrator status` shows a closing time. B right-clicks the queue sign without a kit: warned. B right-clicks the kit sign, then the queue sign: queued at 1. `/queue` shows it. B joins another game type: moved. `/queue leave`: gone. `/orchestrator close`: a join is refused.
7. (O2) Reopen. A and B choose kits and join: one match request with two kits; the queue is empty; both are in a match and cannot queue.
8. (O2) Disable `skywars`, then join: refused. Re-enable.
9. (O2) With arena-manager at capacity: the players stay queued at the front and the request is retried.
10. (O2) End the match: one result is delivered to a test listener; the players can queue again. `/orchestrator match skywars A B` forms a match without a queue.

## Failure checks

- Corrupt `gametypes.toml`: the extension refuses to start and the message names the file.
- Join with no permission for `orchestrator.queue` revoked: denied. Any `/orchestrator` subcommand without permission: denied.
- Quit while queued: gone from `/orchestrator queues`.
