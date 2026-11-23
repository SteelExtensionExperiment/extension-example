# Quickstart: World Cloner extension

Run from the **Steel checkout** (`SteelExtensionMC/`), where this extension sits at `extension-example/world-cloner/`.

## Prerequisites

- The platform tasks this extension waits for are merged into the checkout you build from
  ([plan.md](plan.md#platform-dependencies)).
- A saved Steel world to use as a template: any world folder written by this Steel version, containing
  `region/` and `level.toml`. For a quick one, start a server with an empty `steel:empty` world, build
  something, stop it, and copy `saves/<domain>/worlds/<name>/` somewhere.

## Automated checks

```sh
cargo test --manifest-path extension-example/world-cloner-api/Cargo.toml
cargo test --manifest-path extension-example/world-cloner/Cargo.toml
cargo clippy --manifest-path extension-example/world-cloner/Cargo.toml
cargo fmt --manifest-path extension-example/world-cloner/Cargo.toml --check
typos extension-example/world-cloner extension-example/world-cloner-api
```

The host side has its own checks in `specs/005-world-templates/quickstart.md`. A native test for this
extension is decision O1 (research R2).

## Install by hand

```sh
python3 package-extension.py extension-example/world-cloner
mkdir -p extensions/world-cloner/templates
cp -r /path/to/saved-world extensions/world-cloner/templates/arena_a
```

Create `extensions/world-cloner/config.toml`:

```toml
[[templates]]
name = "arena_a"
path = "templates/arena_a"
default_game_mode = "adventure"
```

Restart Steel after building or changing configuration. Native extensions stay loaded until exit.

## Manual check in a real client

1. Start Steel and `/op` yourself from the console. The log shows the template becoming ready (and a
   clear error for any template that failed). The template folder is not modified.
2. `/worldcloner templates` lists `arena_a` as Ready.
3. `/worldcloner clone arena_a game_1:map new`. You are told when it is ready. Enter it with
   `/domain game_1:map` and check blocks and chests. Animals and armor stands from the original are not
   there.
4. `/worldcloner clone arena_a game_2:map new`, enter it with `/domain game_2:map`, break a block in
   `game_1:map`, and check `game_2:map` is unchanged. Day and night advance independently in both.
5. With a lasting domain configured, `/worldcloner clone arena_a lobby:arena_1 existing` adds a world to
   it; restart the server and check `lobby:arena_1` is gone while the domain's data is intact.
6. Stand in `game_1:map` and run `/worldcloner remove game_1:map`: you are moved to the domain's default
   world, or the server default world when there is none. `remove` on a world from `worlds.toml` is
   refused.
7. `/worldcloner create <generator> game_3:map new` with a registered generator makes a generated RAM
   world with the same lifecycle as a clone.
8. Compare the template folder with the original copy: identical files and modification times.

## Failure checks

- A template path that does not exist: the server starts, the log names the path, and the template shows
  Failed. Other templates still clone.
- A template folder written by a different Steel format version: Failed, and the folder is untouched (no
  `*.bak` file appears).
- An invalid configuration (duplicate name, `..` in a path): the extension refuses to start with a
  message naming the entry.
- A player without `worldcloner.clone` runs `/worldcloner clone ...`: denied, nothing changes.
