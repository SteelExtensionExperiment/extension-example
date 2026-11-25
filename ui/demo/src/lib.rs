use std::{collections::HashMap, sync::LazyLock};

use parking_lot::Mutex;
use steel_extension_api::{
    ApiResult, Command, Commands, ExtensionContext, Hooks, PlayerId, Server,
    menus::MenuItemUpdate,
    stabby::{self, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};

pub mod furnace;
mod reactor;
mod screen;
mod theme;

use furnace::Action;
use screen::{Screen, TICKS_PER_FRAME};

static MENU: LazyLock<furnace::Handler> = LazyLock::new(|| furnace::handler(click));
static SCREENS: LazyLock<Mutex<HashMap<PlayerId, ActiveScreen>>> = LazyLock::new(Mutex::default);

struct ActiveScreen {
    session: furnace::Session,
    state: Screen,
}

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.on_content(furnace::register_assets)?;
    hooks.on_content(reactor::register_assets)?;
    hooks
        .register_commands(commands)
        .match_owned(Ok, |error| Err(error.to_string()))?;
    hooks.on_server_tick(tick);
    Ok(())
}

fn init(_context: ExtensionContext<'_>) -> Result<(), String> {
    steel_ui_api::renderer().map(|_| ())
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
    let state = Screen::new(furnace::asset_key()?);
    let session = furnace::open_with(server, player, *MENU, state.initial())?;
    SCREENS
        .lock()
        .insert(player, ActiveScreen { session, state });
    Ok(())
}

fn click(server: &Server<'_>, session: furnace::Session, action: Action) {
    let player = session.player();
    let updates = {
        let mut screens = SCREENS.lock();
        let Some(active) = screens
            .get_mut(&player)
            .filter(|active| active.session == session)
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
                screens.remove(&player);
                drop(screens);
                session.close(server);
                return;
            }
        }
    };
    publish(server, session, updates);
}

fn tick(server: &Server<'_>, tick: u64) {
    reactor::tick(server, tick);
    if !tick.is_multiple_of(TICKS_PER_FRAME) {
        return;
    }
    let frames: Vec<_> = SCREENS
        .lock()
        .values_mut()
        .map(|active| (active.session, active.state.advance()))
        .collect();
    for (session, updates) in frames {
        publish(server, session, updates);
    }
}

fn publish(server: &Server<'_>, session: furnace::Session, updates: ApiVec<MenuItemUpdate>) {
    match session.update(server, updates) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => eprintln!("Furnace menu update failed: {error}"),
    }
    let player = session.player();
    let mut screens = SCREENS.lock();
    if screens
        .get(&player)
        .is_some_and(|active| active.session == session)
    {
        screens.remove(&player);
    }
}

steel_extension_api::export_extension! {
    depends: ["steel-ui"],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
