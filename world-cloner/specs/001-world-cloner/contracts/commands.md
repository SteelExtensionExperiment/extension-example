# Contract: commands

One command tree registered by the `world-cloner` extension with `CommandBuilder`, in the style of
`extension-example/worldedit/src/lib.rs:359-362`. Each subcommand has its own permission node and is
denied without it, from players and console alike where the host's permission rules apply.

| Syntax | Permission | Effect | Feedback |
| --- | --- | --- | --- |
| `/worldcloner templates` | `worldcloner.templates` | Lists every configured template with its state and, for Failed, the message. | One line per template. |
| `/worldcloner clone <template> <key> <new\|existing>` | `worldcloner.clone` | Clones the template as `<key>`. `new` makes a new temporary domain named by the key's domain part; `existing` adds to a loaded domain. | "Cloning…", then "World `<key>` is ready" or the failure. |
| `/worldcloner create <generator> <key> <new\|existing>` | `worldcloner.create` | Creates a generated RAM world; its seed, spawn, game mode and difficulty come from the `[create]` section of `config.toml` (defaults in the data model). | Same as `clone`. |
| `/worldcloner remove <key>` | `worldcloner.remove` | Removes one runtime world; players inside are moved out. | "Removing…", then "World `<key>` removed" or the failure. |

## Rules

- `<template>` is a configured name. `<key>` is `domain:world`.
- Completion is reported to the player who ran the command with `Server::message`, and always logged.
  When the sender is not a player, only the log shows the result.
- Argument completion suggests template names for `<template>`; no other suggestions are required.
- Optional override flags (seed, spawn, game mode, difficulty) are not part of this iteration; the
  service carries them for consumers.

## Errors

| Condition | Message |
| --- | --- |
| Unknown template name | `unknown template <name>` |
| Template not Ready | The template's state and, for Failed, its message. |
| Host refusal (key in use, domain missing, not removable, …) | The host's diagnostic, unchanged. |
| Operation failed later | The operation's diagnostic. |
| Missing permission | The host's standard denial. |
