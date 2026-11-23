//! Vertical panels for every slab material discovered in Steel's registry.

use steel_extension_api::{
    content::{
        BlockDefinition, BlockItemDefinition, BlockProperties, ContentRegistry,
        HorizontalPlacement, ItemAppearance, ItemDisplay, SlabMaterial,
    },
    stabby::string::String as ApiString,
};

use super::model;

const HALF_TURN_COMPONENT: f32 = std::f32::consts::FRAC_1_SQRT_2;

pub(crate) fn register(registry: &mut ContentRegistry<'_>) -> Result<(), ApiString> {
    for material in registry.slab_materials() {
        let key = key(material.block.as_str())?;
        register_slab(registry, &key, &material)?;
        if material.block.as_str() == "minecraft:spruce_slab" {
            register_slab(registry, "furniture:vertical_slab", &material)?;
        }
    }
    Ok(())
}

fn register_slab(
    registry: &mut ContentRegistry<'_>,
    key: &str,
    material: &SlabMaterial,
) -> Result<(), ApiString> {
    add_model(registry, key, material.block.as_str())?;
    registry
        .register_block_item(definition(key, material))
        .match_owned(|_| Ok(()), Err)
}

fn add_model(registry: &mut ContentRegistry<'_>, key: &str, block: &str) -> Result<(), ApiString> {
    use serde_json::json;
    let name = key
        .strip_prefix("furniture:")
        .ok_or("invalid furniture model key")?;
    let slab = block
        .strip_prefix("minecraft:")
        .ok_or("invalid vanilla slab key")?;
    // Waxed copper slabs use the same asset as their unwaxed counterparts.
    let slab = slab.strip_prefix("waxed_").unwrap_or(slab);
    let model = format!("minecraft:block/{slab}");
    let path = format!("assets/furniture/items/{name}.json");
    // Item displays rotate the slab through entity metadata. Other contexts
    // rotate the same mesh around the center of the model in the item definition.
    let definition = json!({"model": {
        "type": "minecraft:select",
        "property": "minecraft:display_context",
        "cases": [{
            "when": "none",
            "model": {"type": "minecraft:model", "model": model},
        }],
        "fallback": {
            "type": "minecraft:model",
            "model": model,
            "transformation": {
                "translation": [0, 0, 1],
                "left_rotation": [-HALF_TURN_COMPONENT, 0.0, 0.0, HALF_TURN_COMPONENT],
                "scale": [1, 1, 1],
                "right_rotation": [0, 0, 0, 1],
            },
        },
    }});
    registry
        .add_resource_asset(
            path.as_str().into(),
            definition.to_string().as_bytes().into(),
        )
        .match_owned(Ok, Err)
}

pub(crate) fn key(block: &str) -> Result<String, ApiString> {
    let name = block
        .strip_prefix("minecraft:")
        .and_then(|path| path.strip_suffix("_slab"))
        .ok_or_else(|| ApiString::from("discovered slab has an unsupported name"))?;
    Ok(format!("furniture:{name}_vertical_slab"))
}

fn definition(key: &str, material: &SlabMaterial) -> BlockItemDefinition {
    let appearance = ItemAppearance::new(material.item.as_str()).with_model(key);
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
    item.appearance = item.appearance.with_model(key);
    // Keep the half-block mesh over the thinner collision carrier. Extend the
    // mesh slightly across the block boundary to cover the carrier's outer face.
    const HALF_BLOCK: f32 = 0.5;
    const FACE_MARGIN: f32 = 0.001;
    const FULL_FACE_SCALE: f32 = 1.0 + 2.0 * FACE_MARGIN;
    const HALF_FACE_SCALE: f32 = (HALF_BLOCK + 2.0 * FACE_MARGIN) / HALF_BLOCK;
    let translation = [HALF_BLOCK, HALF_BLOCK, HALF_BLOCK + FACE_MARGIN];
    let display = ItemDisplay {
        appearance,
        translation,
        scale: [FULL_FACE_SCALE, HALF_FACE_SCALE, FULL_FACE_SCALE],
        rotation: [0.0, HALF_TURN_COMPONENT, HALF_TURN_COMPONENT, 0.0],
        pack_transform: None.into(),
    };
    BlockItemDefinition {
        block: BlockDefinition::new(key)
            .properties(BlockProperties::of_full_copy(material.block.as_str()).no_occlusion())
            .vanilla_geometry(model::state("minecraft:oak_shelf", &[("facing", "south")]))
            .horizontal_facing(HorizontalPlacement::Player)
            .displays([display]),
        item,
        drop_self: true,
    }
}
