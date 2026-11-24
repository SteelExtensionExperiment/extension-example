use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use parking_lot::Mutex;
use steel_extension_api::{
    ApiResult, Command, Commands, ExtensionContext, Hooks, PlayerId, Server,
    content::ContentRegistry,
    menus::{self, MenuClick, MenuHandler, MenuItemUpdate},
    stabby::{self, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};

pub mod furnace;
mod reactor;
mod screen;
mod theme;

use furnace::Action;
use screen::{Screen, TICKS_PER_FRAME};

static MENU: OnceLock<MenuHandler> = OnceLock::new();
static SCREENS: LazyLock<Mutex<HashMap<PlayerId, ActiveScreen>>> = LazyLock::new(Mutex::default);

struct ActiveScreen {
    menu: u64,
    state: Screen,
}

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks
        .register_content_registration(content)
        .match_owned(Ok, |error| Err(error.to_string()))?;
    hooks
        .register_commands(commands)
        .match_owned(Ok, |error| Err(error.to_string()))?;
    hooks.on_server_tick(tick);
    Ok(())
}

fn init(_context: ExtensionContext<'_>) -> Result<(), String> {
    steel_ui_api::renderer()?;
    MENU.set(menus::handler(click))
        .map_err(|_| "UI demo initialized twice".to_owned())?;
    reactor::init()
}

#[stabby::export(callback)]
fn content(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    furnace::register_assets(&mut registry)
        .and_then(|()| reactor::register_assets(&mut registry))
        .map_err(ApiString::from)
        .into()
}

#[stabby::export(callback)]
fn commands(mut registry: Commands<'_>) -> ApiResult<()> {
    registry.register_player_command(
        "ui-demo:ui-demo".into(),
        [Str::new("open"), Str::new("reactor")].as_slice().into(),
        open,
    )
}

#[stabby::export(callback)]
fn open(command: Command<'_>, action: Str<'_>) -> ApiResult<i32> {
    let result = (|| -> Result<i32, ApiString> {
        let player = command.player().match_owned(Ok, Err)?;
        let server = command.server();
        match action.as_str() {
            "open" => open_furnace(&server, player),
            "reactor" => reactor::open(&server, player),
            _ => return Err("Unknown UI demo".into()),
        }
        .map_err(ApiString::from)?;
        Ok(1)
    })();
    result.into()
}

fn open_furnace(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    let handler = MENU.get().copied().ok_or("UI demo is not initialized")?;
    let state = Screen::new(furnace::asset_key()?);
    let menu = furnace::open_with(server, player, handler, state.initial())?;
    SCREENS.lock().insert(player, ActiveScreen { menu, state });
    Ok(())
}

fn click(server: &Server<'_>, click: MenuClick) {
    let Some(action) = furnace::action(click) else {
        return;
    };
    let updates = {
        let mut screens = SCREENS.lock();
        let Some(active) = screens
            .get_mut(&click.player)
            .filter(|active| active.menu == click.menu)
        else {
            return;
        };
        match action {
            Action::Start => active.state.start(),
            Action::ScrollUp => active.state.scroll(false),
            Action::ScrollDown => active.state.scroll(true),
            Action::RecipeFirst => active.state.select(0),
            Action::RecipeSecond => active.state.select(1),
            Action::RecipeLast => active.state.select(2),
            Action::Close => {
                drop(screens);
                if let Err(error) = Result::<(), ApiString>::from(server.close_menu(click.player)) {
                    eprintln!("Furnace menu close failed: {error}");
                }
                return;
            }
        }
    };
    publish(server, click.player, click.menu, updates);
}

fn tick(server: &Server<'_>, tick: u64) {
    reactor::tick(server, tick);
    if !tick.is_multiple_of(TICKS_PER_FRAME) {
        return;
    }
    let frames: Vec<_> = SCREENS
        .lock()
        .iter_mut()
        .map(|(&player, active)| (player, active.menu, active.state.advance()))
        .collect();
    for (player, menu, updates) in frames {
        publish(server, player, menu, updates);
    }
}

fn publish(server: &Server<'_>, player: PlayerId, menu: u64, updates: ApiVec<MenuItemUpdate>) {
    match furnace::update(server, player, menu, updates) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => eprintln!("Furnace menu update failed: {error}"),
    }
    let mut screens = SCREENS.lock();
    if screens
        .get(&player)
        .is_some_and(|active| active.menu == menu)
    {
        screens.remove(&player);
    }
}

steel_extension_api::export_extension! {
    name: "steel-ui-demo",
    version: "0.1.0",
    depends: ["steel-ui"],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
