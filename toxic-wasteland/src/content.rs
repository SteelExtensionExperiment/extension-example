//! Registers the wasteland biome, the sludge fluid and its bucket, and the nuclear blocks.

use steel_extension_api::{
    ApiResult,
    content::{
        BiomeDefinition, BiomeParticle, BlockDefinition, BlockItemDefinition, BlockProperties,
        ContentRegistry, FluidDefinition, ItemAppearance, ItemDefinition,
    },
    stabby::{self, string::String as ApiString},
};

use crate::config::Config;

pub const BIOME: &str = "toxic_wasteland:wasteland";
pub const SLUDGE: &str = "toxic_wasteland:sludge";
pub const BUCKET: &str = "toxic_wasteland:sludge_bucket";
pub const URANIUM: &str = "toxic_wasteland:uranium_ore";
pub const BARREL: &str = "toxic_wasteland:nuclear_waste";

/// A textured full block, mined like `material` and dropping itself.
fn nuclear_block(key: &str, name: &str, material: &str) -> BlockItemDefinition {
    BlockItemDefinition {
        block: BlockDefinition::new(key).properties(BlockProperties::of_full_copy(material)),
        item: ItemDefinition {
            key: key.into(),
            name: name.into(),
            appearance: ItemAppearance::new(material),
            max_stack_size: 64,
        },
        drop_self: true,
    }
}

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
            appearance: ItemAppearance::new("minecraft:water_bucket").with_model(BUCKET),
            max_stack_size: 1,
        });
    sludge.converts_to_source = config.sludge.converts_to_source;
    Ok(sludge)
}

#[stabby::export(callback)]
pub fn register(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        let config = crate::config_from(registry.config().as_str())?;
        for (key, name, material, skin) in [
            (
                URANIUM,
                "Uranium Ore",
                "minecraft:stone",
                "90614e3abf64d53496794cd8ae68597fc7266c61794bd1e48d4519868ae3cad0",
            ),
            (
                BARREL,
                "Nuclear Waste",
                "minecraft:iron_block",
                "9c64568adebf2844df824332dc27e4c66a8a9af6b4d752782f610db61d065",
            ),
        ] {
            registry
                .register_head_fallback(key.into(), skin.into())
                .match_owned(Ok, Err)?;
            registry
                .register_block_item(nuclear_block(key, name, material).with_cube_model(key))
                .match_owned(|_| Ok(()), Err)?;
        }
        registry
            .register_biome(biome(config)?)
            .match_owned(Ok, Err)?;
        registry
            .register_fluid(sludge(config)?)
            .match_owned(|_| Ok(()), Err)?;
        crate::mobs::register(&mut registry)
    })();
    result.into()
}
