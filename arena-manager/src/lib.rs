//! Arena manager: arenas per game type, match state machine, world limit and cleanup.
#![recursion_limit = "256"]

mod allocator;
mod commands;
mod config;
mod host;
mod matches;
mod runtime;
mod service;
mod state;
mod store;
mod transfer;
mod worlds;

use runtime::Incoming;
use std::path::PathBuf;
use steel_extension_api::{ExtensionContext, Hooks, events::EventPriority, services, stabby};

/// Handlers only enqueue; the tick resolves the batch (research R8).
fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.on_death(EventPriority::Monitor, |event| {
        enqueue(Incoming::Death(event.player()));
    });
    hooks.on_quit(EventPriority::Monitor, |event| {
        enqueue(Incoming::Quit(event.player()));
    });
    hooks.on_respawn(EventPriority::Monitor, |event| {
        enqueue(Incoming::Respawn(event.player()));
    });
    hooks.on_server_tick(service::on_tick);
    hooks.on_commands(commands::register)
}

fn enqueue(item: Incoming<steel_extension_api::PlayerId>) {
    if let Ok(manager) = state::ArenaManager::get() {
        manager.push_incoming(item);
    }
}

/// A corrupt `arenas.toml` or `config.toml` fails init, so the extension refuses to start.
fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let config = config::Config::from_toml(context.config().as_str())
        .map_err(|error| format!("invalid config.toml: {error}"))?;
    let data_directory = PathBuf::from(context.data_directory().as_str());
    let store = store::ArenaStore::load(&data_directory.join(state::ARENAS_FILE))?;
    state::ArenaManager::install(config, store, data_directory)?;
    services::register::<arena_manager_api::DynMatches>(
        &context,
        arena_manager_api::MATCHES,
        0,
        stabby::boxed::Box::new(service::MatchService).into(),
    )
}

steel_extension_api::export_extension! {
    name: "arena-manager",
    version: "0.1.0",
    depends: ["world-cloner"],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
