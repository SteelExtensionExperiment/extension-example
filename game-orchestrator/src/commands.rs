//! `/queue` for players and `/orchestrator` for administrators.
//!
//! `/orchestrator match` is not registered yet: it needs a kit in the match request.

use steel_extension_api::{
    Commands,
    command::{Arg, CommandBuilder, Invocation, Node, argument, literal},
    players::Uuid,
    scheduler,
};

use crate::{
    host::{self, deliver},
    orchestrator::{Notice, Orchestrator},
    store::MIN_PLAYERS,
};

const PERM_QUEUE: &str = "orchestrator.queue";
const PERM_GAMETYPE: &str = "orchestrator.gametype";
const PERM_KIT: &str = "orchestrator.kit";
const PERM_LIST: &str = "orchestrator.list";
const PERM_SESSION: &str = "orchestrator.session";
pub(crate) const PERM_SIGN: &str = "orchestrator.sign";

const NEEDS_PLAYER: &str = "this command needs a player";
const ARG_GAME_TYPE: &str = "gametype";
const ARG_NAME: &str = "name";
const ARG_PLAYERS: &str = "players";
const ARG_KIT: &str = "kit";
const ARG_HOURS: &str = "hours";
const ARG_ID: &str = "id";

fn player_uuid(ctx: &Invocation<'_, '_>) -> Result<Uuid, String> {
    ctx.player()
        .map(|player| player.uuid())
        .map_err(|_| NEEDS_PLAYER.to_owned())
}

fn game_type_names() -> Vec<String> {
    host::with(|orchestrator| {
        orchestrator
            .store
            .game_types
            .iter()
            .map(|found| found.name.clone())
            .collect()
    })
    .unwrap_or_default()
}

fn game_type_arg() -> Node {
    argument(ARG_GAME_TYPE, Arg::word()).suggests(|_| game_type_names())
}

fn admin(name: &str, permission: &str) -> Node {
    literal(name).permission(permission)
}

fn send_lines(ctx: &Invocation<'_, '_>, lines: &[String]) {
    for line in lines {
        ctx.send_legacy(&format!("&a{line}"));
    }
}

/// Runs `action`, prints its reply and delivers notices to affected players.
fn run(
    ctx: &Invocation<'_, '_>,
    action: impl FnOnce(&mut Orchestrator) -> Result<(String, Vec<Notice>), String>,
) -> Result<i32, String> {
    let (reply, notices) = host::with(action)??;
    ctx.send_legacy(&format!("&a{reply}"));
    deliver(&ctx.server(), notices);
    Ok(1)
}

fn reply(
    ctx: &Invocation<'_, '_>,
    action: impl FnOnce(&mut Orchestrator) -> Result<String, String>,
) -> Result<i32, String> {
    run(ctx, |orchestrator| {
        action(orchestrator).map(|text| (text, Vec::new()))
    })
}

fn queue_tree() -> CommandBuilder {
    let join = literal("join").then(game_type_arg().executes(|ctx| {
        let player = player_uuid(ctx)?;
        let game_type = ctx.string(ARG_GAME_TYPE)?;
        let text = host::with(|orchestrator| orchestrator.join(player, &game_type))?
            .map_err(|refusal| refusal.to_string())?;
        ctx.send_legacy(&format!("&a{text}"));
        Ok(1)
    }));
    let leave = literal("leave").executes(|ctx| {
        let player = player_uuid(ctx)?;
        let left = host::with(|orchestrator| orchestrator.leave(player))?;
        ctx.send_legacy(&left.map_or_else(
            || "&eyou are not queued".to_owned(),
            |name| format!("&aleft the queue for {name}"),
        ));
        Ok(1)
    });
    CommandBuilder::new("orchestrator:queue")
        .permission(PERM_QUEUE)
        .default_access()
        .executes(|ctx| {
            let player = player_uuid(ctx)?;
            send_lines(
                ctx,
                &host::with(|orchestrator| orchestrator.queue_status(player))?,
            );
            Ok(1)
        })
        .then(join)
        .then(leave)
}

fn gametype_tree() -> Node {
    let create = admin("create", PERM_GAMETYPE).then(argument(ARG_NAME, Arg::word()).then(
        argument(ARG_PLAYERS, Arg::integer(1, i32::MAX)).executes(|ctx| {
            let (name, players) = (ctx.string(ARG_NAME)?, players_arg(ctx)?);
            reply(ctx, |orchestrator| {
                orchestrator.create_game_type(&name, players)
            })
        }),
    ));
    let players = admin("players", PERM_GAMETYPE).then(game_type_arg().then(
        argument(ARG_PLAYERS, Arg::integer(1, i32::MAX)).executes(|ctx| {
            let (name, players) = (ctx.string(ARG_GAME_TYPE)?, players_arg(ctx)?);
            reply(ctx, |orchestrator| orchestrator.set_players(&name, players))
        }),
    ));
    let switch = |name: &'static str, enabled: bool| {
        admin(name, PERM_GAMETYPE).then(game_type_arg().executes(move |ctx| {
            let game_type = ctx.string(ARG_GAME_TYPE)?;
            run(ctx, |orchestrator| {
                orchestrator.set_enabled(&game_type, enabled)
            })
        }))
    };
    let remove = admin("remove", PERM_GAMETYPE).then(game_type_arg().executes(|ctx| {
        let game_type = ctx.string(ARG_GAME_TYPE)?;
        run(ctx, |orchestrator| {
            orchestrator.remove_game_type(&game_type)
        })
    }));
    let list = admin("list", PERM_LIST).executes(|ctx| {
        send_lines(
            ctx,
            &host::with(|orchestrator| orchestrator.game_type_lines())?,
        );
        Ok(1)
    });
    literal("gametype")
        .then(create)
        .then(players)
        .then(switch("enable", true))
        .then(switch("disable", false))
        .then(remove)
        .then(list)
}

/// The argument is declared with a bound of at least 1, so a smaller N reaches the rules and is
/// answered with the contract message instead of a parse error.
fn players_arg(ctx: &Invocation<'_, '_>) -> Result<u32, String> {
    u32::try_from(ctx.integer(ARG_PLAYERS)?)
        .map_err(|_| format!("players must be at least {MIN_PLAYERS}"))
}

fn kit_save() -> Node {
    admin("save", PERM_KIT).then(
        game_type_arg().then(argument(ARG_KIT, Arg::word()).executes(|ctx| {
            let player = ctx.player().map_err(|_| NEEDS_PLAYER.to_owned())?;
            let (game_type, kit) = (ctx.string(ARG_GAME_TYPE)?, ctx.string(ARG_KIT)?);
            let snapshot = ctx
                .server()
                .inventory_snapshot(player)
                .match_owned(Ok, |error| Err(error.to_string()))?;
            reply(ctx, |orchestrator| {
                orchestrator.save_kit(&game_type, &kit, snapshot)
            })
        })),
    )
}

fn kit_load() -> Node {
    admin("load", PERM_KIT).then(
        game_type_arg().then(argument(ARG_KIT, Arg::word()).executes(|ctx| {
            let player = ctx.player().map_err(|_| NEEDS_PLAYER.to_owned())?;
            let (game_type, kit) = (ctx.string(ARG_GAME_TYPE)?, ctx.string(ARG_KIT)?);
            let snapshot = host::with(|orchestrator| orchestrator.kit_snapshot(&game_type, &kit))??;
            ctx.send_legacy("&eThis replaces your inventory and game mode.");
            ctx.server()
                .restore_inventory(player, &snapshot)
                .match_owned(Ok, |error| Err(error.to_string()))?;
            ctx.send_legacy(&format!("&aloaded kit {game_type}/{kit}"));
            Ok(1)
        })),
    )
}

fn kit_tree() -> Node {
    let remove = admin("remove", PERM_KIT).then(game_type_arg().then(
        argument(ARG_KIT, Arg::word()).executes(|ctx| {
            let (game_type, kit) = (ctx.string(ARG_GAME_TYPE)?, ctx.string(ARG_KIT)?);
            reply(ctx, |orchestrator| {
                orchestrator.remove_kit(&game_type, &kit)
            })
        }),
    ));
    let list = admin("list", PERM_LIST).then(game_type_arg().executes(|ctx| {
        let game_type = ctx.string(ARG_GAME_TYPE)?;
        send_lines(
            ctx,
            &host::with(|orchestrator| orchestrator.kit_lines(&game_type))??,
        );
        Ok(1)
    }));
    literal("kit")
        .then(kit_save())
        .then(kit_load())
        .then(remove)
        .then(list)
}

fn session_tree() -> Vec<Node> {
    let open = admin("open", PERM_SESSION).executes(run_open(None)).then(
        argument(ARG_HOURS, Arg::integer(1, i32::MAX)).executes(|ctx| {
            let hours = u32::try_from(ctx.integer(ARG_HOURS)?)
                .map_err(|_| "hours must be positive".to_owned())?;
            run_open(Some(hours))(ctx)
        }),
    );
    let close = admin("close", PERM_SESSION).executes(|ctx| run(ctx, Orchestrator::close));
    vec![open, close]
}

/// Opens queueing and, for a timed session, schedules the closing.
fn run_open(hours: Option<u32>) -> impl Fn(&Invocation<'_, '_>) -> Result<i32, String> {
    move |ctx| {
        let now = scheduler::current_tick();
        let (text, closes_at) = host::with(|orchestrator| orchestrator.open(now, hours))??;
        ctx.send_legacy(&format!("&a{text}"));
        if let Some(closes_at) = closes_at {
            scheduler::run_later(closes_at.saturating_sub(now), move |server| {
                let Ok(Some(notices)) = host::with(|orchestrator| orchestrator.expire(closes_at))
                else {
                    return;
                };
                deliver(server, notices);
            });
        }
        Ok(1)
    }
}

fn sign_tree() -> Node {
    let list = admin("list", PERM_SIGN).executes(|ctx| {
        send_lines(ctx, &host::with(|orchestrator| orchestrator.sign_lines())?);
        Ok(1)
    });
    let remove = admin("remove", PERM_SIGN).then(argument(ARG_ID, Arg::word()).executes(|ctx| {
        let id = ctx.string(ARG_ID)?;
        reply(ctx, |orchestrator| orchestrator.remove_sign(&id))
    }));
    literal("sign").then(list).then(remove)
}

fn orchestrator_tree() -> CommandBuilder {
    let status = admin("status", PERM_LIST).executes(|ctx| {
        let now = scheduler::current_tick();
        send_lines(
            ctx,
            &host::with(|orchestrator| orchestrator.status_lines(now))?,
        );
        Ok(1)
    });
    let queues = admin("queues", PERM_LIST).executes(|ctx| {
        let server = ctx.server();
        let lines = host::with(|orchestrator| {
            orchestrator.queue_lines(|uuid| {
                Option::from(server.player_by_uuid(uuid))
                    .and_then(|id| server.player_name(id).match_owned(Some, |_| None))
                    .map_or_else(|| uuid.to_string(), |name| name.to_string())
            })
        })?;
        send_lines(ctx, &lines);
        Ok(1)
    });
    let mut tree = CommandBuilder::new("orchestrator:orchestrator")
        .default_access()
        .then(gametype_tree())
        .then(kit_tree())
        .then(status)
        .then(queues)
        .then(sign_tree());
    for node in session_tree() {
        tree = tree.then(node);
    }
    tree
}

pub(crate) fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    commands.register(queue_tree())?;
    commands.register(orchestrator_tree())
}
