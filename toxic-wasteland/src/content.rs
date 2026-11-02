//! Registers the wasteland biome, the sludge fluid and its bucket, and the nuclear blocks.

use steel_extension_api::{
    ApiResult,
    content::{
        BiomeDefinition, BiomeParticle, BlockDefinition, BlockItemDefinition, BlockProperties,
        ContentRegistry, FluidDefinition, ItemAppearance, ItemDefinition,
    },
    stabby::{self, string::String as ApiString},
};

use crate::{
    config::Config,
    model::{NUCLEAR_WASTE, Skin, URANIUM_ORE},
};

pub const BIOME: &str = "toxic_wasteland:wasteland";
pub const SLUDGE: &str = "toxic_wasteland:sludge";
pub const BUCKET: &str = "toxic_wasteland:sludge_bucket";
pub const URANIUM: &str = "toxic_wasteland:uranium_ore";
pub const BARREL: &str = "toxic_wasteland:nuclear_waste";

/// A full block with a head model, mined like `material` and dropping itself.
fn nuclear_block(key: &str, name: &str, material: &str, skin: &Skin) -> BlockItemDefinition {
    BlockItemDefinition {
        block: BlockDefinition::new(key)
            .properties(BlockProperties::of_full_copy(material))
            .displays(skin.block_displays()),
        item: ItemDefinition {
            key: key.into(),
            name: name.into(),
            appearance: skin.appearance(),
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
            .match_owned(|_| Ok(()), Err)?;
        let blocks = [
            nuclear_block(URANIUM, "Uranium Ore", "minecraft:stone", &URANIUM_ORE),
            nuclear_block(
                BARREL,
                "Nuclear Waste",
                "minecraft:iron_block",
                &NUCLEAR_WASTE,
            ),
        ];
        for block in blocks {
            registry
                .register_block_item(block)
                .match_owned(|_| Ok(()), Err)?;
        }
        crate::mobs::register(&mut registry)
    })();
    result.into()
}
