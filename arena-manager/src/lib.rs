//! Arena manager: arenas per game type, match state machine, world limit and cleanup.
#![recursion_limit = "256"]

mod allocator;
mod commands;
mod config;
mod matches;
mod state;
mod store;

use std::path::PathBuf;
use steel_extension_api::{ExtensionContext, Hooks};

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.on_commands(commands::register)
}

/// A corrupt `arenas.toml` or `config.toml` fails init, so the extension refuses to start.
fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let config = config::Config::from_toml(context.config().as_str())
        .map_err(|error| format!("invalid config.toml: {error}"))?;
    let data_directory = PathBuf::from(context.data_directory().as_str());
    let store = store::ArenaStore::load(&data_directory.join(state::ARENAS_FILE))?;
    state::ArenaManager::install(config, store, data_directory)
}

steel_extension_api::export_extension! {
    name: "arena-manager",
    version: "0.1.0",
    depends: ["world-cloner"],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
