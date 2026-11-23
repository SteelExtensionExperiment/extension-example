//! Ball items with resource-pack icons.
use steel_extension_api::{
    ApiResult,
    content::{ContentRegistry, ItemAppearance, ItemDefinition},
    stabby::{self, string::String as ApiString},
};
pub const KEYS: [&str; 3] = [
    "creatures:poke_ball",
    "creatures:super_ball",
    "creatures:hyper_ball",
];
pub const NAMES: [&str; 3] = ["Poké Ball", "Super Ball", "Hyper Ball"];
#[stabby::export(callback)]
pub fn register(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        let skins = [
            "8eca57c46681e21feefb5dc27909ede95a3c1183565cf7474dc49fc064130e0c",
            "f2ca892b97a5a84724938796189ebb9d4a547b4b94509aa7277817470d5d514",
            "648c11509e93d30bb04ab639be299d8e2650bc8c2e84e088677b969bc8b5b725",
        ];
        for ((key, name), skin) in KEYS.into_iter().zip(NAMES).zip(skins) {
            registry
                .register_head_fallback(key.into(), skin.into())
                .match_owned(Ok, Err)?;
            registry
                .register_item(ItemDefinition {
                    key: key.into(),
                    name: name.into(),
                    max_stack_size: 16,
                    appearance: ItemAppearance::new("minecraft:player_head").with_model(key),
                })
                .match_owned(Ok, Err)?;
        }
        Ok(())
    })();
    result.into()
}
