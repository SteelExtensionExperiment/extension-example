//! WorldEdit core features for Steel: wand and position selection, `//set`, `//replace`,
//! `//walls`, `//count`, `//size`, `//expand`, `//contract`, `//copy`, `//cut`, `//paste`,
//! `//undo`, `//redo` and `//sel`, plus the `worldedit:selection/v1` service.
//!
//! Commands are typed with a double slash (`//set`); each requires `worldedit.<name>`
//! (operators have every permission). The wand also needs `worldedit.wand`.
// TODO: `//hpos1`/`//hpos2` need a block raycast API; brushes, schematics and expression
// patterns are not part of this port.
#![recursion_limit = "256"]

mod edit;
mod pattern;
mod session;

use edit::{Clipboard, Direction};
use pattern::{Mask, Pattern, Rng};
use serde::Deserialize;
use session::{Corner, Session, Sessions};
use std::sync::OnceLock;
use steel_extension_api::{
    BlockPos, Commands, Hooks, PlayerId, ExtensionContext,
    command::{Arg, CommandBuilder, Invocation, argument},
    events::{EventPriority, InteractAction},
    is_air, log,
    players::Uuid,
    services,
    stabby::{self, option::Option as StableOption},
};
use worldedit_api::{Cuboid, DynSelections, SELECTION, Selections};

/// WorldEdit settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    /// Item used as the selection wand.
    wand_item: String,
    /// Largest number of blocks one command may change or read.
    max_blocks: u64,
    /// Undoable edits kept per player.
    history_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wand_item: "minecraft:wooden_axe".to_owned(),
            max_blocks: 1_000_000,
            history_size: 15,
        }
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();
static SESSIONS: OnceLock<Sessions> = OnceLock::new();

fn config() -> &'static Config {
    CONFIG.get_or_init(Config::default)
}

fn sessions() -> &'static Sessions {
    SESSIONS.get_or_init(Sessions::default)
}

fn with_session<R>(player: Uuid, action: impl FnOnce(&mut Session) -> R) -> R {
    action(sessions().lock().entry(player).or_default())
}

fn player(ctx: &Invocation<'_, '_>) -> Result<PlayerId, String> {
    ctx.player()
        .map_err(|_| "Only players can use WorldEdit commands.".to_owned())
}

/// The sender's selection, required to be in their current world.
fn selection(ctx: &Invocation<'_, '_>, player: PlayerId) -> Result<Cuboid, String> {
    let cuboid = with_session(player.uuid(), |session| session.selection())
        .ok_or("Make a region selection first (//wand, //pos1, //pos2).")?;
    if cuboid.world.as_str() != ctx.world().key().as_str() {
        return Err("Your selection is in another world.".into());
    }
    Ok(cuboid)
}

fn describe(pos: BlockPos) -> String {
    format!("({}, {}, {})", pos.x, pos.y, pos.z)
}

fn corner_message(session: &Session, first: bool, pos: BlockPos) -> String {
    let which = if first { "First" } else { "Second" };
    let size = session.selection().map_or_else(String::new, |cuboid| {
        format!(" ({} blocks)", cuboid.volume())
    });
    format!("&d{which} position set to {}{size}.", describe(pos))
}

fn set_corner(player: Uuid, world: String, pos: BlockPos, first: bool) -> String {
    with_session(player, |session| {
        let corner = Some(Corner { world, pos });
        if first {
            session.pos1 = corner;
        } else {
            session.pos2 = corner;
        }
        corner_message(session, first, pos)
    })
}

fn pos_command(ctx: &Invocation<'_, '_>, first: bool) -> Result<i32, String> {
    let player = player(ctx)?;
    let pos = ctx.position().block();
    ctx.send_legacy(&set_corner(
        player.uuid(),
        ctx.world().key().to_string(),
        pos,
        first,
    ));
    Ok(1)
}

/// Records an edit and reports how many blocks changed.
fn finish(ctx: &Invocation<'_, '_>, player: PlayerId, set: edit::ChangeSet) -> i32 {
    let changed = set.changes.len();
    with_session(player.uuid(), |session| {
        session.remember(set, config().history_size);
    });
    ctx.send_legacy(&format!("&d{changed} blocks have been changed."));
    i32::try_from(changed).unwrap_or(i32::MAX)
}

fn check_volume(cuboid: &Cuboid) -> Result<(), String> {
    let limit = config().max_blocks;
    if cuboid.volume() > limit {
        return Err(format!(
            "The selection has {} blocks, more than the limit of {limit}.",
            cuboid.volume()
        ));
    }
    Ok(())
}

fn set(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = player(ctx)?;
    let cuboid = selection(ctx, player)?;
    check_volume(&cuboid)?;
    let pattern = Pattern::parse(&ctx.string("pattern")?)?;
    let mut rng = Rng::new();
    let targets = edit::positions(&cuboid).collect();
    let set = edit::apply(&ctx.world(), targets, config().max_blocks, |_, _| {
        pattern.pick(rng.next_f64())
    })?;
    Ok(finish(ctx, player, set))
}

fn replace(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = player(ctx)?;
    let cuboid = selection(ctx, player)?;
    check_volume(&cuboid)?;
    let text = ctx.string("arguments")?;
    let parts: Vec<&str> = text.split_whitespace().collect();
    let (mask, pattern) = match parts.as_slice() {
        [pattern] => (None, Pattern::parse(pattern)?),
        [mask, pattern] => (Some(Mask::parse(mask)?), Pattern::parse(pattern)?),
        _ => return Err("Usage: //replace [from] <to>".into()),
    };
    let mut rng = Rng::new();
    let targets = edit::positions(&cuboid).collect();
    let set = edit::apply(&ctx.world(), targets, config().max_blocks, |_, state| {
        let accepted = match &mask {
            Some(mask) => mask.matches(state, steel_extension_api::block_key(state).as_str()),
            None => !is_air(state),
        };
        accepted.then(|| pattern.pick(rng.next_f64())).flatten()
    })?;
    Ok(finish(ctx, player, set))
}

fn walls(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = player(ctx)?;
    let cuboid = selection(ctx, player)?;
    check_volume(&cuboid)?;
    let pattern = Pattern::parse(&ctx.string("pattern")?)?;
    let mut rng = Rng::new();
    let targets = edit::walls(&cuboid).collect();
    let set = edit::apply(&ctx.world(), targets, config().max_blocks, |_, _| {
        pattern.pick(rng.next_f64())
    })?;
    Ok(finish(ctx, player, set))
}

fn count(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = player(ctx)?;
    let cuboid = selection(ctx, player)?;
    let mask = Mask::parse(&ctx.string("mask")?)?;
    let matches = edit::count(&ctx.world(), &cuboid, config().max_blocks, |state, key| {
        mask.matches(state, key)
    })?;
    ctx.send_legacy(&format!("&dCounted: {matches}"));
    Ok(i32::try_from(matches).unwrap_or(i32::MAX))
}

fn size(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = player(ctx)?;
    let cuboid = selection(ctx, player)?;
    let side = |min: i32, max: i32| max.abs_diff(min) + 1;
    ctx.send_legacy(&format!(
        "&dSelection {} - {}: {}x{}x{} = {} blocks",
        describe(cuboid.min),
        describe(cuboid.max),
        side(cuboid.min.x, cuboid.max.x),
        side(cuboid.min.y, cuboid.max.y),
        side(cuboid.min.z, cuboid.max.z),
        cuboid.volume()
    ));
    Ok(1)
}

/// Direction argument, or the way the sender is looking.
fn direction(ctx: &Invocation<'_, '_>, explicit: bool) -> Result<Direction, String> {
    if explicit {
        let text = ctx.string("direction")?;
        return Direction::parse(&text).ok_or_else(|| format!("Unknown direction {text}."));
    }
    Ok(Direction::facing(ctx.raw().yaw(), ctx.raw().pitch()))
}

fn resize(ctx: &Invocation<'_, '_>, grow: bool, explicit_direction: bool) -> Result<i32, String> {
    let player = player(ctx)?;
    let cuboid = selection(ctx, player)?;
    let amount = ctx.integer("amount")?;
    let direction = direction(ctx, explicit_direction)?;
    let resized = if grow {
        edit::expand(&cuboid, amount, direction)
    } else {
        edit::contract(&cuboid, amount, direction)?
    };
    with_session(player.uuid(), |session| session.select(&resized));
    let verb = if grow { "expanded" } else { "contracted" };
    ctx.send_legacy(&format!(
        "&dRegion {verb} by {amount} blocks ({} blocks).",
        resized.volume()
    ));
    Ok(1)
}

fn expand_vertical(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = player(ctx)?;
    let mut cuboid = selection(ctx, player)?;
    let world = ctx.world();
    cuboid.min.y = world.min_y();
    cuboid.max.y = world.max_y();
    with_session(player.uuid(), |session| session.select(&cuboid));
    ctx.send_legacy(&format!(
        "&dRegion expanded vertically ({} blocks).",
        cuboid.volume()
    ));
    Ok(1)
}

fn copy(ctx: &Invocation<'_, '_>, cut: bool) -> Result<i32, String> {
    let player = player(ctx)?;
    let cuboid = selection(ctx, player)?;
    let origin = ctx.position().block();
    let world = ctx.world();
    let clipboard = edit::copy(&world, &cuboid, origin, config().max_blocks)?;
    let copied = clipboard.blocks.len();
    with_session(player.uuid(), |session| session.clipboard = Some(clipboard));
    if !cut {
        ctx.send_legacy(&format!("&d{copied} blocks were copied."));
        return Ok(1);
    }
    let air = steel_extension_api::parse_block_state("minecraft:air".into())
        .match_owned(Ok, |error| Err(error.to_string()))?;
    let targets = edit::positions(&cuboid).collect();
    let set = edit::apply(&world, targets, config().max_blocks, |_, _| Some(air))?;
    Ok(finish(ctx, player, set))
}

fn paste(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = player(ctx)?;
    let clipboard: Clipboard = with_session(player.uuid(), |session| session.clipboard.clone())
        .ok_or("Your clipboard is empty. Use //copy first.")?;
    let origin = ctx.position().block();
    let placements: std::collections::BTreeMap<(i32, i32, i32), _> = clipboard
        .blocks
        .iter()
        .map(|(offset, state)| {
            let pos = edit::absolute(*offset, origin);
            ((pos.x, pos.y, pos.z), *state)
        })
        .collect();
    let targets = clipboard
        .blocks
        .iter()
        .map(|(offset, _)| edit::absolute(*offset, origin))
        .collect();
    let set = edit::apply(&ctx.world(), targets, config().max_blocks, |pos, _| {
        placements.get(&(pos.x, pos.y, pos.z)).copied()
    })?;
    Ok(finish(ctx, player, set))
}

fn history(ctx: &Invocation<'_, '_>, undo: bool, times: i32) -> Result<i32, String> {
    let player = player(ctx)?;
    let world = ctx.world();
    let mut done = 0;
    for _ in 0..times.max(1) {
        let set = with_session(player.uuid(), |session| {
            if undo {
                session.history.pop()
            } else {
                session.redo.pop()
            }
        });
        let Some(set) = set else {
            break;
        };
        let result = if undo {
            edit::undo(&world, &set)
        } else {
            edit::redo(&world, &set)
        };
        if let Err(error) = result {
            // Put the edit back so it can be retried once the area is loaded again.
            with_session(player.uuid(), |session| {
                if undo {
                    session.history.push(set);
                } else {
                    session.redo.push(set);
                }
            });
            return Err(error);
        }
        with_session(player.uuid(), |session| {
            if undo {
                session.redo.push(set);
            } else {
                session.history.push(set);
            }
        });
        done += 1;
    }
    let verb = if undo { "Undid" } else { "Redid" };
    if done == 0 {
        let what = if undo { "undo" } else { "redo" };
        return Err(format!("Nothing left to {what}."));
    }
    ctx.send_legacy(&format!("&d{verb} {done} edit(s)."));
    Ok(done)
}

fn wand(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = player(ctx)?;
    ctx.server()
        .give_item(player, config().wand_item.as_str().into(), 1, "".into())
        .match_owned(Ok, |error| Err(error.to_string()))?;
    ctx.send_legacy("&dLeft click: select pos #1; Right click: select pos #2");
    Ok(1)
}

/// Registers `//<name>` with permission `worldedit.<name>`.
fn command(name: &str) -> CommandBuilder {
    CommandBuilder::new(format!("worldedit:/{name}")).permission(format!("worldedit.{name}"))
}

fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    commands.register(command("wand").executes(wand))?;
    commands.register(command("pos1").executes(|ctx| pos_command(ctx, true)))?;
    commands.register(command("pos2").executes(|ctx| pos_command(ctx, false)))?;
    commands.register(command("sel").executes(|ctx| {
        let player = player(ctx)?;
        with_session(player.uuid(), |session| {
            session.pos1 = None;
            session.pos2 = None;
        });
        ctx.send_legacy("&dSelection cleared.");
        Ok(1)
    }))?;
    commands.register(command("size").executes(size))?;
    commands.register(command("set").then(argument("pattern", Arg::greedy()).executes(set)))?;
    commands.register(
        command("replace").then(argument("arguments", Arg::greedy()).executes(replace)),
    )?;
    commands.register(command("walls").then(argument("pattern", Arg::greedy()).executes(walls)))?;
    commands.register(command("count").then(argument("mask", Arg::greedy()).executes(count)))?;
    for (name, grow) in [("expand", true), ("contract", false)] {
        let mut builder = command(name).then(
            argument("amount", Arg::integer(1, i32::MAX))
                .executes(move |ctx| resize(ctx, grow, false))
                .then(
                    argument("direction", Arg::word()).executes(move |ctx| resize(ctx, grow, true)),
                ),
        );
        if grow {
            builder =
                builder.then(steel_extension_api::command::literal("vert").executes(expand_vertical));
        }
        commands.register(builder)?;
    }
    commands.register(command("copy").executes(|ctx| copy(ctx, false)))?;
    commands.register(command("cut").executes(|ctx| copy(ctx, true)))?;
    commands.register(command("paste").executes(paste))?;
    for (name, undo) in [("undo", true), ("redo", false)] {
        commands.register(
            command(name)
                .executes(move |ctx| history(ctx, undo, 1))
                .then(
                    argument("times", Arg::integer(1, 100))
                        .executes(move |ctx| history(ctx, undo, ctx.integer("times")?)),
                ),
        )?;
    }
    Ok(())
}

/// WorldEdit selections exposed to other plugins.
struct SelectionService;

#[stabby::export(callback)]
impl Selections for SelectionService {
    fn selection(&self, player: Uuid) -> StableOption<Cuboid> {
        sessions()
            .lock()
            .get(&player)
            .and_then(Session::selection)
            .into()
    }
}

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.on_commands(register)?;
    hooks.on_interact(EventPriority::Low, |event| {
        if event.item().as_str() != config().wand_item {
            return;
        }
        let first = match event.action() {
            InteractAction::LeftClickBlock => true,
            InteractAction::RightClickBlock => false,
            _ => return,
        };
        let Some(pos) = Option::<BlockPos>::from(event.pos()) else {
            return;
        };
        let server = event.server();
        let player = event.player();
        if !server.has_permission(player, "worldedit.wand".into(), false) {
            return;
        }
        event.set_cancelled(true);
        let message = set_corner(player.uuid(), event.world().key().to_string(), pos, first);
        let _ = server.send_message(
            player,
            steel_extension_api::text::legacy(&message)
                .to_json()
                .as_str()
                .into(),
        );
    });
    hooks.on_quit(EventPriority::Monitor, |event| {
        sessions().lock().remove(&event.player().uuid());
    });
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let config: Config =
        toml::from_str(context.config().as_str()).map_err(|error| error.to_string())?;
    if config.max_blocks == 0 || config.history_size == 0 {
        return Err("max_blocks and history_size must be positive".into());
    }
    CONFIG
        .set(config)
        .map_err(|_| "WorldEdit initialized twice".to_owned())?;
    services::register::<DynSelections>(
        &context,
        SELECTION,
        0,
        stabby::boxed::Box::new(SelectionService).into(),
    )?;
    log::info("WorldEdit selection service registered");
    Ok(())
}

steel_extension_api::export_extension! {
    name: "worldedit",
    version: "0.1.0",
    depends: [],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
