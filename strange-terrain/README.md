# Strange terrain

Demonstrates registration of a deterministic terrain-only generator. The
`example:strange` generator creates floating stone islands, hollow end-stone
shells, and twisting purpur pillars with glowstone. It uses overworld dimension
rules and an End biome, without structures or decorators.

This extension registers only the generator. It does not install world ticks or
replace block behavior, and it needs no extension configuration. Positional
seeded sampling chooses geometry offsets; integer shape evaluation keeps chunk
output independent of invocation order.

## Build and install

Run from the parent Steel checkout:

```sh
cargo build --manifest-path extension-example/strange-terrain/Cargo.toml --release
```

Add this top-level entry to `config/config.toml` (Linux shown):

```toml
[[extensions]]
path = "extension-example/strange-terrain/target/release/libsteel_strange_terrain_extension.so"
```

Use `libsteel_strange_terrain_extension.dylib` on macOS or
`steel_strange_terrain_extension.dll` on Windows. For a new world, select the
registered generator in `config/worlds.toml`, for example:

```toml
[[domains.minecraft.worlds]]
name = "strange"
generator = "example:strange"
```

Add this entry to an existing configured `minecraft` domain. Existing worlds
continue to use their selected generators; previously generated chunks are not
regenerated. Start the server with the extension loaded before world creation.

To package it instead:

```sh
python3 package-extension.py extension-example/strange-terrain
```

This writes `extensions/steel-strange-terrain-extension.zip`, which Steel
discovers automatically. Keep the world generator selection above. Select
either the ZIP or the raw library and restart after rebuilding. See the
[example index](../README.MD) for shared setup.

## Validate

```sh
cargo test --manifest-path extension-example/strange-terrain/Cargo.toml
cargo build --manifest-path extension-example/strange-terrain/Cargo.toml
cargo test --profile cicd -p steel-extension-api --features host --test strange_terrain -- --include-ignored
```

Unit tests cover hollow geometry and seed-dependent variation. The native test
loads this library independently, resolves its generator, and verifies that
unexpected per-world generator configuration is rejected.
