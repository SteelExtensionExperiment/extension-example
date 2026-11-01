use steel_extension_api::{
    ApiResult, BlockPos, BlockState, Position, Registry, World,
    block_behavior::{
        BlockBehavior, InteractionResult, OriginalBlockBehavior,
        types::{BlockHit, Inventory, Player},
    },
    content::{BlockDefinition, BlockItemDefinition, BlockProperties, HorizontalPlacement},
    stabby::{self, string::String as ApiString},
};

use super::model;

const KEY: &str = "furniture:chair";

pub(crate) fn definition() -> BlockItemDefinition {
    BlockItemDefinition {
        block: BlockDefinition::new(KEY)
            .properties(BlockProperties::of_full_copy("minecraft:oak_planks"))
            .vanilla_geometry(model::state(
                "minecraft:oak_stairs",
                &[
                    ("facing", "north"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                ],
            ))
            .horizontal_facing(HorizontalPlacement::Player)
            .displays([
                model::cuboid(
                    "minecraft:white_wool",
                    [0.5, 0.5625, 0.75],
                    [0.875, 0.125, 0.4375],
                ),
                model::cuboid(
                    "minecraft:oak_planks",
                    [0.0625, 0.625, 0.5],
                    [0.125, 0.25, 1.0],
                ),
                model::cuboid(
                    "minecraft:oak_planks",
                    [0.9375, 0.625, 0.5],
                    [0.125, 0.25, 1.0],
                ),
            ]),
        item: model::item(KEY, "Oak Chair", "minecraft:oak_stairs", 64),
        drop_self: true,
    }
}

#[stabby::export(callback)]
pub(crate) fn register_behavior(mut registry: Registry<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        let original = OriginalBlockBehavior::get(&registry, KEY.into()).match_owned(Ok, Err)?;
        registry
            .replace_block_behavior(KEY.into(), stabby::boxed::Box::new(Chair(original)).into())
            .match_owned(Ok, Err)
    })();
    result.into()
}

struct Chair(OriginalBlockBehavior);

#[stabby::export(callback)]
impl BlockBehavior for Chair {
    fn original(&self) -> OriginalBlockBehavior {
        self.0
    }

    fn use_without_item(
        &self,
        state: BlockState,
        _world: World<'_>,
        pos: BlockPos,
        player: Player<'_>,
        _hit: BlockHit<'_>,
        _inventory: Inventory<'_>,
    ) -> InteractionResult {
        // Authored for this chair's cushion and the adult player's riding pose.
        // LivingEntityRenderer's transform and the adult humanoid leg pivot
        // put the riding hip this far above the entity origin.
        let hip_height = 0.9375 * (1.501 - 12.0 / 16.0);
        let offset = Position {
            x: 0.5,
            y: 0.625 - hip_height,
            z: 0.75,
        };
        let Some(facing) = state.property("facing".into()).match_owned(Some, || None) else {
            eprintln!("Chair at {pos:?} has no facing state");
            return InteractionResult::Fail;
        };
        let offset = match facing.as_str() {
            "north" => offset,
            "east" => Position {
                x: 1.0 - offset.z,
                y: offset.y,
                z: offset.x,
            },
            "south" => Position {
                x: 1.0 - offset.x,
                y: offset.y,
                z: 1.0 - offset.z,
            },
            "west" => Position {
                x: offset.z,
                y: offset.y,
                z: 1.0 - offset.x,
            },
            _ => return InteractionResult::Fail,
        };
        if let Err(error) = player.sit(pos, offset).match_owned(Ok, Err) {
            eprintln!("Chair at {pos:?} could not be used: {error}");
        }
        InteractionResult::SuccessServer
    }
}
