//! A yellow question-mark lucky block with oak-plank collision and mining.
#![recursion_limit = "512"]

mod config;
mod outcomes;

use steel_extension_api::{
    ApiResult, BlockState, ExtensionContext, Hooks, Registry,
    block_behavior::{DynBlockBehavior, OriginalBlockBehavior},
    content::{
        BlockDefinition, BlockItemDefinition, BlockProperties, ContentRegistry, ItemAppearance,
        ItemDefinition,
    },
    stabby::{self, string::String as ApiString},
};

const KEY: &str = "lucky:lucky_block";
const MATERIAL: &str = "minecraft:oak_planks";
// Luck's published yellow Lucky Block skin is used by players without the pack.
const FALLBACK_SKIN: &str = "519d28a8632fa4d87ca199bbc2e88cf368dedd55747017ae34843569f7a634c5";

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    hooks.register_content_registration(register_content)
}

#[stabby::export(callback)]
fn register_content(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    let result = (|| {
        registry
            .register_head_fallback(KEY.into(), FALLBACK_SKIN.into())
            .match_owned(Ok, Err)?;
        registry
            .register_block_item_with_behavior(
                BlockItemDefinition {
                    block: BlockDefinition::new(KEY)
                        .properties(BlockProperties::of_full_copy(MATERIAL)),
                    item: ItemDefinition {
                        key: KEY.into(),
                        name: "Lucky Block".into(),
                        appearance: ItemAppearance::new(MATERIAL),
                        max_stack_size: 64,
                    },
                    drop_self: false,
                }
                .with_cube_model(KEY),
                create_behavior,
            )
            .match_owned(|_| Ok(()), Err)
    })();
    result.into()
}

#[stabby::export(callback)]
fn create_behavior(
    registry: Registry<'_>,
    _state: BlockState,
    original: OriginalBlockBehavior,
) -> ApiResult<DynBlockBehavior> {
    let result = (|| -> Result<DynBlockBehavior, ApiString> {
        let seed = config::parse_seed(registry.config().as_str())?;
        let behavior = outcomes::LuckyBlock::new(original, seed)?;
        Ok(stabby::boxed::Box::new(behavior).into())
    })();
    result.into()
}

fn init(_context: ExtensionContext<'_>) -> ApiResult<()> {
    Ok(()).into()
}

steel_extension_api::export_extension! {
    name: "lucky-block-example",
    version: "0.1.0",
    depends: [],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
