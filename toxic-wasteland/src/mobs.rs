//! Wasteland mobs and their spawning.
//!
//! - **Sludge Slime**: a slime that poisons what it touches, leaves a sludge puddle
//!   where it dies, and splits into small sludge slimes.
//! - **Irradiated Ghoul**: a new mob, a husk body with a glowing radioactive face,
//!   whose blows poison and sicken.
//!
//! Steel has no natural spawner yet, so the extension spawns them on the surface
//! around players in the wasteland, up to a cap per player.

use steel_extension_api::{
    ApiResult, BlockPos, Position, Server, World,
    block_behavior::types::Entity,
    block_key,
    content::{ContentRegistry, EntityDefinition},
    effects::Effect,
    entity_behavior::{EntityBehavior, api::EntityBehaviors},
    log,
    stabby::{self, string::String as ApiString},
};

use crate::model::GLOWING_ONE;

pub const SLUDGE_SLIME: &str = "toxic_wasteland:sludge_slime";
pub const SMALL_SLUDGE_SLIME: &str = "toxic_wasteland:small_sludge_slime";
pub const GHOUL: &str = "toxic_wasteland:irradiated_ghoul";
pub const WASTELAND_WORLD: &str = "minecraft:wasteland";

/// Players further than this from a mob type count separately toward its cap.
const CAP_RADIUS: f64 = 64.0;

fn definitions() -> [EntityDefinition; 3] {
    [
        EntityDefinition::cube(SLUDGE_SLIME, "minecraft:slime", "Sludge Slime", 2)
            .attribute("minecraft:max_health", 10.0)
            .attribute("minecraft:attack_damage", 3.0)
            .attribute("minecraft:movement_speed", 0.4),
        EntityDefinition::cube(
            SMALL_SLUDGE_SLIME,
            "minecraft:slime",
            "Small Sludge Slime",
            1,
        )
        .attribute("minecraft:max_health", 2.0)
        .attribute("minecraft:attack_damage", 0.0)
        .attribute("minecraft:movement_speed", 0.3),
        EntityDefinition::melee(GHOUL, "minecraft:husk", "Irradiated Ghoul")
            .attribute("minecraft:max_health", 30.0)
            .attribute("minecraft:attack_damage", 5.0)
            .attribute("minecraft:movement_speed", 0.26)
            .attribute("minecraft:armor", 4.0)
            .attribute("minecraft:follow_range", 32.0)
            .equip("head", GLOWING_ONE.appearance())
            .equip(
                "chest",
                steel_extension_api::content::ItemAppearance {
                    item: "minecraft:leather_chestplate".into(),
                    model: None.into(),
                    profile: None.into(),
                },
            ),
    ]
}

/// Registers the three mob types during content registration.
pub fn register(registry: &mut ContentRegistry<'_>) -> Result<(), ApiString> {
    for definition in definitions() {
        registry.register_entity(definition).match_owned(Ok, Err)?;
    }
    Ok(())
}

fn position_block(position: Position) -> BlockPos {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "world coordinates fit i32 once floored"
    )]
    BlockPos {
        x: position.x.floor() as i32,
        y: position.y.floor() as i32,
        z: position.z.floor() as i32,
    }
}

fn poison(target: &Entity<'_>, seconds: i32, amplifier: i32) {
    if seconds > 0 {
        let _ = target.add_effect(Effect::new("minecraft:poison", seconds * 20, amplifier));
    }
}

/// Sludge slimes: poison on contact, a puddle and a split on death.
struct SludgeSlime {
    /// Small slimes spawned when this one dies; 0 for the small kind.
    split: u32,
}

impl EntityBehavior for SludgeSlime {
    extern "C" fn attacked<'a>(&self, _world: World<'a>, _mob: Entity<'a>, target: Entity<'a>) {
        poison(&target, 4, 0);
    }

    extern "C" fn died<'a>(&self, world: World<'a>, mob: Entity<'a>) {
        let at = mob.position();
        let block = position_block(at);
        if let Ok(state) = world.block(block).match_owned(Ok, Err)
            && block_key(state).as_str() == "minecraft:air"
        {
            let _ = world.set_block(block, crate::materials().sludge[0]);
        }
        for index in 0..self.split {
            let offset = f64::from(index) - f64::from(self.split - 1) / 2.0;
            let spot = Position {
                x: at.x + offset * 0.5,
                y: at.y + 0.5,
                z: at.z + offset * 0.3,
            };
            if let Err(error) = world
                .spawn_entity(SMALL_SLUDGE_SLIME.into(), spot)
                .match_owned(Ok, Err)
            {
                log::warn(format!("a sludge slime could not split: {error}"));
            }
        }
    }
}

/// Irradiated ghouls: their blows poison and sicken.
struct Ghoul;

impl EntityBehavior for Ghoul {
    extern "C" fn attacked<'a>(&self, _world: World<'a>, _mob: Entity<'a>, target: Entity<'a>) {
        poison(&target, 5, 0);
        let _ = target.add_effect(Effect::new("minecraft:nausea", 80, 0));
    }
}

#[stabby::export(callback)]
pub fn register_behaviors(mut registry: EntityBehaviors<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        registry
            .set_entity_behavior(
                SLUDGE_SLIME.into(),
                stabby::boxed::Box::new(SludgeSlime { split: 2 }).into(),
            )
            .match_owned(Ok, Err)?;
        registry
            .set_entity_behavior(
                SMALL_SLUDGE_SLIME.into(),
                stabby::boxed::Box::new(SludgeSlime { split: 0 }).into(),
            )
            .match_owned(Ok, Err)?;
        registry
            .set_entity_behavior(GHOUL.into(), stabby::boxed::Box::new(Ghoul).into())
            .match_owned(Ok, Err)
    })();
    result.into()
}

/// A spot on the surface between `min` and `max` blocks from `around`, in a direction
/// picked by `roll`, or `None` if the column has no dry ground near the player's height.
fn surface_spot(
    world: &World<'_>,
    around: Position,
    roll: u64,
    (min, max): (f64, f64),
) -> Option<Position> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a random angle needs no exact integer"
    )]
    let angle = (roll % 3600) as f64 / 3600.0 * std::f64::consts::TAU;
    #[expect(
        clippy::cast_precision_loss,
        reason = "a random distance needs no exact integer"
    )]
    let distance = min + ((roll >> 16) % 1000) as f64 / 1000.0 * (max - min);
    let x = around.x + angle.cos() * distance;
    let z = around.z + angle.sin() * distance;
    let start = position_block(Position {
        x,
        y: around.y + 16.0,
        z,
    });
    for y in (start.y - 40..=start.y).rev() {
        let Ok(below) = world.block(BlockPos { y, ..start }).match_owned(Ok, Err) else {
            return None;
        };
        let key = block_key(below);
        let key = key.as_str();
        if key == "minecraft:air" || key.ends_with("pale_hanging_moss") {
            continue;
        }
        if key == crate::content::SLUDGE || key.ends_with("_log") || key == "minecraft:bedrock" {
            return None;
        }
        return Some(Position {
            x: x.floor() + 0.5,
            y: f64::from(y + 1),
            z: z.floor() + 0.5,
        });
    }
    None
}

/// Every `interval_ticks`, tops each wasteland player's surroundings up to the caps.
#[stabby::export(callback)]
pub fn spawn_tick(server: Server<'_>, tick: u64) {
    let config = &crate::config().mobs;
    if !config.enabled || !tick.is_multiple_of(config.interval_ticks) {
        return;
    }
    let players: Vec<Position> = server
        .players()
        .iter()
        .filter(|player| player.world.as_str() == WASTELAND_WORLD && player.health > 0.0)
        .map(|player| player.position)
        .collect();
    if players.is_empty() {
        return;
    }
    let result = server.with_world(WASTELAND_WORLD, |world| {
        for (index, around) in players.iter().enumerate() {
            let roll = rand::random::<u64>() ^ (index as u64);
            for (kind, cap) in [
                (GHOUL, config.max_ghouls),
                (SLUDGE_SLIME, config.max_slimes),
            ] {
                let count = world
                    .count_entities(kind.into(), *around, CAP_RADIUS)
                    .match_owned(|count| count, |_| u32::MAX);
                if count >= cap {
                    continue;
                }
                if let Some(spot) =
                    surface_spot(&world, *around, roll.rotate_left(17), (20.0, 40.0))
                {
                    let _ = world.spawn_entity(kind.into(), spot);
                }
            }
        }
    });
    if let Err(error) = result {
        log::warn(format!("wasteland mob spawning failed: {error}"));
    }
}
