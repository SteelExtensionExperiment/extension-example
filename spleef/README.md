# Spleef example extension

A minigame using the non-host extension API and the [Minigame room service](../minigame/README.md).
Spleef supplies the lobby and arena maps, commands, timers, and game rules.

Run these commands from the parent Steel checkout. Build it separately from the server:

```sh
cargo build --manifest-path extension-example/minigame/Cargo.toml --release
cargo build --manifest-path extension-example/spleef/Cargo.toml --release
```

Add both extension entries to `config/config.toml` (Linux paths shown):

```toml
[[extensions]]
path = "extension-example/minigame/target/release/libsteel_minigame_extension.so"

[[extensions]]
path = "extension-example/spleef/target/release/libsteel_spleef_extension.so"

[extensions.config]
lobby_seconds = 20
round_seconds = 180
ending_seconds = 5
players = 4
seed = 0
```

Use `steel_spleef_extension.dll` on Windows or `libsteel_spleef_extension.dylib` on macOS.
You can also package it from the Steel checkout:

```sh
python3 package-extension.py extension-example/minigame
python3 package-extension.py extension-example/spleef
```

This writes `extensions/steel-minigame-extension.zip` and
`extensions/steel-spleef-extension.zip`. To use the ZIPs, update both paths above
and keep `[extensions.config]` under the Spleef entry. Install one copy of each
extension. Steel loads Minigame before Spleef through the declared dependency.

Start the server with `cargo run`, then use:

- `/spleef join`: creates the room if needed and joins its predefined quartz lobby.
- `/spleef leave`: returns to your original domain and restores its saved state.
- `/spleef status`: reports the room phase and human-player count.

The command is available to players by default and respects explicit permission
denials for `spleef.command.spleef`. There is one room, supporting 2–8 contestants. Empty
slots fill with visible server-controlled players; additional humans replace bots
while the lobby countdown is running. Bots need no Minecraft accounts or clients
and do not consume client connection slots, so a solo server can keep `max_players = 1`.

After the countdown, the extension creates a fresh arena world in the same temporary
domain. Everyone receives a diamond shovel and survival mode. Mine snow blocks
under opponents; falling below the arena eliminates you and makes you a spectator.
The last contestant wins; the round time limit produces a draw if several remain.
Bots use normal server collision, gravity, tool reach, mining speed, and item drops.
Their simple AI walks on remaining snow and digs near opponents.

After the result, players return to their original domain, bots leave, and the
host retires the temporary domain and releases both worlds. Leaving an empty room
also cleans it up, including a lobby still being prepared. A new join creates a
new domain identity and untouched maps.

World chunks, level data, scoreboards, command storage, and minigame player state
stay in RAM. Normal player state is saved through the existing domain switch.
Disconnecting during a match reconnects you to your last lasting domain. Temporary
routes retain only retired identity markers so late async saves cannot write game
state to disk. Joining from another temporary domain is rejected.

Pre-init subscribes command, generator, and server-tick hooks. Registration callbacks
write directly to the corresponding live registries. Init resolves static materials
and configuration after all registries freeze. The command then requests worlds at
runtime; CPU work uses the existing generation pool, and publication/retirement use
the server job stage between world ticks. Countdown and AI pause during frozen ticks.

This example uses vanilla survival interactions, including player combat. Its maps
and game rules are intentional minigame behavior; normal worlds are unchanged.

See the [example index](../README.MD) for shared checkout and packaging setup.

Validation:

```sh
cargo test --manifest-path extension-example/spleef/Cargo.toml
cargo build --manifest-path extension-example/minigame/Cargo.toml
cargo build --manifest-path extension-example/spleef/Cargo.toml
cargo test -p steel-extension-api --features host --test spleef -- --include-ignored
```
