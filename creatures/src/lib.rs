//! Independent capture/battle example. No client mod, resource pack or magic dependency.
#![recursion_limit = "512"]
mod balls;
mod battle;
mod config;
mod data;
mod gameplay;
mod party;

use std::sync::OnceLock;
use steel_extension_api::stabby;
use steel_extension_api::{
    ApiResult, ExtensionContext, Hooks, PlayerId, Server,
    command::CommandBuilder,
    events::{EventPriority, Hand, InteractAction, InteractEntityEvent, InteractEvent},
    hud::Source,
    items::ItemSpec,
};

static CONFIG: OnceLock<config::Config> = OnceLock::new();
static SOURCE: OnceLock<Source> = OnceLock::new();
fn api<T: stabby::abi::IDeterminantProvider<stabby::string::String>>(
    result: ApiResult<T>,
) -> Result<T, String> {
    result
        .match_owned(Ok, Err)
        .map_err(|error| error.to_string())
}
fn config() -> Result<&'static config::Config, String> {
    CONFIG
        .get()
        .ok_or_else(|| "creatures example is not initialized".into())
}
fn source() -> Result<Source, String> {
    SOURCE
        .get()
        .copied()
        .ok_or_else(|| "creature HUD source is not initialized".into())
}

fn report_error(server: &Server<'_>, player: PlayerId, error: String) {
    steel_extension_api::log::warn(error.as_str());
    if let Err(error) = api(server.message(player, error.as_str().into())) {
        steel_extension_api::log::warn(format!("could not send creature error: {error}"));
    }
}

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    api(hooks.register_content_registration(balls::register))?;
    hooks.subscribe_interact(EventPriority::Normal, true, interact, 0);
    hooks.subscribe_interact_entity(EventPriority::Normal, true, interact_entity, 0);
    hooks.on_server_tick(|server, tick| {
        if tick.is_multiple_of(10)
            && let Err(error) = data::heartbeat(server)
        {
            steel_extension_api::log::error(error);
        }
    });
    hooks.on_commands(|commands| {
        commands.register(
            CommandBuilder::new("creatures:kit")
                .default_access()
                .executes(|ctx| {
                    let player = ctx.player()?;
                    for key in balls::KEYS {
                        api(ctx.server().give(player, ItemSpec::new(key).count(16)))?;
                    }
                    Ok(1)
                }),
        )?;
        commands.register(
            CommandBuilder::new("creatures:party")
                .default_access()
                .executes(|ctx| {
                    party::open(&ctx.server(), ctx.player()?)?;
                    Ok(1)
                }),
        )
    })
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    CONFIG
        .set(config::Config::parse(context.config().as_str())?)
        .map_err(|_| "creatures example initialized twice")?;
    let source = api(context.hud_source("default".into()))?;
    data::bind(source)?;
    SOURCE
        .set(source)
        .map_err(|_| "creature HUD initialized twice")?;
    party::init()?;
    Ok(())
}

#[stabby::export(callback)]
fn interact(mut event: InteractEvent<'_>, _: u64) {
    if event.hand() != Hand::MainHand
        || !matches!(
            event.action(),
            InteractAction::RightClickAir | InteractAction::RightClickBlock
        )
        || !balls::KEYS.contains(&event.item().as_str())
    {
        return;
    }
    event.set_cancelled(true);
    if let Err(error) = gameplay::throw(&event.server(), event.player(), None) {
        report_error(&event.server(), event.player(), error);
    }
}
#[stabby::export(callback)]
fn interact_entity(mut event: InteractEntityEvent<'_>, _: u64) {
    if event.hand() != Hand::MainHand {
        return;
    }
    let server = event.server();
    let player = event.player();
    let held = api(server.held_item(player));
    if !held
        .as_ref()
        .is_ok_and(|key| balls::KEYS.contains(&key.as_str()))
    {
        return;
    }
    event.set_cancelled(true);
    if let Err(error) = gameplay::throw(&server, player, Some(event.hit_position())) {
        report_error(&server, player, error);
    }
}

steel_extension_api::export_extension! {name:"creatures-example",version:"0.1.0",depends:[],soft_depends:[],pre_init:pre_init,init:init}
