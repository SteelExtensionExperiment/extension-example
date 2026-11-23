//! World cloner: loads saved worlds into RAM once and clones them as runtime worlds.
#![recursion_limit = "256"]

pub mod commands;
pub mod config;
pub mod host;
pub mod service;
pub mod startup;

use steel_extension_api::{ExtensionContext, Hooks};

#[expect(
    clippy::unnecessary_wraps,
    reason = "export_extension! requires a fallible pre_init"
)]
fn pre_init(_hooks: Hooks<'_>) -> Result<(), String> {
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    config::Config::parse(context.config().as_str())?;
    Ok(())
}

steel_extension_api::export_extension! {
    name: "world-cloner",
    version: "0.1.0",
    depends: [],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
