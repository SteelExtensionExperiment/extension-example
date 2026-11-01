use steel_extension_api::content::{BedDefinition, BlockModel};

use super::model;

pub(crate) fn definition() -> BedDefinition {
    let mut models = Vec::new();
    for facing in ["north", "east", "south", "west"] {
        for part in ["head", "foot"] {
            let (z, height) = if part == "head" {
                (0.0625, 1.125)
            } else {
                (0.9375, 0.625)
            };
            let pieces = [
                model::cuboid(
                    "minecraft:oak_planks",
                    [0.0625, height / 2.0, z],
                    [0.125, height, 0.125],
                ),
                model::cuboid(
                    "minecraft:oak_planks",
                    [0.9375, height / 2.0, z],
                    [0.125, height, 0.125],
                ),
                model::cuboid(
                    "minecraft:oak_planks",
                    [0.5, height - 0.125, z],
                    [0.875, 0.25, 0.125],
                ),
                model::cuboid(
                    "minecraft:oak_planks",
                    [0.03125, 0.375, 0.5],
                    [0.0625, 0.25, 1.0],
                ),
                model::cuboid(
                    "minecraft:oak_planks",
                    [0.96875, 0.375, 0.5],
                    [0.0625, 0.25, 1.0],
                ),
            ];
            models.push(BlockModel {
                properties: [
                    model::property("facing", facing),
                    model::property("part", part),
                ]
                .into_iter()
                .collect(),
                displays: pieces
                    .into_iter()
                    .map(|piece| model::turn(piece, facing))
                    .collect(),
            });
        }
    }
    BedDefinition {
        key: "furniture:custom_bed".into(),
        template: "minecraft:blue_bed".into(),
        item: model::item(
            "furniture:custom_bed",
            "Oak Frame Bed",
            "minecraft:blue_bed",
            16,
        ),
        models: models.into_iter().collect(),
    }
}
