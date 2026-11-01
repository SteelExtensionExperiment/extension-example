# WorldGuard for Steel

Cuboid region protection built on WorldEdit selections, with owners, members, priorities,
parents, a per-world `__global__` region and these flags:

| Flag | Values | Effect |
|---|---|---|
| `build` | allow/deny | fallback for the four flags below |
| `block-break`, `block-place` | allow/deny | breaking / placing blocks |
| `interact` | allow/deny | doors, buttons, levers, beds, workstations… |
| `chest-access` | allow/deny | chests, barrels, furnaces, hoppers, shulker boxes… |
| `pvp` | allow/deny | players damaging players |
| `mob-damage` | allow/deny | non-player entities damaging players |
| `invincible` | allow/deny | players take no damage |
| `entry`, `exit` | allow/deny | non-members crossing the boundary |
| `greeting`, `farewell` | text with `&` codes | message when entering / leaving |

Rules (simplified from WorldGuard 7): inside any region, non-members cannot build unless a flag
allows it; owners and members of the highest-priority applicable region always can. Higher
priority wins; at equal priority `deny` beats `allow`. Children inherit unset flags, owners and
members from their parent. `__global__` flags apply outside regions and when no region sets the
flag. Region groups, polygon regions and the rest of WorldGuard's flags are TODO.

Regions are stored in `extensions/worldguard/regions/<world>.toml` and saved on every change. If a
file cannot be read at startup, its world is locked: changes are refused instead of
overwriting it.

## Build and install

Requires: `worldedit` (selections define regions).

Run from the parent Steel checkout (Linux library names shown; use `.dylib` on macOS and
`worldguard.dll` on Windows):

```sh
cargo build --manifest-path extension-example/worldguard/Cargo.toml --release
mkdir -p extensions
cp extension-example/worldguard/target/release/libworldguard.so extensions/
```

Steel loads every package and library in `extensions/`, dependencies first, and creates
`extensions/worldguard/config.toml` on first start; edit it and restart. An `[[extensions]]` entry
with an `[extensions.config]` table in `config/config.toml` overrides that file. To ship a
package instead, run `python3 package-extension.py extension-example/worldguard`, which writes
`extensions/worldguard.zip`.

## Commands

`/rg` (aliases `/region`, `/regions`):

| Subcommand | Permission |
|---|---|
| `define <id>`, `redefine <id>` | `worldguard.region.define` / `.redefine` |
| `remove <id>` | `worldguard.region.remove` |
| `list`, `info [id]` | everyone |
| `addowner`/`removeowner <id> <player>` | `worldguard.region.addowner` / `.removeowner` |
| `addmember`/`removemember <id> <player>` | `worldguard.region.addmember` / `.removemember`, or region owner |
| `flag <id> <flag> [value]` (no value clears) | `worldguard.region.flag` |
| `setpriority <id> <n>` | `worldguard.region.setpriority` |
| `setparent <id> [parent]` | `worldguard.region.setparent` |

`worldguard.region.bypass` ignores all protection.

## Manual test

Use A (operator) and B (default group); A must not keep `worldguard.region.bypass` for steps
that test protection (deny it for A in `groups.toml`, or test with B).

1. A selects an area with the WorldEdit wand and runs `/rg define spawn`. `/rg info spawn`
   shows A as owner.
2. B tries to break, place, open a chest and press a button inside: all are refused with
   "Hey! Sorry, but you can't ...". Outside the region everything works.
3. `/rg addmember spawn B`: B can now build. `/rg removemember spawn B`: refused again.
4. `/rg flag spawn greeting &aWelcome to spawn`, `/rg flag spawn farewell &7Bye`: walking in
   and out shows the messages.
5. `/rg flag spawn pvp deny`: B cannot hurt A inside. `/rg flag spawn invincible allow`: falling
   inside does no damage.
6. Define `shop` inside spawn, `/rg setpriority shop 10`, `/rg flag shop build allow`: B can
   build in the shop but not in the rest of spawn.
7. `/rg flag spawn entry deny` then step out and back in as B: pushed back with a message.
8. `/rg setparent shop spawn`, `/rg flag shop greeting` (clears it): entering the shop shows the
   parent's greeting.
9. `/rg flag __global__ pvp deny`: no pvp anywhere outside regions that allow it.
10. Restart the server: `/rg list` and `/rg info` show the same regions and flags.
