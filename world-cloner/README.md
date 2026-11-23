# world-cloner

Steel extension: load saved Steel worlds into memory once and stamp out unlimited RAM-only copies as
runtime worlds, for minigame servers. Exposes the versioned service `world-cloner:clone/v1` (crate
[`world-cloner-api`](../world-cloner-api)) and `/worldcloner` admin commands.

**Status:** the logic is complete and unit-tested against a small host interface (`HostWorlds`).
It is not registered yet: the implementation for the host `Server`, service registration, command
registration and tick wiring wait for host spec `005-world-templates` (see `specs/001-world-cloner/plan.md`,
"Platform gap"). `world-cloner-api` defines the missing host types as prototype stand-ins.

## Layout

This extension lives in `extension-example/` inside a Steel checkout, so the crates reach the API
through `../../steel-extension-api`. The contract crate is a sibling, as with `vault` and `vault-api`:

```text
SteelExtensionMC/
├── steel-extension-api/
├── specs/                      # host platform specs and specs/extensions-workflow.md
└── extension-example/
    ├── world-cloner-api/       # contract crate
    └── world-cloner/           # extension crate
        └── specs/001-world-cloner/
```

Build with the Steel checkout's `rust-toolchain.toml`; Cargo finds it from here.

## Configuration

`extensions/world-cloner/config.toml`:

```toml
[[templates]]
name = "skywars_a"               # unique; letters, digits, '_' and '-'
path = "templates/skywars_a"     # relative to the extension data directory; must not escape it
default_game_mode = "survival"   # survival | creative | adventure | spectator

[create]                         # optional; settings for `/worldcloner create` only
seed = 0
spawn = [0, 64, 0]
default_game_mode = "survival"
difficulty = "normal"            # peaceful | easy | normal | hard
```

An invalid entry stops the extension from starting, with a message naming it.

## Service

Other extensions call `world_cloner_api::cloner()` at use time and import `WorldClonerDyn as _`.
Every `Operation` a method returns belongs to the caller: poll it with `Server::operation_status` on
later ticks until it is `Ready` or `Failed`. Unread results stay in the host table for the life of
the process.

## Commands

| Command | Permission |
| --- | --- |
| `/worldcloner templates` | `worldcloner.templates` |
| `/worldcloner clone <template> <key> <new\|existing>` | `worldcloner.clone` |
| `/worldcloner create <generator> <key> <new\|existing>` | `worldcloner.create` |
| `/worldcloner remove <key>` | `worldcloner.remove` |

`<key>` is `domain:world`. Results are sent to the sender and logged.
