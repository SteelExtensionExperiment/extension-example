use crate::{api, config, state};
use steel_extension_api::{
    BlockPos, PlayerId, Server, World,
    data::Value,
    effects::{Effect, Particle, SoundCategory},
    events::EntityInfo,
    items::ItemSpec,
    projectiles::{Impact, ProjectileSpec},
    stabby,
};

pub const NAMES: [&str; 9] = [
    "Fireball",
    "Ice projectile",
    "Stone projectile",
    "Summon fire",
    "Earth wall",
    "Water spring",
    "Lightning",
    "Projectile shield",
    "Physical shield",
];
pub const ICONS: [&str; 9] = [
    "minecraft:fire_charge",
    "minecraft:snowball",
    "minecraft:cobblestone",
    "minecraft:blaze_powder",
    "minecraft:dirt",
    "minecraft:water_bucket",
    "minecraft:amethyst_shard",
    "minecraft:shield",
    "minecraft:iron_chestplate",
];

pub fn cast(server: &Server<'_>, player: PlayerId, spell: usize) -> Result<(), String> {
    let config = config()?;
    let cost = *config.costs.get(spell).ok_or("unknown spell")?;
    state::initialize(server, player)?;
    let resident = server
        .players()
        .into_iter()
        .find(|p| p.id == player)
        .ok_or("caster is not resident in a world")?;
    let data = api(server.player_data(player))?;
    if !api(data.spend(state::MANA.into(), cost))? {
        return Err("Not enough mana".into());
    }
    let result = server
        .with_world(resident.world.as_str(), |world| {
            if spell >= 7 {
                let expiry = world
                    .game_time()
                    .checked_add(i64::from(config.protection_ticks))
                    .ok_or("protection expiry overflow")?;
                let path = if spell == 7 {
                    state::PROJECTILE
                } else {
                    state::PHYSICAL
                };
                api(data.set(path.into(), Value::integer(expiry)))?;
                api(world.spawn_particle(
                    Particle::new("minecraft:enchant", resident.position, 24).spread(0.7, 1.0, 0.7),
                ))?;
            } else {
                api(world.throw_projectile(
                    player,
                    ItemSpec::new(ICONS[spell]),
                    ProjectileSpec {
                        speed: config.projectile_speed,
                        uncertainty: 0.0,
                        gravity: 0.0,
                        lifetime: 100,
                    },
                    impact,
                    spell as u64,
                ))?;
            }
            Ok::<_, String>(())
        })
        .and_then(|result| result);
    if let Err(error) = result {
        api(data.add_clamped(
            state::MANA.into(),
            cost,
            0.0,
            state::number(server, player, state::MAXIMUM)?,
        ))?;
        return Err(error);
    }
    state::publish(server, player)?;
    api(server.message(player, format!("Cast {}", NAMES[spell]).as_str().into()))?;
    Ok(())
}

#[stabby::export(callback)]
fn impact(server: Server<'_>, impact: Impact<'_>, token: u64) {
    if let Err(error) = resolve(&impact, token as usize) {
        steel_extension_api::log::warn(format!("spell impact: {error}"));
        let owner: Option<EntityInfo> = impact.owner().into();
        if let Some(player) = owner.and_then(|e| Option::from(e.player))
            && let Err(error) = api(server.message(player, error.as_str().into()))
        {
            steel_extension_api::log::warn(error);
        }
    }
}

fn terrain_position(impact: &Impact<'_>) -> BlockPos {
    let block: Option<BlockPos> = impact.block().into();
    let face: Option<steel_extension_api::events::Face> = impact.face().into();
    if let (Some(block), Some(face)) = (block, face) {
        return face.relative(block);
    }
    let position = impact.position();
    BlockPos {
        x: position.x.floor() as i32,
        y: position.y.floor() as i32,
        z: position.z.floor() as i32,
    }
}

fn temporary(world: &World<'_>, position: BlockPos, block: &str) -> Result<bool, String> {
    let air = api(steel_extension_api::default_block_state(
        "minecraft:air".into(),
    ))?;
    let placed = api(steel_extension_api::default_block_state(block.into()))?;
    api(world.set_temporary_block(position, air, placed, config()?.terrain_ticks))
}

fn resolve(impact: &Impact<'_>, spell: usize) -> Result<(), String> {
    let config = config()?;
    let damage = *config.damage.get(spell).ok_or("unknown projectile spell")?;
    let world = impact.world();
    let position = impact.position();
    let target: Option<EntityInfo> = impact.target().into();
    match spell {
        0 => {
            api(impact.hurt_target("minecraft:fireball".into(), damage))?;
            if let Some(target) = target {
                api(world.ignite_entity(target.identity(), 80))?;
            }
            api(world.spawn_particle(
                Particle::new("minecraft:flame", position, 24).spread(0.3, 0.3, 0.3),
            ))?;
        }
        1 => {
            api(impact.hurt_target("minecraft:thrown".into(), damage))?;
            if let Some(target) = target {
                api(world
                    .affect_entity(target.identity(), Effect::new("minecraft:slowness", 100, 2)))?;
            }
            api(world.spawn_particle(
                Particle::new("minecraft:snowflake", position, 24).spread(0.5, 0.5, 0.5),
            ))?;
        }
        2 => {
            api(impact.hurt_target("minecraft:thrown".into(), damage))?;
            api(world.spawn_particle(
                Particle::new("minecraft:block", position, 24)
                    .block("minecraft:stone")
                    .spread(0.3, 0.3, 0.3),
            ))?;
        }
        3 => {
            let owner: Option<EntityInfo> = impact.owner().into();
            for entity in api(world.entities_near(position, 2.0))? {
                if owner
                    .as_ref()
                    .is_some_and(|owner| owner.uuid == entity.uuid)
                {
                    continue;
                }
                api(world.hurt_entity(
                    entity.identity(),
                    "minecraft:magic".into(),
                    damage,
                    owner.as_ref().map(EntityInfo::identity).into(),
                ))?;
                api(world.ignite_entity(entity.identity(), 100))?;
            }
            // Fire keeps native spread and scheduled updates; any intervening change
            // cancels the temporary claim instead of overwriting a newer world state.
            temporary(&world, terrain_position(impact), "minecraft:fire")?;
        }
        4 => {
            let origin = terrain_position(impact);
            let owner: Option<EntityInfo> = impact.owner().into();
            let along_x = owner.as_ref().is_none_or(|owner| {
                (position.z - owner.position.z).abs() >= (position.x - owner.position.x).abs()
            });
            let mut changed = false;
            for height in 0..3 {
                for offset in -1..=1 {
                    let pos = BlockPos {
                        x: origin.x + if along_x { offset } else { 0 },
                        y: origin.y + height,
                        z: origin.z + if along_x { 0 } else { offset },
                    };
                    changed |= temporary(&world, pos, "minecraft:dirt")?;
                }
            }
            if !changed {
                return Err("Earth wall needs loaded air blocks".into());
            }
        }
        5 => {
            if !temporary(&world, terrain_position(impact), "minecraft:water")? {
                return Err("Water spring needs a loaded air block".into());
            }
        }
        6 => {
            // This is an authored area spell: a vanilla visual bolt and explicit damage.
            // It does not create a full LightningBolt entity or transform mobs.
            let _viewers = world.strike_lightning_effect(position);
            let owner: Option<EntityInfo> = impact.owner().into();
            for entity in api(world.entities_near(position, 2.0))? {
                if owner
                    .as_ref()
                    .is_some_and(|owner| owner.uuid == entity.uuid)
                {
                    continue;
                }
                api(world.hurt_entity(
                    entity.identity(),
                    "minecraft:lightning_bolt".into(),
                    damage,
                    owner.as_ref().map(EntityInfo::identity).into(),
                ))?;
                api(world.ignite_entity(entity.identity(), 80))?;
            }
        }
        _ => return Err("unknown projectile spell".into()),
    }
    api(world.play_sound(
        "minecraft:block.enchantment_table.use".into(),
        position,
        SoundCategory::Players,
        0.7,
        1.0,
    ))?;
    Ok(())
}
