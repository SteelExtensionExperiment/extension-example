# Furniture

A native furniture extension for unmodified Minecraft clients. It defaults to no
resource pack and does not depend on the lucky-block extension. An optional
resource pack supplies distinct vertical-slab inventory models.

| Item | Behavior |
| --- | --- |
| `furniture:chair` | A cushioned oak chair. Right-click to sit; sneak to stand up. One player can occupy it. Breaking its block dismounts the occupant and drops the chair. |
| `furniture:<material>_vertical_slab` | A vertical half block with the matching slab texture, exact half-block server collision, and an automatically selected approximate vanilla client collision. |
| `furniture:vertical_slab` | Original spruce alias, now also a vertical half slab. |
| `furniture:custom_bed` | An oak-frame blue bed. Places both halves, faces the placing player’s direction, supports player sleep and respawn, tracks occupancy, and drops exactly one custom bed when either half is mined. |

Chairs and vertical slabs follow the player's horizontal direction when placed,
using the same facing rule as vanilla stairs. Their collision and models rotate
together, and chair seating rotates with the cushion. Chairs stay upright and
vertical slabs stay vertical. The bed also supports all four horizontal directions. The chair uses vanilla stair collision; its cushion and
armrests are additional item displays. The bed uses actual vanilla bed geometry
with additional oak posts, rails, headboard, and footboard. Added model parts
have no independent collision. Their coordinates and seat pose are authored
for the standard adult player model.

A north-facing vertical slab occupies `z=0.0..0.5` with full height and width;
the other facings rotate that half around the block center. Steel discovers
all vanilla slab materials from the real registry: currently 101 variants,
including woods, stone families, copper stages, wool, and concrete. No material
allowlist is maintained. Each variant copies its source slab's gameplay/tool
properties and self-drops its own item. They do not merge, form stair corners, or waterlog.

Vanilla has no exact vertical half-block collision state. This example explicitly
permits approximation: Steel chooses contained client collision while server
physics use the exact half block. Walking into the portion missing on the client
may produce movement corrections. Neither visual mode changes this limitation.
A small model margin covers the visible carrier without coincident faces.

Seats are temporary, invisible vanilla interaction entities with no targeting
box. They are removed after dismounting or when the chair changes, and are
excluded from chunk and player vehicle persistence. Dismounting uses Steel's
existing bed stand-up search. A seat is created only while somebody uses a chair.

The bed has its own server block/item identities and a complete bed state
family. It delegates player gameplay to Steel's ordinary bed behavior,
including dimension rules and bad-respawn-point explosions. Existing missing
villager sleeping/advancement mechanics in Steel are not added by this example;
custom vanilla tags and village POI membership are not inherited.
Its fixed one-item self-drop does not apply vanilla explosion-decay loot.

## Build and install

From the parent Steel checkout with its pinned nightly toolchain:

```sh
cargo build --manifest-path extension-example/furniture/Cargo.toml
python3 package-extension.py extension-example/furniture --debug
```

Steel discovers `extensions/steel-furniture-extension.zip` automatically.
Restart the server after building or replacing the archive; native extensions
remain loaded for the life of the process.

For a release package:

```sh
python3 package-extension.py extension-example/furniture
```

## Optional resource-pack version

The default version rotates vanilla slab models and reuses their ordinary slab
inventory icons. To obtain dedicated vertical-slab inventory icons/models:

```sh
python3 extension-example/furniture/generate-resource-pack.py
```

This produces `extension-example/furniture/dist/furniture-resource-pack.zip`
using the current workspace target, local vanilla model parents, and extracted
slab materials. It fails if `minecraft-src` is missing or targets another version.
It references vanilla textures; it does not replace ordinary vanilla assets.

Install and enable that ZIP in **each Minecraft client's resourcepacks folder**.
Then select the pack version in `config/config.toml`, at the top level:

```toml
[[extensions]]
path = "extensions/steel-furniture-extension.zip"

[extensions.config]
use_resource_pack = true
```

Restart Steel after changing modes. The same extension library implements both
versions. Leave the option absent or `false` for the default no-pack version.
Pack mode expects every client to enable the pack; missing assets show missing
models. This example does not serve or automatically send a pack to clients.

## Try it

As an operator:

```mcfunction
/give @s furniture:chair 16
/give @s furniture:oak_vertical_slab 16
/give @s furniture:spruce_vertical_slab 16
/give @s furniture:stone_vertical_slab 16
/give @s furniture:cut_copper_vertical_slab 16
/give @s furniture:custom_bed 16
/gamemode survival
```

Place furniture on a clear floor. Right-click the chair to sit and sneak to
leave it. Try the bed at night in the Overworld. Its ordinary dimension rules
apply: using it in the Nether or End is dangerous. Leave furniture placed and
restart the server to check saved identities and model reconstruction.

## Validate

After building the debug library:

```sh
cargo test --profile cicd -p steel-extension-api --features host --test furniture -- --ignored
STEEL_TEST_FURNITURE_PACK=1 cargo test --profile cicd -p steel-extension-api --features host --test furniture -- --ignored
```

The native test loads this actual extension in the selected mode. It checks every
slab material/model/projection, placement in all four directions, rotated seating,
saved facing states, and real server collision on both halves of a block,
placement/consumption, saving, mining, chair use/removal/persistence, both bed
halves, all bed states, sleeping/respawn/night skipping, model refresh, drops,
and invalid-dimension destruction.

See [Custom content](../../doc/extensions/custom-content.md) and the
[extension API](../../doc/extensions/api.md).
