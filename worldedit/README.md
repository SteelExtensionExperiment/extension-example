# WorldEdit for Steel

Core WorldEdit features for cuboid selections: wand, positions, `//set`, `//replace`,
`//walls`, `//count`, `//size`, `//expand`, `//contract`, `//copy`, `//cut`, `//paste`,
`//undo`, `//redo`, `//sel`. The selection is also exported as the `worldedit:selection/v1`
service (crate [`worldedit-api`](../worldedit-api/src/lib.rs)), used by WorldGuard.

Edits run synchronously and require every touched chunk to be loaded; the check happens before
the first change, so an edit is never half applied. Blocks outside the world height are
skipped. Differences and TODOs: no `//hpos` (needs a raycast API), brushes, schematics,
expression patterns, `//stack`/`//move`, or block entity contents in undo/clipboard.

## Build and install

Requires: Nothing.

Run from the parent Steel checkout (Linux library names shown; use `.dylib` on macOS and
`worldedit.dll` on Windows):

```sh
cargo build --manifest-path extension-example/worldedit/Cargo.toml --release
mkdir -p extensions
cp extension-example/worldedit/target/release/libworldedit.so extensions/
```

Steel loads every package and library in `extensions/`, dependencies first, and creates
`extensions/worldedit/config.toml` on first start; edit it and restart. An `[[extensions]]` entry
with an `[extensions.config]` table in `config/config.toml` overrides that file. To ship a
package instead, run `python3 package-extension.py extension-example/worldedit`, which writes
`extensions/worldedit.zip`.

## Commands

Every command is typed with a double slash and needs `worldedit.<name>`, for example
`worldedit.set` (operators have every permission). The wand needs `worldedit.wand`.

| Command | Effect |
|---|---|
| `//wand` | gives the wand (left click = pos1, right click = pos2) |
| `//pos1`, `//pos2` | set a corner at your feet |
| `//sel` | clear the selection |
| `//size` | show corners, dimensions and volume |
| `//set <pattern>` | fill the selection |
| `//replace [from] <to>` | replace matching blocks (`from` omitted: any non-air) |
| `//walls <pattern>` | fill the four vertical sides |
| `//count <mask>` | count matching blocks |
| `//expand <n> [direction]`, `//expand vert` | grow the selection (default: where you look) |
| `//contract <n> [direction]` | shrink the selection |
| `//copy`, `//cut` | copy relative to your position (cut also clears to air) |
| `//paste` | paste relative to your position |
| `//undo [n]`, `//redo [n]` | step through your history |

Patterns: `stone`, `minecraft:oak_log[axis=x]`, `50%stone,30%dirt,20%gravel`.
Masks: `stone,dirt` (any state of those blocks) or exact states like `oak_log[axis=y]`.
Directions: `up`, `down`, `north`, `south`, `east`, `west` (or first letters).

## Configuration (`extensions/worldedit/config.toml`)

```toml
wand_item = "minecraft:wooden_axe"
max_blocks = 1000000     # per command
history_size = 15        # undoable edits per player
```

## Manual test

1. `//wand`, left click a block (message "First position set to (x, y, z)") — the block is not
   broken, even in creative. Right click another block 9 blocks away on each axis
   ("... (1000 blocks)").
2. `//size` shows 10x10x10 = 1000. `//set stone` → "1000 blocks have been changed".
3. `//undo` restores the original terrain exactly; `//redo` puts the stone back.
4. `//replace stone 50%glass,50%oak_planks`, `//count glass` shows about 500.
5. `//walls red_wool`, look up and `//expand 5`, `//contract 2 down`, `//size`.
6. `//copy`, walk 20 blocks away, `//paste`: the structure appears at the same offset from you.
   `//undo` removes the paste.
7. `//cut` clears the selection to air; `//undo` brings it back.
8. Set a huge selection (expand 2000 in several directions) and `//set stone`: the block limit
   error is shown and nothing changes.
9. A non-operator cannot use `//set` or the wand.
