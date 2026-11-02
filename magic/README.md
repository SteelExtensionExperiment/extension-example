# Magic

An independent wand, spell menu and mana scoreboard for an unmodified Minecraft
client, without a resource pack. It does not import or communicate with the
creatures example. Both use Steel's shared data and HUD APIs.

From the Steel repository root:

```sh
python3 package-extension.py extension-example/magic --debug
cargo run
```

Use `/kit` when installed alone, or `/magic:kit` when other extensions also
register `kit`, to get a wand. Aim at the target and right-click. Select a spell
in the vanilla chest menu to cast immediately along your current aim. The menu
is read-only; its icons do not become inventory items.

| Spell | Default mana | Behavior |
| --- | ---: | --- |
| Fireball | 12 | Native projectile damage and ignition. |
| Ice projectile | 10 | Damage and native Slowness III for five seconds. |
| Stone projectile | 8 | Native projectile damage. |
| Summon fire | 18 | Ignites nearby targets and creates a native fire block in air. |
| Earth wall | 22 | Creates a temporary three-by-three dirt wall against the impact face. |
| Water spring | 20 | Creates a temporary native water source in air. |
| Lightning | 35 | Vanilla bolt visuals with explicit area damage and ignition. |
| Projectile shield | 25 | Reduces damage classified by the registered projectile tag. |
| Physical shield | 30 | Reduces direct melee damage by default. |

Projectiles use native thrown-item movement/collision. Spells are custom gameplay
rules: the lightning spell does not create a full native lightning entity or
perform mob transformations. Area spells query a two-block-radius cube and
exclude their caster.

Temporary terrain refuses occupied positions, block entities and competing
temporary claims. Its restoration deadline survives chunk unload and restart.
Any intervening state change cancels restoration to preserve newer edits.
Native fire spreading and water flowing remain active: their resulting changes
are ordinary world simulation, not a region-wide rollback. Protection lasts
15 seconds by default and uses persistent domain simulation time.

## Configuration and shared data

Optional values in `extensions/magic-example/config.toml` (restart after editing):

```toml
maximum_mana = 100.0
regeneration = 4.0 # per second
costs = [12.0, 10.0, 8.0, 18.0, 22.0, 20.0, 35.0, 25.0, 30.0]
damage = [6.0, 3.0, 5.0, 2.0, 0.0, 0.0, 8.0]
projectile_speed = 1.4
terrain_ticks = 200
protection_ticks = 300
protection_multiplier = 0.25 # remaining damage; 0 completely cancels damage
physical_causes = ["minecraft:mob_attack", "minecraft:mob_attack_no_aggro", "minecraft:sting"]
physical_tags = ["minecraft:is_player_attack"]
```

Vanilla has no general physical-damage tag. Owners can broaden that policy by
adding registered damage causes or tags; fall, explosions and environmental
damage are otherwise unaffected by the physical shield.

The current-domain player compound is public:

| Path | Meaning |
| --- | --- |
| `magic:state/mana/current` | Current numeric mana; spending is atomic. |
| `magic:state/mana/maximum` | Numeric capacity, initialized once and respected on later dependency writes. |
| `magic:state/protection/projectile_until` | Domain game-time deadline as an NBT long. |
| `magic:state/protection/physical_until` | Domain game-time deadline as an NBT long. |

HUD source `magic-example:default` exports `player_mana` as a gauge alias and
`player_max_mana` as its capacity. It contributes current/maximum mana and a
20-character blue meter to the shared sidebar scoreboard, with an aqua mana
label and no title label. When the creatures example
is installed, both sections appear together without either extension importing
the other. The owner can use `${player_mana}` and `${bar(player_mana, 20)}` in other HUD
surfaces. Dependency fields are preserved. See the host's
[player data and HUD guide](../../doc/extensions/player-data-and-hud.md).

## Verification

```sh
cargo build --manifest-path extension-example/creatures/Cargo.toml
cargo build --manifest-path extension-example/magic/Cargo.toml
cargo test -p steel-extension-api --features host --lib host::creatures_and_magic -- --ignored
```

The integration test loads both native libraries together and exercises all nine
spells, capture, independent HUD updates and dependency edits through a real
server and vanilla packet handlers.
