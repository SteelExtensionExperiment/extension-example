//! Extension-authored model parts using ordinary vanilla block-item models.

use steel_extension_api::content::{
    BlockProperty, BlockStateDefinition, ItemAppearance, ItemDefinition, ItemDisplay,
};

pub(crate) fn item(key: &str, name: &str, vanilla: &str, stack: i32) -> ItemDefinition {
    ItemDefinition {
        key: key.into(),
        name: name.into(),
        max_stack_size: stack,
        appearance: ItemAppearance {
            item: vanilla.into(),
            model: None.into(),
            profile: None.into(),
        },
    }
}

pub(crate) fn state(key: &str, values: &[(&str, &str)]) -> BlockStateDefinition {
    BlockStateDefinition {
        block: key.into(),
        properties: values
            .iter()
            .map(|&(name, value)| property(name, value))
            .collect(),
    }
}

pub(crate) fn property(name: &str, value: &str) -> BlockProperty {
    BlockProperty {
        name: name.into(),
        value: value.into(),
    }
}

pub(crate) fn cuboid(item: &str, center: [f32; 3], scale: [f32; 3]) -> ItemDisplay {
    ItemDisplay {
        appearance: ItemAppearance {
            item: item.into(),
            model: None.into(),
            profile: None.into(),
        },
        translation: center,
        scale,
        rotation: [0.0, 0.0, 0.0, 1.0],
    }
}

/// Rotates extension-authored axis-aligned parts around the block center.
pub(crate) fn turn(mut part: ItemDisplay, facing: &str) -> ItemDisplay {
    let [x, y, z] = part.translation;
    let [sx, sy, sz] = part.scale;
    match facing {
        "east" => {
            part.translation = [1.0 - z, y, x];
            part.scale = [sz, sy, sx];
        }
        "south" => part.translation = [1.0 - x, y, 1.0 - z],
        "west" => {
            part.translation = [z, y, 1.0 - x];
            part.scale = [sz, sy, sx];
        }
        _ => {}
    }
    part
}
