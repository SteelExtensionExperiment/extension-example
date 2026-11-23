# Lucky block

Registers `lucky:lucky_block` as a custom block and item. A single PNG supplies
the yellow question-mark model in inventory and on the placed block for clients
that load Steel's resource pack. Players without the pack see the
[published yellow Lucky Block skin](https://minecraft-heads.com/custom-heads/head/4663-lucky-block-yellow)
by Luck as a single-head preview. After the generated tiles have been published
and configured, the placed block uses eight head displays with the pixels from
the PNG. The inventory continues to show one head.

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

This is an independent workspace using `steel-extension-api` without `host`.
It also links `steel-utils` privately for its RNG and mutex; those Rust values
stay inside the extension and never cross the native API boundary.
See the [example index](../README.MD) for the checkout layout.

## Build and install

```sh
python3 package-extension.py extension-example/lucky-block
```

The package is `extensions/steel-lucky-block-extension.zip`. Steel discovers it
automatically in `./extensions`. The ZIP carries the PNG and model JSON from
`assets/lucky/`. Use `--debug` to package a debug build, then restart Steel.

## Appearance without the pack

The example selects one published Lucky Block head through `register_head_fallback`.
Players without the pack see the same head in inventory and on the placed block.

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

Build the test ZIP first. The native test loads it in a separate host
process and exercises registration, inventory and creative codecs, nested items,
placement and inventory consumption, every reward and both traps with a fixed
seed, creative removal, terrain preservation, unavailable trap footprints,
ordinary oak-plank drops, chunk persistence, and display cleanup:

```sh
python3 package-extension.py extension-example/lucky-block --debug --output extension-example/lucky-block/target/test-extension.zip
cargo test --profile cicd -p steel-extension-api --features host --test lucky_block -- --ignored
```

The API and model coordinate convention are documented in
[Custom items and blocks](../../doc/extensions/custom-content.md).
