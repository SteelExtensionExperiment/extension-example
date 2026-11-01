# prison-enchants

The OP Prison pickaxe for Steel. The pickaxe carries Efficiency 100 so blocks break instantly even while sprinting or jumping.
Every block mined with it inside a `prison-mines` mine is sold
straight into the Vault economy (Essentials provides one), and its custom enchants break the
mine in bulk with plenty of particles and sounds.

| Enchant | Max | Effect |
|---|---|---|
| Jackhammer | 100 | Breaks the whole mine layer you dig in: dust outline, firework sparks, explosions (1% chance per block at level 1, always at level 100) |
| Explosive | 50 | Blows up a sphere (radius 2 to 6) around the block |
| Nuke | 5 | Wipes out the entire mine: lightning at its corners, a title for everyone, refill 3 s later |
| Soundwave | 50 | A shell expanding ring by ring from the block, made of colored note particles over rising bells, shattering everything it reaches |
| Fortune | 10 | Each level adds 50% to every block's sell value |
| Haste, Speed | 3 | Effects while the pickaxe is held |

The sidebar shows balance, blocks mined, earnings, the mine you stand in (mined percentage and
reset countdown) and your enchants.

## Build and install

Requires: `prison-mines`; `vault` and `essentials` for the money (without an economy, blocks are not paid and upgrades are disabled).

Run from the parent Steel checkout (Linux library names shown; use `.dylib` on macOS and
`prison_enchants.dll` on Windows):

```sh
cargo build --manifest-path extension-example/prison-enchants/Cargo.toml --release
mkdir -p extensions
cp extension-example/prison-enchants/target/release/libprison_enchants.so extensions/
```

Steel loads every package and library in `extensions/`, dependencies first, and creates
`extensions/prison-enchants/config.toml` on first start; edit it and restart. An `[[extensions]]` entry
with an `[extensions.config]` table in `config/config.toml` overrides that file. To ship a
package instead, run `python3 package-extension.py extension-example/prison-enchants`, which writes
`extensions/prison-enchants.zip`.

## Commands

| Command | Permission | |
|---|---|---|
| `/pickaxe` | everyone | Get a Prison Pickaxe (Jackhammer I) |
| `/enchants` (or right-click with the pickaxe) | everyone | Upgrade menu: left-click +1 level, right-click +10 |
| `/penchant <enchant> <level> [player]` | `prison.enchants.admin` | Set a level on the held pickaxe |

## Config (`extensions/prison-enchants/config.toml`)

```toml
default_value = 0.5      # price of blocks missing from [values]
fortune_bonus = 0.5      # sell multiplier per Fortune level

[values]
"minecraft:stone" = 1.0
"minecraft:diamond_ore" = 60.0

[enchants.jackhammer]    # any enchant id; missing fields are not allowed
max = 100
base_cost = 2000.0       # price of level 1
growth = 1.06            # each level costs this much more than the previous
base_chance = 0.01       # proc chance per block at level 1
chance_per_level = 0.01  # Jackhammer reaches 100% at level 100
```

## Demo script

Build `vault`, `essentials`, `worldedit`, `prison-mines` and `prison-enchants` and copy their
libraries into `extensions/` (see the [example index](../README.MD#op-prison-demo)), start the
server, `/op` yourself, then:

1. Build a mine as in the `prison-mines` README (`/mine create a`, `/mine set a ...`,
   `/mine reset a`, `/mine interval a 60`).
2. `/pickaxe`, then `/penchant jackhammer 100`, `/penchant soundwave 50`,
   `/penchant explosive 50`, `/penchant fortune 10`, `/penchant haste 3`.
3. Mine inside the box: layers vanish at once with their outline in orange dust, shells of note
   particles ripple out with rising bells, and your balance on the sidebar climbs.
4. `/eco give <you> 10000000`, right-click the pickaxe and buy levels; `/penchant nuke 5` for
   the finale, then watch the mine refill.
