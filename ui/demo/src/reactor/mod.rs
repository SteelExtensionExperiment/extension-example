use std::{collections::HashMap, sync::LazyLock};

use parking_lot::Mutex;
use steel_extension_api::{
    ApiResultExt, PlayerId, Server, content::ContentRegistry, menus::MenuItemUpdate,
    stabby::vec::Vec as ApiVec,
};
use steel_ui_api::labelled_item;

mod gauges;
mod layout;
use gauges::{ENERGY_GAUGE, HEAT_GAUGE, LEVELS};
use layout::{Action, METER_OFF, slots};

const TICKS_PER_FRAME: u64 = 3;

static HANDLER: LazyLock<layout::Handler> = LazyLock::new(|| layout::handler(click));
static SCREENS: LazyLock<Mutex<HashMap<PlayerId, Screen>>> = LazyLock::new(Mutex::default);

struct Screen {
    session: layout::Session,
    assets: &'static str,
    packed: bool,
    charge: usize,
    cycle: Option<usize>,
}

pub(super) fn register_assets(registry: &mut ContentRegistry<'_>) -> Result<(), String> {
    layout::register_assets(registry)?;
    let assets = layout::asset_key()?;
    ENERGY_GAUGE.register(registry, assets)?;
    HEAT_GAUGE.register(registry, assets)
}

pub(super) fn open(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    let assets = layout::asset_key()?;
    let packed = server.player_uses_resource_pack(player).into_result()?;
    let mut initial = vec![
        material(slots::FEED, "minecraft:raw_copper", "Raw copper", "Feed"),
        material(slots::FLUX, "minecraft:redstone", "Redstone", "Flux"),
        idle_core(),
        output(assets, false),
    ];
    if packed {
        initial.extend(ENERGY_GAUGE.clear().chain(HEAT_GAUGE.clear()));
    }
    let session = layout::open_with(server, player, *HANDLER, initial)?;
    SCREENS.lock().insert(
        player,
        Screen {
            session,
            assets,
            packed,
            charge: 0,
            cycle: None,
        },
    );
    Ok(())
}

fn click(server: &Server<'_>, session: layout::Session, action: Action) {
    let player = session.player();
    let updates = {
        let mut screens = SCREENS.lock();
        let Some(screen) = screens
            .get_mut(&player)
            .filter(|screen| screen.session == session)
        else {
            return;
        };
        match action {
            Action::Run => screen.start(),
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

pub(super) fn tick(server: &Server<'_>, tick: u64) {
    if !tick.is_multiple_of(TICKS_PER_FRAME) {
        return;
    }
    let frames: Vec<_> = SCREENS
        .lock()
        .values_mut()
        .map(|screen| (screen.session, screen.advance()))
        .collect();
    for (session, updates) in frames {
        publish(server, session, updates);
    }
}

fn publish(server: &Server<'_>, session: layout::Session, updates: ApiVec<MenuItemUpdate>) {
    match session.update(server, updates) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => eprintln!("Reactor menu update failed: {error}"),
    }
    let player = session.player();
    let mut screens = SCREENS.lock();
    if screens
        .get(&player)
        .is_some_and(|screen| screen.session == session)
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
        HEAT_GAUGE.update(
            self.assets,
            &mut updates,
            self.cycle.unwrap_or_default(),
            0,
            self.packed,
        );
        self.cycle = Some(0);
        updates.push(material(
            slots::CORE,
            "minecraft:glowstone_dust",
            "Core active",
            "Refining copper",
        ));
        updates.push(output(self.assets, false));
        updates
    }

    fn advance(&mut self) -> ApiVec<MenuItemUpdate> {
        let mut updates = ApiVec::new();
        let previous = self.charge;
        self.charge = (previous + 1) % (LEVELS + 1);
        ENERGY_GAUGE.update(
            self.assets,
            &mut updates,
            previous,
            self.charge,
            self.packed,
        );

        if let Some(frame) = self.cycle.filter(|frame| *frame < LEVELS) {
            let next = frame + 1;
            self.cycle = Some(next);
            HEAT_GAUGE.update(self.assets, &mut updates, frame, next, self.packed);
            if next == LEVELS {
                updates.push(idle_core());
                updates.push(output(self.assets, true));
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

fn output(assets: &str, ready: bool) -> MenuItemUpdate {
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
            item: METER_OFF.item(assets, "Output pending", ""),
        }
    }
}
