use crate::{api, config, state};
use steel_extension_api::{PlayerId, Server, events::DamageEvent, stabby};

fn protected(
    event: &DamageEvent<'_>,
    server: &Server<'_>,
    player: PlayerId,
) -> Result<bool, String> {
    let now = server.with_world(event.world().as_str(), |world| world.game_time())?;
    if state::active(server, player, state::PROJECTILE, now)?
        && api(event.cause_in_tag("minecraft:is_projectile".into()))?
    {
        return Ok(true);
    }
    if !state::active(server, player, state::PHYSICAL, now)? {
        return Ok(false);
    }
    if config()?
        .physical_causes
        .iter()
        .any(|key| key == event.cause().as_str())
    {
        return Ok(true);
    }
    for tag in &config()?.physical_tags {
        if api(event.cause_in_tag(tag.as_str().into()))? {
            return Ok(true);
        }
    }
    Ok(false)
}
#[stabby::export(callback)]
pub fn damage(mut event: DamageEvent<'_>, _: u64) {
    let Some(player): Option<PlayerId> = event.victim().player.into() else {
        return;
    };
    let Some(server): Option<Server<'_>> = event.player_server().into() else {
        return;
    };
    match protected(&event, &server, player) {
        Ok(true) => match config() {
            Ok(config) => {
                if config.protection_multiplier == 0.0 {
                    event.set_cancelled(true);
                } else {
                    event.set_amount(event.amount() * config.protection_multiplier);
                }
            }
            Err(error) => steel_extension_api::log::error(error),
        },
        Ok(false) => {}
        Err(error) => steel_extension_api::log::error(error),
    }
}
