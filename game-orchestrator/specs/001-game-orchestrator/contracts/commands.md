# Contract: commands

Two trees registered with `CommandBuilder` (pattern: `extension-example/worldedit/src/lib.rs:359-386`). Each subcommand has its own permission node and is denied without it. Player nodes default to allowed; administrator nodes default to denied.

## `/queue` (players)

| Syntax | Permission (default) | Effect |
| --- | --- | --- |
| `/queue join <gametype>` | `orchestrator.queue` (allow) | Joins the queue (spec D2-D3, FR-005). Moves the player if already queued elsewhere. Player only. |
| `/queue leave` | `orchestrator.queue` (allow) | Leaves the queue. |
| `/queue` | `orchestrator.queue` (allow) | Own queue, position, players needed, chosen kit; if not queued, enabled game types with queue sizes. |

## `/orchestrator` (administrators)

| Syntax | Permission (deny) | Effect |
| --- | --- | --- |
| `/orchestrator gametype create <name> <players>` | `orchestrator.gametype` | Creates a game type, enabled, no kits. `<players>` at least 2. |
| `/orchestrator gametype players <name> <players>` | `orchestrator.gametype` | Changes N; the queue is re-evaluated. |
| `/orchestrator gametype enable\|disable <name>` | `orchestrator.gametype` | Switches a game type. Disable clears its queue with a message. |
| `/orchestrator gametype remove <name>` | `orchestrator.gametype` | Removes the game type, its kits, signs and choices; clears its queue. |
| `/orchestrator gametype list` | `orchestrator.list` | Game types: N, enabled, kits, queue size. |
| `/orchestrator kit save <gametype> <kit>` | `orchestrator.kit` | Stores the sender's slots 0-40 and game mode as the kit, replacing an earlier one. Player only. **Waits for O1.** |
| `/orchestrator kit load <gametype> <kit>` | `orchestrator.kit` | Replaces the sender's inventory and game mode with the kit (O4). Player only. **Waits for O1.** |
| `/orchestrator kit remove <gametype> <kit>` | `orchestrator.kit` | Removes the kit; choices for it become invalid. |
| `/orchestrator kit list <gametype>` | `orchestrator.list` | Kits of the game type. |
| `/orchestrator open [<hours>]` | `orchestrator.session` | Opens queueing; with hours, closes it after that time (at most `max_session_hours`). Without hours it stays open (O7). |
| `/orchestrator close` | `orchestrator.session` | Closes queueing, clears every queue with a message. Running matches finish. |
| `/orchestrator status` | `orchestrator.list` | Open or closed, time left, game types, running matches. |
| `/orchestrator queues` | `orchestrator.list` | Each queue with player names in order. |
| `/orchestrator match <gametype> <player>...` | `orchestrator.match` | Requests a match on behalf of exactly N named players (O8). |
| `/orchestrator sign list` | `orchestrator.sign` | Registered signs with ids and positions (O9). |
| `/orchestrator sign remove <id>` | `orchestrator.sign` | Unregisters a sign (O9). |

## Signs (not commands)

An administrator edits a sign and writes, on the first lines:

```text
[Queue]            [Kit]
skywars            skywars
                   knight
```

Line 1 is the header, line 2 the game type, line 3 (kit signs) the kit. The sign-edit event validates the game type and kit; on success it registers the sign, rewrites the lines to a readable form (game type in a color for queue signs, kit name for kit signs) and replies; on failure it cancels the edit with a message. Permission to register: `orchestrator.sign`. Right-click: `orchestrator.queue`. The exact rewritten text is not contract.

## Rules

- `<gametype>` and `<kit>` completion come from the stored game types and kits.
- `<player>` accepts online players only.
- Every change to game types, kits, signs or state is saved at once and the reply says so (same as arena-manager O8: saving small files on the tick thread is accepted unless the user decides otherwise).
- Matches cannot be requested from the console except through `match` (`<player>` names online players).

## Errors

| Condition | Message |
| --- | --- |
| Queueing closed | `queueing is closed` |
| Game type unknown | `unknown game type <name>` |
| Game type disabled | `game type <name> is disabled` |
| No kit chosen | `choose a kit for <gametype> at a kit sign first` |
| In a match | `you are in a match` |
| Already queued, same game type | `you are already queued for <gametype>` |
| Game type exists | `game type <name> already exists` |
| Kit unknown | `unknown kit <gametype>/<kit>` |
| Wrong player count | `<gametype> needs exactly <n> players` |
| Non-player runs `kit save/load` or `queue join` | `this command needs a player` |
| Player not online | `player <name> is not online` |
| Missing permission | The host's standard denial. |
