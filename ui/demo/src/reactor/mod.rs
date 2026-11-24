use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use parking_lot::Mutex;
use steel_extension_api::{
    PlayerId, Server,
    content::ContentRegistry,
    menus::{self, MenuClick, MenuHandler, MenuItemUpdate},
    stabby::vec::Vec as ApiVec,
};
use steel_ui_api::labelled_item;

mod gauges;
mod layout;
use gauges::{ENERGY_GAUGE, HEAT_GAUGE, LEVELS};
use layout::{Action, METER_OFF, MODEL_PREFIX, slots};

const TICKS_PER_FRAME: u64 = 3;

static HANDLER: OnceLock<MenuHandler> = OnceLock::new();
static SCREENS: LazyLock<Mutex<HashMap<PlayerId, Screen>>> = LazyLock::new(Mutex::default);

struct Screen {
    menu: u64,
    packed: bool,
    charge: usize,
    cycle: Option<usize>,
}

pub(super) fn register_assets(registry: &mut ContentRegistry<'_>) -> Result<(), String> {
    layout::register_assets(registry)?;
    ENERGY_GAUGE.register(registry)?;
    HEAT_GAUGE.register(registry)
}

pub(super) fn init() -> Result<(), String> {
    HANDLER
        .set(menus::handler(click))
        .map_err(|_| "Reactor demo initialized twice".into())
}

pub(super) fn open(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    let handler = HANDLER.get().copied().ok_or("Reactor is not initialized")?;
    let packed = server
        .player_uses_resource_pack(player)
        .match_owned(Ok, |error| Err(error.to_string()))?;
    let mut initial = vec![
        material(slots::FEED, "minecraft:raw_copper", "Raw copper", "Feed"),
        material(slots::FLUX, "minecraft:redstone", "Redstone", "Flux"),
        idle_core(),
        output(false),
    ];
    if packed {
        initial.extend(ENERGY_GAUGE.clear().chain(HEAT_GAUGE.clear()));
    }
    let menu = layout::open_with(server, player, handler, initial)?;
    SCREENS.lock().insert(
        player,
        Screen {
            menu,
            packed,
            charge: 0,
            cycle: None,
        },
    );
    Ok(())
}

fn click(server: &Server<'_>, click: MenuClick) {
    let Some(action) = layout::action(click) else {
        return;
    };
    let updates = {
        let mut screens = SCREENS.lock();
        let Some(screen) = screens
            .get_mut(&click.player)
            .filter(|screen| screen.menu == click.menu)
        else {
            return;
        };
        match action {
            Action::Run => screen.start(),
            Action::Close => {
                screens.remove(&click.player);
                drop(screens);
                if let Err(error) = server.close_menu(click.player).match_owned(Ok, Err) {
                    eprintln!("Reactor menu close failed: {error}");
                }
                return;
            }
        }
    };
    publish(server, click.player, click.menu, updates);
}

pub(super) fn tick(server: &Server<'_>, tick: u64) {
    if !tick.is_multiple_of(TICKS_PER_FRAME) {
        return;
    }
    let frames: Vec<_> = SCREENS
        .lock()
        .iter_mut()
        .map(|(&player, screen)| (player, screen.menu, screen.advance()))
        .collect();
    for (player, menu, updates) in frames {
        publish(server, player, menu, updates);
    }
}

fn publish(server: &Server<'_>, player: PlayerId, menu: u64, updates: ApiVec<MenuItemUpdate>) {
    match layout::update(server, player, menu, updates) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => eprintln!("Reactor menu update failed: {error}"),
    }
    let mut screens = SCREENS.lock();
    if screens
        .get(&player)
        .is_some_and(|screen| screen.menu == menu)
    {
        screens.remove(&player);
    }
}

impl Screen {
    fn start(&mut self) -> ApiVec<MenuItemUpdate> {
        let mut updates = ApiVec::new();
        if self.cycle.is_some_and(|frame| frame < LEVELS) {
            return updates;
        }
        HEAT_GAUGE.update(&mut updates, self.cycle.unwrap_or_default(), 0, self.packed);
        self.cycle = Some(0);
        updates.push(material(
            slots::CORE,
            "minecraft:glowstone_dust",
            "Core active",
            "Refining copper",
        ));
        updates.push(output(false));
        updates
    }

    fn advance(&mut self) -> ApiVec<MenuItemUpdate> {
        let mut updates = ApiVec::new();
        let previous = self.charge;
        self.charge = (previous + 1) % (LEVELS + 1);
        ENERGY_GAUGE.update(&mut updates, previous, self.charge, self.packed);

        if let Some(frame) = self.cycle.filter(|frame| *frame < LEVELS) {
            let next = frame + 1;
            self.cycle = Some(next);
            HEAT_GAUGE.update(&mut updates, frame, next, self.packed);
            if next == LEVELS {
                updates.push(idle_core());
                updates.push(output(true));
            }
        }
        updates
    }
}

fn material(slot: i32, key: &str, label: &str, tip: &str) -> MenuItemUpdate {
    MenuItemUpdate {
        slot,
        item: labelled_item(key, label, tip),
    }
}

fn idle_core() -> MenuItemUpdate {
    material(
        slots::CORE,
        "minecraft:amethyst_shard",
        "Core on standby",
        "Press Run cycle",
    )
}

fn output(ready: bool) -> MenuItemUpdate {
    if ready {
        material(
            slots::OUTPUT,
            "minecraft:copper_ingot",
            "Refined copper",
            "Cycle complete",
        )
    } else {
        MenuItemUpdate {
            slot: slots::OUTPUT,
            item: METER_OFF.item(MODEL_PREFIX, "Output pending", ""),
        }
    }
}
