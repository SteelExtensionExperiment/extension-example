# Contract: commands

One command tree registered with `CommandBuilder`, in the style of
`extension-example/worldedit/src/lib.rs:359-362`. Each subcommand has its own permission node and is
denied without it.

| Syntax | Permission | Effect | Feedback |
| --- | --- | --- | --- |
| `/statsboard place <id> <gametype>` | `statsboard.place` | Places a board at the sender's position in the sender's world. Player only. | "Board `<id>` placed", or the failure. |
| `/statsboard place <id> <gametype> <x> <y> <z> [<world>]` | `statsboard.place` | Places a board at the coordinates. `<world>` defaults to the sender's world; the console must give it. | Same. |
| `/statsboard remove <id>` | `statsboard.remove` | Removes the board and its hologram and drops it from storage. | "Board `<id>` removed", or the failure. |
| `/statsboard list` | `statsboard.list` | Lists every board: id, game type, world, position, and `inactive` or `unknown game type` where it applies. | One line per board. |

## Rules

- `<id>` is letters, digits, `_` and `-`, unique among boards.
- `<gametype>` must be listed by the orchestrator at the moment of the command.
- `<x> <y> <z>` are numbers; the world key is `domain:world`. The sender's own world is the default.
- A board is saved immediately on `place` and `remove` (D4).
- Completion: orchestrator game types for `<gametype>`; existing ids for `remove`.

## Errors

| Condition | Message |
| --- | --- |
| Unknown game type | `unknown game type <name>` |
| Id in use | `board <id> already exists` |
| Unknown id | `unknown board <id>` |
| Console without coordinates | `the console must give <x> <y> <z> <world>` |
| Unknown or unloaded world | `unknown world <key>` |
| Hologram creation refused by the host | The host's diagnostic, unchanged. |
| Missing permission | The host's standard denial. |
