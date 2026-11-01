# Essentials for Steel

Core EssentialsX features: homes, warps, spawn, teleport requests, `/back`, private messages,
kits, economy (also provided to Vault as `vault:economy/v1`), god, fly, heal, feed, game mode
shortcuts, nicknames, broadcasts, AFK, chat formatting and join/quit messages.

Data lives in `extensions/essentials/`: `userdata/<uuid>.toml`, `warps.toml`, `spawn.toml`.
Changes are saved every 5 seconds and on shutdown; files are replaced atomically and an
unreadable file is renamed to `*.broken` instead of being overwritten.

Differences from EssentialsX: teleports are instant (no warmup, cooldown or safe-spot search);
`/tp` stays the vanilla command and the Essentials version is `/etp` (alias `/tpto`); the
economy requires the target to be online for `/pay` and `/eco`. Signs, jails, mail, homes in
other domains and many minor commands are out of scope (TODO).

## Build and install

Requires: `vault` (optional, so other extensions can find its economy).

Run from the parent Steel checkout (Linux library names shown; use `.dylib` on macOS and
`essentials.dll` on Windows):

```sh
cargo build --manifest-path extension-example/essentials/Cargo.toml --release
mkdir -p extensions
cp extension-example/essentials/target/release/libessentials.so extensions/
```

Steel loads every package and library in `extensions/`, dependencies first, and creates
`extensions/essentials/config.toml` on first start; edit it and restart. An `[[extensions]]` entry
with an `[extensions.config]` table in `config/config.toml` overrides that file. To ship a
package instead, run `python3 package-extension.py extension-example/essentials`, which writes
`extensions/essentials.zip`.

## Commands and permissions

Permissions derive from command keys: `/home` needs `essentials.command.home`. Commands marked
*everyone* work unless the permission is explicitly denied; the others need a grant (operators
have `*`).

| Command | Access |
|---|---|
| `/home [name]`, `/sethome [name]`, `/delhome <name>`, `/homes` | everyone |
| `/warp [name]`, `/warps` | everyone |
| `/setwarp <name>`, `/delwarp <name>` | staff |
| `/spawn` | everyone; `/setspawn` staff |
| `/tpa <player>`, `/tpahere <player>`, `/tpaccept` (`/tpyes`), `/tpdeny` (`/tpno`) | everyone |
| `/back` | everyone |
| `/msg <player> <message>` (`/tell`, `/w`, `/m`, `/whisper`), `/r <message>` (`/reply`) | everyone |
| `/kit [name]`, `/kits` | everyone; each kit may require `essentials.kits.<name>` |
| `/balance [player]` (`/bal`, `/money`), `/pay <player> <amount>`, `/baltop` | everyone (`essentials.balance.others` for another player) |
| `/eco <give\|take\|set> <player> <amount>` | staff |
| `/god [player]` (`/tgm`), `/fly [player]`, `/heal [player]`, `/feed [player]` (`/eat`) | staff |
| `/gmc`, `/gms`, `/gma`, `/gmsp` `[player]` | staff |
| `/nick <nick\|off>` (`/nickname`), `/broadcast <message>` (`/bc`) | staff |
| `/etp <player> [target]` (`/tpto`), `/tphere <player>` | staff |
| `/afk` | everyone |

Extra permissions: `essentials.sethome.unlimited`, `essentials.chat.color` (use `&` codes in
chat). The home limit is the integer permission metadata `essentials:homes`, else
`default_home_limit`.

## Configuration (`extensions/essentials/config.toml`)

```toml
chat_format = "{prefix}&f{displayname}{suffix}&7: &f{message}"   # "" keeps vanilla chat
join_message = "&e{player} joined the game"                      # "" keeps vanilla
quit_message = "&e{player} left the game"
starting_balance = 100.0
max_money = 10000000000000.0
currency_symbol = "$"
default_home_limit = 3
tpa_timeout_seconds = 120
afk_timeout_seconds = 300          # 0 disables automatic AFK
baltop_entries = 10

[kits.starter]
items = ["minecraft:stone_sword 1", "minecraft:bread 16"]
cooldown_seconds = 86400
default_access = true

[messages]
home_set = "&aHome {home} saved!"  # override any message key from src/messages.rs
```

## Manual test

Use two accounts, A (operator) and B (default group).

1. **Homes**: B runs `/sethome base`, walks away, `/home` (single home is chosen), `/homes`.
   Set 3 more homes: the 4th fails with the limit message. Restart the server; `/home base`
   still works.
2. **Warps/spawn**: A runs `/setwarp shop` and `/setspawn`. B runs `/warp` (lists), `/warp shop`,
   `/spawn`. B cannot run `/setwarp`.
3. **Requests**: B `/tpa A`; A `/tpaccept` → B arrives at A. B `/tpahere A`; wait more than
   `tpa_timeout_seconds`; A `/tpaccept` → "no pending request".
4. **Back**: B teleports with `/home base`, then `/back` returns to the previous spot. Die
   (fall in lava), respawn, `/back` returns to the death location.
5. **Economy**: B `/bal` shows `$100.00`. B `/pay A 12.50`; both get messages. B `/pay A 1000`
   fails. `/baltop` lists both. A `/eco give B 5` → `$92.50`. Restart; balances persist.
   With Vault installed, `/vault-info` lists Essentials as the economy.
6. **Utilities**: A `/god`, jump from height → no damage; `/god` again → damage. A `/fly B` →
   B can double-jump to fly. `/heal`, `/feed` refill bars. `/gmc` / `/gms` switch modes.
7. **Chat**: chat as A: the configured format (with Vault prefix if set) is shown. A `/nick &bCaptain`,
   chat again: the nickname appears. B types `&aHi`: shown literally (no color permission).
8. **Messages**: A `/msg B hello`, B `/r hi back`.
9. **AFK**: B stays still for `afk_timeout_seconds`: everyone sees "B is now AFK". B moves: "no
   longer AFK". `/afk` toggles manually.
10. **Kits**: B `/kit starter` receives items; again → cooldown message.
11. **Join/quit**: reconnect B: the custom join and quit messages are broadcast.
