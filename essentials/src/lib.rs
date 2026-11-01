//! EssentialsX core features for Steel: homes, warps, spawn, teleport requests, `/back`,
//! private messages, kits, an economy (also provided to Vault), god/fly/heal/feed, game mode
//! shortcuts, nicknames, broadcasts, AFK, chat formatting and join/quit messages.
//!
//! Command names follow EssentialsX. `/tp` stays vanilla; the Essentials player-to-player
//! teleport is `/etp` (alias `/tpto`). Permissions derive from command keys, for example
//! `essentials.command.home`; player commands use default access and staff commands require
//! a grant (operators have every permission).
#![recursion_limit = "256"]

mod afk;
mod chat;
mod config;
mod economy;
mod kits;
mod messages;
mod player_cmds;
mod state;
mod storage;
mod teleport;
mod util;

use std::path::PathBuf;
use steel_extension_api::{Hooks, ExtensionContext, log, scheduler, services, stabby};
use vault_api::DynEconomy;

/// Ticks between saves of changed data.
const SAVE_INTERVAL_TICKS: u64 = 100;

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.enable_scheduler();
    hooks.on_commands(|commands| {
        teleport::register(commands)?;
        economy::register(commands)?;
        player_cmds::register(commands)?;
        chat::register(commands)?;
        afk::register(commands)?;
        kits::register(commands)
    })?;
    teleport::subscribe(&mut hooks);
    player_cmds::subscribe(&mut hooks);
    chat::subscribe(&mut hooks);
    afk::subscribe(&mut hooks);
    scheduler::run_repeating(SAVE_INTERVAL_TICKS, SAVE_INTERVAL_TICKS, |_| {
        if let Ok(ess) = state::essentials() {
            ess.save();
        }
    });
    hooks.on_shutdown(|_| {
        if let Ok(ess) = state::essentials() {
            ess.save();
            log::info("Saved Essentials data");
        }
    });
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let config: config::Config =
        toml::from_str(context.config().as_str()).map_err(|error| error.to_string())?;
    config.validate()?;
    let storage = storage::Storage::new(PathBuf::from(context.data_directory().as_str()));
    let data = storage.load()?;
    log::info(format!("Loaded {} Essentials users", data.users.len()));
    state::Essentials::install(config, storage, data)?;
    services::register::<DynEconomy>(
        &context,
        vault_api::ECONOMY,
        10,
        stabby::boxed::Box::new(economy::EssentialsEconomy).into(),
    )?;
    Ok(())
}

steel_extension_api::export_extension! {
    name: "essentials",
    version: "0.1.0",
    depends: [],
    soft_depends: ["vault"],
    pre_init: pre_init,
    init: init,
}
