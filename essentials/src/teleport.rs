//! Homes, warps, spawn, teleport requests, `/back`, `/tp` and `/tphere`.
//!
//! Differences from EssentialsX: teleports are instant (no warmup or cooldown) and requests
//! do not check for a safe destination.
// TODO: teleport warmup/cooldown and safe-location search.
use crate::{
    state::{TeleportRequest, essentials},
    storage::Loc,
    util::{api, location, name, teleport, tell, valid_name},
};
use steel_extension_api::{
    Commands, Hooks, PlayerId, Server,
    command::{Arg, CommandBuilder, Invocation, SuggestionContext, argument},
    events::EventPriority,
    players::{MetadataKind, MetadataValue, Uuid},
};

fn sender(ctx: &Invocation<'_, '_>) -> Result<PlayerId, String> {
    let ess = essentials()?;
    ctx.player().map_err(|_| ess.msg("player_only", &[]))
}

fn go(ctx: &Invocation<'_, '_>, player: PlayerId, target: &Loc) -> Result<i32, String> {
    let ess = essentials()?;
    ctx.send_legacy(&ess.msg("teleporting", &[]));
    teleport(&ctx.server(), player, target)?;
    Ok(1)
}

fn home_limit(server: &Server<'_>, player: PlayerId) -> i64 {
    let default = essentials().map_or(1, |ess| ess.config.default_home_limit);
    if server.has_permission(player, "essentials.sethome.unlimited".into(), false) {
        return i64::MAX;
    }
    let value: Option<MetadataValue> =
        api!(server.permission_metadata(player, "essentials:homes".into()))
            .ok()
            .and_then(Option::from);
    value
        .filter(|value| value.kind == MetadataKind::Integer)
        .map_or(default, |value| value.integer)
}

fn set_home(ctx: &Invocation<'_, '_>, name: &str) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let name = valid_name(name).ok_or_else(|| ess.msg("invalid_name", &[]))?;
    let server = ctx.server();
    let here = location(&server, player)?;
    let limit = home_limit(&server, player);
    {
        let mut data = ess.data();
        let user = data.user_mut(player.uuid());
        let count = i64::try_from(user.homes.len()).unwrap_or(i64::MAX);
        if !user.homes.contains_key(&name) && count >= limit {
            return Err(ess.msg("home_limit", &[("limit", &limit.to_string())]));
        }
        user.homes.insert(name.clone(), here);
    }
    ctx.send_legacy(&ess.msg("home_set", &[("home", &name)]));
    Ok(1)
}

fn home(ctx: &Invocation<'_, '_>, name: Option<String>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let target = {
        let data = ess.data();
        let homes = data.users.get(&player.uuid()).map(|user| &user.homes);
        let homes_count = homes.map_or(0, std::collections::BTreeMap::len);
        let name = match name {
            Some(name) => name.to_lowercase(),
            None if homes_count == 1 => homes
                .and_then(|homes| homes.keys().next().cloned())
                .unwrap_or_default(),
            None => "home".to_owned(),
        };
        homes
            .and_then(|homes| homes.get(&name).cloned())
            .ok_or_else(|| ess.msg("home_missing", &[("home", &name)]))?
    };
    go(ctx, player, &target)
}

fn del_home(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let name = ctx.string("name")?.to_lowercase();
    let removed = ess
        .data()
        .user_mut(player.uuid())
        .homes
        .remove(&name)
        .is_some();
    if !removed {
        return Err(ess.msg("home_missing", &[("home", &name)]));
    }
    ctx.send_legacy(&ess.msg("home_deleted", &[("home", &name)]));
    Ok(1)
}

fn home_names(uuid: Option<Uuid>) -> Vec<String> {
    let (Ok(ess), Some(uuid)) = (essentials(), uuid) else {
        return Vec::new();
    };
    ess.data()
        .users
        .get(&uuid)
        .map_or_else(Vec::new, |user| user.homes.keys().cloned().collect())
}

fn suggest_homes(ctx: &SuggestionContext<'_, '_>) -> Vec<String> {
    home_names(ctx.player_uuid())
}

fn list_homes(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let homes = home_names(Some(player.uuid()));
    if homes.is_empty() {
        ctx.send_legacy(&ess.msg("homes_none", &[]));
    } else {
        ctx.send_legacy(&ess.msg("homes_list", &[("homes", &homes.join(", "))]));
    }
    Ok(1)
}

fn warp_names() -> Vec<String> {
    essentials().map_or_else(
        |_| Vec::new(),
        |ess| ess.data().warps.keys().cloned().collect(),
    )
}

fn warp(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let name = ctx.string("name")?.to_lowercase();
    let target = ess
        .data()
        .warps
        .get(&name)
        .cloned()
        .ok_or_else(|| ess.msg("warp_missing", &[("warp", &name)]))?;
    go(ctx, player, &target)
}

fn set_warp(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let name = valid_name(&ctx.string("name")?).ok_or_else(|| ess.msg("invalid_name", &[]))?;
    let here = location(&ctx.server(), player)?;
    {
        let mut data = ess.data();
        data.warps.insert(name.clone(), here);
        data.touch_warps();
    }
    ctx.send_legacy(&ess.msg("warp_set", &[("warp", &name)]));
    Ok(1)
}

fn del_warp(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let name = ctx.string("name")?.to_lowercase();
    let removed = {
        let mut data = ess.data();
        let removed = data.warps.remove(&name).is_some();
        data.touch_warps();
        removed
    };
    if !removed {
        return Err(ess.msg("warp_missing", &[("warp", &name)]));
    }
    ctx.send_legacy(&ess.msg("warp_deleted", &[("warp", &name)]));
    Ok(1)
}

fn list_warps(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let warps = warp_names();
    if warps.is_empty() {
        ctx.send_legacy(&ess.msg("warps_none", &[]));
    } else {
        ctx.send_legacy(&ess.msg("warps_list", &[("warps", &warps.join(", "))]));
    }
    Ok(1)
}

fn spawn(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let target = ess
        .data()
        .spawn
        .clone()
        .ok_or_else(|| ess.msg("spawn_missing", &[]))?;
    go(ctx, player, &target)
}

fn set_spawn(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let here = location(&ctx.server(), player)?;
    {
        let mut data = ess.data();
        data.spawn = Some(here);
        data.touch_spawn();
    }
    ctx.send_legacy(&ess.msg("spawn_set", &[]));
    Ok(1)
}

fn back(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let target = ess
        .data()
        .users
        .get(&player.uuid())
        .and_then(|user| user.last_location.clone())
        .ok_or_else(|| ess.msg("back_none", &[]))?;
    go(ctx, player, &target)
}

fn request(ctx: &Invocation<'_, '_>, here: bool) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let target = ctx.player_arg("player")?;
    if target.uuid() == player.uuid() {
        return Err(ess.msg("tpa_self", &[]));
    }
    let server = ctx.server();
    ess.runtime().requests.insert(
        target.uuid(),
        TeleportRequest {
            requester: player.uuid(),
            here,
            sent_tick: server.current_tick(),
        },
    );
    let requester_name = name(&server, player);
    ctx.send_legacy(&ess.msg("tpa_sent", &[("player", &name(&server, target))]));
    let key = if here {
        "tpahere_received"
    } else {
        "tpa_received"
    };
    tell(
        &server,
        target,
        &ess.msg(key, &[("player", &requester_name)]),
    );
    tell(
        &server,
        target,
        &ess.msg(
            "tpa_hint",
            &[("seconds", &ess.config.tpa_timeout_seconds.to_string())],
        ),
    );
    Ok(1)
}

/// Takes the live, unexpired request targeting `target`.
fn take_request(server: &Server<'_>, target: Uuid) -> Option<TeleportRequest> {
    let ess = essentials().ok()?;
    let request = ess.runtime().requests.remove(&target)?;
    let timeout_ticks = ess.config.tpa_timeout_seconds.saturating_mul(20);
    (server.current_tick().saturating_sub(request.sent_tick) <= timeout_ticks).then_some(request)
}

fn respond(ctx: &Invocation<'_, '_>, accept: bool) -> Result<i32, String> {
    let ess = essentials()?;
    let player = sender(ctx)?;
    let server = ctx.server();
    let request = take_request(&server, player.uuid()).ok_or_else(|| ess.msg("tpa_none", &[]))?;
    let requester: Option<PlayerId> = server.player_by_uuid(request.requester).into();
    let requester = requester.ok_or_else(|| ess.msg("tpa_none", &[]))?;
    let target_name = name(&server, player);
    if !accept {
        ctx.send_legacy(&ess.msg("tpa_denied", &[]));
        tell(
            &server,
            requester,
            &ess.msg("tpa_denied_other", &[("player", &target_name)]),
        );
        return Ok(1);
    }
    ctx.send_legacy(&ess.msg("tpa_accepted", &[]));
    tell(
        &server,
        requester,
        &ess.msg("tpa_accepted_other", &[("player", &target_name)]),
    );
    let (moving, destination) = if request.here {
        (player, location(&server, requester)?)
    } else {
        (requester, location(&server, player)?)
    };
    teleport(&server, moving, &destination)?;
    Ok(1)
}

fn tp(ctx: &Invocation<'_, '_>, target_arg: Option<&str>) -> Result<i32, String> {
    let ess = essentials()?;
    let server = ctx.server();
    let (moving, destination) = match target_arg {
        Some(target) => (ctx.player_arg("player")?, ctx.player_arg(target)?),
        None => (sender(ctx)?, ctx.player_arg("player")?),
    };
    let destination = location(&server, destination)?;
    ctx.send_legacy(&ess.msg("teleporting", &[]));
    teleport(&server, moving, &destination)?;
    Ok(1)
}

fn tphere(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = sender(ctx)?;
    let server = ctx.server();
    let target = ctx.player_arg("player")?;
    let destination = location(&server, player)?;
    teleport(&server, target, &destination)?;
    Ok(1)
}

/// Registers teleport commands.
pub fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    commands.register(
        CommandBuilder::new("essentials:sethome")
            .default_access()
            .executes(|ctx| set_home(ctx, "home"))
            .then(
                argument("name", Arg::word()).executes(|ctx| set_home(ctx, &ctx.string("name")?)),
            ),
    )?;
    commands.register(
        CommandBuilder::new("essentials:home")
            .default_access()
            .executes(|ctx| home(ctx, None))
            .then(
                argument("name", Arg::word())
                    .suggests(suggest_homes)
                    .executes(|ctx| home(ctx, Some(ctx.string("name")?))),
            ),
    )?;
    commands.register(
        CommandBuilder::new("essentials:delhome")
            .default_access()
            .then(
                argument("name", Arg::word())
                    .suggests(suggest_homes)
                    .executes(del_home),
            ),
    )?;
    commands.register(
        CommandBuilder::new("essentials:homes")
            .default_access()
            .executes(list_homes),
    )?;
    commands.register(
        CommandBuilder::new("essentials:warp")
            .default_access()
            .executes(list_warps)
            .then(
                argument("name", Arg::word())
                    .suggests(|_| warp_names())
                    .executes(warp),
            ),
    )?;
    commands.register(
        CommandBuilder::new("essentials:warps")
            .default_access()
            .executes(list_warps),
    )?;
    commands.register(
        CommandBuilder::new("essentials:setwarp")
            .then(argument("name", Arg::word()).executes(set_warp)),
    )?;
    commands.register(
        CommandBuilder::new("essentials:delwarp").then(
            argument("name", Arg::word())
                .suggests(|_| warp_names())
                .executes(del_warp),
        ),
    )?;
    commands.register(
        CommandBuilder::new("essentials:spawn")
            .default_access()
            .executes(spawn),
    )?;
    commands.register(CommandBuilder::new("essentials:setspawn").executes(set_spawn))?;
    commands.register(
        CommandBuilder::new("essentials:back")
            .default_access()
            .executes(back),
    )?;
    commands.register(
        CommandBuilder::new("essentials:tpa")
            .default_access()
            .then(argument("player", Arg::player()).executes(|ctx| request(ctx, false))),
    )?;
    commands.register(
        CommandBuilder::new("essentials:tpahere")
            .default_access()
            .then(argument("player", Arg::player()).executes(|ctx| request(ctx, true))),
    )?;
    commands.register(
        CommandBuilder::new("essentials:tpaccept")
            .alias("tpyes")
            .default_access()
            .executes(|ctx| respond(ctx, true)),
    )?;
    commands.register(
        CommandBuilder::new("essentials:tpdeny")
            .alias("tpno")
            .default_access()
            .executes(|ctx| respond(ctx, false)),
    )?;
    commands.register(
        CommandBuilder::new("essentials:etp").alias("tpto").then(
            argument("player", Arg::player())
                .executes(|ctx| tp(ctx, None))
                .then(argument("target", Arg::player()).executes(|ctx| tp(ctx, Some("target")))),
        ),
    )?;
    commands.register(
        CommandBuilder::new("essentials:tphere")
            .then(argument("player", Arg::player()).executes(tphere)),
    )?;
    Ok(())
}

/// Records `/back` locations from teleports and deaths.
pub fn subscribe(hooks: &mut Hooks<'_>) {
    hooks.on_teleport_with(EventPriority::Monitor, true, |event| {
        let Ok(ess) = essentials() else {
            return;
        };
        let from = Loc::from(&event.from());
        ess.data().user_mut(event.player().uuid()).last_location = Some(from);
    });
    hooks.on_death(EventPriority::Monitor, |event| {
        let Ok(ess) = essentials() else {
            return;
        };
        let at = Loc::from(&event.location());
        ess.data().user_mut(event.player().uuid()).last_location = Some(at);
    });
}
