# Wool shearing

Demonstrates block behavior replacement and delegation. Right-click a full wool
block with shears to replace it with oak wood and drop one item of the original
wool color. Other interactions delegate to the captured behavior. Drops respect
the block-drops game rule; survival consumes one durability and creative follows
the host's infinite-materials rule.

This extension registers only block behavior. It needs no configuration and does
not install world ticks or a generator.

## Build and install

Run from the parent Steel checkout:

```sh
cargo build --manifest-path extension-example/wool-shearing/Cargo.toml --release
```

Add this top-level entry to `config/config.toml` (Linux shown):

```toml
[[extensions]]
path = "extension-example/wool-shearing/target/release/libsteel_wool_shearing_extension.so"
```

Use `libsteel_wool_shearing_extension.dylib` on macOS or
`steel_wool_shearing_extension.dll` on Windows. Start the server, connect, and use
shears on wool in a loaded world.

To package it instead:

```sh
python3 package-extension.py extension-example/wool-shearing
```

This writes `extensions/steel-wool-shearing-extension.zip`, which Steel discovers
automatically. Select either the ZIP or the raw library; remove an unused ZIP
from `./extensions` to avoid loading the same extension twice. Restart after
rebuilding. See the [example index](../README.MD) for shared setup.

## Validate

The native test checks replacement and captured-behavior delegation using a
separately compiled library. It expects a debug build:

```sh
cargo build --manifest-path extension-example/wool-shearing/Cargo.toml
cargo test --profile cicd -p steel-extension-api --features host --test native -- --include-ignored
```
