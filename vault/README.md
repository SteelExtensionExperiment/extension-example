# Vault for Steel

Service contracts (crate [`vault-api`](../vault-api/src/lib.rs)) and Steel-backed providers:

- `vault:economy/v1` — economy (provided by Essentials, not by this extension)
- `vault:permission/v1` — permission checks and groups from Steel's `groups.toml`
- `vault:chat/v1` — chat prefix/suffix/options from permission metadata

Differences from Bukkit Vault: players are identified by UUID only, there are no per-world
overloads or banks, and permission/chat calls take the caller's borrowed `Server` handle
because permission state lives on the host. Group membership changes return `false` until
Steel exposes runtime group editing (TODO).

## Build and install

Requires: Nothing; Essentials provides the economy behind it.

Run from the parent Steel checkout (Linux library names shown; use `.dylib` on macOS and
`vault.dll` on Windows):

```sh
cargo build --manifest-path extension-example/vault/Cargo.toml --release
mkdir -p extensions
cp extension-example/vault/target/release/libvault.so extensions/
```

Steel loads every package and library in `extensions/`, dependencies first, and creates
`extensions/vault/config.toml` on first start; edit it and restart. An `[[extensions]]` entry
with an `[extensions.config]` table in `config/config.toml` overrides that file. To ship a
package instead, run `python3 package-extension.py extension-example/vault`, which writes
`extensions/vault.zip`.

## Configure prefixes

In `config/groups.toml`:

```toml
[groups.op]
priority = 100
allow = ["*"]
metadata = [{ key = "vault:prefix", value = "&c[Admin] " }]

[groups.default]
metadata = [{ key = "vault:prefix", value = "&7[Member] " }, { key = "essentials:homes", value = 5 }]
```

Metadata keys may carry context selectors such as `vault:prefix{domain=lobby}`.

## Use from another extension

```rust
let economy = vault_api::economy()?;              // Err when no provider is installed
let response = economy.deposit(player.uuid(), 25.0);
if response.is_success() { /* ... */ }
```

Call methods through the generated `EconomyDyn`/`PermissionDyn`/`ChatDyn` traits
(`use vault_api::EconomyDyn as _;`).

## Commands

| Command | Permission |
|---|---|
| `/vault-info` | `vault.command.vault-info` (operators) |

## Manual test

1. Start the server with Vault and Essentials; as an operator run `/vault-info`.
   Expect `vault:economy/v1: essentials (priority 10)` and `vault` for permission and chat.
2. Stop the server, remove Essentials, start again: `/vault-info` shows `none` for economy.
3. Add a `vault:prefix` metadata entry for your group, restart, and chat: with Essentials
   installed, the prefix appears in front of your name.
