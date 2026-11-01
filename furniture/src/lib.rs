//! Furniture for unmodified clients: a usable chair, vertical panel, and bed.
#![recursion_limit = "512"]

mod bed;
mod chair;
mod model;
mod vertical_slab;

use steel_extension_api::{
    ApiResult, ExtensionContext, Hooks,
    content::ContentRegistry,
    stabby::{self, string::String as ApiString},
};

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        hooks
            .register_content_registration(register_content)
            .match_owned(Ok, Err)?;
        hooks
            .register_block_registration(chair::register_behavior)
            .match_owned(Ok, Err)
    })();
    result.into()
}

#[stabby::export(callback)]
fn register_content(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        registry
            .register_block_item(chair::definition())
            .match_owned(Ok, Err)?;
        let config: toml::Table = registry
            .config()
            .as_str()
            .parse()
            .map_err(|error: toml::de::Error| ApiString::from(error.to_string()))?;
        if config.keys().any(|key| key != "use_resource_pack") {
            return Err("furniture configuration only accepts use_resource_pack".into());
        }
        let use_resource_pack = match config.get("use_resource_pack") {
            None => false,
            Some(toml::Value::Boolean(enabled)) => *enabled,
            Some(_) => return Err("use_resource_pack must be a boolean".into()),
        };
        vertical_slab::register(&mut registry, use_resource_pack)?;
        registry
            .register_bed(bed::definition())
            .match_owned(Ok, Err)?;
        Ok(())
    })();
    result.into()
}

fn init(_context: ExtensionContext<'_>) -> ApiResult<()> {
    Ok(()).into()
}

steel_extension_api::export_extension! {
    name: "furniture-example",
    version: "0.1.0",
    depends: [],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
