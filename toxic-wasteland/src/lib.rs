//! Toxic Wasteland: a green, poisonous sludge fluid, the wasteland biome it belongs to,
//! a generated wasteland world with sludge lakes, and `/toxic` tools to bring both into
//! any world, plus its own mobs. Vanilla clients need no mod or resource pack: they see
//! sludge as tinted water, the biome as a regular synced biome, and the mobs as
//! vanilla slimes and husks.
#![recursion_limit = "256"]

mod commands;
mod config;
mod content;
mod features;
mod generator;
mod mobs;
mod model;
mod sludge;

use std::sync::OnceLock;

use config::Config;
use steel_extension_api::{
    ApiResult, BlockState, ExtensionContext, GeneratorRegistry, Hooks, default_block_state,
    fluid_behavior::api::FluidBehaviors,
    parse_block_state,
    stabby::{self, string::String as ApiString},
};

/// Block states the extension writes, resolved once the registries are frozen.
pub struct Materials {
    /// Sludge at each `level`, 0 (source) through 15.
    pub sludge: [BlockState; 16],
    pub air: BlockState,
    pub bedrock: BlockState,
    pub stone: BlockState,
    pub gravel: BlockState,
    pub coarse_dirt: BlockState,
    pub cobblestone: BlockState,
    /// Pale oak logs along the x, y, and z axes.
    pub dead_log: [BlockState; 3],
    pub hanging_moss: BlockState,
    pub uranium: BlockState,
    pub barrel: BlockState,
}

static CONFIG: OnceLock<Config> = OnceLock::new();
static MATERIALS: OnceLock<Materials> = OnceLock::new();

/// Parses the configuration the first time a callback receives it.
fn config_from(text: &str) -> Result<&'static Config, String> {
    if let Some(config) = CONFIG.get() {
        return Ok(config);
    }
    let config = Config::parse(text)?;
    Ok(CONFIG.get_or_init(|| config))
}

/// The configuration, available from content registration on.
fn config() -> &'static Config {
    match CONFIG.get() {
        Some(config) => config,
        None => panic!("toxic-wasteland callback ran before content registration"),
    }
}

fn materials() -> &'static Materials {
    match MATERIALS.get() {
        Some(materials) => materials,
        None => panic!("toxic-wasteland callback ran before its materials were resolved"),
    }
}

fn state(key: &str) -> Result<BlockState, ApiString> {
    default_block_state(key.into()).match_owned(Ok, Err)
}

fn parse(text: &str) -> Result<BlockState, ApiString> {
    parse_block_state(text.into()).match_owned(Ok, Err)
}

fn resolve_materials() -> Result<&'static Materials, ApiString> {
    if let Some(materials) = MATERIALS.get() {
        return Ok(materials);
    }
    let mut sludge = [state(content::SLUDGE)?; 16];
    for (level, slot) in sludge.iter_mut().enumerate() {
        let text = format!("{}[level={level}]", content::SLUDGE);
        *slot = parse(&text)?;
    }
    let materials = Materials {
        sludge,
        air: state("minecraft:air")?,
        bedrock: state("minecraft:bedrock")?,
        stone: state("minecraft:stone")?,
        gravel: state("minecraft:gravel")?,
        coarse_dirt: state("minecraft:coarse_dirt")?,
        cobblestone: state("minecraft:cobblestone")?,
        dead_log: [
            parse("minecraft:pale_oak_log[axis=x]")?,
            parse("minecraft:pale_oak_log[axis=y]")?,
            parse("minecraft:pale_oak_log[axis=z]")?,
        ],
        hanging_moss: state("minecraft:pale_hanging_moss")?,
        uranium: state(content::URANIUM)?,
        barrel: state(content::BARREL)?,
    };
    Ok(MATERIALS.get_or_init(|| materials))
}

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        hooks
            .register_content_registration(content::register)
            .match_owned(Ok, Err)?;
        hooks
            .register_fluid_behaviors(register_fluid_behaviors)
            .match_owned(Ok, Err)?;
        hooks
            .register_generator_registration(register_generators)
            .match_owned(Ok, Err)?;
        hooks
            .register_entity_behaviors(mobs::register_behaviors)
            .match_owned(Ok, Err)?;
        hooks.register_server_tick(mobs::spawn_tick);
        hooks
            .on_commands(commands::register)
            .map_err(ApiString::from)
    })();
    result.into()
}

#[stabby::export(callback)]
fn register_fluid_behaviors(mut registry: FluidBehaviors<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        let config = config_from(registry.config().as_str())?;
        let materials = resolve_materials()?;
        let behavior = sludge::Sludge::new(config, materials);
        registry
            .set_fluid_behavior(
                content::SLUDGE.into(),
                stabby::boxed::Box::new(behavior).into(),
            )
            .match_owned(Ok, Err)
    })();
    result.into()
}

#[stabby::export(callback)]
fn register_generators(mut registry: GeneratorRegistry<'_>) -> ApiResult<()> {
    registry.register_terrain_generator(
        "toxic_wasteland:wasteland".into(),
        "minecraft:overworld".into(),
        content::BIOME.into(),
        63,
        generator::generate,
    )
}

fn init(context: ExtensionContext<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        config_from(context.config().as_str())?;
        resolve_materials()?;
        Ok(())
    })();
    result.into()
}

steel_extension_api::export_extension! {
    name: "toxic-wasteland",
    version: "0.1.0",
    depends: [],
    soft_depends: ["worldedit"],
    pre_init: pre_init,
    init: init,
}
