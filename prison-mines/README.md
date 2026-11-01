# prison-mines

OP Prison mines for Steel. A mine is a box taken from your WorldEdit selection, refilled from a
weighted block mix. Resets write whole chunk sections at once (`World::fill`, about ten million
blocks per second) and print their speed.

## Build and install

Requires: `worldedit` (mines are created from selections).

Run from the parent Steel checkout (Linux library names shown; use `.dylib` on macOS and
`prison_mines.dll` on Windows):

```sh
cargo build --manifest-path extension-example/prison-mines/Cargo.toml --release
mkdir -p extensions
cp extension-example/prison-mines/target/release/libprison_mines.so extensions/
```

Steel loads every package and library in `extensions/`, dependencies first, and creates
`extensions/prison-mines/config.toml` on first start; edit it and restart. An `[[extensions]]` entry
with an `[extensions.config]` table in `config/config.toml` overrides that file. To ship a
package instead, run `python3 package-extension.py extension-example/prison-mines`, which writes
`extensions/prison-mines.zip`.

## Commands

| Command | Permission | |
|---|---|---|
| `/mine create <name>` | `prison.mines.admin` | New mine from your `//wand` selection (must not overlap another) |
| `/mine set <name> <mix>` | `prison.mines.admin` | Blocks, such as `60%stone,30%coal_ore,10%diamond_ore` (weights are relative; `stone` alone means 1 part) |
| `/mine interval <name> <seconds>` | `prison.mines.admin` | Timed reset; `0` disables |
| `/mine threshold <name> <percent>` | `prison.mines.admin` | Reset once this much was mined; `0` disables |
| `/mine reset <name>` | `prison.mines.admin` | Reset now |
| `/mine delete <name>` | `prison.mines.admin` | |
| `/mine list`, `/mine info <name>`, `/mine tp <name>` | everyone | |

Before a reset, players inside the mine are lifted to its top. Players within 24 blocks see a
boss bar with the countdown and how much was mined. The reset message reads
`Mine a reset: 34992 blocks in 3.4 ms (10291764 blocks/s)`.

Mines are stored in `extensions/prison-mines/mines.toml` and survive restarts.

## Service

`prison:mines/v1` (crate `prison-api`): `mine_at(world, pos)`, `mines()`, `add_mined(name,
count)` and `reset(server, name)`. `prison-enchants` uses it to know where enchants work and to
keep threshold resets accurate when it breaks blocks in bulk.

## Manual check

1. `//wand`, select a large box (for example 60×40×60), `/mine create a`.
2. `/mine set a 70%stone,20%coal_ore,8%iron_ore,2%diamond_ore`, `/mine reset a`: the box fills
   at once and the chat shows the time and blocks per second.
3. `/mine interval a 30` and stand nearby: the boss bar counts down; stand inside when it hits
   zero and you are lifted out before the refill.
4. Restart the server: `/mine info a` still lists the mine.
