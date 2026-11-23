# Research: Game Orchestrator extension

Findings verified on 2026-11-23 against the Steel checkout. `minecraft-src/` is missing, so nothing here claims vanilla behavior (R8).

## R1. The extension item snapshot is lossy (blocks kits, O1)

`ItemSpec` carries only: item key, display name, lore, vanilla enchantments, extension data entries, count, glint override and unbreakable (`steel-extension-api/src/items.rs:55-70`). The module doc states: "Reading a stack returns only the fields above; writing a spec replaces the whole stack" (`items.rs:9-10`). A kit built from an administrator's inventory would therefore lose other components (potion contents, stored enchantments of books, custom model data, attribute modifiers, banner patterns, fireworks, container contents and similar).

**Consequence:** the stored kit may differ from what the administrator sees in `/invsee`. Options for the user: (a) accept the loss and document which item types are safe; (b) specify a host capability that snapshots and restores a full item stack as an opaque value (host spec 007). Not decided. Not stubbed.

## R2. Slots, game mode and inventory access exist

- Slots: `Server::slot`, `set_slot`, `give`, with vanilla indices 0-8 hotbar, 9-35 main, 36-39 armor (feet to head), 40 off hand (`items.rs:13-14, 206-241`).
- Game mode read: `Server::game_mode` (`players.rs:202`); set: `set_game_mode` (`runtime/bindings.rs:171`); clear: `clear_inventory` (`runtime/bindings.rs:177`).
- Chat, action bar and title: `send_message`, `send_action_bar`, `send_title`, `broadcast` (`players.rs:229-264`).
- Lookup: `player_by_name`, `player_by_uuid`, `player_name` (`players.rs:123-135`).
- Permissions with a default: `has_permission(player, node, default)` (`players.rs:278`).

## R3. Signs: edit event and interact event exist; reading a sign block does not (open)

- `SignChangeEvent` gives player, position, front/back, four lines, `set_line`, cancellation (`events/more.rs:66-68, 754-800`); subscription `subscribe_sign_change` (`events/handlers.rs:291-297`).
- `InteractEvent` gives player, world, action, hand, clicked block position and face, held item, cancellation (`events/mod.rs:695-741`); subscription `subscribe_interact` (`events/handlers.rs:141-146`).
- Not found: a binding to read the text of an existing sign, to check that a position is still a sign, or a block-break event for signs. The plan therefore persists the registry itself and validates by the click: a click on a registered position acts; a removed sign cannot be clicked. A stale entry stays in `signs.toml` until `/orchestrator sign remove` (O9). Whether a stale entry can cause a wrong action (a different block placed at that position) must be checked: the interact handler can compare against the block key if a block read exists, otherwise it cannot. **To verify before T012.**

## R4. Join, quit and death events exist

`subscribe_join`, `subscribe_quit`, `subscribe_death` (`events/handlers.rs:92, 100, 162`). Quit removes the player from the queue at once (spec edge cases).

## R5. Service pattern

`steel-extension-api/src/services.rs:78-109` (`register`, `get`, `providers`) is the pattern the contract will use; dependencies are declared in the long `export_extension!` form (`specs/extensions-workflow.md`, cross-extension contracts). Host rejects dependency cycles, soft ones included (`steel-extension-api/src/host/lifecycle.rs:366-368`, as recorded in arena-manager D9). Decided (D12, O2): no cycle; results travel as hook actions, which need no dependency.

## R6. Scheduler

`scheduler::run_later`, `run_repeating`, `cancel`, `current_tick` (`scheduler.rs:64-104`). Used for the session expiry and capacity retries; no polling loop on the tick.

## R7. `/invsee` is a host command, not an extension API

`steel-core/src/command/builtins/invsee.rs` opens a menu of the target player's inventory for the source player; modification needs `steel.command.invsee.modify` (`invsee.rs:19-20, 45-50, 53-74`). It is unavailable while either player switches domains (`invsee.rs:78-80`). The orchestrator does not call it. The kit edit flow is: `kit load` into the administrator's inventory, edit directly or with `/invsee`, `kit save`.

## R8. Vanilla reference

`minecraft-src/` is missing in this checkout; run `update-minecraft-src.sh`. This spec makes no vanilla gameplay claim.

## R9. Contract consequences for arena-manager

Arena-manager's draft says the kit "arrives inside each start-match request" (its D4). Here every player has their own kit, so the request must carry a kit per player. Arena-manager's draft `service.md` and its `MatchResult` outcome (won, lost, draw) are drafts (its O2), so this is recorded as input for O2, not as a change to its files.

## R10. Native test location

Open (O10), as in the other extensions: the shared native-test helper cannot be imported from outside `steel-extension-api` (`specs/extensions-workflow.md`, "Native tests").
