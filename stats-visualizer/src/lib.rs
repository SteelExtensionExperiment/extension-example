//! Stats visualizer for Steel: tracks per-player minigame wins and losses and shows them
//! as leaderboards.
#![recursion_limit = "256"]

mod ranking;
mod stats;

use steel_extension_api::{ExtensionContext, Hooks};

fn pre_init(_hooks: Hooks<'_>) -> Result<(), String> {
    Ok(())
}

fn init(_context: ExtensionContext<'_>) -> Result<(), String> {
    Ok(())
}

steel_extension_api::export_extension! {
    name: "stats-visualizer",
    version: "0.1.0",
    depends: [],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
