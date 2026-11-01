//! Vertical halves for every slab material discovered in Steel's registry.

use steel_extension_api::{
    content::{
        BlockDefinition, BlockItemDefinition, BlockProperties, ContentRegistry,
        HorizontalPlacement, ItemAppearance, ItemDisplay, SlabMaterial, VoxelShape,
    },
    stabby::string::String as ApiString,
};

use super::model;

pub(crate) fn register(
    registry: &mut ContentRegistry<'_>,
    use_resource_pack: bool,
) -> Result<(), ApiString> {
    for material in registry.slab_materials() {
        let key = key(material.block.as_str())?;
        registry
            .register_block_item(definition(&key, &material, use_resource_pack))
            .match_owned(|_| Ok(()), Err)?;
        if material.block.as_str() == "minecraft:spruce_slab" {
            registry
                .register_block_item(definition(
                    "furniture:vertical_slab",
                    &material,
                    use_resource_pack,
                ))
                .match_owned(|_| Ok(()), Err)?;
        }
    }
    Ok(())
}

pub(crate) fn key(block: &str) -> Result<String, ApiString> {
    let name = block
        .strip_prefix("minecraft:")
        .and_then(|path| path.strip_suffix("_slab"))
        .ok_or_else(|| ApiString::from("discovered slab has an unsupported name"))?;
    Ok(format!("furniture:{name}_vertical_slab"))
}

pub(crate) fn definition(
    key: &str,
    material: &SlabMaterial,
    use_resource_pack: bool,
) -> BlockItemDefinition {
    let mut appearance = ItemAppearance {
        item: material.item.clone(),
        model: None.into(),
        profile: None.into(),
    };
    let name = key
        .strip_prefix("furniture:")
        .unwrap_or(key)
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut item = model::item(key, &name, material.item.as_str(), 64);
    if use_resource_pack {
        appearance.model = Some(key.into()).into();
        item.appearance.model = Some(key.into()).into();
    }
    // Item models are centered at (8,8,8). A -90-degree X rotation turns a
    // bottom slab into z=8..16 without stretching its textures.
    let display = ItemDisplay {
        appearance,
        translation: [0.5, 0.5, 0.5005],
        scale: [1.002; 3],
        // A 180-degree Y turn maps the south-half item model to canonical north.
        rotation: if use_resource_pack {
            [0.0, 1.0, 0.0, 0.0]
        } else {
            [
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
            ]
        },
    };
    BlockItemDefinition {
        block: BlockDefinition::new(key)
            .properties(BlockProperties::of_full_copy(material.block.as_str()).no_occlusion())
            .shape(VoxelShape::box_shape(0.0, 0.0, 0.0, 16.0, 16.0, 8.0))
            .horizontal_facing(HorizontalPlacement::Player)
            .allow_approximation()
            .displays([display]),
        item,
        drop_self: true,
    }
}
