# Bulk Chest

Build a hollow cube with Bulk Chest Walls and a single Bulk Chest Core on
its surface. Right-click any part of the completed chest to open its shared
inventory.

| Cube edge | Wall blocks | Slots |
| --- | ---: | ---: |
| 2 | 7 | 216 |
| 3 | 25 | 729 |
| 4 | 55 | 1,728 |
| 5 | 97 | 3,375 |

The bottom arrows switch between pages of 45 slots. Clear your cursor before
turning a page. Shift-click transfers stacks between the current page and
your inventory.

An incomplete shell closes the inventory and keeps its contents in the
core. Repair the shell to reopen it. A smaller shell can open once all
items fit in its slot range. Breaking the core drops its stored items.

## Run

From the Steel checkout, package the extension and start the server.

```sh
python3 package-extension.py extension-example/bulk-chest
cargo run -r
```

An operator can issue these commands to obtain the parts for a 2×2×2 chest.

```text
/give @s bulk-chest:core
/give @s bulk-chest:wall 7
```

The server pack supplies wood-and-metal textures for placing items and
incomplete shell parts. Players without the pack see spruce planks and
stripped spruce logs during construction. Both modes show one oversized
chest model after assembly, with every original block still in place.
Breaking the shell restores the construction textures on the remaining parts.
