//! Stats visualizer for Steel: tracks per-player minigame wins and losses and shows them
//! as leaderboards.
#![recursion_limit = "256"]

mod boards;
mod commands;
mod config;
mod game_types;
mod ranking;
mod restore;
mod results;
mod state;
mod stats;
mod storage;

use std::path::PathBuf;

use steel_extension_api::{ExtensionContext, Hooks, log, scheduler};

use config::Config;
use state::State;
use storage::Storage;

fn log_errors(errors: Vec<String>) {
    for error in errors {
        log::error(error);
    }
}

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.enable_scheduler();
    hooks.on_shutdown(|_| {
        let Ok(state) = state::state() else {
            return;
        };
        let errors = state.save(true);
        if errors.is_empty() {
            log::info("Saved stats visualizer data");
        }
        log_errors(errors);
    });
    results::register(&mut hooks)?;
    restore::register(&mut hooks);
    hooks.on_commands(commands::register)?;
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let config = Config::parse(context.config().as_str())?;
    let storage = Storage::new(PathBuf::from(context.data_directory().as_str()));
    let stats = storage.load_stats();
    let boards = storage.load_boards();
    for error in [&stats.error, &boards.error].into_iter().flatten() {
        log::error(error);
    }
    log::info(format!(
        "Loaded stats of {} players and {} boards",
        stats.value.players().len(),
        boards.value.len()
    ));
    State::install(storage, stats.value, boards.value)?;
    // The scheduler is enabled in pre-init; tasks may be scheduled from init.
    scheduler::run_repeating(
        config.autosave_interval_ticks,
        config.autosave_interval_ticks,
        |_| {
            if let Ok(state) = state::state() {
                log_errors(state.save(false));
            }
        },
    );
    restore::restore_on_next_tick();
    Ok(())
}

steel_extension_api::export_extension! {
    name: "stats-visualizer",
    version: "0.1.0",
    depends: ["game-orchestrator"],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
