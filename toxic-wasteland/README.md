# Toxic Wasteland

A green, poisonous **toxic sludge** fluid, the **wasteland** biome it belongs to, a
generated wasteland world full of sludge lakes, and `/toxic` tools to bring both
into any world. Vanilla clients need no mod and no resource pack.

- **Sludge** looks like green water everywhere, even outside the wasteland biome,
  and swims like water.
  - It spreads slower than water (about half its reach) and does not form new
    sources.
  - Touching it poisons living entities and damages them about once a second.
    Creative and spectator players are safe.
  - It slowly corrupts water next to it into sludge, and withers grass, dirt,
    podzol and mycelium into coarse dirt.
  - Lava that touches it cools into coarse dirt (lava source) or cobblestone
    (flowing lava).
  - Flowing sludge washes away plants and crops, dropping them like water does.
- **Bucket of Toxic Sludge**: use an empty bucket on a sludge source, then pour it
  like water. It evaporates in the Nether.
- **Wasteland biome**: an olive sky and fog, sickly grass and foliage, green water,
  drifting ash, no rain.
- **Wasteland world**: seeded hills of coarse dirt and gravel over stone, all in the
  wasteland biome, with sludge lakes. On top of that:
  - **Dead trees**: bleached pale-oak trunks that lean over, with two or three bare,
    crooked branches, roots breaking through the ground, and moss hanging from some
    branches.
  - **Uranium Ore**: patches of green-speckled ore in the surface. It is mined like
    stone and drops itself.
  - **Nuclear Waste** barrels: yellow barrels with the radiation symbol and green goo
    on top, leaking at the edge of every sludge lake. They are mined like iron and
    drop themselves.

  Both nuclear blocks are full blocks with a player-head model, like the lucky-block
  example, so they need no resource pack.

- **Mobs**, spawned on the surface around players in the wasteland (up to 3 ghouls
  and 4 slimes within 64 blocks of each player):
  - **Sludge Slime**: hops like a slime and poisons what it touches. When killed it
    leaves a sludge puddle and splits into two Small Sludge Slimes.
  - **Irradiated Ghoul**: a new mob with a glowing radioactive face. It hunts players,
    and its blows poison and give nausea.

  Clients see them as a slime and as a husk wearing the ghoul's head, so no mod is
  needed.

## Build and install

From the Steel checkout:

```sh
cargo build --release --manifest-path extension-example/toxic-wasteland/Cargo.toml
python3 package-extension.py extension-example/toxic-wasteland
```

This creates `extensions/toxic-wasteland.zip`. You can also copy
`extension-example/toxic-wasteland/target/release/libtoxic_wasteland.so` into
`extensions/`. WorldEdit is optional: install it to use selections with `/toxic`.

To add the wasteland world, put this in `config/worlds.toml`:

```toml
[[domains.minecraft.worlds]]
name = "wasteland"
generator = "toxic_wasteland:wasteland"
```

## Commands

All of them require the `toxic_wasteland.admin` permission (operators have it).
Without corners, they use your WorldEdit selection.

| Command | Effect |
| --- | --- |
| `/toxic paint <biome>` / `/toxic paint <from> <to> <biome>` | Paints any biome, for example `toxic_wasteland:wasteland` or `minecraft:plains` to undo. |
| `/toxic pour` / `/toxic pour <from> <to>` | Fills air and replaceable blocks with sludge sources. The sludge then spreads. |
| `/toxic drain` / `/toxic drain <from> <to>` | Removes all sludge in the region. |

Regions over `commands.max_volume` blocks, or touching unloaded chunks, are
refused without changes.

## Configuration

`extensions/toxic-wasteland/config.toml`. Every key is optional, and an invalid
value stops the extension with a message naming it.

| Key | Default | Meaning |
| --- | --- | --- |
| `colors.water` / `colors.water_fog` | `"#4CBB17"` / `"#2E5E0E"` | Sludge color and underwater fog, also used by the biome. |
| `colors.sky` / `fog` / `grass` / `foliage` | `"#9AA35A"` / `"#B4BA7A"` / `"#7C8A3A"` / `"#6B7A2E"` | Biome colors. |
| `sludge.tick_delay` | `15` | Ticks between spread steps (water: 5). |
| `sludge.drop_off` | `2` | Level lost per block (water: 1). |
| `sludge.converts_to_source` | `false` | Whether two sources make a new one. |
| `effects.poison_seconds` / `poison_amplifier` | `5` / `0` | Poison applied on contact; 0 seconds disables it. |
| `effects.contact_damage` | `1.0` | Damage per hit (half a heart). |
| `effects.damage_interval_ticks` | `20` | Minimum ticks between hits on one entity. |
| `spread.corrupt_water_chance` | `0.25` | Chance per random tick and neighbor that water becomes sludge; 0 disables. |
| `spread.wither_ground_chance` | `0.15` | Same, for ground becoming coarse dirt. |
| `world.lake_chance` | `0.3` | Chance that a wasteland chunk holds a lake. |
| `world.tree_chance` | `0.35` | Chance per tree slot (two per chunk) of a dead tree. |
| `world.uranium_chance` | `0.25` | Chance that a chunk shows a uranium patch. |
| `world.base_height` / `relief` | `70` / `18` | Average surface height and hill size. |
| `commands.max_volume` | `2000000` | Largest region `/toxic` edits. |
| `mobs.enabled` | `true` | Whether the extension spawns its mobs. |
| `mobs.interval_ticks` | `200` | Ticks between spawn attempts (20–72000). |
| `mobs.max_ghouls` / `mobs.max_slimes` | `3` / `4` | How many stay within 64 blocks of each player. |

## How it works and its limits

The extension uses Steel's [custom fluid, biome, and tint
projection](../../doc/extensions/custom-content.md#custom-fluids) API. The server
stores real sludge blocks and the real biome of every cell. Clients receive water
blocks, and a green copy of the biome in cells that contain sludge.

- Biome colors are per 4×4×4 cell, so plain water sharing a cell with sludge also
  looks green.
- Inside a sludge cell, F3 shows a biome name such as
  `toxic_wasteland:sludge_tint/minecraft/plains`.
- Sludge does not waterlog blocks. A bucket used on a slab places sludge next to it.
- Dispensers do not pour sludge buckets yet.
- Mobs spawn only through the extension (Steel has no natural spawner yet), always
  on the surface, day and night.
- Mobs don't spawn in the wasteland biome, since extension biomes have no spawn
  lists yet.
- Each uranium or barrel block is one item display per player who sees it, so the
  generator keeps them few: at most 16 per chunk.
- Decorations stay inside the chunk that generates them, so trees never cross a chunk
  border. Chunks generated before this version have no trees or nuclear blocks.

## Tests

```sh
cargo test --manifest-path extension-example/toxic-wasteland/Cargo.toml
cargo build --manifest-path extension-example/toxic-wasteland/Cargo.toml
cargo test -p steel-extension-api --features host --test toxic_wasteland -- --include-ignored
```

The first command covers the configuration, generator determinism, lake coverage
and containment, decoration placement (inside the chunk, trees and uranium on dry
ground, barrels on lake rims, a bounded number of displays), and command regions. The last loads the built library into a real
world and checks:

- registration and client projection, including the nuclear blocks' models and drops
- the mobs: client carriers, spawning, the ghoul's head, an AI tick, the slime's
  split and puddle on death, and the ghoul being saved and reloaded as its own type
- spreading
- the tint and its refresh
- poison and damage
- corruption and withering rates
- lava
- the bucket
- persistence
