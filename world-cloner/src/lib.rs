//! World cloner: loads saved worlds into RAM once and clones them as runtime worlds.
#![recursion_limit = "256"]

pub mod commands;
pub mod config;
pub mod host;
mod provider;
mod runtime;
pub mod server_host;
pub mod service;
pub mod startup;

use std::path::PathBuf;

use steel_extension_api::{ExtensionContext, Hooks, log, services, stabby};
use world_cloner_api::{CLONER, DynWorldCloner};

use crate::config::Config;
use crate::service::Cloner;

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.on_commands(|commands| {
        commands.register(commands::tree(runtime::handle, runtime::template_names))
    })?;
    hooks.register_server_tick(runtime::on_tick);
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let config = Config::parse(context.config().as_str())?;
    let cloner = Cloner::new(config, PathBuf::from(context.data_directory().as_str()));
    let runtime = runtime::install(cloner);
    services::register::<DynWorldCloner>(
        &context,
        CLONER,
        0,
        stabby::boxed::Box::new(provider::ServiceImpl(runtime)).into(),
    )?;
    log::info("World cloner service registered");
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
