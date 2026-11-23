# Research: Arena Manager extension

File:line references are relative to the Steel checkout (`SteelExtensionMC/`), checked on 2026-11-23 (`master` at `55a3fb29`). Re-verify before implementing. `minecraft-src/` is missing in this checkout, so no vanilla claim is verified here; this extension has no vanilla logic beyond what the host already does.

## R1. Host capabilities already present

**Finding.** Existing `Server<'_>` bindings cover what the extension needs:

| Need | Binding | Source |
| --- | --- | --- |
| Move to another domain | `switch_domain(player, domain)`, queued, runs through the save/restore pipeline | `steel-extension-api/src/runtime/bindings.rs:153`, `host/runtime.rs:160` |
| Move within a domain | `teleport_to(player, Location)`; the destination world must be loaded and in the player's domain | `players.rs:154`, `host/players.rs:69-72` |
| Select a world | `select_world(player, key)` | `runtime/bindings.rs:159` |
| Game mode | `set_game_mode(player, mode)` for a resident player | `runtime/bindings.rs:171` |
| Clear inventory | `clear_inventory(player)`, current domain inventory | `runtime/bindings.rs:177`, `doc/extensions/api.md:476` |
| Give items | `give_item(player, item, count, name)`; slot read/write in `items.rs:207-235` | `players.rs:321` |
| Countdown | `send_action_bar`, `send_title(player, title, subtitle, fade_in, stay, fade_out)` | `players.rs:235-260` |
| Player state | `players()` (resident only), `is_connected`, `player_ready` | `runtime/bindings.rs:137-151` |
| Events | `subscribe_death`, `subscribe_quit`, `subscribe_join`, `subscribe_teleport` | `events/mod.rs:293-409` |
| Domain destroy | `destroy_temporary_domain(domain)` returns an `Operation` | `runtime/bindings.rs:123` |

**Decision.** No host change is needed for the arena logic itself. Open items are R4, R6 and R7.

## R2. World creation and removal

**Finding.** Worlds come from world-cloner (`world-cloner:clone/v1`), which wraps spec 005. `clone_world` with `NewTemporaryDomain` makes the clone the default world of a new, never-used domain. `remove_world` relocates players inside, leaves the domain registered, and a temporary domain with no default world refuses `switch_domain` (`specs/005-world-templates/contracts/host-api.md`, `research.md:240-241`).

**Decision.** One new domain per world (O4) keeps removal simple: removing the world and destroying its domain are two steps, and a domain never outlives its only world.

## R3. Services and dependencies

**Finding.**

- A service is registered with `services::register` and looked up with `services::get` **at use time**, not at init (`steel-extension-api/src/services.rs:5-9,95`). A consumer need not declare `depends` to look one up, but only a declared dependency guarantees initialization order.
- A dependency cycle is an error even when one edge is soft (`host/lifecycle.rs:366-368`), and registrations cannot be rolled back (`host/mod.rs:393`).
- Dependencies are declared in the long `export_extension!` form, not `extension.toml` (`steel-extension-api/src/lib.rs:221-264`, `host/package.rs:15-26`).

**Decision.** Arena-manager `depends: ["world-cloner"]`. It never depends on the orchestrator; the orchestrator `depends: ["arena-manager"]`. Results use hook actions, which need no dependency and cannot form a cycle. Emit actions after dropping own locks.

## R4. Moving a player into a match

**Finding.** `switch_domain` is queued, not immediate. `players()` lists only players "actually resident in their world", `player_ready` reports "finished relocation and client loading", and `set_game_mode` works on a "resident player". So after `switch_domain` the player is not ready until a later tick.

**Decision.** The match runs a per-player step machine on the server tick: `switch_domain` (when the player is not already in the match domain), then wait for `player_ready` or `!is_connected`, then `teleport_to` the slot spawn (a `Location` with the match world key), then `set_game_mode`, `clear_inventory`, kit items. A player who is not ready within a timeout, or who disconnects, cancels the match (O9). The timeout is a configuration value, not a literal.

**Unverified:** whether `teleport_to` right after the world is ready needs the destination chunk loaded (`teleport` states that it must be; `teleport_to` does not say). Check before T010.

## R5. Inventory per domain

**Finding.** `clear_inventory` clears "the current domain inventory" and temporary domains use temporary player-state routing (`runtime/bindings.rs:123-135`, `doc/extensions/api.md:476`).

**Decision.** The match domain's inventory is separate from the lobby domain's, so clearing and giving the kit does not touch the lobby inventory, and leaving the match domain restores the lobby one. **Unverified** for the exact behavior on return; the quickstart checks it (step 6). The user chose "no inventory handling" on return; if the check fails it becomes a platform gap.

## R6. Death and the respawn screen (O3)

**Finding.** `DeathEvent` exposes the player, location and death message; it cannot cancel death (`events/mod.rs:807-840`). A `RespawnEvent` type exists (`events/mod.rs:33`). Respawn in a temporary domain uses the domain's default world (`specs/005-world-templates/research.md:250`, `player_lifecycle.rs:517-564`).

**Decision (proposed, unverified).** On `DeathEvent`, mark the player lost. Move them to the lobby only once they have respawned (the respawn lands them in the match domain's default world, the cloned map). Unknown: whether a `switch_domain` is accepted while the death screen is shown, and whether a respawn subscription exists in the extension API (the grep shows `subscribe_join/quit/teleport/death` only). If neither is possible, this is a **platform gap** and needs a host spec (next free number is 007); do not work around it.

## R7. World and domain cleanup

**Finding.** `remove_world` on the last world leaves the domain registered; `destroy_temporary_domain` "rejects new entries and retires the domain after everyone has left" (`runtime/bindings.rs:121-123`). Whether it also retires the domain's remaining worlds is not stated.

**Decision.** Call world-cloner `remove_world`, poll the `Operation` to completion, then `destroy_temporary_domain` and poll that. **Verify** in `specs/005-world-templates/research.md:200-240` and the implementation which of the two is needed, or both, before T012.

## R8. Draws and ordering

**Finding.** Handlers for different players can run concurrently and death runs on a world thread (`events/mod.rs:7`). Within one server tick, order between two players' deaths is not defined.

**Decision.** Death and quit handlers only push `(player, kind)` into a `SyncMutex<Vec<_>>` queue. The server tick callback drains the queue once and resolves the match: if every remaining player is in the drained set, the result is a draw. This makes "simultaneous" mean "same tick" and removes order dependence.

## R9. Persistence

**Finding.** Config is `extensions/<name>/config.toml` and `data_directory()` is an existing absolute directory (`host/mod.rs:315-329`, `bindings.rs:158-162`). The extension API forbids neither threads nor file writes.

**Decision.** `arenas.toml` in the data directory holds the lobby and all arenas, written to a temp file and renamed on each command that changes it (O8). It is tiny and written only on admin commands.

## R10. Test strategy (O1)

Same situation as world-cloner research R2: the native harness is a test module in `steel-extension-api` and cannot be imported. The plan assumes unit tests beside the code for the pure logic (allocation, match state machine, persistence, commands), plus the manual check in [quickstart.md](quickstart.md). Decision needed before the Polish phase.

## R11. Risks and open points

1. **O2**: decided (actions); the kit field waits for host spec 007.
2. **R6**: moving dead players may need a host change.
3. **R4**: teleport timing after a domain switch.
4. **R7**: domain destruction mechanics.
5. **Operation leaks**: every world-cloner and domain `Operation` must be polled to completion.
6. **Platform timing**: world-cloner needs spec 005 tasks T012 and T018 merged.
