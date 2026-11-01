//! `/kit` with per-player cooldowns.
use crate::{
    config::parse_kit_item,
    state::essentials,
    util::{api, duration, now_seconds},
};
use steel_extension_api::{
    Commands,
    command::{Arg, CommandBuilder, Invocation, argument},
};

fn kit_names(ctx: Option<&Invocation<'_, '_>>) -> Vec<String> {
    let Ok(ess) = essentials() else {
        return Vec::new();
    };
    ess.config
        .kits
        .iter()
        .filter(|(name, kit)| {
            ctx.is_none_or(|ctx| {
                ctx.has_permission(&format!("essentials.kits.{name}"), kit.default_access)
            })
        })
        .map(|(name, _)| name.clone())
        .collect()
}

fn list(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    ctx.send_legacy(&ess.msg("kits_list", &[("kits", &kit_names(Some(ctx)).join(", "))]));
    Ok(1)
}

fn give(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = ctx.player().map_err(|_| ess.msg("player_only", &[]))?;
    let name = ctx.string("kit")?.to_lowercase();
    let kit = ess
        .config
        .kits
        .get(&name)
        .ok_or_else(|| ess.msg("kit_missing", &[("kit", &name)]))?;
    if !ctx.has_permission(&format!("essentials.kits.{name}"), kit.default_access) {
        return Err(ess.msg("no_permission", &[]));
    }
    let now = now_seconds();
    {
        let mut data = ess.data();
        let user = data.user_mut(player.uuid());
        if let Some(last) = user.kit_cooldowns.get(&name) {
            let ready = last.saturating_add(kit.cooldown_seconds);
            if kit.cooldown_seconds > 0 && ready > now {
                return Err(ess.msg("kit_cooldown", &[("time", &duration(ready - now))]));
            }
        }
        user.kit_cooldowns.insert(name.clone(), now);
    }
    let server = ctx.server();
    let mut dropped = 0;
    for entry in &kit.items {
        let (item, count) = parse_kit_item(entry)?;
        dropped += api!(server.give_item(player, item.as_str().into(), count, "".into()))?;
    }
    ctx.send_legacy(&ess.msg("kit_given", &[("kit", &name)]));
    if dropped > 0 {
        ctx.send_legacy(&ess.msg("kit_dropped", &[("count", &dropped.to_string())]));
    }
    Ok(1)
}

/// Registers `/kit` and `/kits`.
pub fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    commands.register(
        CommandBuilder::new("essentials:kit")
            .default_access()
            .executes(list)
            .then(
                argument("kit", Arg::word())
                    .suggests(|_| kit_names(None))
                    .executes(give),
            ),
    )?;
    commands.register(
        CommandBuilder::new("essentials:kits")
            .default_access()
            .executes(list),
    )
}
