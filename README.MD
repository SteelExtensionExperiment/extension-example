# Steel extension examples

Each crate demonstrates one feature or one complete game. The examples are
independent: loading wool shearing does not also enable Game of Life or install
a terrain generator. The ports of Bukkit plugins (Vault, Essentials, WorldEdit,
WorldGuard) and the OP Prison extensions do depend on each other through small
service contract crates (`vault-api`, `worldedit-api`, `prison-api`), declared
with `depends`/`soft_depends` so Steel loads them in order. Spleef remains one application because its commands, maps,
bot controls, and world lifecycle implement the same minigame.

| Example | What it does | Instructions |
| --- | --- | --- |
| Wool shearing | Replace wool interaction while delegating other block behavior. | [Build and use](wool-shearing/README.md) |
| Lucky block | Register a yellow question-mark item and full block for vanilla clients; survival breaking opens random item rewards or block traps. | [Build and use](lucky-block/README.md) |
| Creatures | Throw textured balls, capture native mobs, deploy nonlethal battles, and publish a party scoreboard. | [Build and use](creatures/README.md) |
| Magic | Cast nine wand spells with shared mana data, temporary terrain, protection and a colored scoreboard mana meter. | [Build and use](magic/README.md) |
| Furniture | Add a usable chair, vertical half slabs for all vanilla slab materials, and custom bed with sleeping and respawn. | [Build and use](furniture/README.md) |
| Game of Life | Advance a configured block board through a world-tick callback. | [Build and use](game-of-life/README.md) |
| Strange terrain | Register seeded floating-island, shell, and pillar terrain. | [Build and use](strange-terrain/README.md) |
| Toxic wasteland | A poisonous green sludge fluid shown as tinted water, its biome, a generated wasteland with sludge lakes, and `/toxic` paint/pour/drain tools. | [Build and use](toxic-wasteland/README.md) |
| Spleef | Run a minigame with commands, temporary worlds, bots, and player restoration. | [Build and use](spleef/README.md) |
| Actions and filters | Run a custom score through ordered filters in two libraries, then observe an action. | [Build and use](hooks-and-filters/README.md) |
| Vault | Economy, permission and chat service contracts other extensions use, like Bukkit's Vault. | [Build and use](vault/README.md) |
| Essentials | Homes, warps, spawn, teleport requests, kits, economy and chat formatting (EssentialsX core). | [Build and use](essentials/README.md) |
| WorldEdit | Selections, `//set`, `//replace`, copy/paste, undo/redo and the `worldedit:selection/v1` service. | [Build and use](worldedit/README.md) |
| WorldGuard | Region protection with flags, owners and members, built on WorldEdit selections. | [Build and use](worldguard/README.md) |
| Prison mines | OP Prison mines refilled from block mixes with the bulk fill, on a timer or when mined out. | [Build and use](prison-mines/README.md) |
| Prison enchants | Custom pickaxe with Jackhammer, Explosive, Nuke, Soundwave and more, auto-selling into Vault. | [Build and use](prison-enchants/README.md) |

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
    ├── lucky-block/
    ├── furniture/
    ├── game-of-life/
    ├── strange-terrain/
    ├── spleef/
    ├── hooks-and-filters/
    ├── vault-api/, vault/, essentials/
    ├── worldedit-api/, worldedit/, worldguard/
    └── prison-api/, prison-mines/, prison-enchants/
```

If you cloned this repository on its own, place it in that layout before
building. Run the following commands from the parent Steel checkout, using its
`rust-toolchain.toml` and pinned Stabby dependency:

```sh
git submodule update --init --recursive
cargo build --manifest-path extension-example/wool-shearing/Cargo.toml --release
cargo build --manifest-path extension-example/lucky-block/Cargo.toml --release
cargo build --manifest-path extension-example/furniture/Cargo.toml --release
cargo build --manifest-path extension-example/creatures/Cargo.toml --release
cargo build --manifest-path extension-example/magic/Cargo.toml --release
cargo build --manifest-path extension-example/game-of-life/Cargo.toml --release
cargo build --manifest-path extension-example/strange-terrain/Cargo.toml --release
cargo build --manifest-path extension-example/spleef/Cargo.toml --release
cargo build --manifest-path extension-example/hooks-and-filters/Cargo.toml --release
for crate in vault essentials worldedit worldguard prison-mines prison-enchants; do
    cargo build --manifest-path extension-example/$crate/Cargo.toml --release
done
```

Examples use isolated workspaces so they build separately from the host and use
`steel-extension-api` without the `host` feature. The actions/filters example has
two extension crates and one shared contract in its extension-side workspace;
its host test has a separate workspace. Build only the examples you intend to
install. Native library names and full configuration appear in each README.

## OP Prison demo

The prison bundle shows block regeneration speed: a mine refills a million blocks in about
a tenth of a second. Build and install it from the Steel checkout (Linux names shown):

```sh
mkdir -p extensions
for crate in vault essentials worldedit prison-mines prison-enchants; do
    cargo build --manifest-path extension-example/$crate/Cargo.toml --release
    cp extension-example/$crate/target/release/lib${crate//-/_}.so extensions/
done
```

Start Steel, `/op` yourself from the console, and follow the
[prison-mines](prison-mines/README.md#manual-check) and
[prison-enchants](prison-enchants/README.md#demo-script) scripts.

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
ZIP packages and native libraries in `./extensions`, ordered by their declared
dependencies. Each extension reads `extensions/<name>/config.toml` (created empty
on first start) unless a `[[extensions]]` entry pointing to it supplies an
`[extensions.config]` table. You can alternatively select a
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
for crate in essentials worldedit-api worldedit worldguard prison-mines prison-enchants; do
    cargo test --manifest-path extension-example/$crate/Cargo.toml
done
```

For native integration tests, first build the four standalone examples in debug mode
(omit `--release` in the build commands above), then run from Steel:

```sh
cargo test --profile cicd -p steel-extension-api --features host --test native --test game_of_life --test strange_terrain --test spleef -- --include-ignored
```

The ports and prison extensions have native tests too. Build them and the
`event-probe` test extension in debug mode first:

```sh
for crate in vault essentials worldedit worldguard prison-mines prison-enchants; do
    cargo build --manifest-path extension-example/$crate/Cargo.toml
done
cargo build --manifest-path steel-extension-api/tests/event-probe/Cargo.toml
cargo test --profile cicd -p steel-extension-api --features host --test ports --test prison --test folder --test events -- --include-ignored
```

The actions/filters example has its own native tests, which load its two
libraries independently of the host build:

```sh
cargo build --manifest-path extension-example/hooks-and-filters/Cargo.toml
cargo test --manifest-path extension-example/hooks-and-filters/host-test/Cargo.toml
```

For a first extension and an overview of the API, read the parent Steel
checkout's `doc/extensions/getting-started.md` and `doc/extensions/api.md`.
