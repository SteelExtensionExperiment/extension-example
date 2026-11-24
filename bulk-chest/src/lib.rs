//! Hollow multiblock storage for vanilla clients.
#![recursion_limit = "512"]

mod shell;

use std::sync::OnceLock;

use steel_extension_api::{
    ApiResult, BlockPos, BlockState, ExtensionContext, Hooks, Registry, World,
    block_behavior::{
        BlockBehavior, DynBlockBehavior, InteractionResult, OriginalBlockBehavior,
        types::{
            BlockEntityCreation, BlockEntityTicker, BlockEntityType, BlockHit, Inventory, Player,
            WeakWorld,
        },
    },
    content::{
        BlockDefinition, BlockItemDefinition, BlockProperties, ContentRegistry, ItemAppearance,
        ItemDefinition, MultiblockDisplay,
    },
    stabby::{self, boxed::Box, option::Option as ApiOption, string::String as ApiString},
};

use shell::{DISPLAY_RADIUS, MAX_CAPACITY, Parts, Shell};

const WALL_KEY: &str = "bulk-chest:wall";
const CORE_KEY: &str = "bulk-chest:core";
const WALL_MATERIAL: &str = "minecraft:spruce_planks";
const CORE_MATERIAL: &str = "minecraft:stripped_spruce_log";
const BLOCK_STACK_SIZE: i32 = 64;

static PARTS: OnceLock<Parts> = OnceLock::new();

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    hooks.register_content_registration(register_content)
}

fn init(_context: ExtensionContext<'_>) -> Result<(), String> {
    Ok(())
}

#[stabby::export(callback)]
fn register_content(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        for (key, texture) in [
            (WALL_KEY, include_bytes!("assets/wall.png").as_slice()),
            (CORE_KEY, include_bytes!("assets/core.png").as_slice()),
        ] {
            registry
                .register_block_texture(key.into(), texture.into())
                .match_owned(Ok, Err)?;
        }
        let wall = registry
            .register_block_item_with_behavior(
                block(WALL_KEY, "Bulk Chest Wall", WALL_MATERIAL),
                create_wall,
            )
            .match_owned(Ok, Err)?;
        let core = registry
            .register_storage_block(
                block(CORE_KEY, "Bulk Chest Core", CORE_MATERIAL),
                MAX_CAPACITY,
                create_core,
            )
            .match_owned(Ok, Err)?;
        PARTS
            .set(Parts { core, wall })
            .map_err(|_| "bulk chest content was already registered")?;
        registry
            .register_multiblock_storage_display(
                CORE_KEY.into(),
                DISPLAY_RADIUS,
                [core, wall].as_slice().into(),
                chest_display,
            )
            .match_owned(Ok, Err)?;
        Ok(())
    })();
    result.into()
}

fn block(key: &str, name: &str, material: &str) -> BlockItemDefinition {
    BlockItemDefinition {
        block: BlockDefinition::new(key)
            .properties(BlockProperties::of_full_copy(material))
            .vanilla_geometry(material.into()),
        item: ItemDefinition {
            key: key.into(),
            name: name.into(),
            appearance: ItemAppearance::new(material),
            max_stack_size: BLOCK_STACK_SIZE,
        },
        drop_self: true,
    }
    .with_cube_model(key)
}

#[stabby::export(callback)]
fn create_core(
    _registry: Registry<'_>,
    _state: BlockState,
    original: OriginalBlockBehavior,
) -> ApiResult<DynBlockBehavior> {
    create_part(original, true)
}

#[stabby::export(callback)]
fn create_wall(
    _registry: Registry<'_>,
    _state: BlockState,
    original: OriginalBlockBehavior,
) -> ApiResult<DynBlockBehavior> {
    create_part(original, false)
}

fn create_part(original: OriginalBlockBehavior, is_core: bool) -> ApiResult<DynBlockBehavior> {
    let Some(&parts) = PARTS.get() else {
        return Err(ApiString::from("bulk chest parts were not registered")).into();
    };
    let behavior: DynBlockBehavior = Box::new(ChestPart {
        original,
        parts,
        is_core,
    })
    .into();
    Ok(behavior).into()
}

#[stabby::export(callback)]
fn chest_display(world: World<'_>, pos: BlockPos) -> ApiOption<MultiblockDisplay> {
    let Some(&parts) = PARTS.get() else {
        return ApiOption::None();
    };
    Shell::find(&world, pos, parts)
        .ok()
        .map(|shell| shell.display())
        .into()
}

struct ChestPart {
    original: OriginalBlockBehavior,
    parts: Parts,
    is_core: bool,
}

#[stabby::export(callback)]
impl BlockBehavior for ChestPart {
    fn original(&self) -> OriginalBlockBehavior {
        self.original
    }

    fn new_block_entity(
        &self,
        level: WeakWorld,
        pos: BlockPos,
        state: BlockState,
    ) -> BlockEntityCreation {
        if self.is_core {
            BlockEntityCreation::storage(level, pos, state)
        } else {
            BlockEntityCreation::no_entity()
        }
    }

    fn get_block_entity_ticker(
        &self,
        _world: World<'_>,
        _state: BlockState,
        _kind: BlockEntityType,
    ) -> ApiOption<BlockEntityTicker> {
        if !self.is_core {
            return ApiOption::None();
        }
        BlockEntityTicker::for_storage_display(CORE_KEY.into()).match_owned(
            |ticker| Some(ticker).into(),
            |error| {
                eprintln!("Could not start Bulk Chest display: {error}");
                ApiOption::None()
            },
        )
    }

    fn use_without_item(
        &self,
        _state: BlockState,
        world: World<'_>,
        pos: BlockPos,
        player: Player<'_>,
        _hit: BlockHit<'_>,
        inventory: Inventory<'_>,
    ) -> InteractionResult {
        let shell = match Shell::find(&world, pos, self.parts) {
            Ok(shell) => shell,
            Err(error) => {
                if self.is_core
                    && !inventory.held_item_is(WALL_KEY.into())
                    && !inventory.held_item_is(CORE_KEY.into())
                {
                    player.send_message(error.into());
                    return InteractionResult::Consume;
                }
                return InteractionResult::Pass;
            }
        };
        let result = player
            .open_storage_from(
                shell.core_position,
                pos,
                "Bulk Chest".into(),
                shell.slots(),
                Box::new(shell).into(),
            )
            .match_owned(Ok, Err);
        if let Err(error) = result {
            player.send_message(error.as_str().into());
        }
        InteractionResult::SuccessServer
    }
}

steel_extension_api::export_extension! {
    name: "bulk-chest",
    version: "0.1.0",
    depends: [],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
