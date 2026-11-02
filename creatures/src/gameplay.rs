use super::{api, balls, battle, config, data};
use steel_extension_api::stabby;
use steel_extension_api::{
    PlayerId, Position, Server,
    items::ItemSpec,
    projectiles::{Impact, ProjectileSpec},
};

const OCCUPIED: u64 = 1 << 32;
pub(super) const DEPLOYMENT_OFFSETS: [(f64, f64); 6] = [
    (2.0, 0.0),
    (-2.0, 0.0),
    (0.0, 2.0),
    (0.0, -2.0),
    (2.0, 2.0),
    (-2.0, -2.0),
];

pub fn throw(
    server: &Server<'_>,
    player: PlayerId,
    target: Option<Position>,
) -> Result<(), String> {
    let held = api(server.held_slot(player))?;
    let mut item = api(server.slot(player, held))?;
    let Some(grade) = balls::KEYS
        .iter()
        .position(|key| *key == item.item.as_str())
    else {
        return Ok(());
    };
    let config = config()?;
    let resident = server
        .players()
        .into_iter()
        .find(|p| p.id == player)
        .ok_or("trainer is not resident in a world")?;
    let now = server.with_world(resident.world.as_str(), |world| world.game_time())?;
    if !data::reserve_throw(server, player, now, config.throw_cooldown_ticks)? {
        return Ok(());
    }
    let token = if let Some(slot) = item.data("creatures:slot") {
        if item.data("creatures:owner") != Some(player.uuid().to_string().as_str()) {
            return Err("this occupied ball belongs to another trainer".into());
        }
        let slot = slot.parse::<usize>().map_err(|e| e.to_string())?;
        if slot >= data::PARTY {
            return Err("invalid party slot".into());
        }
        let path = data::path(slot);
        data::validate_ball(server, player, slot, &item)?;
        if api(server.stored_entity_instances(player))?
            .iter()
            .any(|d| d.path.as_str() == path)
        {
            api(server.return_entity(player, path.as_str().into()))?;
            return Ok(());
        }
        OCCUPIED | slot as u64
    } else {
        grade as u64
    };
    server.with_world(resident.world.as_str(), |world| {
        let item = if token & OCCUPIED == 0 {
            ItemSpec::new(balls::KEYS[grade])
        } else {
            data::token(server, player, (token & !OCCUPIED) as usize)?
        };
        let spec = ProjectileSpec {
            speed: config.projectile_speed,
            uncertainty: 0.0,
            gravity: 0.03,
            lifetime: 200,
        };
        match target {
            Some(target) => {
                api(world.throw_projectile_towards(player, target, item, spec, impact, token))
            }
            None => api(world.throw_projectile(player, item, spec, impact, token)),
        }
    })??;
    if token & OCCUPIED == 0 {
        item.count -= 1;
        api(server.set_slot(player, held, item))?;
    }
    Ok(())
}

#[stabby::export(callback)]
fn impact(server: Server<'_>, impact: Impact<'_>, token: u64) {
    if let Err(error) = resolve(&server, &impact, token) {
        let owner: Option<steel_extension_api::events::EntityInfo> = impact.owner().into();
        if let Some(owner) = owner
            && let Some(player) = Option::<PlayerId>::from(owner.player)
        {
            super::report_error(&server, player, error);
        } else {
            steel_extension_api::log::warn(format!("ball impact: {error}"));
        }
    }
}

fn resolve(server: &Server<'_>, impact: &Impact<'_>, token: u64) -> Result<(), String> {
    let owner: Option<steel_extension_api::events::EntityInfo> = impact.owner().into();
    let owner = owner.ok_or("ball's trainer left this world")?;
    let player: Option<PlayerId> = owner.player.into();
    let player = player.ok_or("ball has no player owner")?;
    let target: Option<steel_extension_api::events::EntityInfo> = impact.target().into();
    if token & OCCUPIED != 0 {
        let slot = (token & !OCCUPIED) as usize;
        data::validate_ball(server, player, slot, &impact.item())?;
        return deploy(server, player, slot, impact.position(), target);
    }
    let Some(target) = target else {
        return Ok(());
    };
    if target.player.is_some() {
        return Err("players cannot be captured".into());
    }
    let world = impact.world();
    if !api(world.entity_has_ai(target.identity()))? {
        return Err("only mobs can be captured".into());
    }
    let vitals = api(world.entity_vitals(target.identity()))?;
    if vitals.health <= 0.0 {
        return Err("only living mobs can be captured".into());
    }
    if rand::random::<f64>() >= config()?.chance(token as usize, vitals.health, vitals.maximum)? {
        api(server.message(
            player,
            "The creature escaped! Weaken it or use a better ball.".into(),
        ))?;
        return Ok(());
    }
    let mut vacant = None;
    for slot in 0..data::PARTY {
        if data::read(server, player, &data::path(slot))?.is_none() {
            vacant = Some(slot);
            break;
        }
    }
    let slot = vacant.ok_or("your party is full")?;
    let path = data::path(slot);
    api(world.store_entity(
        player,
        target.identity(),
        path.as_str().into(),
        battle::policy(),
    ))?;
    let name = target
        .kind
        .as_str()
        .strip_prefix("minecraft:")
        .unwrap_or(target.kind.as_str())
        .replace('_', " ");
    let player_data = api(server.player_data(player))?;
    api(player_data.set(
        format!("{path}/label").as_str().into(),
        steel_extension_api::data::Value::text(name.as_str().into()),
    ))?;
    api(player_data.set(
        format!("{path}/grade").as_str().into(),
        steel_extension_api::data::Value::integer(token as i64),
    ))?;
    api(server.give(player, data::token(server, player, slot)?))?;
    data::hud(server, player)?;
    api(server.message(player, format!("Captured {name}!").as_str().into()))?;
    Ok(())
}

fn deploy(
    server: &Server<'_>,
    player: PlayerId,
    slot: usize,
    impact: Position,
    target: Option<steel_extension_api::events::EntityInfo>,
) -> Result<(), String> {
    let path = data::path(slot);
    let anchor = target.as_ref().map_or(impact, |entity| entity.position);
    let mut error = "no deployment position".to_owned();
    for (x, z) in DEPLOYMENT_OFFSETS {
        match api(server.restore_entity(
            player,
            path.as_str().into(),
            Position {
                x: anchor.x + x,
                y: anchor.y,
                z: anchor.z + z,
            },
            battle::policy(),
        )) {
            Ok(ally) => {
                if let Some(target) = target
                    && let Err(error) = battle::start(server, player, ally, target)
                {
                    api(server.return_entity(player, path.as_str().into()))?;
                    return Err(error);
                }
                return Ok(());
            }
            Err(reason) => error = reason,
        }
    }
    Err(error)
}
