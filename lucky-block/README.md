# Lucky block

Registers `lucky:lucky_block` as a real custom block and item. An unmodified
Minecraft client sees a textured player-head item in inventory, oak planks for
collision and mining, and a full-block yellow question-mark head display covering the placed
block. No client mod or resource pack is required.

The inventory item and placed model share the
[yellow question-mark lucky-block skin](https://minecraft-heads.com/custom-heads/head/4663-lucky-block-yellow)
by Luck. One enlarged head displays a complete question mark on each face. The
model expands by 0.2% around the block center to avoid coincident surfaces with
the planks; collision stays one block. The skin uses Minecraft's normal texture
downloads.

Survival placement consumes one item. Hand and axe mining follow oak-plank
rules. Each successful survival break opens the block with one random outcome;
it does not return the lucky-block item. Creative breaking only removes it.
Explosions and other non-player removal do not activate it. Ordinary oak planks
keep their behavior. Block and item identities survive saving and reloading,
and visual parts follow chunk tracking.

## Rewards and traps

These are this example's own outcomes, inspired by Lucky Block rather than a
copy of the mod's full reward table. Each row has a 12.5% chance: 75% item
rewards and 25% traps overall.

| Outcome | Result |
| --- | --- |
| Diamonds | 3 diamonds |
| Emeralds | 5 emeralds |
| Iron | 8 iron ingots |
| Gold | 8 gold ingots |
| Food | 8 bread |
| Coal | 16 coal |
| Cobweb snare | A 3×3 area of cobwebs, two blocks high, around the miner. |
| Obsidian cage | A 3×3 cage around the miner, with walls two blocks high and a roof. The center stays clear. |

Traps only fill air, cave air, and void air; they preserve existing terrain,
inventories, fluids, and other custom blocks. All positions must be in bounds
and already loaded before placement starts. If the footprint is unavailable,
the opening is consumed and the server logs the failed outcome. Item rewards
use normal block-drop pickup delays and honor the block-drops gamerule.

Outcome selection uses Steel's seeded Xoroshiro implementation inside the
extension. The default seed is random at startup. For a repeatable sequence,
add this explicit package entry at the top level of `config/config.toml`:

```toml
[[extensions]]
path = "extensions/steel-lucky-block-extension.zip"

[extensions.config]
seed = 42
```

The live sequence is shared by this example's lucky blocks across worlds. It
restarts when the server restarts; its RNG state is not saved with chunks.
This custom behavior does not use or change vanilla loot tables.

## Build

From the parent Steel checkout, using its nightly toolchain:

```sh
cargo build --manifest-path extension-example/lucky-block/Cargo.toml --release
```

This is an independent workspace using `steel-extension-api` without `host`.
It also links `steel-utils` privately for its RNG and mutex; those Rust values
stay inside the extension and never cross the native API boundary.
See the [example index](../README.MD) for the checkout layout.

## Package and install

```sh
python3 package-extension.py extension-example/lucky-block
```

The package is `extensions/steel-lucky-block-extension.zip`. Steel discovers it
automatically in `./extensions`; no configuration is needed. For a raw debug
library instead, build without `--release` and add this at the top level of
`config/config.toml` on Linux:

```toml
[[extensions]]
path = "extension-example/lucky-block/target/debug/libsteel_lucky_block_extension.so"
```

Use the `.dylib` or `.dll` listed in `extension.toml` on macOS or Windows. Select
one installed copy of the example, then restart the server.

## Try it

As an operator:

```mcfunction
/give @s lucky:lucky_block 16
/gamemode survival
```

Place an item on a solid surface and break it by hand or with an axe. Collect
the reward or escape the trap. Leave an unbroken block placed, then re-enter
the chunk or restart the server to check that its model returns. Keep the
extension installed to load its saved content. Restart after rebuilding the
package so Steel loads the updated native library.

## Validate

Build the debug library first. The native test loads it in a separate host
process and exercises registration, inventory and creative codecs, nested items,
placement and inventory consumption, every reward and both traps with a fixed
seed, creative removal, terrain preservation, unavailable trap footprints,
ordinary oak-plank drops, chunk persistence, and display cleanup:

```sh
cargo build --manifest-path extension-example/lucky-block/Cargo.toml
cargo test --profile cicd -p steel-extension-api --features host --test lucky_block -- --ignored
```

The API and model coordinate convention are documented in
[Custom items and blocks](../../doc/extensions/custom-content.md).
