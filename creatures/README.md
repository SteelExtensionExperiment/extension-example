# Creatures

A separate capture-and-battle extension for an unmodified Minecraft client. It
does not depend on the magic example or require a resource pack.

From the Steel repository root:

```sh
python3 package-extension.py extension-example/creatures --debug
cargo run
```

Use `/kit` for Poké, Super, and Hyper balls. When several extensions register
`kit`, use `/creatures:kit` to select this extension. Aim and right-click to
throw an empty ball at a mob. Higher grades improve the capture chance; lower
target health improves it further. A miss or failed attempt consumes the ball.
Successful capture gives an occupied ball and adds the creature to the sidebar.
The scoreboard shows the first three occupied party slots with their names, current/maximum
health, ball grades and stored/deployed status. The six-slot party remains available
through `/party`. With the magic example installed, mana appears in the same
scoreboard through the shared HUD compositor.
Names are gold, health is green, and deployed status is aqua. The default
scoreboard has no title label.
Ball use has a four-domain-tick interval by default, coalescing vanilla
entity-interaction/item-use packets from the same click.

Throw an occupied ball at another mob to deploy your creature and begin reciprocal
melee combat. Ground mobs use native navigation; bats pursue their opponent using
native flight steering. Both use native reach, sight and damage. This extension's
combat controller stops at one health point; your creature is then recalled.
Throw an empty ball to capture the weakened opponent. Throw an occupied ball at
terrain to deploy without fighting; use that occupied ball again to recall it.
`/party` opens a six-slot roster and lets you recover occupied-ball
controls. Extra controls reference the same creature; they cannot duplicate it. If another
extension also registers `party`, `/creatures:party` selects this extension.
The saved party owns the creatures; inventory balls are reusable controls.
Clearing your inventory keeps the party, and `/party` can give its balls back.
Click the red **Release** button beneath a creature in `/party` to return it to
the world and free its slot. A stored creature appears beside you if there is a
loaded, collision-free position; a deployed creature stays in place and leaves
combat. Blocked release keeps the creature in your party. Released creatures
keep their native identity and health, save with their chunk, and can be captured
again. Stored creatures saved at zero health by older builds recover to one
health point when deployed or released.

Balls and in-flight projectiles carry a per-capture control identity. Releasing
and recapturing the same mob creates a new identity, so old controls cannot deploy
the replacement in a reused slot. If you have balls from an older build, recover
fresh controls with `/party`.

Capture preserves the entity's UUID, native saved state, health, attributes and
effects. Deployed creatures belong to the player's current domain save and are
excluded from chunk saves. Disconnect, domain departure and shutdown recall
them. Respawn transfers their ownership to the replacement player. Native
replacement conversions, such as shearing a mooshroom into a cow, keep the
converted mob in the same party slot. The destination must also be a supported,
loadable mob. Independent
portal travel is disabled for owned creatures. Owned creatures cannot fall below
one health point, including from external damage or direct health changes.
They remain deployable, healable and releasable at that threshold. Released
creatures return to normal vanilla health and death rules.
Recall preserves health. A creature at one health point needs healing before
another battle; an opponent already at that threshold is ready for capture.
To recover a living creature, throw its occupied ball at terrain, splash it with
a vanilla Potion of Healing, then recall it. This example has no separate
healing menu; vanilla healing works because creatures remain alive.

Living mobs with a native load factory are eligible for capture and deployment,
including bats. Battles require ground navigation or a direct-flight steering
capability. Bats can fight other bats and ground mobs; their battle flight controller
retires when combat ends, releasing, recalling, or replacing the creature.
Outside battles they use their native resting and random flight AI. Unsupported
entities, players, leashed mobs, vehicles and passengers are rejected. Existing
native AI flags and supported entity state remain meaningful; a `NoAI` mob does
not acquire navigation merely because it was captured.

## Configuration and shared data

Put these optional values in `extensions/creatures-example/config.toml` and restart:

```toml
base_chance = [0.25, 0.45, 0.70]
weakened_bonus = 0.40
battle_damage = 3.0
projectile_speed = 1.5
throw_cooldown_ticks = 4
```

Chance is `base_chance[grade] + weakened_bonus * (1 - health / maximum)`, clamped
to 0–1. These are authored game rules, not vanilla mechanics.

Each current-domain slot is a compound at `creatures:state/party/0` through `/5`:

| Field | Meaning |
| --- | --- |
| `entity` | Native serialized entity snapshot; use the generic entity storage API to restore it. |
| `uuid`, `kind` | Persistent identity and registered entity type. |
| `health/current`, `health/maximum` | Public numeric gauge. |
| `deployed` | Native ownership flag, stored as an NBT byte. |
| `label`, `grade` | Example-owned display name and ball grade. |
| `control` | Example-owned identity for this capture; invalidated by release. |

Dependencies may add fields such as `nickname`; snapshot updates preserve them.
HUD source `creatures-example:default` exports aliases `creature_1` through
`creature_6`, `creature_1_health` through `creature_6_health`, and
`creature_1_max_health` through `creature_6_max_health`. See the host's
[player data and HUD guide](../../doc/extensions/player-data-and-hud.md) for owner layouts.

The Poké, Super (Great), and Hyper (Ultra) head skins come from
[Poké Ball](https://minecraft-heads.com/custom-heads/head/77222-pokeball),
[Great Ball](https://minecraft-heads.com/custom-heads/head/77224-great-ball), and
[Ultra Ball](https://minecraft-heads.com/index.php/custom-heads/head/77223-ultra-ball)
on Minecraft Heads. Their texture payloads are in `src/balls.rs`.

## Verification

```sh
cargo build --manifest-path extension-example/creatures/Cargo.toml
cargo build --manifest-path extension-example/magic/Cargo.toml
cargo test -p steel-extension-api --features host --lib host::creatures_and_magic -- --ignored
cargo test -p steel-extension-api --features host --lib host::entities
```

The SDK exposes generic entity storage and AI controls. `src/battle.rs` owns this
example's health minimum, combat timing, movement requests, damage, completion,
and cancellation. Neither `steel-core` nor the SDK defines a creature or battle API.
