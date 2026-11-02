//! Registers the wasteland biome, the sludge fluid, and its bucket.

use steel_extension_api::{
    ApiResult,
    content::{
        BiomeDefinition, BiomeParticle, ContentRegistry, FluidDefinition, ItemAppearance,
        ItemDefinition,
    },
    stabby::{self, string::String as ApiString},
};

use crate::config::Config;

pub const BIOME: &str = "toxic_wasteland:wasteland";
pub const SLUDGE: &str = "toxic_wasteland:sludge";
pub const BUCKET: &str = "toxic_wasteland:sludge_bucket";

fn biome(config: &Config) -> Result<BiomeDefinition, String> {
    let palette = config.palette()?;
    let mut biome = BiomeDefinition::new(
        BIOME,
        palette.sky,
        palette.fog,
        palette.water,
        palette.water_fog,
    );
    biome.grass_color = Some(palette.grass).into();
    biome.foliage_color = Some(palette.foliage).into();
    biome.dry_foliage_color = Some(palette.foliage).into();
    biome.temperature = 1.2;
    biome.downfall = 0.0;
    biome.has_precipitation = false;
    biome.particle = Some(BiomeParticle {
        particle: "minecraft:white_ash".into(),
        probability: 0.02,
    })
    .into();
    Ok(biome)
}

fn sludge(config: &Config) -> Result<FluidDefinition, String> {
    let palette = config.palette()?;
    let mut sludge = FluidDefinition::water_like(SLUDGE)
        .tint(palette.water, palette.water_fog)
        .spread(config.sludge.tick_delay, config.sludge.drop_off)
        .randomly_ticking()
        .bucket(ItemDefinition {
            key: BUCKET.into(),
            name: "Bucket of Toxic Sludge".into(),
            appearance: ItemAppearance {
                item: "minecraft:water_bucket".into(),
                model: None.into(),
                profile: None.into(),
            },
            max_stack_size: 1,
        });
    sludge.converts_to_source = config.sludge.converts_to_source;
    Ok(sludge)
}

#[stabby::export(callback)]
pub fn register(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        let config = crate::config_from(registry.config().as_str())?;
        registry
            .register_biome(biome(config)?)
            .match_owned(Ok, Err)?;
        registry
            .register_fluid(sludge(config)?)
            .match_owned(|_| Ok(()), Err)
    })();
    result.into()
}
