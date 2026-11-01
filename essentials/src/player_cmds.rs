//! `/god`, `/fly`, `/heal`, `/feed`, game mode shortcuts, `/nick` and `/broadcast`.
use crate::{
    state::essentials,
    util::{api, broadcast, name},
};
use steel_extension_api::{
    Commands, GameMode, Hooks, PlayerId,
    command::{Arg, CommandBuilder, Invocation, argument, literal},
    events::EventPriority,
    text,
};

/// The target given as `player`, or the sender.
fn target(ctx: &Invocation<'_, '_>, explicit: bool) -> Result<PlayerId, String> {
    if explicit {
        return ctx.player_arg("player");
    }
    let ess = essentials()?;
    ctx.player().map_err(|_| ess.msg("player_only", &[]))
}

/// Registers a command acting on the sender or an optional `player` argument.
fn self_or_other(
    commands: &mut Commands<'_>,
    key: &str,
    alias: Option<&str>,
    action: fn(&Invocation<'_, '_>, PlayerId) -> Result<i32, String>,
) -> Result<(), String> {
    let mut command = CommandBuilder::new(key)
        .executes(move |ctx| action(ctx, target(ctx, false)?))
        .then(
            argument("player", Arg::player()).executes(move |ctx| action(ctx, target(ctx, true)?)),
        );
    if let Some(alias) = alias {
        command = command.alias(alias);
    }
    commands.register(command)
}

fn god(ctx: &Invocation<'_, '_>, player: PlayerId) -> Result<i32, String> {
    let ess = essentials()?;
    let enabled = {
        let mut data = ess.data();
        let user = data.user_mut(player.uuid());
        user.god = !user.god;
        user.god
    };
    let key = if enabled { "god_on" } else { "god_off" };
    ctx.send_legacy(&ess.msg(key, &[("player", &name(&ctx.server(), player))]));
    Ok(1)
}

fn fly(ctx: &Invocation<'_, '_>, player: PlayerId) -> Result<i32, String> {
    let ess = essentials()?;
    let server = ctx.server();
    let allowed = !api!(server.allow_flight(player))?;
    api!(server.set_allow_flight(player, allowed))?;
    let key = if allowed { "fly_on" } else { "fly_off" };
    ctx.send_legacy(&ess.msg(key, &[("player", &name(&server, player))]));
    Ok(1)
}

fn heal(ctx: &Invocation<'_, '_>, player: PlayerId) -> Result<i32, String> {
    let ess = essentials()?;
    let server = ctx.server();
    let max = api!(server.max_health(player))?;
    api!(server.set_health(player, max))?;
    api!(server.set_food(player, 20, 5.0))?;
    ctx.send_legacy(&ess.msg("healed", &[("player", &name(&server, player))]));
    Ok(1)
}

fn feed(ctx: &Invocation<'_, '_>, player: PlayerId) -> Result<i32, String> {
    let ess = essentials()?;
    let server = ctx.server();
    api!(server.set_food(player, 20, 5.0))?;
    ctx.send_legacy(&ess.msg("fed", &[("player", &name(&server, player))]));
    Ok(1)
}

fn game_mode(
    ctx: &Invocation<'_, '_>,
    player: PlayerId,
    mode: GameMode,
    label: &str,
) -> Result<i32, String> {
    let ess = essentials()?;
    let server = ctx.server();
    api!(server.set_game_mode(player, mode))?;
    ctx.send_legacy(&ess.msg(
        "gamemode",
        &[("mode", label), ("player", &name(&server, player))],
    ));
    Ok(1)
}

fn nick(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let player = ctx.player().map_err(|_| ess.msg("player_only", &[]))?;
    let nick = ctx.string("nick")?;
    let plain = text::strip_legacy(&nick);
    if plain.is_empty() || plain.chars().count() > 16 || plain.contains(char::is_whitespace) {
        return Err(ess.msg("nick_invalid", &[]));
    }
    ess.data().user_mut(player.uuid()).nickname = Some(nick.clone());
    ctx.send_legacy(&ess.msg("nick_set", &[("nick", &nick)]));
    Ok(1)
}

/// Registers the utility commands.
pub fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    self_or_other(commands, "essentials:god", Some("tgm"), god)?;
    self_or_other(commands, "essentials:fly", None, fly)?;
    self_or_other(commands, "essentials:heal", None, heal)?;
    self_or_other(commands, "essentials:feed", Some("eat"), feed)?;
    for (key, mode, label) in [
        ("essentials:gmc", GameMode::Creative, "creative"),
        ("essentials:gms", GameMode::Survival, "survival"),
        ("essentials:gma", GameMode::Adventure, "adventure"),
        ("essentials:gmsp", GameMode::Spectator, "spectator"),
    ] {
        commands.register(
            CommandBuilder::new(key)
                .executes(move |ctx| game_mode(ctx, target(ctx, false)?, mode, label))
                .then(
                    argument("player", Arg::player())
                        .executes(move |ctx| game_mode(ctx, target(ctx, true)?, mode, label)),
                ),
        )?;
    }
    commands.register(
        CommandBuilder::new("essentials:nick")
            .alias("nickname")
            .then(argument("nick", Arg::greedy()).executes(nick))
            .then(literal("off").executes(|ctx| {
                let ess = essentials()?;
                let player = ctx.player().map_err(|_| ess.msg("player_only", &[]))?;
                ess.data().user_mut(player.uuid()).nickname = None;
                ctx.send_legacy(&ess.msg("nick_off", &[]));
                Ok(1)
            })),
    )?;
    commands.register(
        CommandBuilder::new("essentials:broadcast")
            .alias("bc")
            .then(argument("message", Arg::greedy()).executes(|ctx| {
                let ess = essentials()?;
                let message = ctx.string("message")?;
                broadcast(
                    &ctx.server(),
                    &ess.msg("broadcast", &[("message", &message)]),
                );
                Ok(1)
            })),
    )?;
    Ok(())
}

/// Cancels damage to players in god mode.
pub fn subscribe(hooks: &mut Hooks<'_>) {
    hooks.on_damage(EventPriority::High, |event| {
        let Ok(ess) = essentials() else {
            return;
        };
        let victim = event.victim();
        let player: Option<PlayerId> = victim.player.into();
        let Some(player) = player else {
            return;
        };
        if ess
            .data()
            .users
            .get(&player.uuid())
            .is_some_and(|user| user.god)
        {
            event.set_cancelled(true);
        }
    });
}
