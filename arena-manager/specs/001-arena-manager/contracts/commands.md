# Contract: commands

One `/arena` tree registered with `CommandBuilder` (pattern: `extension-example/worldedit/src/lib.rs:359-386`). Each subcommand has its own permission node and is denied without it. `<gametype>` completion and validation depend on the registry contract (O2, deferred); until then a name is accepted as typed and validated against the saved game types only.

| Syntax | Permission | Effect |
| --- | --- | --- |
| `/arena create <gametype> <arena>` | `arena.create` | Creates an empty arena for the game type (creating the game type entry when it is new and the registry allows it, O2). |
| `/arena remove <gametype> <arena>` | `arena.remove` | Removes the arena. Refused while a match uses it. |
| `/arena spawn set <gametype> <arena> <slot>` | `arena.spawn` | Stores the sender's current position, yaw and pitch as spawn `<slot>` (1 or higher), replacing an earlier one. Player only. |
| `/arena spawn remove <gametype> <arena> <slot>` | `arena.spawn` | Removes that slot. |
| `/arena spawn list <gametype> <arena>` | `arena.list` | Lists slots and positions. |
| `/arena template <gametype> <template>` | `arena.template` | Sets the world-cloner template the game type clones (O6, confirm). |
| `/arena list` | `arena.list` | Lists game types, arenas, slot count and complete or incomplete. |
| `/arena lobby set <gamemode>` | `arena.lobby` | Stores the sender's position, world and the game mode players get when they return. Player only. |
| `/arena matches` | `arena.matches` | Lists current matches (id, game type, arena, world, state, players alive). |

## Rules

- Spawn commands save the position with no world key (FR-002). The administrator stands in a copy of the map, for example one made with `/worldcloner clone`, so the coordinates match the template.
- Every change is saved at once (FR-005, O8) and the reply says so.
- `spawn set` on a slot that would leave a gap is allowed; the arena is then listed incomplete until the gap is filled.
- Matches cannot be started from the console; they come from the service (FR-014).

## Errors

| Condition | Message |
| --- | --- |
| Arena exists | `arena <name> already exists for <gametype>` |
| Unknown game type or arena | `unknown arena <gametype>/<arena>` |
| Arena in use | `arena <gametype>/<arena> has a running match` |
| Non-player runs `spawn set` or `lobby set` | `this command needs a player` |
| Unknown template **(O6)** | `unknown template <name>` |
| Missing permission | The host's standard denial. |
