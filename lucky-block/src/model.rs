//! Yellow question-mark lucky-block skin, shared by the item and placed model.
//!
//! Skin: https://minecraft-heads.com/custom-heads/head/4663-lucky-block-yellow

use steel_extension_api::content::{HeadProfile, ItemAppearance, ItemDisplay};

const TEXTURE: &str = "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvNTE5ZDI4YTg2MzJmYTRkODdjYTE5OWJiYzJlODhjZjM2OGRlZGQ1NTc0NzAxN2FlMzQ4NDM1NjlmN2E2MzRjNSJ9fX0=";
// UUID v5 of the texture URL gives this skin its own stable client cache identity.
const PROFILE_ID: [i32; 4] = [-702_410_499, 700_013_637, -1_554_449_174, 971_496_620];

pub(crate) fn inventory_appearance() -> ItemAppearance {
    ItemAppearance {
        item: "minecraft:player_head".into(),
        model: None.into(),
        profile: Some(HeadProfile {
            id: PROFILE_ID,
            texture: TEXTURE.into(),
        })
        .into(),
    }
}

pub(crate) fn displays() -> Vec<ItemDisplay> {
    // A vanilla head is half a block wide. Enlarge the complete skin as one
    // cube, with a small margin to cover the opaque planks carrier.
    const MODEL_SCALE: f32 = 1.002;
    vec![ItemDisplay {
        appearance: inventory_appearance(),
        translation: [0.5, 0.5 + MODEL_SCALE * 0.5, 0.5],
        scale: [2.0 * MODEL_SCALE; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
    }]
}
