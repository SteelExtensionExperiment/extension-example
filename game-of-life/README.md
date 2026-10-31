# Game of Life

Demonstrates a world-tick callback with extension-owned configuration. It runs
Conway's Game of Life on one X/Z board: black concrete is alive, every other
block is dead, and dying cells become white concrete. Each generation uses a
complete snapshot, with dead boundaries rather than wrapping edges.

This extension installs only a world-tick callback. It does not replace block
behaviors or register generators. Updates pause during frozen ticks and whenever
any board chunk is unavailable. Width and depth must each be 1–128 cells, and
the interval must be positive.

## Build and install

Run from the parent Steel checkout:

```sh
cargo build --manifest-path extension-example/game-of-life/Cargo.toml --release
```

Add this top-level entry to `config/config.toml` (Linux shown):

```toml
[[extensions]]
path = "extension-example/game-of-life/target/release/libsteel_game_of_life_extension.so"

[extensions.config.life]
world = "minecraft:overworld"
x = 0
y = 96
z = 0
width = 32
depth = 32
interval = 10
```

Use `libsteel_game_of_life_extension.dylib` on macOS or
`steel_game_of_life_extension.dll` on Windows. Choose a loaded world key and a Y
coordinate within its build bounds. The board extends from `(x, y, z)` through
`(x + width - 1, y, z + depth - 1)`. The interval is measured in world ticks.

Start the server, keep all board chunks loaded, and place black concrete on that
plane to seed a pattern. A line of three live cells away from the boundary
alternates between horizontal and vertical lines on successive generations.

To package it instead:

```sh
python3 package-extension.py extension-example/game-of-life
```

This writes `extensions/steel-game-of-life-extension.zip`. Replace only the
configured `path` with that archive path and keep `[extensions.config.life]`.
Select either the ZIP or the raw library, and restart after rebuilding or
changing configuration. See the [example index](../README.MD) for shared setup.

## Validate

```sh
cargo test --manifest-path extension-example/game-of-life/Cargo.toml
cargo build --manifest-path extension-example/game-of-life/Cargo.toml
cargo test --profile cicd -p steel-extension-api --features host --test game_of_life -- --include-ignored
```

Unit tests cover simultaneous generations, stable patterns, dead boundaries, and
chunk readiness across negative coordinates. The native test loads and
initializes this library independently of the other examples.
