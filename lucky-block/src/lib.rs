//! A yellow question-mark lucky block with oak-plank collision and mining.
#![recursion_limit = "512"]

mod model;
mod outcomes;

use steel_extension_api::{
    ApiResult, ExtensionContext, Hooks, Registry,
    block_behavior::OriginalBlockBehavior,
    content::{
        BlockDefinition, BlockItemDefinition, BlockProperties, ContentRegistry, ItemDefinition,
    },
    stabby::{self, string::String as ApiString},
};

const KEY: &str = "lucky:lucky_block";

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    hooks
        .register_content_registration(register_content)
        .match_owned(Ok, Err)
        .and_then(|()| {
            hooks
                .register_block_registration(register_behavior)
                .match_owned(Ok, Err)
        })
        .into()
}

#[stabby::export(callback)]
fn register_content(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    let result: Result<(), ApiString> = registry
        .register_block_item(BlockItemDefinition {
            block: BlockDefinition::new(KEY)
                .properties(BlockProperties::of_full_copy("minecraft:oak_planks"))
                .displays(model::displays()),
            item: ItemDefinition {
                key: KEY.into(),
                name: "Lucky Block".into(),
                appearance: model::inventory_appearance(),
                max_stack_size: 64,
            },
            drop_self: false,
        })
        .match_owned(|_| Ok(()), Err);
    result.into()
}

#[stabby::export(callback)]
fn register_behavior(mut registry: Registry<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        let original = OriginalBlockBehavior::get(&registry, KEY.into()).match_owned(Ok, Err)?;
        let behavior = outcomes::LuckyBlock::new(original, registry.config().as_str())?;
        registry
            .replace_block_behavior(KEY.into(), stabby::boxed::Box::new(behavior).into())
            .match_owned(Ok, Err)
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
