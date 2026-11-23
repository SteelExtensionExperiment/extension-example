# Furniture

A native furniture extension for unmodified Minecraft clients. It defaults to no
resource pack and does not depend on the lucky-block extension. An optional
resource pack supplies distinct vertical-slab inventory models.

| Item | Behavior |
| --- | --- |
| `furniture:chair` | A cushioned oak chair. Right-click to sit; sneak to stand up. One player can occupy it. Breaking its block dismounts the occupant and drops the chair. |
| `furniture:<material>_vertical_slab` | A vertical half-block model with the source slab texture and exact oak-shelf collision. |
| `furniture:vertical_slab` | Original spruce alias for the same vertical panel. |
| `furniture:custom_bed` | An oak-frame blue bed. Places both halves, faces the placing player’s direction, supports player sleep and respawn, tracks occupancy, and drops exactly one custom bed when either half is mined. |

Chairs and vertical slabs follow the player's horizontal direction when placed,
using the same facing rule as vanilla stairs. Their collision and models rotate
together, and chair seating rotates with the cushion. Chairs stay upright and
vertical slabs stay vertical. The bed also supports all four horizontal directions. The chair uses vanilla stair collision; its cushion and
armrests are additional item displays. The bed uses actual vanilla bed geometry
with additional oak posts, rails, headboard, and footboard. Added model parts
have no independent collision. Their coordinates and seat pose are authored
for the standard adult player model.

A vertical slab looks half a block thick and uses the hitbox of a vanilla
oak shelf. Steel rotates the collision, targeting outline, and model
together so Minecraft predicts the same collision the server applies. Each variant copies its source slab’s
gameplay and tool properties and drops its own item. Steel discovers these
materials from the vanilla registry. Vertical slabs do not merge, form stair
corners, or waterlog.

A vanilla client may wait for Steel's block update when placing against the
front face of a vertical slab. Steel processes the click against the slab's
server-side state.

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

## Resource pack

Furniture adds item definitions to Steel's combined resource pack during
registration. Set `server.resource_pack.mode` to `optional` or `required` and
configure the public pack URL to deliver them. Both client modes use the same
vanilla slab mesh and placement transform. The pack also rotates that mesh
in inventory. Players without the pack see the ordinary slab icon.

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
```

The native test loads the built extension. It checks every
slab material/model/projection, placement in all four directions, rotated seating,
saved facing states, and real server collision on both halves of a block,
placement/consumption, saving, mining, chair use/removal/persistence, both bed
halves, all bed states, sleeping/respawn/night skipping, model refresh, drops,
and invalid-dimension destruction.

See [Custom content](../../doc/extensions/custom-content.md) and the
[extension API](../../doc/extensions/api.md).
