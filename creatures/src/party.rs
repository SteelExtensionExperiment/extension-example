use super::{api, data, gameplay, report_error};
use std::sync::OnceLock;
use steel_extension_api::{
    PlayerId, Position, Server,
    events::ClickKind,
    items::ItemSpec,
    menus::{self, MenuHandler},
};

static MENU: OnceLock<MenuHandler> = OnceLock::new();
const RELEASE_ROW: usize = 9;

pub fn init() -> Result<(), String> {
    MENU.set(menus::handler(|server, click| {
        if click.kind != ClickKind::Pickup || click.button != 0 {
            return;
        }
        let Ok(slot) = usize::try_from(click.slot) else {
            return;
        };
        let result = if slot < data::PARTY {
            data::token(server, click.player, slot)
                .and_then(|item| api(server.give(click.player, item)).map(|_| ()))
        } else if (RELEASE_ROW..RELEASE_ROW + data::PARTY).contains(&slot) {
            release(server, click.player, slot - RELEASE_ROW)
        } else {
            return;
        };
        if let Err(error) = result {
            report_error(server, click.player, error);
        }
    }))
    .map_err(|_| "party menu initialized twice".into())
}

pub fn open(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    let mut icons = Vec::new();
    for slot in 0..RELEASE_ROW + data::PARTY {
        let member = if slot < data::PARTY {
            Some(slot)
        } else if slot >= RELEASE_ROW {
            Some(slot - RELEASE_ROW)
        } else {
            None
        };
        let Some(member) = member else {
            icons.push(ItemSpec::new("minecraft:gray_stained_glass_pane").name(" "));
            continue;
        };
        if data::read(server, player, &data::path(member))?.is_none() {
            icons.push(ItemSpec::new("minecraft:gray_stained_glass_pane").name("Empty"));
        } else if slot < RELEASE_ROW {
            icons.push(
                data::token(server, player, member)?
                    .lore(["Click to get this creature's ball", "Release button below"]),
            );
        } else {
            icons.push(
                ItemSpec::new("minecraft:red_dye")
                    .name(&format!("Release {}", data::label(server, player, member)?))
                    .lore([
                        "Click to return this creature to the world",
                        "It will leave your party",
                    ]),
            );
        }
    }
    let handler = MENU.get().copied().ok_or("party menu is not initialized")?;
    server.open_menu_with(player, "Creature party · release below", 2, icons, handler)?;
    Ok(())
}

fn release(server: &Server<'_>, player: PlayerId, slot: usize) -> Result<(), String> {
    let name = data::label(server, player, slot)?;
    let resident = server
        .players()
        .into_iter()
        .find(|entry| entry.id == player)
        .ok_or("trainer is not in a world")?;
    let mut failure = "no free release position beside you".to_owned();
    for (x, z) in gameplay::DEPLOYMENT_OFFSETS {
        match api(server.release_entity(
            player,
            data::path(slot).as_str().into(),
            Position {
                x: resident.position.x + x,
                y: resident.position.y,
                z: resident.position.z + z,
            },
            super::battle::policy(),
        )) {
            Ok(entity) => {
                super::battle::cancel(entity.identity());
                data::hud(server, player)?;
                api(server.message(
                    player,
                    format!("Released {name} into the world.").as_str().into(),
                ))?;
                return open(server, player);
            }
            Err(error) => failure = error,
        }
    }
    Err(failure)
}
