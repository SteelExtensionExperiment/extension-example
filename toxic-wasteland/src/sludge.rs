//! Sludge gameplay: poison and contact damage, water corruption, ground withering,
//! and lava reactions. Spreading itself is the host's vanilla algorithm.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use rand::RngExt as _;
use steel_extension_api::{
    BlockPos, BlockState, Position, World,
    block_behavior::types::Entity,
    block_key,
    effects::{Effect, Particle, SoundCategory},
    fluid_behavior::FluidBehavior,
    log,
};

use crate::{Materials, config::Config};

/// Blocks next to each fluid cell, including below and above.
const NEIGHBORS: [(i32, i32, i32); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

/// Ground that sludge withers into coarse dirt.
const WITHERING: [&str; 5] = [
    "minecraft:grass_block",
    "minecraft:dirt",
    "minecraft:podzol",
    "minecraft:mycelium",
    "minecraft:rooted_dirt",
];

/// How often stale damage pacing entries are dropped, in ticks.
const PRUNE_INTERVAL: i64 = 100;

const fn offset(pos: BlockPos, (x, y, z): (i32, i32, i32)) -> BlockPos {
    BlockPos {
        x: pos.x + x,
        y: pos.y + y,
        z: pos.z + z,
    }
}

const fn center(pos: BlockPos) -> Position {
    Position {
        x: pos.x as f64 + 0.5,
        y: pos.y as f64 + 0.5,
        z: pos.z as f64 + 0.5,
    }
}

/// The `level` of a vanilla water state, or `None` for anything else.
fn water_level(state: BlockState) -> Option<u8> {
    if block_key(state).as_str() != "minecraft:water" {
        return None;
    }
    let level: Option<_> = state.property("level".into()).into();
    level.and_then(|level: steel_extension_api::stabby::string::String| level.as_str().parse().ok())
}

pub struct Sludge {
    config: &'static Config,
    materials: &'static Materials,
    /// Game time of each entity's last contact damage, by UUID.
    last_hit: Mutex<HashMap<[u8; 16], i64>>,
}

impl Sludge {
    pub fn new(config: &'static Config, materials: &'static Materials) -> Self {
        Self {
            config,
            materials,
            last_hit: Mutex::new(HashMap::new()),
        }
    }

    /// Whether this entity may take contact damage now, recording the hit if so.
    fn take_hit_slot(&self, entity: [u8; 16], now: i64) -> bool {
        let interval = self.config.effects.damage_interval_ticks;
        let mut last_hit = self.last_hit.lock().unwrap_or_else(PoisonError::into_inner);
        if now % PRUNE_INTERVAL == 0 {
            last_hit.retain(|_, tick| now - *tick < interval.max(PRUNE_INTERVAL));
        }
        match last_hit.get(&entity) {
            Some(tick) if now - *tick < interval => false,
            _ => {
                last_hit.insert(entity, now);
                true
            }
        }
    }

    fn set(world: &World<'_>, pos: BlockPos, state: BlockState) {
        if let Err(error) = world.set_block(pos, state).match_owned(Ok, Err) {
            log::warn(format!("sludge could not change {pos:?}: {error}"));
        }
    }

    fn fizz(world: &World<'_>, pos: BlockPos) {
        let at = center(pos);
        let _ = world.play_sound(
            "minecraft:block.lava.extinguish".into(),
            at,
            SoundCategory::Blocks,
            0.5,
            2.6,
        );
        let _ = world.spawn_particle(Particle::new("minecraft:large_smoke", at, 8));
    }
}

impl FluidBehavior for Sludge {
    extern "C" fn entity_inside<'a>(
        &self,
        world: World<'a>,
        _pos: BlockPos,
        _state: BlockState,
        entity: Entity<'a>,
    ) {
        if !entity.is_living() || entity.is_creative_or_spectator() {
            return;
        }
        if !self.take_hit_slot(entity.uuid().bytes, world.game_time()) {
            return;
        }
        let effects = &self.config.effects;
        if effects.poison_seconds > 0 {
            // Undead and other immune entities refuse poison but still take damage.
            let poison = Effect::new(
                "minecraft:poison",
                effects.poison_seconds * 20,
                effects.poison_amplifier,
            );
            let _ = entity.add_effect(poison);
        }
        if effects.contact_damage > 0.0 {
            let _ = entity.hurt("minecraft:magic".into(), effects.contact_damage);
        }
    }

    extern "C" fn random_tick(&self, world: World<'_>, pos: BlockPos, _state: BlockState) {
        let spread = &self.config.spread;
        let mut rng = rand::rng();
        for direction in NEIGHBORS {
            let target = offset(pos, direction);
            let Ok(state) = world.block(target).match_owned(Ok, Err) else {
                continue;
            };
            if let Some(level) = water_level(state) {
                if rng.random_bool(spread.corrupt_water_chance) {
                    Self::set(&world, target, self.materials.sludge[usize::from(level)]);
                }
            } else if WITHERING.contains(&block_key(state).as_str())
                && rng.random_bool(spread.wither_ground_chance)
            {
                Self::set(&world, target, self.materials.coarse_dirt);
            }
        }
    }

    /// Lava touching sludge cools like it does against water, into coarse dirt (source)
    /// or cobblestone (flowing). The sludge itself stays and keeps spreading.
    extern "C" fn react_with_neighbors(
        &self,
        world: World<'_>,
        pos: BlockPos,
        _state: BlockState,
    ) -> bool {
        for direction in NEIGHBORS {
            if direction == (0, 1, 0) {
                // Lava above flows down onto sludge and reacts from its own side.
                continue;
            }
            let target = offset(pos, direction);
            let Ok(state) = world.block(target).match_owned(Ok, Err) else {
                continue;
            };
            if block_key(state).as_str() != "minecraft:lava" {
                continue;
            }
            let level: Option<steel_extension_api::stabby::string::String> =
                state.property("level".into()).into();
            let cooled = if level.is_some_and(|level| level.as_str() == "0") {
                self.materials.coarse_dirt
            } else {
                self.materials.cobblestone
            };
            Self::set(&world, target, cooled);
            Self::fizz(&world, target);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_cover_all_six_faces() {
        let origin = BlockPos { x: 0, y: 0, z: 0 };
        let sum = NEIGHBORS.iter().fold((0, 0, 0), |acc, d| {
            let p = offset(origin, *d);
            (acc.0 + p.x.abs(), acc.1 + p.y.abs(), acc.2 + p.z.abs())
        });
        assert_eq!(sum, (2, 2, 2));
    }
}
