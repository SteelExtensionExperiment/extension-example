//! Independent, vanilla-client spell example. Mana is public domain-scoped data.
#![recursion_limit = "512"]
mod config;
mod protection;
mod spells;
mod state;

use std::sync::OnceLock;
use steel_extension_api::{
    ApiResult, ExtensionContext, Hooks, Server,
    command::CommandBuilder,
    content::{ContentRegistry, ItemAppearance, ItemDefinition},
    events::{EventPriority, Hand, InteractAction, InteractEntityEvent, InteractEvent},
    hud::Source,
    items::ItemSpec,
    menus::{self, MenuHandler},
    stabby,
};

const WAND: &str = "magic:wand";
static CONFIG: OnceLock<config::Config> = OnceLock::new();
static SOURCE: OnceLock<Source> = OnceLock::new();
static MENU: OnceLock<MenuHandler> = OnceLock::new();
fn api<T: stabby::abi::IDeterminantProvider<stabby::string::String>>(
    result: ApiResult<T>,
) -> Result<T, String> {
    result.match_owned(Ok, Err).map_err(|e| e.to_string())
}
fn config() -> Result<&'static config::Config, String> {
    CONFIG
        .get()
        .ok_or_else(|| "magic is not initialized".into())
}
fn source() -> Result<Source, String> {
    SOURCE
        .get()
        .copied()
        .ok_or_else(|| "magic HUD is not initialized".into())
}

#[stabby::export(callback)]
fn register(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    registry.register_item(ItemDefinition {
        key: WAND.into(),
        name: "Spell wand".into(),
        max_stack_size: 1,
        appearance: ItemAppearance {
            item: "minecraft:blaze_rod".into(),
            model: None.into(),
            profile: None.into(),
        },
    })
}
fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    api(hooks.register_content_registration(register))?;
    hooks.subscribe_interact(EventPriority::Normal, true, interact, 0);
    hooks.subscribe_interact_entity(EventPriority::Normal, true, interact_entity, 0);
    hooks.subscribe_damage(EventPriority::High, true, protection::damage, 0);
    hooks.on_server_tick(|server, tick| {
        if tick.is_multiple_of(20)
            && let Err(error) = state::heartbeat(server)
        {
            steel_extension_api::log::error(error);
        }
    });
    hooks.on_commands(|commands| {
        commands.register(
            CommandBuilder::new("magic:kit")
                .default_access()
                .executes(|ctx| {
                    api(ctx.server().give(ctx.player()?, ItemSpec::new(WAND)))?;
                    Ok(1)
                }),
        )
    })
}
fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    CONFIG
        .set(config::Config::parse(context.config().as_str())?)
        .map_err(|_| "magic initialized twice")?;
    let source = api(context.hud_source("default".into()))?;
    api(source.bind("player_mana".into(), "magic:state/mana".into()))?;
    api(source.bind("player_max_mana".into(), "magic:state/mana/maximum".into()))?;
    SOURCE
        .set(source)
        .map_err(|_| "magic HUD initialized twice")?;
    MENU.set(menus::handler(|server, click| {
        let Ok(spell) = usize::try_from(click.slot) else {
            return;
        };
        if spell >= spells::NAMES.len() {
            return;
        }
        let result = (|| {
            api(server.close_menu(click.player))?;
            spells::cast(server, click.player, spell)
        })();
        if let Err(error) = result {
            steel_extension_api::log::warn(&error);
            if let Err(error) = api(server.message(click.player, error.as_str().into())) {
                steel_extension_api::log::warn(error);
            }
        }
    }))
    .map_err(|_| "spell menu initialized twice")?;
    Ok(())
}
fn menu(server: &Server<'_>, player: steel_extension_api::PlayerId) -> Result<(), String> {
    state::initialize(server, player)?;
    let handler = MENU.get().copied().ok_or("spell menu is not initialized")?;
    let config = config()?;
    let items = spells::NAMES
        .iter()
        .zip(spells::ICONS)
        .enumerate()
        .map(|(i, (name, item))| {
            ItemSpec::new(item).name(&format!("{name} · {} mana", config.costs[i]))
        });
    server.open_menu_with(
        player,
        "Cast a spell (aim before opening)",
        1,
        items,
        handler,
    )?;
    Ok(())
}
#[stabby::export(callback)]
fn interact(mut event: InteractEvent<'_>, _: u64) {
    if event.hand() != Hand::MainHand
        || event.item().as_str() != WAND
        || !matches!(
            event.action(),
            InteractAction::RightClickAir | InteractAction::RightClickBlock
        )
    {
        return;
    }
    event.set_cancelled(true);
    if let Err(error) = menu(&event.server(), event.player()) {
        steel_extension_api::log::warn(error);
    }
}
#[stabby::export(callback)]
fn interact_entity(mut event: InteractEntityEvent<'_>, _: u64) {
    if event.hand() != Hand::MainHand {
        return;
    }
    let server = event.server();
    if !api(server.held_item(event.player())).is_ok_and(|key| key.as_str() == WAND) {
        return;
    }
    event.set_cancelled(true);
    if let Err(error) = menu(&server, event.player()) {
        steel_extension_api::log::warn(error);
    }
}
steel_extension_api::export_extension! {name: "magic-example", version: "0.1.0", depends: [], soft_depends: [], pre_init: pre_init, init: init}
