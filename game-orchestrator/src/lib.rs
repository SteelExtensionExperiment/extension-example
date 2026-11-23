//! Game orchestrator for Steel: game types, kits, queues, signs and tournament control.
#![recursion_limit = "256"]

mod access;
mod config;
mod kits;
mod orchestrator;
mod queues;
mod signs;
mod store;

use steel_extension_api::{ExtensionContext, Hooks};

fn pre_init(_hooks: Hooks<'_>) -> Result<(), String> {
    Ok(())
}

fn init(_context: ExtensionContext<'_>) -> Result<(), String> {
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
