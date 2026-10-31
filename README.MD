# Steel extension examples

Each crate demonstrates one feature or one complete game. The examples are
independent: loading wool shearing does not also enable Game of Life or install
a terrain generator. Spleef remains one application because its commands, maps,
bot controls, and world lifecycle implement the same minigame.

| Example | What it does | Instructions |
| --- | --- | --- |
| Wool shearing | Replace wool interaction while delegating other block behavior. | [Build and use](wool-shearing/README.md) |
| Game of Life | Advance a configured block board through a world-tick callback. | [Build and use](game-of-life/README.md) |
| Strange terrain | Register seeded floating-island, shell, and pillar terrain. | [Build and use](strange-terrain/README.md) |
| Spleef | Run a minigame with commands, temporary worlds, bots, and player restoration. | [Build and use](spleef/README.md) |
| Actions and filters | Run a custom score through ordered filters in two libraries, then observe an action. | [Build and use](hooks-and-filters/README.md) |

## Checkout and build layout

This is a separate Git repository, kept at `extension-example/` inside a Steel
checkout. Its crates depend on the sibling `steel-extension-api` source:

```text
SteelExtensionMC/
├── Cargo.toml
├── package-extension.py
├── steel-extension-api/
└── extension-example/
    ├── wool-shearing/
    ├── game-of-life/
    ├── strange-terrain/
    ├── spleef/
    └── hooks-and-filters/
```

If you cloned this repository on its own, place it in that layout before
building. Run the following commands from the parent Steel checkout, using its
`rust-toolchain.toml` and pinned Stabby dependency:

```sh
git submodule update --init --recursive
cargo build --manifest-path extension-example/wool-shearing/Cargo.toml --release
cargo build --manifest-path extension-example/game-of-life/Cargo.toml --release
cargo build --manifest-path extension-example/strange-terrain/Cargo.toml --release
cargo build --manifest-path extension-example/spleef/Cargo.toml --release
cargo build --manifest-path extension-example/hooks-and-filters/Cargo.toml --release
```

Examples use isolated workspaces so they build separately from the host and use
`steel-extension-api` without the `host` feature. The actions/filters example has
two extension crates and one shared contract in its extension-side workspace;
its host test has a separate workspace. Build only the examples you intend to
install. Native library names and full configuration appear in each README.

## Package and install

From the Steel checkout, Python 3.11 or newer can build a ZIP package:

```sh
python3 package-extension.py extension-example/wool-shearing
```

Use another crate directory to package a different example. The command defaults
to release mode; `--debug` selects a debug build, `--target` chooses a configured
Rust cross-compilation target, and `--output` chooses a ZIP destination.

For the actions/filters pair, package the individual
`extension-example/hooks-and-filters/base` and
`extension-example/hooks-and-filters/bonus` crate directories, as shown in its
[guide](hooks-and-filters/README.md#package).

The default destination is `extensions/<cargo-crate-name>.zip`. Steel discovers
ZIP packages in `./extensions`; examples requiring configuration must also have
a `[[extensions]]` entry pointing to the archive. You can alternatively select a
raw library path through `[[extensions]]`. All server paths are relative to the
server working directory. Configure only one copy of each example, including
any archives discovered automatically alongside a raw library.

Restart after building or changing configuration. Native extensions are trusted
code and remain loaded until the process exits; hot reload is unsupported.

## Development checks

```sh
cargo test --manifest-path extension-example/game-of-life/Cargo.toml
cargo test --manifest-path extension-example/strange-terrain/Cargo.toml
cargo test --manifest-path extension-example/spleef/Cargo.toml
```

For native integration tests, first build the four standalone examples in debug mode
(omit `--release` in the build commands above), then run from Steel:

```sh
cargo test --profile cicd -p steel-extension-api --features host --test native --test game_of_life --test strange_terrain --test spleef -- --include-ignored
```

The actions/filters example has its own native tests, which load its two
libraries independently of the host build:

```sh
cargo build --manifest-path extension-example/hooks-and-filters/Cargo.toml
cargo test --manifest-path extension-example/hooks-and-filters/host-test/Cargo.toml
```

For a first extension and an overview of the API, read the parent Steel
checkout's `doc/extensions/getting-started.md` and `doc/extensions/api.md`.
