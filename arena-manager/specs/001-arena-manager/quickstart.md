# Quickstart: Arena Manager extension

Run from the **Steel checkout** (`SteelExtensionMC/`), where this extension sits at `extension-example/arena-manager/`.

## Prerequisites

- world-cloner built and its host platform merged ([plan.md](plan.md#platform-dependencies)), with one template configured, for example `arena_a`.
- The service match flow also needs the registry and orchestrator contract (O2); until then steps 1-5 (arenas) can be checked, and the match steps need a temporary test caller that the user approves.

## Automated checks

```sh
cargo test --manifest-path extension-example/arena-manager-api/Cargo.toml
cargo test --manifest-path extension-example/arena-manager/Cargo.toml
cargo clippy --manifest-path extension-example/arena-manager/Cargo.toml
cargo fmt --manifest-path extension-example/arena-manager/Cargo.toml --check
typos extension-example/arena-manager extension-example/arena-manager-api
```

A native test for this extension is O1 (research R10).

## Install by hand

```sh
python3 package-extension.py extension-example/arena-manager
python3 package-extension.py extension-example/world-cloner
```

Create `extensions/arena-manager/config.toml`:

```toml
max_worlds = 2
winner_return_delay_seconds = 10
```

Restart Steel after building or changing configuration. Native extensions stay loaded until exit.

## Manual check in a real client

1. `/op` yourself. `/worldcloner clone arena_a edit:map new`, then `/domain edit:map`.
2. `/arena create skywars north`, stand at the first start position, `/arena spawn set skywars north 1`, move, `/arena spawn set skywars north 2`.
3. `/arena template skywars arena_a`. `/arena list` shows `skywars/north` complete with 2 slots.
4. Go to the real lobby, `/arena lobby set adventure`.
5. Restart the server, `/arena list` and check the arena and lobby are still there.
6. With two players and a match request (see Prerequisites): both are moved to a new world, stand on their own spawn, have the kit's game mode, an empty inventory plus the kit. A third request while `max_worlds` is reached and no arena is free is rejected with "no capacity". Check the lobby inventory is untouched after returning (research R5).
7. Kill one player: that player is in the lobby with the lobby game mode. The survivor sees a countdown from 10 and then lands in the lobby. One result is delivered (once O2 is decided).
8. After the match, `/arena matches` is empty and the world and domain are gone (`/domain` no longer lists the match domain).
9. Both players die in the same tick (for example a lava arena): the result is a draw.
10. One player disconnects mid-match: they count as lost and the other wins.

## Failure checks

- `/arena spawn set` from the console: denied (needs a player).
- A corrupt `arenas.toml`: the extension refuses to start with the file and cause in the log.
- A game type whose template is missing or Failed: the request is rejected with the template message.
- A player without `arena.create` runs `/arena create ...`: denied, nothing changes.
