//! Registers a deterministic terrain generator with floating islands, shells, and pillars.
mod terrain;

use std::sync::OnceLock;
use steel_extension_api::{
    ApiResult, BlockState, ExtensionContext, GeneratorRegistry, Hooks, default_block_state,
    stabby::{self, string::String as ApiString},
};

struct Materials {
    stone: BlockState,
    shell: BlockState,
    pillar: BlockState,
    light: BlockState,
}

static MATERIALS: OnceLock<Materials> = OnceLock::new();

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    hooks.register_generator_registration(register_generators)
}

#[stabby::export(callback)]
fn register_generators(mut registry: GeneratorRegistry<'_>) -> ApiResult<()> {
    registry.register_terrain_generator(
        "example:strange".into(),
        "minecraft:overworld".into(),
        "minecraft:the_end".into(),
        0,
        terrain::generate,
    )
}

fn init(_context: ExtensionContext<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        let materials = Materials {
            stone: default_block_state("minecraft:stone".into()).match_owned(Ok, Err)?,
            shell: default_block_state("minecraft:end_stone".into()).match_owned(Ok, Err)?,
            pillar: default_block_state("minecraft:purpur_block".into()).match_owned(Ok, Err)?,
            light: default_block_state("minecraft:glowstone".into()).match_owned(Ok, Err)?,
        };
        MATERIALS
            .set(materials)
            .map_err(|_| ApiString::from("strange terrain extension was initialized twice"))?;
        Ok(())
    })();
    result.into()
}

fn materials() -> &'static Materials {
    match MATERIALS.get() {
        Some(materials) => materials,
        None => panic!("terrain callback invoked before successful initialization"),
    }
}

steel_extension_api::export_extension!("strange-terrain-example", pre_init, init);
