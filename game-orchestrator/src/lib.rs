//! Game orchestrator for Steel: game types, kits, queues, signs and tournament control.
//!
//! Not yet wired: forming matches from full queues, `kit save|load` and `/orchestrator match`
//! (they need a kit per player in the match request, host spec 007), and registering signs from
//! their text (the sign-edit event does not say which world the sign is in).
#![recursion_limit = "256"]

mod access;
mod commands;
mod config;
mod host;
mod kits;
mod orchestrator;
mod queues;
mod service;
mod signs;
mod store;

use std::path::PathBuf;

use arena_manager_api::{MatchFailed, MatchFinished};
use game_orchestrator_api::{DynGames, GAMES};
use steel_extension_api::{
    ExtensionContext, Hooks,
    events::{EventPriority, Hand, InteractAction},
    log, services, stabby,
};

use crate::{
    config::Config,
    orchestrator::Orchestrator,
    service::{OrchestratorGames, on_match_failed, on_match_finished},
    signs::SignPos,
    store::Store,
};

/// Block keys of signs end like this (`oak_sign`, `oak_wall_sign`, `oak_hanging_sign`).
const SIGN_KEY_SUFFIX: &str = "_sign";

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.on_commands(commands::register)?;
    hooks.on_quit(EventPriority::Monitor, |event| {
        let player = event.player().uuid();
        if let Err(error) = host::with(|orchestrator| orchestrator.quit(player)) {
            log::error(error);
        }
    });
    hooks.on_interact_with(EventPriority::Normal, true, |event| {
        if event.action() != InteractAction::RightClickBlock || event.hand() != Hand::MainHand {
            return;
        }
        let Some(pos) = Option::<steel_extension_api::BlockPos>::from(event.pos()) else {
            return;
        };
        let world = event.world();
        let is_sign = world.block(pos).match_owned(Ok, Err).is_ok_and(|state| {
            steel_extension_api::block_key(state)
                .as_str()
                .ends_with(SIGN_KEY_SUFFIX)
        });
        if !is_sign {
            return;
        }
        let at = SignPos {
            world: world.key().to_string(),
            x: pos.x,
            y: pos.y,
            z: pos.z,
        };
        let player = event.player().uuid();
        let Ok(Some(result)) = host::with(|orchestrator| orchestrator.click_sign(player, &at))
        else {
            return;
        };
        event.set_cancelled(true);
        let server = event.server();
        match result {
            Ok(text) => host::tell(&server, player, &format!("&a{text}")),
            Err(text) => host::tell(&server, player, &format!("&c{text}")),
        }
    });
    hooks
        .add_action::<MatchFinished>(on_match_finished)
        .register()
        .match_owned(Ok, |error| Err(error.to_string()))?;
    hooks
        .add_action::<MatchFailed>(on_match_failed)
        .register()
        .match_owned(Ok, |error| Err(error.to_string()))?;
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let config =
        Config::parse(Some(context.config().as_str())).map_err(|error| error.to_string())?;
    let store = Store::load(PathBuf::from(context.data_directory().as_str()))
        .map_err(|error| error.to_string())?;
    host::install(Orchestrator::new(store, config))?;
    services::register::<DynGames>(
        &context,
        GAMES,
        0,
        stabby::boxed::Box::new(OrchestratorGames).into(),
    )?;
    log::info("Game orchestrator ready");
    Ok(())
}

steel_extension_api::export_extension! {
    name: "game-orchestrator",
    version: "0.1.0",
    depends: ["arena-manager"],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
