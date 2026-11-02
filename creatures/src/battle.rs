//! Creature-game rules built exclusively from the public entity controls.

use super::{api, config};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};
use steel_extension_api::{
    PlayerId, Server, World,
    entities::{EntityControl, EntityPolicy},
    events::EntityInfo,
    stabby,
    world_effects::EntityId,
};
use steel_utils::locks::SyncMutex;

pub const MINIMUM_HEALTH: f32 = 1.0;
pub fn policy() -> EntityPolicy {
    EntityPolicy {
        minimum_health: MINIMUM_HEALTH,
        drop_loot: false,
    }
}

#[derive(Default)]
struct Timing {
    cooldown: u32,
    repath: u32,
}
struct Battle {
    owner: steel_extension_api::players::Uuid,
    instance: steel_extension_api::players::Uuid,
    ally: EntityId,
    target: EntityId,
    damage: f32,
    active: bool,
    timing: [Timing; 2],
}
static BATTLES: SyncMutex<BTreeMap<u64, Battle>> = SyncMutex::new(BTreeMap::new());
static NEXT: AtomicU64 = AtomicU64::new(1);

pub fn start(
    server: &Server<'_>,
    owner: PlayerId,
    ally: EntityInfo,
    target: EntityInfo,
) -> Result<(), String> {
    let resident = server
        .players()
        .into_iter()
        .find(|p| p.id == owner)
        .ok_or("trainer is not in a world")?;
    let instance = api(server.stored_entity_instances(owner))?
        .into_iter()
        .find(|entry| entry.entity.identity() == ally.identity())
        .ok_or("deployed entity has no storage activation")?
        .instance;
    server.with_world(resident.world.as_str(), |world| {
        start_in(world, owner, instance, ally, target)
    })?
}
fn start_in(
    world: World<'_>,
    owner: PlayerId,
    instance: steel_extension_api::players::Uuid,
    ally: EntityInfo,
    target: EntityInfo,
) -> Result<(), String> {
    if ally.identity() == target.identity() || target.player.is_some() {
        return Err("battle needs two distinct living mobs".into());
    }
    for entity in [&ally, &target] {
        if !api(world.entity_has_ai(entity.identity()))? {
            return Err("combatant has no mob AI".into());
        }
        if api(world.entity_is_controlled(entity.identity()))? {
            return Err("combatant already has an AI controller".into());
        }
    }
    if api(world.entity_vitals(ally.identity()))?.health <= MINIMUM_HEALTH {
        return Err("your creature needs healing before it can battle again".into());
    }
    if api(world.entity_vitals(target.identity()))?.health <= MINIMUM_HEALTH {
        return Err("the opponent is already weakened; capture it or choose another mob".into());
    }
    let token = NEXT
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(1)
        })
        .map_err(|_| "battle identity exhausted")?;
    BATTLES.lock().insert(
        token,
        Battle {
            owner: owner.uuid(),
            instance,
            ally: ally.identity(),
            target: target.identity(),
            damage: config()?.battle_damage,
            active: true,
            timing: [Timing::default(), Timing::default()],
        },
    );
    for entity in [&ally, &target] {
        if let Err(error) = api(world.control_entity(entity.identity(), control, token)) {
            finish(token);
            return Err(error);
        }
    }
    Ok(())
}
fn finish(token: u64) {
    if let Some(battle) = BATTLES.lock().get_mut(&token) {
        battle.active = false;
    }
}
pub fn cancel(ally: EntityId) {
    for battle in BATTLES.lock().values_mut() {
        if battle.ally == ally {
            battle.active = false;
        }
    }
}
pub fn completed(
    owner: PlayerId,
    instance: steel_extension_api::players::Uuid,
    ally: EntityId,
) -> bool {
    let mut battles = BATTLES.lock();
    let token = battles.iter().find_map(|(token, battle)| {
        (battle.owner == owner.uuid()
            && battle.instance == instance
            && (!battle.active || battle.ally != ally))
            .then_some(*token)
    });
    if let Some(token) = token {
        battles.remove(&token);
        return true;
    }
    false
}

#[stabby::export(callback)]
fn control(control: EntityControl<'_>, token: u64) -> bool {
    match tick(&control, token) {
        Ok(true) => true,
        result => {
            if let Err(error) = result {
                steel_extension_api::log::warn(format!("creature battle: {error}"));
            }
            finish(token);
            false
        }
    }
}
fn tick(control: &EntityControl<'_>, token: u64) -> Result<bool, String> {
    let entity = control.entity();
    let (opponent, damage, ready, repath) = {
        let mut battles = BATTLES.lock();
        let Some(battle) = battles.get_mut(&token).filter(|battle| battle.active) else {
            return Ok(false);
        };
        let side = if entity.identity() == battle.ally {
            0
        } else if entity.identity() == battle.target {
            1
        } else {
            return Ok(false);
        };
        let timing = &mut battle.timing[side];
        timing.cooldown = timing.cooldown.saturating_sub(1);
        timing.repath = timing.repath.saturating_sub(1);
        let repath = timing.repath == 0;
        if repath {
            timing.repath = 10;
        }
        (
            if side == 0 {
                battle.target
            } else {
                battle.ally
            },
            battle.damage,
            timing.cooldown == 0,
            repath,
        )
    };
    let world = control.world();
    let Ok(target) = api(world.entity_info(opponent)) else {
        return Ok(false);
    };
    let own = api(world.entity_vitals(entity.identity()))?;
    let other = api(world.entity_vitals(opponent))?;
    let a = entity.position;
    let b = target.position;
    if own.health <= MINIMUM_HEALTH
        || other.health <= MINIMUM_HEALTH
        || (a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2) > 32.0 * 32.0
    {
        return Ok(false);
    }
    if !control.can_move() {
        return Err("combatant has no supported battle movement".into());
    }
    api(control.move_to_entity(opponent, 1.1, repath))?;
    if ready && api(control.can_melee_attack(opponent))? {
        api(control.hurt_target(
            opponent,
            "minecraft:mob_attack".into(),
            damage.min(other.health - MINIMUM_HEALTH),
        ))?;
        if let Some(battle) = BATTLES.lock().get_mut(&token) {
            let side = usize::from(entity.identity() != battle.ally);
            battle.timing[side].cooldown = 20;
        }
    }
    Ok(true)
}

pub fn retain_instances(owner: PlayerId, live: &[steel_extension_api::players::Uuid]) {
    BATTLES
        .lock()
        .retain(|_, battle| battle.owner != owner.uuid() || live.contains(&battle.instance));
}

pub fn retain_owners(live: &[steel_extension_api::players::Uuid]) {
    BATTLES
        .lock()
        .retain(|_, battle| live.contains(&battle.owner));
}
